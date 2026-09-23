//! The lowest PostgreSQL version that a parsed value needs.
//!
//! A build reproduces the raw parser of one target version (ADR 0009), but a
//! consumer can analyse SQL for a server that is older than the build. It must
//! reject every construct that the older grammar cannot parse, and give the
//! message that server gives. This module answers that question from the
//! version gates themselves, so no hand-kept list can drift from them
//! (ADR 0010).
//!
//! Each version gate is a `#[config(since = pgN)]` declaration on an AST node,
//! field or variant. Recursa removes the element from an older build, exactly
//! as the earlier `#[cfg]` did, and records the requirement beside the node.
//! A scan over one parsed value then reports the construct that sets the
//! minimum.
//!
//! # Two questions
//!
//! [`minimum_version`] gives the lowest version whose grammar accepts the
//! whole value, with the first construct, in source order, that sets it.
//!
//! [`minimum_version_above`] gives the first construct, in source order, whose
//! requirement is above a target version. This is the one a consumer reports:
//! it names the construct the older server rejects first.
//!
//! The [`MinimumVersion`] trait gives the same two answers as methods, so a
//! caller writes `parsed.ast().minimum_version_above(target)`.
//!
//! The two answers differ, and both are right. A value can need 17 because of
//! a construct late in the statement while an earlier construct needs 16.
//!
//! # Example
//!
//! ```
//! use pg_sql::ast::Statement;
//! use pg_sql::minimum_version::{minimum_version, minimum_version_above};
//! use pg_sql::TargetVersion;
//!
//! let lexed = pg_sql::lex("SELECT * FROM (SELECT 1)");
//! let mut input = lexed.input();
//! let parsed = Statement::parse(&mut input).expect("statement");
//!
//! // A subquery in FROM needs no alias only from PostgreSQL 16.
//! let minimum = minimum_version(parsed.ast()).expect("a requirement above 14");
//! assert_eq!(minimum.version(), TargetVersion::Pg16);
//! assert_eq!(minimum.message(), Some("subquery in FROM must have an alias"));
//! assert_eq!(minimum.sqlstate(), Some("42601"));
//!
//! // A PostgreSQL 15 server rejects it; a PostgreSQL 16 server does not.
//! assert!(minimum_version_above(parsed.ast(), TargetVersion::Pg15).is_some());
//! assert!(minimum_version_above(parsed.ast(), TargetVersion::Pg16).is_none());
//! ```
//!
//! # The lexical gates
//!
//! A gate on a `tokens!` entry removes the entry but records nothing, because
//! two same-named entries under opposite predicates cannot be told apart in a
//! parsed value. A lexical requirement is therefore not a property of the
//! value's shape, and the scan over the AST cannot report it.
//!
//! [`lexical_minimum_version`] and [`lexical_minimum_version_above`] answer
//! that part from the tokens, against [`LEXICAL_GATES`], which names every
//! `tokens!` gate that widens what a build accepts. A consumer that reports
//! one diagnostic per statement runs both scans and keeps the higher version,
//! or, above a floor, the earlier span.
//!
//! # Provenance
//!
//! [`VersionRequirement::span`] carries the extent of the construct when the
//! parse retained it. This grammar builds an arena-backed AST, and Recursa
//! does not yet run the requirement scan from an
//! [`ArenaParsed`](recursa::ArenaParsed) root cursor, so the span is `None`
//! today. Use the statement's own
//! [`source_bounds`](recursa::ArenaParsed::source_bounds) until Recursa adds
//! it.

use recursa::{Requirement, Requires, Span};

use crate::TargetVersion;

