//! The psql differential: pg-psql against psql's own lexer.
//!
//! `pg-sql` compares its parses with PostgreSQL's raw parser through
//! `pg-oracle`. This suite does the same one layer up: `pg-psql-oracle` links
//! `psqlscan.l` and `psqlscanslash.l` of the pinned release of the build's
//! target version, and every document below is read by both.
//!
//! The corpus is `baselines/psql-oracle/corpus.tsv`, the PostgreSQL 17.9
//! regression SQL files, frozen by Git blob ID exactly as the SQL
//! differential corpus is (`docs/differential-baseline.md`), plus a set of
//! shaped documents that hold the constructs pg-psql's own tests cover.
//!
//! The outcome of every document is pinned in
//! `baselines/psql-oracle/<feature>.json`. A document that does not agree
//! with psql is an **expected psql gap** with a recorded reason, and the
//! reasons are the deliberate limits of what pg-psql models (ADR 0007): it
//! models `psqlscan.l` lexing and the send commands, and keeps every other
//! backslash command as one unparsed token (freshtonic/pg-sql#11, #12, #13).
//!
//! Regenerate after a change of pin, of oracle or of the psql grammar:
//!
//! ```sh
//! cargo test -p pg-psql --no-default-features --features pg17,psql-oracle \
//!   --test psql_oracle -- --ignored regenerate_psql_oracle_baseline --nocapture
//! ```

use std::collections::BTreeMap;
use std::ops::Range;
use std::path::PathBuf;

use pg_psql::{PsqlItem, Terminator, Variables};
use pg_psql_oracle::{Command, Event};

// --- The corpus -------------------------------------------------------------

const CORPUS: &str = include_str!("../../baselines/psql-oracle/corpus.tsv");
const CORPUS_COMMIT: &str = "6d396980fc5aed4f1a525e0bd75cb16b25ed40ca";

/// The pin table of the oracle: `feature<TAB>ref<TAB>commit`.
const ORACLE_PINS: &str = include_str!("../../pg-oracle/pins.tsv");

/// The version feature of this build.
fn target_version() -> &'static str {
    // The features are mutually exclusive (ADR 0009), so exactly one arm
    // applies.
    #[cfg(not(feature = "since-pg15"))]
    return "pg14";
    #[cfg(all(feature = "since-pg15", not(feature = "since-pg16")))]
    return "pg15";
    #[cfg(all(feature = "since-pg16", not(feature = "since-pg17")))]
    return "pg16";
    #[cfg(all(feature = "since-pg17", not(feature = "since-pg18")))]
    return "pg17";
    #[cfg(all(feature = "since-pg18", not(feature = "since-pg19")))]
    return "pg18";
    #[cfg(feature = "since-pg19")]
    return "pg19-beta";
}

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/postgres")
}

fn baseline_path(feature: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../baselines/psql-oracle")
        .join(format!("{feature}.json"))
}

/// One corpus document: the regression file name and its frozen blob.
struct CorpusFile {
    name: String,
    blob: String,
}

fn corpus() -> Vec<CorpusFile> {
    CORPUS
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| {
            let (name, blob) = line
                .split_once('\t')
                .unwrap_or_else(|| panic!("corpus row has two fields: {line:?}"));
            assert!(
                blob.len() == 40 && blob.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "corpus row {name} has no Git blob ID",
            );
            CorpusFile {
                name: name.to_owned(),
                blob: blob.to_owned(),
            }
        })
        .collect()
}

impl CorpusFile {
    /// The frozen source, read by its Git blob ID from the `vendor/postgres`
    /// object database rather than from the checked-out tree.
    fn source(&self) -> String {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repository())
            .args(["cat-file", "blob", &self.blob])
            .output()
            .unwrap_or_else(|error| panic!("cannot run git cat-file: {error}"));
        assert!(
            output.status.success(),
            "cannot read the frozen corpus blob {} ({}). \
             Initialise the submodule: git submodule update --init vendor/postgres",
            self.blob,
            self.name,
        );
        String::from_utf8(output.stdout)
            .unwrap_or_else(|_| panic!("corpus blob {} is not UTF-8", self.blob))
    }
}

