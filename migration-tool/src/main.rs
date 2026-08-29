use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use pg_sql_migrate::baseline::{
    CaptureOptions, capture_baseline, to_canonical_json, write_baseline,
};
use pg_sql_migrate::execution::{
    publication_tree_digest, published_source_digest, verify_execution,
};
use pg_sql_migrate::grammar_rewrite::{GeneratedWhitespaceCleanupPass, GrammarRewritePass};
use pg_sql_migrate::rewrite::{RewriteTreeRequest, SourceRewritePass, rewrite_tree};
use pg_sql_migrate::test_call_rewrite::TestCallRewritePass;
use pg_sql_migrate::{Mapping, inventory, to_canonical_inventory_json};

#[derive(Parser)]
#[command(
    name = "pg-sql-migrate",
    about = "Auditable pg-sql grammar migration tooling"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the complete legacy migration inventory as JSON. This command is read-only.
    Inventory {
        #[arg(default_value = ".")]
        root: PathBuf,
        /// Byte-compare stdout's canonical report with this file instead of printing it.
        #[arg(long)]
        check: Option<PathBuf>,
    },
    /// Capture or review the pinned PostgreSQL differential baseline.
    Baseline {
        #[command(subcommand)]
        command: BaselineCommand,
    },
    /// Verify the checked record of the one-shot grammar migration.
    Execution {
        #[command(subcommand)]
        command: ExecutionCommand,
    },
    /// Publish a validated rewritten copy; the source tree is never modified.
    Rewrite {
        #[command(subcommand)]
        command: RewriteCommand,
    },
}

#[derive(Subcommand)]
enum ExecutionCommand {
    /// Match the immutable identities and reviewed digest to the published tree.
    Verify {
        #[arg(long, default_value = ".")]
        repository_root: PathBuf,
        #[arg(long, default_value = "migration/execution.json")]
        record: PathBuf,
    },
    /// Print the canonical digest of a migrated pg-sql source tree.
    Digest {
        #[arg(long)]
        tree_root: PathBuf,
        #[arg(long, default_value = "docs/import-provenance.tsv")]
        manifest: PathBuf,
        #[arg(long = "omit")]
        omitted_paths: Vec<String>,
    },
    /// Print the canonical digest of the exact published Cargo/source membership and modes.
    PublicationDigest {
        #[arg(long, default_value = ".")]
        repository_root: PathBuf,
        #[arg(long, default_value = "docs/import-provenance.tsv")]
        manifest: PathBuf,
    },
}

#[derive(Subcommand)]
enum RewriteCommand {
    /// Apply only the reviewed grammar transformations.
    Grammar {
        source_root: PathBuf,
        destination_root: PathBuf,
        #[arg(long)]
        new_repository_root: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Apply only the repetitive imported-test call transformations.
    TestCalls {
        source_root: PathBuf,
        destination_root: PathBuf,
        #[arg(long)]
        new_repository_root: PathBuf,
    },
}

#[derive(Subcommand)]
enum BaselineCommand {
    Capture {
        #[arg(long, default_value = "../recursa-old")]
        legacy_repository: PathBuf,
        #[arg(long, default_value = "vendor/postgres")]
        postgres_repository: PathBuf,
        #[arg(long, default_value = "baselines/postgresql-17.9.json")]
        output: PathBuf,
    },
    Review {
        #[arg(long, default_value = "../recursa-old")]
        legacy_repository: PathBuf,
        #[arg(long, default_value = "vendor/postgres")]
        postgres_repository: PathBuf,
        #[arg(long, default_value = "baselines/postgresql-17.9.json")]
        baseline: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::Inventory { root, check } => {
            let report = inventory(&root, &Mapping::migration_contract())?;
            let canonical = to_canonical_inventory_json(&report)?;
            if let Some(path) = check {
                let expected = fs::read(&path)?;
                if expected != canonical.as_bytes() {
                    return Err(format!("inventory differs from {}", path.display()).into());
                }
                println!("inventory matches {}", path.display());
            } else {
                print!("{canonical}");
            }
        }
        Command::Baseline { command } => match command {
            BaselineCommand::Capture {
                legacy_repository,
                postgres_repository,
                output,
            } => {
                let options = CaptureOptions::new(legacy_repository, postgres_repository);
                let baseline = capture_baseline(&options)?;
                write_baseline(&output, &baseline)?;
                println!("wrote {}", output.display());
            }
            BaselineCommand::Review {
                legacy_repository,
                postgres_repository,
                baseline,
            } => {
                let expected = fs::read_to_string(&baseline)?;
                let options = CaptureOptions::new(legacy_repository, postgres_repository);
                let captured = capture_baseline(&options)?;
                let actual = to_canonical_json(&captured)?;
                if expected.as_bytes() != actual.as_bytes() {
                    return Err(format!(
                        "baseline differs from fresh capture: {}",
                        baseline.display()
                    )
                    .into());
                }
                println!("baseline matches fresh capture: {}", baseline.display());
            }
        },
        Command::Execution { command } => match command {
            ExecutionCommand::Verify {
                repository_root,
                record,
            } => {
                verify_execution(&repository_root, &record)?;
                println!("migration execution matches {}", record.display());
            }
            ExecutionCommand::Digest {
                tree_root,
                manifest,
                omitted_paths,
            } => {
                let omitted_paths = omitted_paths.into_iter().collect();
                println!(
                    "{}",
                    published_source_digest(&tree_root, &manifest, &omitted_paths)?
                );
            }
            ExecutionCommand::PublicationDigest {
                repository_root,
                manifest,
            } => println!("{}", publication_tree_digest(&repository_root, &manifest)?),
        },
        Command::Rewrite { command } => match command {
            RewriteCommand::Grammar {
                source_root,
                destination_root,
                new_repository_root,
                manifest,
            } => {
                let manifest = fs::read_to_string(manifest)?;
                let pass = GrammarRewritePass::from_manifest_json(&manifest)?;
                publish_rewrites(
                    &source_root,
                    &destination_root,
                    &new_repository_root,
                    &[&pass, &GeneratedWhitespaceCleanupPass],
                )?;
            }
            RewriteCommand::TestCalls {
                source_root,
                destination_root,
                new_repository_root,
            } => publish_rewrite(
                &source_root,
                &destination_root,
                &new_repository_root,
                &TestCallRewritePass,
            )?,
        },
    }
    Ok(())
}

fn publish_rewrite(
    source_root: &std::path::Path,
    destination_root: &std::path::Path,
    new_repository_root: &std::path::Path,
    pass: &dyn SourceRewritePass,
) -> Result<(), Box<dyn std::error::Error>> {
    publish_rewrites(source_root, destination_root, new_repository_root, &[pass])
}

fn publish_rewrites(
    source_root: &std::path::Path,
    destination_root: &std::path::Path,
    new_repository_root: &std::path::Path,
    passes: &[&dyn SourceRewritePass],
) -> Result<(), Box<dyn std::error::Error>> {
    rewrite_tree(RewriteTreeRequest {
        source_root,
        destination_root,
        new_repository_root,
        passes,
    })?;
    println!("published {}", destination_root.display());
    Ok(())
}