/// One construct of a parsed value that needs a newer PostgreSQL version.
///
/// It names the version, the construct, and, where `gram.y` raises a specific
/// `ereport` in the older version, the SQLSTATE, message and hint that server
/// gives. A gate that carries message text also cites the `gram.y` line the
/// text comes from (`CLAUDE.md` principle 11), which
/// [`citation`](Self::citation) reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VersionRequirement {
    /// The lowest target version whose grammar accepts the construct.
    version: TargetVersion,
    /// The extent of the construct, when it is known.
    span: Option<Span>,
    /// The authored path of the construct.
    construct: &'static str,
    /// Whether the requirement belongs to a missing value.
    absence: bool,
    /// The SQLSTATE the older server gives, when it has a specific one.
    code: Option<&'static str>,
    /// The message the older server gives, under the same condition.
    message: Option<&'static str>,
    /// The hint the older server gives, under the same condition.
    hint: Option<&'static str>,
    /// The source that justifies the gate.
    citation: Option<&'static str>,
}

impl VersionRequirement {
    /// Rebuilds one requirement from the generated scan over a parsed value.
    fn from_scan(requirement: Requirement) -> Self {
        let note = requirement.note();
        let configuration = crate::Configuration::from_id(requirement.configuration())
            .expect("a recorded requirement names a declared configuration");
        Self {
            version: target_version(configuration),
            span: requirement.span(),
            construct: note.site(),
            absence: note.is_absence(),
            code: note.code(),
            message: note.message(),
            hint: note.hint(),
            citation: note.citation(),
        }
    }

    /// The lowest target version whose grammar accepts this construct.
    pub const fn version(&self) -> TargetVersion {
        self.version
    }

    /// The extent of the construct, when it is known.
    ///
    /// A lexical requirement always carries the span of its token. An item or
    /// shape requirement carries the span only when the parse retained
    /// provenance; see the module note on provenance.
    pub const fn span(&self) -> Option<Span> {
        self.span
    }

    /// The authored path of the construct, for example `ParenQueryRef.alias`.
    ///
    /// It names the gate, not the SQL. A diagnostic shows the source, not
    /// this; it is for a log line or a bug report.
    pub const fn construct(&self) -> &'static str {
        self.construct
    }

    /// Whether the requirement comes from a missing value, not a present one.
    ///
    /// A newer grammar sometimes makes optional what an older one required, so
    /// the requirement belongs to the absence. A subquery in `FROM` with no
    /// alias is the first such rule: PostgreSQL 15 requires the alias.
    pub const fn is_absence(&self) -> bool {
        self.absence
    }

    /// The SQLSTATE the older server gives, when `gram.y` raises a specific
    /// `ereport` for this construct.
    ///
    /// `None` means the older grammar gives a plain syntax error.
    pub const fn sqlstate(&self) -> Option<&'static str> {
        self.code
    }

    /// The message the older server gives, with the same condition as
    /// [`sqlstate`](Self::sqlstate).
    pub const fn message(&self) -> Option<&'static str> {
        self.message
    }

    /// The hint the older server gives, with the same condition as
    /// [`sqlstate`](Self::sqlstate).
    pub const fn hint(&self) -> Option<&'static str> {
        self.hint
    }

    /// The source that justifies the gate: the `gram.y` or `scan.l` rule, and
    /// the line of the `ereport` when the gate carries message text.
    pub const fn citation(&self) -> Option<&'static str> {
        self.citation
    }
}

/// Returns the lowest target version whose grammar accepts `value`.
///
/// The answer names the first construct, in source order, that sets the
/// version. `None` means PostgreSQL 14, the oldest supported version, accepts
/// the whole value.
///
/// Pass the AST of a parse: `parsed.ast()`.
///
/// ```
/// use pg_sql::ast::Statement;
/// use pg_sql::TargetVersion;
/// use pg_sql::minimum_version::minimum_version;
///
/// let lexed = pg_sql::lex("SELECT 1");
/// let mut input = lexed.input();
/// let parsed = Statement::parse(&mut input).expect("statement");
/// // Every supported version parses this statement.
/// assert!(minimum_version(parsed.ast()).is_none());
///
/// let lexed = pg_sql::lex("MERGE INTO t USING s ON t.a = s.a WHEN MATCHED THEN DELETE");
/// let mut input = lexed.input();
/// let parsed = Statement::parse(&mut input).expect("statement");
/// // MERGE arrived in PostgreSQL 15.
/// assert_eq!(
///     minimum_version(parsed.ast()).map(|found| found.version()),
///     Some(TargetVersion::Pg15)
/// );
/// ```
pub fn minimum_version<T: Requires + ?Sized>(value: &T) -> Option<VersionRequirement> {
    value
        .minimum_requirement()
        .map(VersionRequirement::from_scan)
}