/// Documents shaped after pg-psql's own tests: every construct those tests
/// cover, read here by psql instead of by a written expectation.
///
/// These are held once, in source order by subject, and the count is pinned
/// in the baseline. A construct that only some target versions have is gated
/// like the grammar it exercises.
fn shaped_documents() -> Vec<(&'static str, String)> {
    let mut documents: Vec<(&'static str, String)> = vec![
        // Interpolation in every position.
        ("interp/target", "SELECT :v".into()),
        ("interp/literal", "SELECT :'v'".into()),
        ("interp/identifier", "SELECT :\"v\"".into()),
        ("interp/test", "SELECT :{?v}".into()),
        ("interp/argument", "SELECT f(:v)".into()),
        ("interp/typed-literal", "SELECT int :'v'".into()),
        ("interp/copy-target", "COPY t FROM :'f'".into()),
        ("interp/subquery", "SELECT (SELECT :v)".into()),
        ("interp/start-of-input", ":v".into()),
        ("interp/after-punctuation", "(:v)".into()),
        // Not an interpolation: every lexical shell psqlscan.l enters.
        ("shell/string", "SELECT ':v'".into()),
        ("shell/doubled-quote", "SELECT ':''v'".into()),
        ("shell/escape-string", "SELECT E':v'".into()),
        ("shell/escape-backslash", "SELECT E'\\\\:v'".into()),
        ("shell/unicode-string", "SELECT U&':v'".into()),
        ("shell/bit-string", "SELECT B'0:1'".into()),
        ("shell/hex-string", "SELECT X'0:1'".into()),
        ("shell/quoted-identifier", "SELECT \":v\"".into()),
        ("shell/unicode-identifier", "SELECT U&\":v\"".into()),
        ("shell/dollar-quote", "SELECT $$:v$$".into()),
        ("shell/tagged-dollar-quote", "SELECT $tag$:v$tag$".into()),
        ("shell/non-ascii-tag", "SELECT $tagé$:v$tagé$".into()),
        ("shell/line-comment", "SELECT 1 -- :v\nSELECT 2".into()),
        ("shell/block-comment", "SELECT 1 /* :v */".into()),
        ("shell/nested-block-comment", "SELECT 1 /* /* :v */ */".into()),
        ("shell/escaped-newline", "SELECT E'a\\\nb' :v".into()),
        // Operators that begin with a colon.
        ("colon/cast", "SELECT a::v".into()),
        ("colon/assignment", "SELECT a :=v".into()),
        ("colon/slice", "SELECT a[:2]".into()),
        ("colon/escaped", "SELECT \\:v".into()),
        ("colon/escaped-literal", "SELECT \\:'v'".into()),
        ("colon/escaped-then-interpolation", "SELECT \\::v".into()),
        // Incomplete interpolation forms, which fall back to a bare colon.
        ("incomplete/literal", "SELECT :'v b'".into()),
        ("incomplete/identifier", "SELECT :\"v b\"".into()),
        ("incomplete/test", "SELECT :{?v b}".into()),
        ("incomplete/brace", "SELECT :{v}".into()),
        ("incomplete/bare-colon", "SELECT : v".into()),
        // Terminators.
        ("terminator/one", "SELECT 1;".into()),
        ("terminator/two", "SELECT 1; SELECT 2;".into()),
        ("terminator/batch-separator", "SELECT 1 \\; SELECT 2;".into()),
        ("terminator/in-parentheses", "SELECT (1;".into()),
        ("terminator/in-string", "SELECT 'a;b';".into()),
        (
            "terminator/begin-atomic",
            "CREATE FUNCTION f() RETURNS int BEGIN ATOMIC SELECT 1; END;".into(),
        ),
        // Send commands.
        ("send/g", "SELECT 1 \\g".into()),
        ("send/gx", "SELECT 1 \\gx".into()),
        ("send/gset", "SELECT 1 \\gset".into()),
        ("send/gset-prefix", "SELECT 1 \\gset pre_".into()),
        ("send/gexec", "SELECT 1 \\gexec".into()),
        ("send/gdesc", "SELECT 1 \\gdesc".into()),
        ("send/crosstabview", "SELECT 1 \\crosstabview".into()),
        ("send/then-more-sql", "SELECT 1 \\gset\nSELECT 2;".into()),
        // A command name is read whole.
        ("name/gsetfoo", "SELECT 1 \\gsetfoo".into()),
        ("name/gexecx", "SELECT 1 \\gexecx".into()),
        ("name/getenv", "\\getenv abs_srcdir".into()),
        // Unmodelled meta-commands.
        ("meta/set", "\\set foo 1".into()),
        ("meta/d", "\\d users".into()),
        ("meta/timing", "\\timing on".into()),
        ("meta/bare-backslash", "SELECT 1 \\".into()),
        // Unterminated lexical regions.
        ("open/string", "SELECT 'a".into()),
        ("open/quoted-identifier", "SELECT \"a".into()),
        ("open/dollar-quote", "SELECT $$a".into()),
        ("open/block-comment", "SELECT /* a".into()),
        // Numbers, whose extent moved in 15.
        ("number/exponent-string", "SELECT 1e'\\' :v".into()),
        ("number/junk", "SELECT 123abc :v".into()),
        ("number/param-junk", "SELECT $1e'\\' :v".into()),
        ("number/comment-after", "SELECT 1e--c\n:v".into()),
        // Leading trivia.
        ("trivia/leading-comment", "-- a\nSELECT :v;".into()),
        ("trivia/leading-space", "   SELECT :v;".into()),
        ("trivia/between-statements", "SELECT 1;\n-- b\nSELECT 2;".into()),
        // COPY ... FROM STDIN, whose data psql reads itself.
        (
            "copy/from-stdin",
            "COPY t FROM STDIN;\n1\t:v\n\\.\nSELECT 2;".into(),
        ),
    ];
    // Added in 17: a vertical tab ends a command name
    // (docs/research/psql-14-19-syntax-changes.md, item A4).
    documents.push(("name/vertical-tab", "SELECT 1 \\g\x0bx".into()));
    // Added in 18: the pipeline and prepared-statement commands, two of
    // which send the buffer text and seven of which do not
    // (docs/research/psql-14-19-syntax-changes.md, class (A) item A1).
    #[cfg(feature = "since-pg18")]
    for (name, command) in [
        ("pipeline/parse", "parse"),
        ("pipeline/sendpipeline", "sendpipeline"),
        ("pipeline/startpipeline", "startpipeline"),
        ("pipeline/syncpipeline", "syncpipeline"),
        ("pipeline/endpipeline", "endpipeline"),
        ("pipeline/flush", "flush"),
        ("pipeline/flushrequest", "flushrequest"),
        ("pipeline/getresults", "getresults"),
        ("pipeline/close-prepared", "close_prepared"),
    ] {
        documents.push((name, format!("SELECT 1; SELECT 2 \\{command}\nSELECT 3;")));
    }
    documents
}

