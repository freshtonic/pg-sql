//! The reported minimum version against the oracle of every target version.
//!
//! ADR 0010 asks that, for each statement of the frozen corpus, the reported
//! minimum version equal the oldest target version whose oracle accepts it.
//! The six version baselines already record that oracle outcome for every
//! statement of every version (`baselines/target-versions/*.json`), and the
//! differential gate keeps each one true of its oracle. So one build checks
//! its own reports against all six oracles, and `scripts/gate-versions` runs
//! the check once for each version. The check itself needs no PostgreSQL
//! build: it reads only the frozen corpus and the pinned baselines.
//!
//! The equality holds with two named exceptions, because an oracle records
//! only "accepts" or "rejects" and cannot say what it parsed. Both are frozen
//! lists below, and `docs/minimum-version.md` explains them.
//!
//! - **An older grammar reads the same text as something else.** Before 17,
//!   `JSON_VALUE(j, '$.a')` is an ordinary function call, and before 16
//!   `SELECT 0x42F` is the integer `0` with the column label `x42F`. The
//!   older oracle accepts the text, so the report looks too high, and it is
//!   right: the statement that this build parsed needs the newer version.
//! - **A widening that depends on a word, not on a shape.** From 16 `REVOKE
//!   ColId OPTION FOR` takes any name, where 15 takes only `ADMIN`. The
//!   requirement belongs to which word appears, which no gate can declare, so
//!   the report is too low.

#[allow(dead_code)]
#[path = "support/baseline.rs"]
mod baseline;

use pg_sql::minimum_version::{lexical_minimum_version, minimum_version};
use pg_sql::{TargetVersion, ast::Statement, lex};

/// The scan that a lexical gate answers, named as a construct.
const LEXICAL: &str = "<lexical>";

/// The constructs whose gate an older grammar reads as something else.
///
/// A report from one of these may name a version above the oldest one whose
/// oracle accepts the text. A report from any other construct may not.
const REINTERPRETED_BY_AN_OLDER_GRAMMAR: &[&str] = &[
    // A word that is a keyword only from 15 or later. Before it the word is
    // an identifier, so the same text is a function call, a column reference,
    // a type name, or an older form of the same clause.
    "AlterPublicationAction::AddObjs",
    "AlterPublicationAction::DropObjs",
    "AlterPublicationAction::SetObjs",
    "Expr::JsonArray",
    "Expr::JsonArrayAgg",
    "Expr::JsonCtor",
    "Expr::JsonExists",
    "Expr::JsonObject",
    "Expr::JsonQuery",
    "Expr::JsonScalar",
    "Expr::JsonSerialize",
    "Expr::JsonValue",
    "FunctionBuiltinTypeName::Json",
    "PublicationForClause::Objects",
    "SqlValueFunction::SystemUser",
    "TypeCastFunc::Json",
    // A lexical gate: an older lexer reads the same characters as more than
    // one token. `pg_sql::minimum_version::LEXICAL_GATES` names them.
    LEXICAL,
];

/// The corpus statements whose requirement no gate can declare, because the
/// newer grammar widened an admission set and the requirement depends on
/// which word appears, not on the shape of the value.
///
/// - `ALTER OPERATOR ... SET (MERGES)`: from 17 `operator_def_elem` accepts a
///   bare `ColLabel` (REL_17_11 gram.y 10252; REL_16_15 gram.y 10103).
/// - `REVOKE ColId OPTION FOR`: from 16 the name is any `ColId`, where
///   REL_15_19 gram.y has only `REVOKE ADMIN OPTION FOR` (commit e3ce2de09).
const UNRECORDED_WIDENINGS: &[(&str, usize)] = &[
    ("alter_operator.sql", 32),
    ("alter_operator.sql", 33),
    ("alter_operator.sql", 42),
    ("alter_operator.sql", 43),
    ("create_role.sql", 79),
    ("privileges.sql", 84),
    ("privileges.sql", 237),
];

/// What one statement reports: the version, and the construct that set it.
struct Reported {
    version: TargetVersion,
    construct: &'static str,
}

/// Lexes one frozen corpus statement without its document-level semicolon,
/// exactly as the differential check does, so `Statement` sees the statement
/// and not the terminator.
fn lex_statement_source(source: &str) -> pg_sql::LexResult<'_> {
    let lexed = lex(source);
    if lexed.errors().next().is_some() {
        return lexed;
    }
    let terminator = lexed
        .tokens()
        .last()
        .filter(|token| token.kind() == pg_sql::TokenKind::SEMI)
        .map(|token| token.span());
    let Some(terminator) = terminator else {
        return lexed;
    };
    let mut statement = pg_sql::LexBuilder::new(source);
    for token in lexed.tokens() {
        if token.span() != terminator {
            statement
                .append(token.kind(), token.span())
                .expect("tokens copied from pg-sql lexing retain valid ordered spans");
        }
    }
    statement.finish()
}