/// The two answers as methods on any parsed value of this grammar.
///
/// It is the same pair as [`minimum_version`] and [`minimum_version_above`].
/// A consumer that reports one diagnostic per statement usually wants
/// [`minimum_version_above`](Self::minimum_version_above).
///
/// ```
/// use pg_sql::ast::Statement;
/// use pg_sql::{MinimumVersion, TargetVersion};
///
/// let lexed = pg_sql::lex("SELECT * FROM (SELECT 1)");
/// let mut input = lexed.input();
/// let parsed = Statement::parse(&mut input).expect("statement");
///
/// let rejected = parsed
///     .ast()
///     .minimum_version_above(TargetVersion::Pg15)
///     .expect("PostgreSQL 15 rejects the missing alias");
/// assert_eq!(rejected.version(), TargetVersion::Pg16);
/// assert_eq!(rejected.sqlstate(), Some("42601"));
/// ```
pub trait MinimumVersion {
    /// See [`minimum_version`].
    fn minimum_version(&self) -> Option<VersionRequirement>;

    /// See [`minimum_version_above`].
    fn minimum_version_above(&self, target: TargetVersion) -> Option<VersionRequirement>;
}

impl<T: Requires + ?Sized> MinimumVersion for T {
    fn minimum_version(&self) -> Option<VersionRequirement> {
        minimum_version(self)
    }

    fn minimum_version_above(&self, target: TargetVersion) -> Option<VersionRequirement> {
        minimum_version_above(self, target)
    }
}

/// Returns the first construct, in source order, that `target` rejects.
///
/// This is the question a consumer asks about a server older than the build:
/// the answer is the construct that server refuses first, with the message it
/// gives where `gram.y` has one. `None` means `target` accepts the whole
/// value.
///
/// Pass the AST of a parse: `parsed.ast()`.
///
/// ```
/// use pg_sql::ast::Statement;
/// use pg_sql::TargetVersion;
/// use pg_sql::minimum_version::minimum_version_above;
///
/// let lexed = pg_sql::lex("CREATE STATISTICS ON a, b FROM t");
/// let mut input = lexed.input();
/// let parsed = Statement::parse(&mut input).expect("statement");
///
/// // The name of a statistics object is optional only from PostgreSQL 16.
/// let found = minimum_version_above(parsed.ast(), TargetVersion::Pg15).expect("rejected by 15");
/// assert_eq!(found.version(), TargetVersion::Pg16);
/// assert!(found.is_absence());
/// assert!(minimum_version_above(parsed.ast(), TargetVersion::Pg16).is_none());
/// ```
pub fn minimum_version_above<T: Requires + ?Sized>(
    value: &T,
    target: TargetVersion,
) -> Option<VersionRequirement> {
    value
        .requirement_above(configuration(target).id())
        .map(VersionRequirement::from_scan)
}

/// Maps a declared grammar configuration onto the target version it names.
const fn target_version(configuration: crate::Configuration) -> TargetVersion {
    match configuration {
        crate::Configuration::Pg14 => TargetVersion::Pg14,
        crate::Configuration::Pg15 => TargetVersion::Pg15,
        crate::Configuration::Pg16 => TargetVersion::Pg16,
        crate::Configuration::Pg17 => TargetVersion::Pg17,
        crate::Configuration::Pg18 => TargetVersion::Pg18,
        // The configuration list drops the pre-release suffix, so the gates
        // need no edit when PostgreSQL 19.0 is tagged (ADR 0009).
        crate::Configuration::Pg19 => TargetVersion::Pg19Beta,
    }
}