// --- What both sides report -------------------------------------------------

/// The facts a document yields, from either side.
#[derive(Debug, PartialEq, Eq)]
struct Facts {
    /// The text forwarded to the server.
    sql: String,
    /// Each variable interpolation: its extent in the document and its name.
    interpolations: Vec<(Range<usize>, String)>,
    /// The extent of each construct that ends the query buffer.
    submissions: Vec<Range<usize>>,
    /// Whether the document ends with a complete query buffer.
    complete: bool,
}

/// The extent of `part` inside `source`. Every token pg-psql reads is a
/// subslice of the parsed source.
fn span_of(source: &str, part: &str) -> Range<usize> {
    let base = source.as_ptr() as usize;
    let start = part.as_ptr() as usize;
    assert!(
        start >= base && start + part.len() <= base + source.len(),
        "token text is not a subslice of the parsed source",
    );
    (start - base)..(start - base + part.len())
}

/// The end offset of each `;` terminator token, in order. `Terminator::Semi`
/// carries no text, so its position comes from the lexer, as
/// `pg_psql::substitution` does.
fn semicolon_spans(source: &str) -> Vec<Range<usize>> {
    pg_psql::lex(source)
        .tokens()
        .filter(|token| token.kind() == pg_psql::TokenKind::SEMI)
        .map(|token| token.span().range())
        .collect()
}

