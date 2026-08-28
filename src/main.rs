use std::io::{self, Read};
use std::process;

#[cfg(feature = "arbitrary")]
use clap::{CommandFactory, FromArgMatches};
use clap::{Parser, Subcommand};
use recursa_core::fmt::FormatStyle;

use pg_sql::ast::{FileItem, parse_sql_file_with_spans};
use pg_sql::formatter::format_file;

#[derive(Parser)]
#[command(name = "pg-sql", about = "PostgreSQL SQL tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Format a SQL file
    Fmt {
        /// SQL file to format, or `-` for stdin
        file: String,
    },
    /// Generate random SQL statements using Arbitrary (requires --features arbitrary)
    #[cfg(feature = "arbitrary")]
    Gen {
        /// Number of statements to generate
        count: usize,
        /// Filter by node name (exact match against meta name)
        #[arg(long)]
        node_name: Option<String>,
        /// Filter by tag (repeatable, matches if any tag matches)
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// Dump the first 200 chars of every statement pg-sql could not parse
    /// structurally (each surfaces as a [`FileItem::ParseError`]).
    DumpRaw {
        /// SQL file to analyze
        file: String,
        /// Maximum number of failing statements to print
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
}

fn read_sql(file: &str) -> String {
    if file == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).unwrap_or_else(|e| {
            eprintln!("error reading stdin: {e}");
            process::exit(1);
        });
        buf
    } else {
        std::fs::read_to_string(file).unwrap_or_else(|e| {
            eprintln!("error reading {file}: {e}");
            process::exit(1);
        })
    }
}

#[cfg(feature = "arbitrary")]
fn gen_help_text() -> String {
    use std::collections::BTreeSet;

    let metas = pg_sql::ast::Statement::variant_metas();

    let mut tags = BTreeSet::new();
    for (_, meta) in &metas {
        for tag in meta.tags.iter() {
            tags.insert(*tag);
        }
    }

    let mut names: Vec<(&str, &[&str])> = metas
        .iter()
        .map(|(_, meta)| (meta.name, meta.tags))
        .collect();
    names.sort_by_key(|(name, _)| *name);

    let tag_descriptions = [
        ("ddl", "Schema / object definitions (CREATE, ALTER, DROP)"),
        ("dcl", "Access control (GRANT, REVOKE)"),
        (
            "dml",
            "Data modification (INSERT, UPDATE, DELETE, MERGE, TRUNCATE)",
        ),
        ("misc", "Special forms (COPY, LOCK, NOTIFY, LISTEN, ...)"),
        ("procedural", "Procedural execution (DO, CALL)"),
        ("dql", "Queries (SELECT, WITH, VALUES, TABLE)"),
        ("session", "Session / configuration (SET, SHOW, RESET)"),
        ("tcl", "Transaction control (BEGIN, COMMIT, ROLLBACK)"),
        (
            "utility",
            "Maintenance / utility (EXPLAIN, ANALYZE, VACUUM, ...)",
        ),
    ];

    let mut help = String::from("Tags:\n");
    for (tag, desc) in &tag_descriptions {
        if tags.contains(tag) {
            help.push_str(&format!("  {tag:<14} {desc}\n"));
        }
    }
    for tag in &tags {
        if !tag_descriptions.iter().any(|(t, _)| t == tag) {
            help.push_str(&format!("  {tag:<14} (undocumented)\n"));
        }
    }

    help.push_str("\nNode names:\n");
    for (name, node_tags) in &names {
        if name.is_empty() {
            continue;
        }
        let tags_str = if node_tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", node_tags.join(", "))
        };
        help.push_str(&format!("  {name}{tags_str}\n"));
    }

    help
}

fn main() {
    #[cfg(feature = "arbitrary")]
    let cli = {
        let help = gen_help_text();
        let cmd = Cli::command().mut_subcommand("gen", |sub| sub.after_long_help(help));
        let matches = cmd.get_matches();
        Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
    };
    #[cfg(not(feature = "arbitrary"))]
    let cli = Cli::parse();

    match cli.command {
        Command::Fmt { file } => {
            let sql = read_sql(&file);
            let lexed = pg_sql::tokens::pg_lex(&sql);
            let mut input = recursa::Input::new(&sql, &lexed);
            let items = match pg_sql::ast::parse_sql_file(&mut input) {
                Ok(items) => items,
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            };
            print!("{}", format_file(&items, FormatStyle::default()));
        }
        #[cfg(feature = "arbitrary")]
        Command::Gen {
            count,
            node_name,
            tags,
        } => {
            use arbitrary::Unstructured;
            use pg_sql::formatter::format_tokens_sql;

            // Resolve matching variant indices once upfront.
            let candidates: Vec<u32> = {
                let all = pg_sql::ast::Statement::variant_metas();
                all.into_iter()
                    .filter(|(_, meta)| {
                        if let Some(ref name) = node_name {
                            if meta.name != name.as_str() {
                                return false;
                            }
                        }
                        if !tags.is_empty() && !tags.iter().any(|t| meta.tags.contains(&t.as_str()))
                        {
                            return false;
                        }
                        true
                    })
                    .map(|(idx, _)| idx)
                    .collect()
            };

            if candidates.is_empty() {
                eprintln!("no matching statement types found");
                process::exit(1);
            }

            let mut rng_state: u64 = 0xdeadbeef;
            for _ in 0..count {
                rng_state ^= rng_state << 13;
                rng_state ^= rng_state >> 7;
                rng_state ^= rng_state << 17;
                let seed: Vec<u8> = (0..1024u64)
                    .map(|i| {
                        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(i);
                        (rng_state >> 33) as u8
                    })
                    .collect();
                let mut u = Unstructured::new(&seed);
                let idx = candidates[u.choose_index(candidates.len()).unwrap_or(0)];
                let stmt: pg_sql::ast::Statement<'_> =
                    match pg_sql::ast::Statement::generate_variant(idx, &mut u) {
                        Some(s) => s,
                        None => continue,
                    };
                let sql = format_tokens_sql(&stmt, FormatStyle::default());
                eprintln!("{sql};");
            }
        }
        Command::DumpRaw { file, limit } => {
            let sql = read_sql(&file);
            let lexed = pg_sql::tokens::pg_lex(&sql);
            let mut input = recursa::Input::new(&sql, &lexed);
            let items = match parse_sql_file_with_spans(&mut input) {
                Ok(items) => items,
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            };
            let mut count = 0usize;
            for (item, _) in &items {
                if let FileItem::ParseError { span, .. } = item {
                    let text = sql[span.clone()].trim().replace('\n', " ");
                    let truncated: String = text.chars().take(200).collect();
                    println!("{truncated}");
                    count += 1;
                    if count >= limit {
                        break;
                    }
                }
            }
        }
    }
}