/// Maps a target version onto the grammar configuration that names it.
const fn configuration(version: TargetVersion) -> crate::Configuration {
    match version {
        TargetVersion::Pg14 => crate::Configuration::Pg14,
        TargetVersion::Pg15 => crate::Configuration::Pg15,
        TargetVersion::Pg16 => crate::Configuration::Pg16,
        TargetVersion::Pg17 => crate::Configuration::Pg17,
        TargetVersion::Pg18 => crate::Configuration::Pg18,
        TargetVersion::Pg19Beta => crate::Configuration::Pg19,
    }
}

/// One gate on a `tokens!` entry that widens what a build accepts.
///
/// A gate on a lexical entry removes the entry and records nothing (ADR 0010),
/// so the requirement is a property of the token, not of the parsed value.
/// This table names every such gate, and the scan below applies it.
///
/// A gate that only narrows a pattern has no entry: a build that is older
/// accepts more, so nothing in a parsed value needs the newer configuration.
/// The trailing-junk check of 15, the `param_junk` check of 15 and the
/// parameter-number range of 18 are all of that kind.
#[derive(Clone, Copy, Debug)]
pub struct LexicalGate {
    /// The token kind whose pattern the newer configuration widened.
    kind: crate::TokenKind,
    /// The lowest target version whose lexer accepts the widened text.
    version: TargetVersion,
    /// Reports whether one token's text needs that version.
    needs: fn(&str) -> bool,
    /// The authored path of the gated `tokens!` entry.
    site: &'static str,
    /// The `scan.l` change that justifies the gate.
    citation: &'static str,
}

impl LexicalGate {
    /// The token kind whose pattern the newer configuration widened.
    pub const fn kind(&self) -> crate::TokenKind {
        self.kind
    }

    /// The lowest target version whose lexer accepts the widened text.
    pub const fn version(&self) -> TargetVersion {
        self.version
    }

    /// The authored path of the gated `tokens!` entry.
    pub const fn site(&self) -> &'static str {
        self.site
    }

    /// Reports whether one token of this kind needs [`version`](Self::version).
    pub fn needs(&self, text: &str) -> bool {
        (self.needs)(text)
    }
}

/// Every lexical gate of this grammar, in configuration order.
///
/// The list is short because most `tokens!` gates either narrow a pattern or
/// add a keyword, and a new keyword arrives together with a gated node, field
/// or variant, which the parsed value records.
pub static LEXICAL_GATES: &[LexicalGate] = &[
    LexicalGate {
        kind: crate::TokenKind::IntegerLit,
        version: TargetVersion::Pg16,
        needs: non_decimal_or_separated,
        site: "tokens!.lexer_tokens.IntegerLit",
        citation: "scan.l REL_15_19:395-403 `integer {digit}+`; hexadecimal, octal and binary \
                   integers and `_` digit separators arrive in 16, commits 6fcda9aba, faff8f8e4",
    },
    LexicalGate {
        kind: crate::TokenKind::NumericLit,
        version: TargetVersion::Pg16,
        needs: has_digit_separator,
        site: "tokens!.lexer_tokens.NumericLit",
        citation: "scan.l REL_15_19:395-403 `decimal`, `real`; `_` digit separators arrive in \
                   16, commit faff8f8e4",
    },
    LexicalGate {
        kind: crate::TokenKind::StringLitSequence,
        version: TargetVersion::Pg17,
        needs: has_vertical_tab,
        site: "tokens!.literals.string_sequence",
        citation: "scan.l REL_16_15:222-240 `space [ \\t\\n\\r\\f]`, `horiz_space [ \\t\\f]`; \
                   vertical tab joins both in 17, commit ae6d06f0968",
    },
];