/// What pg-psql makes of `source`. `None` when it refuses the document.
fn pg_psql_facts(source: &str, variables: &Variables) -> Option<Facts> {
    let document = pg_psql::parse(source).ok()?;
    let rendered = pg_psql::render(source, variables).ok()?;

    let mut interpolations = Vec::new();
    let mut submissions = Vec::new();
    let mut semicolons = semicolon_spans(source).into_iter();
    for item in &document.items {
        match item {
            PsqlItem::Interpolation(interpolation) => interpolations.push((
                span_of(source, interpolation.text()),
                interpolation.name().to_owned(),
            )),
            PsqlItem::Terminator(Terminator::Semi) => submissions.push(
                semicolons
                    .next()
                    .expect("each `;` terminator is one SEMI token"),
            ),
            PsqlItem::Terminator(Terminator::Send(send)) => {
                submissions.push(span_of(source, send.text()))
            }
            #[cfg(feature = "since-pg18")]
            PsqlItem::Terminator(Terminator::Discard(discard)) => {
                submissions.push(span_of(source, discard.text()))
            }
            PsqlItem::Sql(_) => {}
        }
    }

    Some(Facts {
        sql: rendered.sql().to_owned(),
        interpolations,
        submissions,
        // pg-psql has no notion of an incomplete query buffer: it has no
        // open lexical region, so a document it parses at all is complete.
        // Where psql is still inside a string, a comment or a dollar-quoted
        // body, that is the `open-lexical-region` gap.
        complete: true,
    })
}

/// What psql makes of `source`.
///
/// psql resets the query buffer at every submission and sends each one on its
/// own, while pg-psql renders one SQL string for the whole document (ADR
/// 0007). The two are made comparable here, on psql's own facts: a send
/// command becomes the `;` that separates the two statements it stands
/// between, and a discard command (18) removes the buffer it ends, keeping
/// the whitespace before it. That is exactly what
/// `pg_psql::substitution::render_document` does.
fn oracle_facts(source: &str, variables: &[(&str, &str)]) -> Facts {
    let scanned = pg_psql_oracle::scan(source, variables);
    let raw = scanned.sql();
    let mut sql = String::with_capacity(raw.len());
    let mut copied = 0;
    let mut buffer_start = 0;
    let mut interpolations = Vec::new();
    let mut submissions = Vec::new();
    for event in scanned.events() {
        match event {
            Event::Interpolation { name, source, .. } => {
                if let Some(range) = source {
                    interpolations.push((range.clone(), name.clone()));
                }
            }
            Event::Terminator { source, output } => {
                submissions.push(source.clone());
                sql.push_str(&raw[copied..output.end]);
                copied = output.end;
                buffer_start = sql.len();
            }
            Event::Backslash {
                name,
                command,
                output,
                ..
            } => {
                sql.push_str(&raw[copied..*output]);
                copied = *output;
                match pg_psql_oracle::classify(name) {
                    Command::Send => {
                        submissions.push(command.clone());
                        sql.push(';');
                        buffer_start = sql.len();
                    }
                    Command::Discard => {
                        submissions.push(command.clone());
                        let buffer = &sql[buffer_start..];
                        let leading = buffer.len() - buffer.trim_start().len();
                        sql.truncate(buffer_start + leading);
                        buffer_start = sql.len();
                    }
                    Command::Other => {}
                }
            }
            Event::End { .. } | Event::Incomplete { .. } => {}
        }
    }
    sql.push_str(&raw[copied..]);

    Facts {
        sql,
        interpolations,
        submissions,
        complete: scanned.is_complete(),
    }
}

// --- One document's outcome -------------------------------------------------

/// Which of the four comparisons agreed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    text: bool,
    interpolations: bool,
    submissions: bool,
    complete: bool,
}

impl Outcome {
    fn agrees(self) -> bool {
        self.text && self.interpolations && self.submissions && self.complete
    }

    /// The comparisons that disagreed, in a stable order.
    fn reasons(self) -> Vec<&'static str> {
        [
            (!self.text, "text"),
            (!self.interpolations, "interpolations"),
            (!self.submissions, "submissions"),
            (!self.complete, "complete"),
        ]
        .into_iter()
        .filter_map(|(failed, name)| failed.then_some(name))
        .collect()
    }
}

/// The result of reading one document with both sides.
#[derive(Debug)]
struct Comparison {
    outcome: Outcome,
    /// Why the document does not agree; `""` when it does.
    class: &'static str,
    /// The first difference, for the failure message.
    detail: String,
}