/// The reported minimum version of one statement, from both scans.
///
/// `None` means this build does not parse the statement as one complete
/// statement, so it has nothing to report about it.
fn reported_minimum(source: &str) -> Option<Reported> {
    let lexed = lex_statement_source(source);
    if lexed.errors().next().is_some() {
        return None;
    }
    let lexical = lexical_minimum_version(lexed.tokens());
    let mut input = lexed.input();
    let parsed = Statement::parse(&mut input).ok()?;
    if !input.is_eof() {
        return None;
    }
    let item = minimum_version(parsed.ast());
    // The two scans answer about the same statement, so the minimum of the
    // whole statement is the higher of the two.
    Some(match (item, lexical) {
        (Some(item), Some(lexical)) if lexical.version() > item.version() => Reported {
            version: lexical.version(),
            construct: LEXICAL,
        },
        (Some(item), _) => Reported {
            version: item.version(),
            construct: item.construct(),
        },
        (None, Some(lexical)) => Reported {
            version: lexical.version(),
            construct: LEXICAL,
        },
        (None, None) => Reported {
            version: TargetVersion::Pg14,
            construct: "",
        },
    })
}

/// The six target versions, oldest first.
const VERSIONS: [TargetVersion; 6] = [
    TargetVersion::Pg14,
    TargetVersion::Pg15,
    TargetVersion::Pg16,
    TargetVersion::Pg17,
    TargetVersion::Pg18,
    TargetVersion::Pg19Beta,
];

#[test]
fn the_reported_minimum_is_the_oldest_version_whose_oracle_accepts_the_statement() {
    let frozen = baseline::FrozenStatements::pinned();
    let oracles =
        VERSIONS.map(|version| (version, baseline::VersionBaseline::of(version.feature())));
    let build = pg_sql::TARGET_VERSION;

    let mut checked = 0_usize;
    let mut over_accepted = 0_usize;
    let mut reported_above_the_baseline = 0_usize;
    let mut unrecorded = Vec::new();

    for name in frozen.file_names() {
        let file = frozen.file(name);
        let source = file.source();
        let statements = file
            .statements(&source)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let accepts = oracles
            .each_ref()
            .map(|(version, baseline)| (*version, baseline.oracle_accepts(name)));

        for (index, statement) in statements.iter().enumerate() {
            let Some(reported) = reported_minimum(statement) else {
                continue;
            };
            let build_accepts = accepts
                .iter()
                .find(|(version, _)| *version == build)
                .is_some_and(|(_, outcomes)| outcomes[index]);
            if !build_accepts {
                // pg-sql parses what its own oracle rejects: a frozen
                // over-acceptance gap (`tests/accepted_legacy_gaps.rs`), not a
                // version question.
                over_accepted += 1;
                continue;
            }
            checked += 1;
            if reported.version > TargetVersion::Pg14 {
                reported_above_the_baseline += 1;
            }

            // No under-report: every version from the reported minimum up to
            // this build must accept the statement. A report that is too low
            // would let a consumer send the statement to a server that
            // refuses it, which is the dangerous direction.
            let rejected_above = accepts.iter().any(|(version, outcomes)| {
                *version >= reported.version && *version <= build && !outcomes[index]
            });
            if rejected_above {
                assert!(
                    UNRECORDED_WIDENINGS.contains(&(name, index)),
                    "{name}:{index}: reported {:?} from {}, but an oracle at or above it \
                     rejects the statement\n  {statement}",
                    reported.version,
                    reported.construct
                );
                unrecorded.push((name, index));
            }

            // No unexplained over-report: every version below the reported
            // minimum must reject the statement, unless the construct is one
            // that an older grammar reads as something else.
            let accepted_below = accepts
                .iter()
                .any(|(version, outcomes)| *version < reported.version && outcomes[index]);
            if accepted_below {
                assert!(
                    REINTERPRETED_BY_AN_OLDER_GRAMMAR.contains(&reported.construct),
                    "{name}:{index}: reported {:?} from {}, but an older oracle accepts the \
                     statement and the construct is not a known reinterpretation\n  {statement}",
                    reported.version,
                    reported.construct
                );
            }
        }
    }

    assert!(
        over_accepted < 100,
        "pg-sql parses {over_accepted} statements that its own oracle rejects; the frozen \
         over-acceptance gaps are few"
    );
    assert!(
        checked > 30_000,
        "the check must cover the corpus, not a few statements: {checked}"
    );
    assert!(
        reported_above_the_baseline > 0,
        "the corpus must exercise at least one gate on {build:?}"
    );
    // Every frozen widening must still be one: a gate that later records the
    // requirement takes its statement off the list, and the list must shrink.
    let expected = UNRECORDED_WIDENINGS
        .iter()
        .filter(|(name, index)| {
            oracles
                .iter()
                .find(|(version, _)| *version == build)
                .is_some_and(|(_, baseline)| baseline.oracle_accepts(name)[*index])
        })
        .count();
    assert_eq!(
        unrecorded.len(),
        expected,
        "the frozen list of unrecorded widenings is {UNRECORDED_WIDENINGS:?}, and \
         {unrecorded:?} of them were seen on {build:?}"
    );
}