/// Reports whether an integer literal uses a 16 form: a non-decimal radix, or
/// a `_` digit separator.
fn non_decimal_or_separated(text: &str) -> bool {
    if has_digit_separator(text) {
        return true;
    }
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    let Some(rest) = digits.strip_prefix('0') else {
        return false;
    };
    rest.starts_with(['x', 'X', 'o', 'O', 'b', 'B'])
}

/// Reports whether a numeric literal uses the 16 `_` digit separator.
fn has_digit_separator(text: &str) -> bool {
    text.contains('_')
}

/// Reports whether a quote-continued string joins its parts across a vertical
/// tab, which only 17 and later treat as space.
fn has_vertical_tab(text: &str) -> bool {
    text.contains('\u{b}')
}

/// Returns the lowest target version whose lexer accepts every token.
///
/// The answer names the first token, in source order, that sets the version.
/// `None` means PostgreSQL 14 lexes them all.
///
/// Pass the tokens of one statement. `lexed.tokens()` covers a whole source,
/// so a caller that reports one diagnostic per statement narrows it first.
///
/// ```
/// use pg_sql::TargetVersion;
/// use pg_sql::minimum_version::lexical_minimum_version;
///
/// let lexed = pg_sql::lex("SELECT 0x42F");
/// let found = lexical_minimum_version(lexed.tokens()).expect("a hexadecimal literal");
/// assert_eq!(found.version(), TargetVersion::Pg16);
/// assert_eq!(found.span().map(|span| span.start()), Some(7));
///
/// let lexed = pg_sql::lex("SELECT 1071");
/// assert!(lexical_minimum_version(lexed.tokens()).is_none());
/// ```
pub fn lexical_minimum_version<'input>(
    tokens: impl IntoIterator<Item = crate::Token<'input>>,
) -> Option<VersionRequirement> {
    scan_tokens(tokens, TargetVersion::Pg14, false)
}

/// Returns the first token, in source order, that `target` cannot lex.
///
/// `None` means `target` lexes every token. Pass the tokens of one statement,
/// as for [`lexical_minimum_version`].
///
/// ```
/// use pg_sql::TargetVersion;
/// use pg_sql::minimum_version::lexical_minimum_version_above;
///
/// let lexed = pg_sql::lex("SELECT 1_000");
/// let tokens = lexed.tokens().collect::<Vec<_>>();
/// assert!(lexical_minimum_version_above(tokens.iter().copied(), TargetVersion::Pg15).is_some());
/// assert!(lexical_minimum_version_above(tokens.iter().copied(), TargetVersion::Pg16).is_none());
/// ```
pub fn lexical_minimum_version_above<'input>(
    tokens: impl IntoIterator<Item = crate::Token<'input>>,
    target: TargetVersion,
) -> Option<VersionRequirement> {
    scan_tokens(tokens, target, true)
}

/// Runs one scan over the tokens, with the folds of [`Requires`] itself: the
/// minimum keeps a running maximum and the earliest token that reached it;
/// the first above a floor stops at the first token above it.
fn scan_tokens<'input>(
    tokens: impl IntoIterator<Item = crate::Token<'input>>,
    floor: TargetVersion,
    first_only: bool,
) -> Option<VersionRequirement> {
    let mut best: Option<VersionRequirement> = None;
    for token in tokens {
        for gate in LEXICAL_GATES {
            if gate.kind != token.kind() || gate.version <= floor || !gate.needs(token.text()) {
                continue;
            }
            if best.is_none_or(|found| gate.version > found.version) {
                best = Some(VersionRequirement {
                    version: gate.version,
                    span: Some(token.span()),
                    construct: gate.site,
                    absence: false,
                    // The older lexer's message depends on which older
                    // version reads the text, so no one message fits: 14
                    // lexes `0x42F` as `0` and the label `x42F`, and 15
                    // raises "trailing junk after numeric literal".
                    code: None,
                    message: None,
                    hint: None,
                    citation: Some(gate.citation),
                });
            }
            if first_only && best.is_some() {
                return best;
            }
        }
    }
    best
}