/// Reads `source` with both sides, unbound and then with every variable psql
/// recognised bound to an inert value.
fn compare(source: &str) -> Comparison {
    let scanned = pg_psql_oracle::scan(source, &[]);
    let unbound_oracle = oracle_facts(source, &[]);

    // Bind each variable psql recognised. The values hold no colon, quote,
    // backslash or semicolon, so psql's rescan of a `:name` value echoes it
    // unchanged and the two sides stay comparable even though pg-psql
    // substitutes in one pass (ADR 0007).
    let names = unbound_oracle
        .interpolations
        .iter()
        .map(|(_, name)| name.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let bindings = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), format!("Vv{index}")))
        .collect::<Vec<_>>();
    let oracle_bindings = bindings
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let variables = bindings.iter().cloned().collect::<Variables>();

    let bound_oracle = oracle_facts(source, &oracle_bindings);
    let unbound = pg_psql_facts(source, &Variables::new());
    let bound = pg_psql_facts(source, &variables);

    let Some(unbound_psql) = unbound else {
        // pg-psql refused the document.
        let outcome = Outcome {
            text: false,
            interpolations: false,
            submissions: false,
            complete: !unbound_oracle.complete,
        };
        return Comparison {
            outcome,
            class: classify(&scanned, None, &unbound_oracle, outcome),
            detail: format!(
                "pg-psql refuses the document; psql ends with {:?}",
                scanned.final_prompt(),
            ),
        };
    };
    let bound_psql = bound.expect("a document that parses unbound also parses bound");

    let outcome = Outcome {
        text: unbound_psql.sql == unbound_oracle.sql && bound_psql.sql == bound_oracle.sql,
        interpolations: unbound_psql.interpolations == unbound_oracle.interpolations,
        submissions: unbound_psql.submissions == unbound_oracle.submissions,
        complete: unbound_psql.complete == unbound_oracle.complete,
    };
    let detail = if outcome.agrees() {
        String::new()
    } else if unbound_psql.sql != unbound_oracle.sql {
        first_difference(&unbound_psql.sql, &unbound_oracle.sql)
    } else if bound_psql.sql != bound_oracle.sql {
        first_difference(&bound_psql.sql, &bound_oracle.sql)
    } else {
        format!(
            "pg-psql {:?} / {:?}; psql {:?} / {:?}",
            unbound_psql.interpolations,
            unbound_psql.submissions,
            unbound_oracle.interpolations,
            unbound_oracle.submissions,
        )
    };
    Comparison {
        outcome,
        class: classify(&scanned, Some(&unbound_psql), &unbound_oracle, outcome),
        detail,
    }
}

/// A short, readable account of where two forwarded texts part.
fn first_difference(psql_side: &str, oracle_side: &str) -> String {
    let at = psql_side
        .bytes()
        .zip(oracle_side.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(psql_side.len().min(oracle_side.len()));
    let window = |text: &str| {
        let start = at.saturating_sub(20).min(text.len());
        let end = (at + 30).min(text.len());
        text.get(start..end).unwrap_or("<not a boundary>").to_owned()
    };
    format!(
        "forwarded text differs at byte {at}: pg-psql {:?} vs psql {:?}",
        window(psql_side),
        window(oracle_side),
    )
}

/// Why a document does not agree.
///
/// Every class is a deliberate limit of what pg-psql models, and a
/// disagreement that fits none of them is `unclassified`, so a new one cannot
/// be recorded without a reason. The order of the rules is the order of
/// causes: a later rule only speaks for a document no earlier one explains.
fn classify(
    scanned: &pg_psql_oracle::Scan,
    psql: Option<&Facts>,
    oracle: &Facts,
    outcome: Outcome,
) -> &'static str {
    if outcome.agrees() {
        return "";
    }
    // The document reaches end of input inside a string, a quoted identifier,
    // a dollar-quoted body or a comment. psql asks for another line; pg-psql
    // has no open region, so it either refuses the document or reads the
    // region's text as other tokens (ADR 0007, freshtonic/recursa#131).
    if !scanned.is_complete() && scanned.final_prompt().is_open_lexical_region() {
        return "open-lexical-region";
    }
    if psql.is_none() {
        return "refused";
    }
    // psql stops SQL lexing at every backslash command and reads the rest of
    // its line as arguments. pg-psql keeps a command it does not model as
    // ordinary text (freshtonic/pg-sql#11, #12, #13), so the forwarded text
    // and the boundaries both move.
    if scanned
        .backslash_commands()
        .any(|name| pg_psql_oracle::classify(name) == Command::Other)
    {
        return "unmodelled-meta-command";
    }
    // A send or discard command can take arguments. psql reads them and the
    // server never sees them; pg-psql ends the statement at the command name
    // and forwards the rest as SQL text.
    if scanned.events().iter().any(|event| match event {
        Event::Backslash {
            source, command, ..
        } => source.end > command.end,
        _ => false,
    }) {
        return "send-command-arguments";
    }
    // psql suppresses the `;` boundary inside parentheses and inside a
    // tracked `BEGIN ... END` block of a `CREATE FUNCTION`. pg-psql tracks
    // neither, because it renders one string for the whole document rather
    // than splitting it into submissions, so it only ever has fewer psql
    // boundaries than its own.
    let psql = psql.expect("the refusal case returned above");
    if oracle.submissions.len() < psql.submissions.len()
        && is_subsequence(&oracle.submissions, &psql.submissions)
    {
        return "boundary-suppressed-by-nesting";
    }
    "unclassified"
}

/// Whether `part` appears in `whole` in order.
fn is_subsequence(part: &[Range<usize>], whole: &[Range<usize>]) -> bool {
    let mut remaining = whole.iter();
    part.iter()
        .all(|wanted| remaining.any(|candidate| candidate == wanted))
}

// --- The pinned baseline ----------------------------------------------------

/// The oracle pin of `feature`, from `pg-oracle/pins.tsv`.
fn oracle_pin(feature: &str) -> (String, String) {
    ORACLE_PINS
        .lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .find_map(|line| {
            let fields = line.split('\t').collect::<Vec<_>>();
            (fields[0] == feature).then(|| (fields[1].to_owned(), fields[2].to_owned()))
        })
        .unwrap_or_else(|| panic!("pins.tsv has no pin for {feature}"))
}

/// Reads every document with both sides, in a stable order.
fn run_all() -> Vec<(String, Comparison)> {
    let mut results = Vec::new();
    for document in corpus() {
        let comparison = compare(&document.source());
        results.push((document.name.clone(), comparison));
    }
    for (name, source) in shaped_documents() {
        results.push((format!("shaped/{name}"), compare(&source)));
    }
    results
}

fn render_baseline(feature: &str, results: &[(String, Comparison)]) -> String {
    let (reference, commit) = oracle_pin(feature);
    let agree = results
        .iter()
        .filter(|(_, comparison)| comparison.outcome.agrees())
        .count();
    let mut by_class: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, comparison) in results {
        if !comparison.outcome.agrees() {
            *by_class.entry(comparison.class).or_default() += 1;
        }
    }

    let mut out = String::new();
    out.push_str("{\n  \"schema_version\": 1,\n");
    out.push_str(&format!(
        "  \"corpus\": {{ \"name\": \"postgresql-17.9\", \"commit\": \"{CORPUS_COMMIT}\" }},\n"
    ));
    out.push_str(&format!(
        "  \"oracle\": {{ \"feature\": \"{feature}\", \"ref\": \"{reference}\", \
         \"commit\": \"{commit}\" }},\n"
    ));
    out.push_str(&format!(
        "  \"totals\": {{ \"documents\": {}, \"agree\": {}, \"gaps\": {} }},\n",
        results.len(),
        agree,
        results.len() - agree,
    ));
    out.push_str("  \"gap_classes\": {");
    let classes = by_class
        .iter()
        .map(|(class, count)| format!(" \"{class}\": {count}"))
        .collect::<Vec<_>>()
        .join(",");
    out.push_str(&classes);
    out.push_str(if classes.is_empty() { "},\n" } else { " },\n" });
    out.push_str("  \"gaps\": [\n");
    let gaps = results
        .iter()
        .filter(|(_, comparison)| !comparison.outcome.agrees())
        .map(|(name, comparison)| {
            format!(
                "    {{ \"document\": \"{name}\", \"class\": \"{}\", \"reasons\": [{}] }}",
                comparison.class,
                comparison
                    .outcome
                    .reasons()
                    .iter()
                    .map(|reason| format!("\"{reason}\""))
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");
    out.push_str(&gaps);
    out.push_str(if gaps.is_empty() { "  ]\n}\n" } else { "\n  ]\n}\n" });
    out
}

// --- The tests --------------------------------------------------------------

#[test]
fn the_oracle_is_the_pin_of_the_target_version() {
    let feature = target_version();
    let (reference, commit) = oracle_pin(feature);
    assert_eq!(pg_psql_oracle::POSTGRES_REF, reference);
    assert_eq!(pg_psql_oracle::POSTGRES_COMMIT, commit);
}

#[test]
fn the_corpus_is_the_frozen_regression_corpus() {
    let files = corpus();
    assert_eq!(
        files.len(),
        225,
        "the frozen corpus has 226 SQL files, one of which is not UTF-8",
    );
    let names = files.iter().map(|file| &file.name).collect::<Vec<_>>();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "corpus names are unique");
}

#[test]
fn every_document_matches_its_pinned_outcome() {
    let feature = target_version();
    let path = baseline_path(feature);
    let recorded: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display())),
    )
    .expect("the baseline is JSON");

    let (reference, commit) = oracle_pin(feature);
    assert_eq!(recorded["oracle"]["ref"].as_str(), Some(reference.as_str()));
    assert_eq!(
        recorded["oracle"]["commit"].as_str(),
        Some(commit.as_str())
    );
    assert_eq!(recorded["corpus"]["commit"].as_str(), Some(CORPUS_COMMIT));

    let expected = recorded["gaps"]
        .as_array()
        .expect("the baseline lists its gaps")
        .iter()
        .map(|gap| {
            (
                gap["document"].as_str().expect("gap document").to_owned(),
                (
                    gap["class"].as_str().expect("gap class").to_owned(),
                    gap["reasons"]
                        .as_array()
                        .expect("gap reasons")
                        .iter()
                        .map(|reason| reason.as_str().expect("a reason").to_owned())
                        .collect::<Vec<_>>(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let results = run_all();
    assert_eq!(
        results.len() as u64,
        recorded["totals"]["documents"].as_u64().unwrap(),
        "the document count moved; regenerate the baseline",
    );

    let mut unexpected = Vec::new();
    let mut resolved = Vec::new();
    for (name, comparison) in &results {
        let reasons = comparison
            .outcome
            .reasons()
            .iter()
            .map(|reason| (*reason).to_owned())
            .collect::<Vec<_>>();
        match (expected.get(name), comparison.outcome.agrees()) {
            (None, true) => {}
            (None, false) => unexpected.push(format!(
                "{name}: {} ({}) — {}",
                reasons.join("+"),
                comparison.class,
                comparison.detail,
            )),
            (Some(_), true) => resolved.push(name.clone()),
            (Some((class, expected_reasons)), false) => {
                assert_eq!(
                    (&comparison.class.to_owned(), &reasons),
                    (class, expected_reasons),
                    "{name} disagrees for a different reason than the baseline records",
                );
            }
        }
    }
    assert!(
        unexpected.is_empty(),
        "{} documents disagree with psql and are not in the baseline:\n{}",
        unexpected.len(),
        unexpected.join("\n"),
    );
    assert!(
        resolved.is_empty(),
        "{} recorded psql gaps now agree; remove them from the baseline:\n{}",
        resolved.len(),
        resolved.join("\n"),
    );

    // No gap may be recorded without a reason the crate documents.
    for (_, (class, _)) in &expected {
        assert!(
            [
                "unmodelled-meta-command",
                "send-command-arguments",
                "boundary-suppressed-by-nesting",
                "open-lexical-region",
                "refused",
            ]
            .contains(&class.as_str()),
            "unknown psql gap class {class:?}",
        );
    }
}

/// Writes the baseline of the build's target version. Run it after a change
/// of pin, of oracle or of the psql grammar, and review the diff.
#[test]
#[ignore = "writes baselines/psql-oracle/<feature>.json"]
fn regenerate_psql_oracle_baseline() {
    let feature = target_version();
    let results = run_all();
    for (name, comparison) in &results {
        if !comparison.outcome.agrees() {
            println!(
                "gap {name}: {} ({}) — {}",
                comparison.outcome.reasons().join("+"),
                comparison.class,
                comparison.detail,
            );
        }
    }
    let path = baseline_path(feature);
    std::fs::create_dir_all(path.parent().expect("the baseline has a directory"))
        .expect("create the baseline directory");
    std::fs::write(&path, render_baseline(feature, &results)).expect("write the baseline");
    println!("wrote {}", path.display());
}
