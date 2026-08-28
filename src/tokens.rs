#![allow(non_camel_case_types)]

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

// Single-declaration token site.
//
// The `tokens!` proc macro is logos-backed. It generates:
//   - one `pub struct` per `keywords` / `soft_keywords` / `punctuation` /
//     `literals` token (e.g. `pub struct SELECT;`, `pub struct LParen;`,
//     `pub struct DollarStringLit<'input>(...)`),
//   - the combined `Keyword` / `SoftKeyword` / `Punctuation` / `Literal`
//     enums,
//   - a flat `#[repr(u16)] #[derive(Logos)] pub enum TokenKind` covering
//     every declared token (including `lexer_tokens` entries, which get a
//     `TokenKind` variant but no struct),
//   - `pub fn lex(&str) -> LexResult` — the logos lexing pass.
//
// All token types live at this module's top level; the `keyword`, `punct`,
// and `literal` sub-modules below re-export them under the legacy paths so
// the rest of the crate continues to use `keyword::SELECT`, `punct::LParen`,
// `literal::DollarStringLit`, etc.
/// logos callback for `DollarStringLit`.
///
/// The `#[regex]` matches only the opening `$tag$`. PG dollar-quoted strings
/// close at a tag *matching the opener* — a back-reference no regex DFA can
/// express. The callback reads the matched opener (which is identical to the
/// close tag), scans the remainder for the next occurrence of that exact
/// tag, and `bump`s the lexer past it. No matching close ⇒ `Error`.
///
/// Mirrors the byte-walker in `skip_failed_statement` (`pg-sql/src/ast/mod.rs`):
/// all delimiters are ASCII so byte scanning is correct in UTF-8 source.
pub fn scan_dollar_string(
    lex: &mut ::logos::Lexer<'_, TokenKind>,
) -> ::logos::FilterResult<(), ()> {
    // The matched slice is the opening `$tag$`; the close tag is byte-identical.
    let close = lex.slice();
    let remainder = lex.remainder();
    match remainder.find(close) {
        Some(pos) => {
            // Advance past the body and the closing tag.
            lex.bump(pos + close.len());
            ::logos::FilterResult::Emit(())
        }
        None => ::logos::FilterResult::Error(()),
    }
}

/// logos callback for `BlockComment` — skips a `/* ... */` block comment,
/// PostgreSQL-style with arbitrary nesting.
///
/// The `#[regex]` matches only the opening `/*`. logos's `#[logos(skip ...)]`
/// directive takes a single regular expression, and a *nested* block comment
/// is not a regular language — so block-comment skipping is done here: the
/// callback scans `lexer.remainder()` tracking nesting depth, `bump`s past
/// the matching close `*/`, and returns `Skip` so the comment never reaches
/// the token array. An unterminated comment consumes to end-of-input,
/// mirroring the old scannerless `ignored_len` behaviour.
pub fn skip_block_comment(lex: &mut ::logos::Lexer<'_, TokenKind>) -> ::logos::Skip {
    let bytes = lex.remainder().as_bytes();
    let mut depth: u32 = 1;
    let mut j = 0;
    while j + 1 < bytes.len() {
        if bytes[j] == b'/' && bytes[j + 1] == b'*' {
            depth += 1;
            j += 2;
        } else if bytes[j] == b'*' && bytes[j + 1] == b'/' {
            depth -= 1;
            j += 2;
            if depth == 0 {
                break;
            }
        } else {
            j += 1;
        }
    }
    // depth != 0 ⇒ unclosed comment: consume to end-of-input.
    lex.bump(if depth == 0 { j } else { bytes.len() });
    ::logos::Skip
}

/// logos callback for `NumericLit` / `IntegerLit`.
///
/// logos has no `\b`, so the `\b`/`\B` trailing-boundary anchors are stripped
/// from the patterns and this callback reproduces their effect: a numeric
/// literal immediately butted against a word character (`[A-Za-z0-9_]`) — no
/// whitespace separator — is a PostgreSQL "trailing junk after numeric
/// literal" lex error, not a literal followed by an identifier.
///
/// Both the digit-terminated forms (old trailing `\b`) and the bare
/// trailing-dot form `1.` (old trailing `\B` after `.`) reject on exactly the
/// same condition: the next char being a word char. A rejected literal makes
/// `lex` emit a `TOKEN_KIND_NONE` span, the statement falls to `Raw`, and
/// psql emits the canonical PG error — preserving the differential-test
/// behaviour the `\b`/`\B` anchors gave.
pub fn reject_trailing_word(
    lex: &mut ::logos::Lexer<'_, TokenKind>,
) -> ::logos::FilterResult<(), ()> {
    match lex.remainder().bytes().next() {
        Some(b) if b.is_ascii_alphanumeric() || b == b'_' => ::logos::FilterResult::Error(()),
        _ => ::logos::FilterResult::Emit(()),
    }
}

recursa::tokens! {
    // Ignored content: ASCII whitespace and `-- line comments`. Both are
    // regular, so logos's `#[logos(skip ...)]` handles them. `/* block
    // comments */` are NOT regular when nested, so they are skipped by the
    // `BlockComment` `lexer_tokens` entry's `skip_block_comment` callback.
    ignore = r"\s+|--[^\n]*",
    // Postgres keyword categories, mirroring `kwlist.h`'s four-way
    // partition. Every `keywords` / `soft_keywords` entry carries an
    // `in <CATEGORY>` clause; classifications come from PG 17 kwlist.h
    // via docs/plans/2026-06-04-pg-keyword-audit.md.
    categories { UNRESERVED, COL_NAME, TYPE_FUNC_NAME, RESERVED }
    // Orthogonal per-keyword booleans. `bare_label` marks keywords that
    // PG admits as bare-form column labels (no `AS` required); composes
    // with the category to derive `BareColLabel`'s admit set.
    flags { bare_label }
    keywords {
        SELECT       => r"SELECT" in RESERVED + bare_label,
        FROM         => r"FROM" in RESERVED,
        WHERE        => r"WHERE" in RESERVED,
        AS           => r"AS" in RESERVED,
        AND          => r"AND" in RESERVED + bare_label,
        OR           => r"OR" in RESERVED + bare_label,
        NOT          => r"NOT" in RESERVED + bare_label,
        TRUE         => r"TRUE" in RESERVED + bare_label,
        FALSE        => r"FALSE" in RESERVED + bare_label,
        NULL         => r"NULL" in RESERVED + bare_label,
        IS           => r"IS" in TYPE_FUNC_NAME + bare_label,
        CREATE       => r"CREATE" in RESERVED,
        TABLE        => r"TABLE" in RESERVED + bare_label,
        INTO         => r"INTO" in RESERVED,
        VALUES       => r"VALUES" in COL_NAME + bare_label,
        ORDER        => r"ORDER" in RESERVED,
        PRIMARY      => r"PRIMARY" in RESERVED + bare_label,
        ASC          => r"ASC" in RESERVED + bare_label,
        DESC         => r"DESC" in RESERVED + bare_label,
        NULLS        => r"NULLS" in UNRESERVED + bare_label,
        USING        => r"USING" in RESERVED + bare_label,
        OFFSET       => r"OFFSET" in RESERVED,
        LIMIT        => r"LIMIT" in RESERVED,
        ANALYZE      => r"ANALYZE" in RESERVED + bare_label,
        SET          => r"SET" in UNRESERVED + bare_label,
        TO           => r"TO" in RESERVED,
        ON           => r"ON" in RESERVED,
        FOR          => r"FOR" in RESERVED,
        UNION        => r"UNION" in RESERVED,
        ALL          => r"ALL" in RESERVED + bare_label,
        IN           => r"IN" in RESERVED + bare_label,
        DEFAULT      => r"DEFAULT" in RESERVED + bare_label,
        LATERAL      => r"LATERAL" in RESERVED + bare_label,
        PARTITION    => r"PARTITION" in UNRESERVED + bare_label,
        WITH         => r"WITH" in RESERVED,
        EXCEPT       => r"EXCEPT" in RESERVED,
        INTERSECT    => r"INTERSECT" in RESERVED,
        DISTINCT     => r"DISTINCT" in RESERVED + bare_label,
        JOIN         => r"JOIN" in TYPE_FUNC_NAME + bare_label,
        LEFT         => r"LEFT" in TYPE_FUNC_NAME + bare_label,
        RIGHT        => r"RIGHT" in TYPE_FUNC_NAME + bare_label,
        FULL         => r"FULL" in TYPE_FUNC_NAME + bare_label,
        INNER        => r"INNER" in TYPE_FUNC_NAME + bare_label,
        CROSS        => r"CROSS" in TYPE_FUNC_NAME + bare_label,
        GROUP        => r"GROUP" in RESERVED,
        HAVING       => r"HAVING" in RESERVED,
        RETURNING    => r"RETURNING" in RESERVED,
        WHEN         => r"WHEN" in RESERVED + bare_label,
        THEN         => r"THEN" in RESERVED + bare_label,
        DO           => r"DO" in RESERVED + bare_label,
        ARRAY        => r"ARRAY" in RESERVED,
        UNIQUE       => r"UNIQUE" in RESERVED + bare_label,
        REFERENCES   => r"REFERENCES" in RESERVED + bare_label,
        ANY          => r"ANY" in RESERVED + bare_label,
        SOME         => r"SOME" in RESERVED + bare_label,
        LIKE         => r"LIKE" in TYPE_FUNC_NAME + bare_label,
        ILIKE        => r"ILIKE" in TYPE_FUNC_NAME + bare_label,
        COLLATE      => r"COLLATE" in RESERVED + bare_label,
        // `COLUMN` is a `reserved_keyword` in Postgres' `gram.y` — never an
        // identifier — so it is a hard keyword.
        COLUMN       => r"COLUMN" in RESERVED + bare_label,
        CASE         => r"CASE" in RESERVED + bare_label,
        ELSE         => r"ELSE" in RESERVED + bare_label,
        END          => r"END" in RESERVED + bare_label,
        VERBOSE      => r"VERBOSE" in TYPE_FUNC_NAME + bare_label,
        ONLY         => r"ONLY" in RESERVED + bare_label,
        GRANT        => r"GRANT" in RESERVED,
        FETCH        => r"FETCH" in RESERVED,
        USER         => r"USER" in RESERVED + bare_label,
        CAST         => r"CAST" in RESERVED + bare_label,
        COLLATION    => r"COLLATION" in TYPE_FUNC_NAME + bare_label,
        FOREIGN      => r"FOREIGN" in RESERVED + bare_label,
        CONCURRENTLY => r"CONCURRENTLY" in TYPE_FUNC_NAME + bare_label,
        CONSTRAINT   => r"CONSTRAINT" in RESERVED + bare_label,
        CHECK        => r"CHECK" in RESERVED + bare_label,
        DEFERRABLE   => r"DEFERRABLE" in RESERVED + bare_label,
        INITIALLY    => r"INITIALLY" in RESERVED + bare_label,
        VARIADIC     => r"VARIADIC" in RESERVED + bare_label,
        WINDOW       => r"WINDOW" in RESERVED,
        NATURAL      => r"NATURAL" in TYPE_FUNC_NAME + bare_label,
        OUTER        => r"OUTER" in TYPE_FUNC_NAME + bare_label,
        NOTNULL      => r"NOTNULL" in TYPE_FUNC_NAME,
        ISNULL       => r"ISNULL" in TYPE_FUNC_NAME,
        LEADING      => r"LEADING" in RESERVED + bare_label,
        TRAILING     => r"TRAILING" in RESERVED + bare_label,
        BOTH         => r"BOTH" in RESERVED + bare_label,
        SIMILAR      => r"SIMILAR" in TYPE_FUNC_NAME + bare_label,
        PLACING      => r"PLACING" in RESERVED + bare_label,
        TABLESAMPLE  => r"TABLESAMPLE" in TYPE_FUNC_NAME + bare_label,
    },
    // Soft (non-reserved) keywords: recognised as keywords only where the
    // grammar asks for them; otherwise reclaimable as ordinary identifiers
    // (see `UnquotedIdent`'s `token_kind_is_soft` check). Covers Postgres
    // non-reserved / col-name / type-name keywords plus the SQL/JSON
    // function family — all common identifiers, so none may be reserved.
    soft_keywords {
        JSON            => r"JSON" in COL_NAME + bare_label,
        JSON_VALUE      => r"JSON_VALUE" in COL_NAME + bare_label,
        JSON_QUERY      => r"JSON_QUERY" in COL_NAME + bare_label,
        JSON_EXISTS     => r"JSON_EXISTS" in COL_NAME + bare_label,
        JSON_OBJECT     => r"JSON_OBJECT" in COL_NAME + bare_label,
        JSON_ARRAY      => r"JSON_ARRAY" in COL_NAME + bare_label,
        JSON_OBJECTAGG  => r"JSON_OBJECTAGG" in COL_NAME + bare_label,
        JSON_ARRAYAGG   => r"JSON_ARRAYAGG" in COL_NAME + bare_label,
        JSON_SERIALIZE  => r"JSON_SERIALIZE" in COL_NAME + bare_label,
        JSON_SCALAR     => r"JSON_SCALAR" in COL_NAME + bare_label,
        JSON_TABLE      => r"JSON_TABLE" in COL_NAME + bare_label,
        FORMAT          => r"FORMAT" in UNRESERVED + bare_label,
        ENCODING        => r"ENCODING" in UNRESERVED + bare_label,
        PASSING         => r"PASSING" in UNRESERVED + bare_label,
        PATH            => r"PATH" in UNRESERVED + bare_label,
        COLUMNS         => r"COLUMNS" in UNRESERVED + bare_label,
        KEYS            => r"KEYS" in UNRESERVED + bare_label,
        SCALAR          => r"SCALAR" in UNRESERVED + bare_label,
        QUOTES          => r"QUOTES" in UNRESERVED + bare_label,
        NESTED          => r"NESTED" in UNRESERVED + bare_label,
        OMIT            => r"OMIT" in UNRESERVED + bare_label,
        KEEP            => r"KEEP" in UNRESERVED + bare_label,
        CONDITIONAL     => r"CONDITIONAL" in UNRESERVED + bare_label,
        UNCONDITIONAL   => r"UNCONDITIONAL" in UNRESERVED + bare_label,
        ABSENT          => r"ABSENT" in UNRESERVED + bare_label,
        ERROR           => r"ERROR" in UNRESERVED + bare_label,
        EMPTY           => r"EMPTY" in UNRESERVED + bare_label,
        OBJECT          => r"OBJECT" in UNRESERVED + bare_label,
        STRING          => r"STRING" in UNRESERVED + bare_label,
        UNKNOWN         => r"UNKNOWN" in UNRESERVED + bare_label,
        INSERT          => r"INSERT" in UNRESERVED + bare_label,
        DROP            => r"DROP" in UNRESERVED + bare_label,
        DELETE          => r"DELETE" in UNRESERVED + bare_label,
        BY              => r"BY" in UNRESERVED + bare_label,
        BOOL            => r"BOOL" in UNRESERVED + bare_label,
        BOOLEAN         => r"BOOLEAN" in COL_NAME + bare_label,
        TEXT            => r"TEXT" in UNRESERVED + bare_label,
        INT             => r"INT" in COL_NAME + bare_label,
        SERIAL          => r"SERIAL" in UNRESERVED + bare_label,
        KEY             => r"KEY" in UNRESERVED + bare_label,
        FIRST           => r"FIRST" in UNRESERVED + bare_label,
        LAST            => r"LAST" in UNRESERVED + bare_label,
        RESET           => r"RESET" in UNRESERVED + bare_label,
        OFF             => r"OFF" in UNRESERVED + bare_label,
        TEMP            => r"TEMP" in UNRESERVED + bare_label,
        INDEX           => r"INDEX" in UNRESERVED + bare_label,
        EXPLAIN         => r"EXPLAIN" in UNRESERVED + bare_label,
        UPDATE          => r"UPDATE" in UNRESERVED + bare_label,
        FUNCTION        => r"FUNCTION" in UNRESERVED + bare_label,
        RETURNS         => r"RETURNS" in UNRESERVED + bare_label,
        SETOF           => r"SETOF" in COL_NAME + bare_label,
        LANGUAGE        => r"LANGUAGE" in UNRESERVED + bare_label,
        IMMUTABLE       => r"IMMUTABLE" in UNRESERVED + bare_label,
        OF              => r"OF" in UNRESERVED + bare_label,
        COSTS           => r"COSTS" in UNRESERVED + bare_label,
        TIMING          => r"TIMING" in UNRESERVED + bare_label,
        SUMMARY         => r"SUMMARY" in UNRESERVED + bare_label,
        RECURSIVE       => r"RECURSIVE" in UNRESERVED + bare_label,
        MATERIALIZED    => r"MATERIALIZED" in UNRESERVED + bare_label,
        MERGE           => r"MERGE" in UNRESERVED + bare_label,
        MATCHED         => r"MATCHED" in UNRESERVED + bare_label,
        CONFLICT        => r"CONFLICT" in UNRESERVED + bare_label,
        NOTHING         => r"NOTHING" in UNRESERVED + bare_label,
        EXCLUDED        => r"EXCLUDED" in UNRESERVED + bare_label,
        VIEW            => r"VIEW" in UNRESERVED + bare_label,
        REPLACE         => r"REPLACE" in UNRESERVED + bare_label,
        TEMPORARY       => r"TEMPORARY" in UNRESERVED + bare_label,
        EXISTS          => r"EXISTS" in COL_NAME + bare_label,
        SEARCH          => r"SEARCH" in UNRESERVED + bare_label,
        DEPTH           => r"DEPTH" in UNRESERVED + bare_label,
        BREADTH         => r"BREADTH" in UNRESERVED + bare_label,
        CYCLE           => r"CYCLE" in UNRESERVED + bare_label,
        ROW             => r"ROW" in COL_NAME + bare_label,
        OVER            => r"OVER" in UNRESERVED,
        INTEGER         => r"INTEGER" in COL_NAME + bare_label,
        NUMERIC         => r"NUMERIC" in COL_NAME + bare_label,
        VARCHAR         => r"VARCHAR" in COL_NAME + bare_label,
        ALTER           => r"ALTER" in UNRESERVED + bare_label,
        ADD             => r"ADD" in UNRESERVED + bare_label,
        RULE            => r"RULE" in UNRESERVED + bare_label,
        TRIGGER         => r"TRIGGER" in UNRESERVED + bare_label,
        BEFORE          => r"BEFORE" in UNRESERVED + bare_label,
        AFTER           => r"AFTER" in UNRESERVED + bare_label,
        EACH            => r"EACH" in UNRESERVED + bare_label,
        STATEMENT       => r"STATEMENT" in UNRESERVED + bare_label,
        EXECUTE         => r"EXECUTE" in UNRESERVED + bare_label,
        PROCEDURE       => r"PROCEDURE" in UNRESERVED + bare_label,
        ROUTINE         => r"ROUTINE" in UNRESERVED + bare_label,
        INSTEAD         => r"INSTEAD" in UNRESERVED + bare_label,
        ALSO            => r"ALSO" in UNRESERVED + bare_label,
        NEW             => r"NEW" in UNRESERVED + bare_label,
        OLD             => r"OLD" in UNRESERVED + bare_label,
        BEGIN           => r"BEGIN" in UNRESERVED + bare_label,
        COMMIT          => r"COMMIT" in UNRESERVED + bare_label,
        TRUNCATE        => r"TRUNCATE" in UNRESERVED + bare_label,
        NOTIFY          => r"NOTIFY" in UNRESERVED + bare_label,
        INHERITS        => r"INHERITS" in UNRESERVED + bare_label,
        GENERATED       => r"GENERATED" in UNRESERVED + bare_label,
        ALWAYS          => r"ALWAYS" in UNRESERVED + bare_label,
        IDENTITY        => r"IDENTITY" in UNRESERVED + bare_label,
        LOCAL           => r"LOCAL" in UNRESERVED + bare_label,
        BETWEEN         => r"BETWEEN" in COL_NAME + bare_label,
        UNLOGGED        => r"UNLOGGED" in UNRESERVED + bare_label,
        DATABASE        => r"DATABASE" in UNRESERVED + bare_label,
        PRIVILEGES      => r"PRIVILEGES" in UNRESERVED + bare_label,
        CHECKPOINT      => r"CHECKPOINT" in UNRESERVED + bare_label,
        MODULUS         => r"MODULUS" in UNRESERVED + bare_label,
        REMAINDER       => r"REMAINDER" in UNRESERVED + bare_label,
        IF              => r"IF" in UNRESERVED + bare_label,
        NO              => r"NO" in UNRESERVED + bare_label,
        DATA            => r"DATA" in UNRESERVED + bare_label,
        ROLLBACK        => r"ROLLBACK" in UNRESERVED + bare_label,
        SAVEPOINT       => r"SAVEPOINT" in UNRESERVED + bare_label,
        RELEASE         => r"RELEASE" in UNRESERVED + bare_label,
        PREPARE         => r"PREPARE" in UNRESERVED + bare_label,
        DEALLOCATE      => r"DEALLOCATE" in UNRESERVED + bare_label,
        REVOKE          => r"REVOKE" in UNRESERVED + bare_label,
        COMMENT         => r"COMMENT" in UNRESERVED + bare_label,
        COPY            => r"COPY" in UNRESERVED + bare_label,
        LOCK            => r"LOCK" in UNRESERVED + bare_label,
        DECLARE         => r"DECLARE" in UNRESERVED + bare_label,
        CLOSE           => r"CLOSE" in UNRESERVED + bare_label,
        MOVE            => r"MOVE" in UNRESERVED + bare_label,
        CURSOR          => r"CURSOR" in UNRESERVED + bare_label,
        REINDEX         => r"REINDEX" in UNRESERVED + bare_label,
        REFRESH         => r"REFRESH" in UNRESERVED + bare_label,
        LISTEN          => r"LISTEN" in UNRESERVED + bare_label,
        UNLISTEN        => r"UNLISTEN" in UNRESERVED + bare_label,
        DISCARD         => r"DISCARD" in UNRESERVED + bare_label,
        REASSIGN        => r"REASSIGN" in UNRESERVED + bare_label,
        SECURITY        => r"SECURITY" in UNRESERVED + bare_label,
        LABEL           => r"LABEL" in UNRESERVED + bare_label,
        CLUSTER         => r"CLUSTER" in UNRESERVED + bare_label,
        VACUUM          => r"VACUUM" in UNRESERVED + bare_label,
        ROLE            => r"ROLE" in UNRESERVED + bare_label,
        SCHEMA          => r"SCHEMA" in UNRESERVED + bare_label,
        SEQUENCE        => r"SEQUENCE" in UNRESERVED + bare_label,
        TYPE            => r"TYPE" in UNRESERVED + bare_label,
        DOMAIN          => r"DOMAIN" in UNRESERVED + bare_label,
        AGGREGATE       => r"AGGREGATE" in UNRESERVED + bare_label,
        OPERATOR        => r"OPERATOR" in UNRESERVED + bare_label,
        EXTENSION       => r"EXTENSION" in UNRESERVED + bare_label,
        POLICY          => r"POLICY" in UNRESERVED + bare_label,
        STATISTICS      => r"STATISTICS" in UNRESERVED + bare_label,
        PUBLICATION     => r"PUBLICATION" in UNRESERVED + bare_label,
        SUBSCRIPTION    => r"SUBSCRIPTION" in UNRESERVED + bare_label,
        OWNED           => r"OWNED" in UNRESERVED + bare_label,
        ACCESS          => r"ACCESS" in UNRESERVED + bare_label,
        METHOD          => r"METHOD" in UNRESERVED + bare_label,
        CONVERSION      => r"CONVERSION" in UNRESERVED + bare_label,
        SERVER          => r"SERVER" in UNRESERVED + bare_label,
        WRAPPER         => r"WRAPPER" in UNRESERVED + bare_label,
        MAPPING         => r"MAPPING" in UNRESERVED + bare_label,
        EVENT           => r"EVENT" in UNRESERVED + bare_label,
        MATCH           => r"MATCH" in UNRESERVED + bare_label,
        PARTIAL         => r"PARTIAL" in UNRESERVED + bare_label,
        SIMPLE          => r"SIMPLE" in UNRESERVED + bare_label,
        RESTRICT        => r"RESTRICT" in UNRESERVED + bare_label,
        ACTION          => r"ACTION" in UNRESERVED + bare_label,
        DEFERRED        => r"DEFERRED" in UNRESERVED + bare_label,
        IMMEDIATE       => r"IMMEDIATE" in UNRESERVED + bare_label,
        INHERIT         => r"INHERIT" in UNRESERVED + bare_label,
        CASCADE         => r"CASCADE" in UNRESERVED + bare_label,
        INCLUDE         => r"INCLUDE" in UNRESERVED + bare_label,
        BTREE           => r"BTREE" in UNRESERVED + bare_label,
        GIN             => r"GIN" in UNRESERVED + bare_label,
        GIST            => r"GIST" in UNRESERVED + bare_label,
        HASH            => r"HASH" in UNRESERVED + bare_label,
        SPGIST          => r"SPGIST" in UNRESERVED + bare_label,
        BRIN            => r"BRIN" in UNRESERVED + bare_label,
        SHOW            => r"SHOW" in UNRESERVED + bare_label,
        TRANSACTION     => r"TRANSACTION" in UNRESERVED + bare_label,
        ISOLATION       => r"ISOLATION" in UNRESERVED + bare_label,
        LEVEL           => r"LEVEL" in UNRESERVED + bare_label,
        SERIALIZABLE    => r"SERIALIZABLE" in UNRESERVED + bare_label,
        REPEATABLE      => r"REPEATABLE" in UNRESERVED + bare_label,
        READ            => r"READ" in UNRESERVED + bare_label,
        WRITE           => r"WRITE" in UNRESERVED + bare_label,
        COMMITTED       => r"COMMITTED" in UNRESERVED + bare_label,
        UNCOMMITTED     => r"UNCOMMITTED" in UNRESERVED + bare_label,
        CONSTRAINTS     => r"CONSTRAINTS" in UNRESERVED + bare_label,
        START           => r"START" in UNRESERVED + bare_label,
        WORK            => r"WORK" in UNRESERVED + bare_label,
        ABORT           => r"ABORT" in UNRESERVED + bare_label,
        CHARACTERISTICS => r"CHARACTERISTICS" in UNRESERVED + bare_label,
        WITHOUT         => r"WITHOUT" in UNRESERVED,
        TIMESTAMP       => r"TIMESTAMP" in COL_NAME + bare_label,
        SESSION         => r"SESSION" in UNRESERVED + bare_label,
        AUTHORIZATION   => r"AUTHORIZATION" in TYPE_FUNC_NAME + bare_label,
        TIME            => r"TIME" in COL_NAME + bare_label,
        ZONE            => r"ZONE" in UNRESERVED + bare_label,
        NONE            => r"NONE" in COL_NAME + bare_label,
        UNBOUNDED       => r"UNBOUNDED" in UNRESERVED + bare_label,
        PRECEDING       => r"PRECEDING" in UNRESERVED + bare_label,
        FOLLOWING       => r"FOLLOWING" in UNRESERVED + bare_label,
        CURRENT         => r"CURRENT" in UNRESERVED + bare_label,
        EXCLUDE         => r"EXCLUDE" in UNRESERVED + bare_label,
        OTHERS          => r"OTHERS" in UNRESERVED + bare_label,
        TIES            => r"TIES" in UNRESERVED + bare_label,
        SOURCE          => r"SOURCE" in UNRESERVED + bare_label,
        TARGET          => r"TARGET" in UNRESERVED + bare_label,
        STRICT          => r"STRICT" in UNRESERVED + bare_label,
        STABLE          => r"STABLE" in UNRESERVED + bare_label,
        VOLATILE        => r"VOLATILE" in UNRESERVED + bare_label,
        CALLED          => r"CALLED" in UNRESERVED + bare_label,
        INPUT           => r"INPUT" in UNRESERVED + bare_label,
        ORDINALITY      => r"ORDINALITY" in UNRESERVED + bare_label,
        XMLELEMENT      => r"XMLELEMENT" in COL_NAME + bare_label,
        XMLATTRIBUTES   => r"XMLATTRIBUTES" in COL_NAME + bare_label,
        XMLFOREST       => r"XMLFOREST" in COL_NAME + bare_label,
        XMLPI           => r"XMLPI" in COL_NAME + bare_label,
        NAME            => r"NAME" in UNRESERVED + bare_label,
        OUT             => r"OUT" in COL_NAME + bare_label,
        INOUT           => r"INOUT" in COL_NAME + bare_label,
        CALL            => r"CALL" in UNRESERVED + bare_label,
        LOAD            => r"LOAD" in UNRESERVED + bare_label,
        TABLESPACE      => r"TABLESPACE" in UNRESERVED + bare_label,
        OWNER           => r"OWNER" in UNRESERVED + bare_label,
        LOCATION        => r"LOCATION" in UNRESERVED + bare_label,
        STORED          => r"STORED" in UNRESERVED + bare_label,
        UESCAPE         => r"UESCAPE" in UNRESERVED + bare_label,
        WITHIN          => r"WITHIN" in UNRESERVED,
        FILTER          => r"FILTER" in UNRESERVED,
        TRIM            => r"TRIM" in COL_NAME + bare_label,
        SUBSTRING       => r"SUBSTRING" in COL_NAME + bare_label,
        POSITION        => r"POSITION" in COL_NAME + bare_label,
        OVERLAY         => r"OVERLAY" in COL_NAME + bare_label,
        EXTRACT         => r"EXTRACT" in COL_NAME + bare_label,
        PRESERVE        => r"PRESERVE" in UNRESERVED + bare_label,
        INCREMENT       => r"INCREMENT" in UNRESERVED + bare_label,
        MINVALUE        => r"MINVALUE" in UNRESERVED + bare_label,
        MAXVALUE        => r"MAXVALUE" in UNRESERVED + bare_label,
        CACHE           => r"CACHE" in UNRESERVED + bare_label,
        ESCAPE          => r"ESCAPE" in UNRESERVED + bare_label,
        SNAPSHOT        => r"SNAPSHOT" in UNRESERVED + bare_label,
        AT              => r"AT" in UNRESERVED + bare_label,
        GROUPING        => r"GROUPING" in COL_NAME + bare_label,
        SETS            => r"SETS" in UNRESERVED + bare_label,
        ROLLUP          => r"ROLLUP" in UNRESERVED + bare_label,
        CUBE            => r"CUBE" in UNRESERVED + bare_label,
        INTERVAL        => r"INTERVAL" in COL_NAME + bare_label,
        YEAR            => r"YEAR" in UNRESERVED,
        MONTH           => r"MONTH" in UNRESERVED,
        DAY             => r"DAY" in UNRESERVED,
        HOUR            => r"HOUR" in UNRESERVED,
        MINUTE          => r"MINUTE" in UNRESERVED,
        SECOND          => r"SECOND" in UNRESERVED,
        INCLUDING       => r"INCLUDING" in UNRESERVED + bare_label,
        EXCLUDING       => r"EXCLUDING" in UNRESERVED + bare_label,
        DEFAULTS        => r"DEFAULTS" in UNRESERVED + bare_label,
        INDEXES         => r"INDEXES" in UNRESERVED + bare_label,
        STORAGE         => r"STORAGE" in UNRESERVED + bare_label,
        COMMENTS        => r"COMMENTS" in UNRESERVED + bare_label,
        COMPRESSION     => r"COMPRESSION" in UNRESERVED + bare_label,
        RETURN          => r"RETURN" in UNRESERVED + bare_label,
        OIDS            => r"OIDS" in UNRESERVED + bare_label,
        OPTIONS         => r"OPTIONS" in UNRESERVED + bare_label,
        OVERRIDING      => r"OVERRIDING" in UNRESERVED + bare_label,
        SYSTEM          => r"SYSTEM" in UNRESERVED + bare_label,
        VALUE           => r"VALUE" in UNRESERVED + bare_label,
        CASCADED        => r"CASCADED" in UNRESERVED + bare_label,
        OPTION          => r"OPTION" in UNRESERVED + bare_label,
        // PG kwlist.h: `xml` UNRESERVED_KEYWORD. Needed at least for
        // `SET XML OPTION { DOCUMENT | CONTENT }` parsing.
        XML             => r"XML" in UNRESERVED + bare_label,
        // PG kwlist.h: `atomic` UNRESERVED_KEYWORD. Used by
        // CREATE FUNCTION/PROCEDURE `BEGIN ATOMIC ... END` body.
        ATOMIC          => r"ATOMIC" in UNRESERVED + bare_label,
        VALID           => r"VALID" in UNRESERVED + bare_label,
        PARALLEL        => r"PARALLEL" in UNRESERVED + bare_label,
        RENAME          => r"RENAME" in UNRESERVED + bare_label,
        TRUSTED         => r"TRUSTED" in UNRESERVED + bare_label,
        PROCEDURAL      => r"PROCEDURAL" in UNRESERVED + bare_label,
        HANDLER         => r"HANDLER" in UNRESERVED + bare_label,
        VALIDATOR       => r"VALIDATOR" in UNRESERVED + bare_label,
        INLINE          => r"INLINE" in UNRESERVED + bare_label,
        NEXT            => r"NEXT" in UNRESERVED + bare_label,
        PRIOR           => r"PRIOR" in UNRESERVED + bare_label,
        FORWARD         => r"FORWARD" in UNRESERVED + bare_label,
        BACKWARD        => r"BACKWARD" in UNRESERVED + bare_label,
        ABSOLUTE        => r"ABSOLUTE" in UNRESERVED + bare_label,
        RELATIVE        => r"RELATIVE" in UNRESERVED + bare_label,
        DOUBLE          => r"DOUBLE" in UNRESERVED + bare_label,
        PRECISION       => r"PRECISION" in COL_NAME,
        SAFE            => r"SAFE" in UNRESERVED + bare_label,
        UNSAFE          => r"UNSAFE" in UNRESERVED + bare_label,
        RESTRICTED      => r"RESTRICTED" in UNRESERVED + bare_label,
        BIT             => r"BIT" in COL_NAME + bare_label,
        VARYING         => r"VARYING" in UNRESERVED,
        CHARACTER       => r"CHARACTER" in COL_NAME,
        SHARE           => r"SHARE" in UNRESERVED + bare_label,
        IMPORT          => r"IMPORT" in UNRESERVED + bare_label,
        DEFINER         => r"DEFINER" in UNRESERVED + bare_label,
        INVOKER         => r"INVOKER" in UNRESERVED + bare_label,
        LEAKPROOF       => r"LEAKPROOF" in UNRESERVED + bare_label,
        COST            => r"COST" in UNRESERVED + bare_label,
        SUPPORT         => r"SUPPORT" in UNRESERVED + bare_label,
        TRANSFORM       => r"TRANSFORM" in UNRESERVED + bare_label,
        // `SQL` — `unreserved_keyword` in `kwlist.h` (`SQL_P`). Used by
        // `CREATE TRANSFORM` / `DROP TRANSFORM` (`FROM SQL WITH FUNCTION ...`)
        // and `CREATE FUNCTION ... LANGUAGE SQL`. Soft here so it remains
        // reclaimable as an identifier outside those grammar positions.
        SQL             => r"SQL" in UNRESERVED + bare_label,
        // Unicode normalisation forms — `unreserved_keyword` in kwlist.h.
        // Used by `a_expr IS [NOT] [NFC|NFD|NFKC|NFKD] NORMALIZED` and
        // `NORMALIZE(expr, NFx)`. Soft so they remain reclaimable as
        // identifiers outside those predicate positions.
        NORMALIZED      => r"NORMALIZED" in UNRESERVED + bare_label,
        // NF-prefixed names: list longer matches before shorter (`NFKC`
        // shares the `NF` prefix with `NFC` — the longest-match-wins lexer
        // handles this purely via regex priorities, but order is explicit
        // for readability).
        NFKC            => r"NFKC" in UNRESERVED + bare_label,
        NFKD            => r"NFKD" in UNRESERVED + bare_label,
        NFC             => r"NFC" in UNRESERVED + bare_label,
        NFD             => r"NFD" in UNRESERVED + bare_label,
        EXTERNAL        => r"EXTERNAL" in UNRESERVED + bare_label,
        PLAIN           => r"PLAIN" in UNRESERVED + bare_label,
        EXTENDED        => r"EXTENDED" in UNRESERVED + bare_label,
        MAIN            => r"MAIN" in UNRESERVED + bare_label,
        NOWAIT          => r"NOWAIT" in UNRESERVED + bare_label,
        SKIP            => r"SKIP" in UNRESERVED + bare_label,
        LOCKED          => r"LOCKED" in UNRESERVED + bare_label,
        LARGE           => r"LARGE" in UNRESERVED + bare_label,
        XMLSERIALIZE    => r"XMLSERIALIZE" in COL_NAME + bare_label,
        XMLROOT         => r"XMLROOT" in COL_NAME + bare_label,
        XMLEXISTS       => r"XMLEXISTS" in COL_NAME + bare_label,
        XMLTABLE        => r"XMLTABLE" in COL_NAME + bare_label,
        XMLPARSE        => r"XMLPARSE" in COL_NAME + bare_label,
        XMLNAMESPACES   => r"XMLNAMESPACES" in COL_NAME + bare_label,
        DOCUMENT        => r"DOCUMENT" in UNRESERVED + bare_label,
        CONTENT         => r"CONTENT" in UNRESERVED + bare_label,
        VERSION         => r"VERSION" in UNRESERVED + bare_label,
        STANDALONE      => r"STANDALONE" in UNRESERVED + bare_label,
        INDENT          => r"INDENT" in UNRESERVED + bare_label,
        REF             => r"REF" in UNRESERVED + bare_label,
        YES             => r"YES" in UNRESERVED + bare_label,
        OVERLAPS        => r"OVERLAPS" in TYPE_FUNC_NAME,
        // Window frame units — safe to soften: the window `ref_name` slot
        // that would otherwise eat them is guarded by the `not_frame_unit`
        // postcondition. `PARTITION` is NOT in that guard, so it stays hard.
        ROWS            => r"ROWS" in UNRESERVED + bare_label,
        RANGE           => r"RANGE" in UNRESERVED + bare_label,
        GROUPS          => r"GROUPS" in UNRESERVED + bare_label,
        // Transaction-control non-reserved keywords. Soft so they remain
        // reclaimable as identifiers outside transaction-control positions
        // (all four are `unreserved_keyword` in Postgres' `gram.y`).
        CHAIN           => r"CHAIN" in UNRESERVED + bare_label,
        PREPARED        => r"PREPARED" in UNRESERVED + bare_label,
        PLANS           => r"PLANS" in UNRESERVED + bare_label,
        SEQUENCES       => r"SEQUENCES" in UNRESERVED + bare_label,
        // DROP-family object-kind non-reserved keywords. Soft so they remain
        // reclaimable as identifiers outside DROP positions (all six are
        // `unreserved_keyword` in Postgres' `gram.y` / `kwlist.h`).
        CLASS           => r"CLASS" in UNRESERVED + bare_label,
        FAMILY          => r"FAMILY" in UNRESERVED + bare_label,
        CONFIGURATION   => r"CONFIGURATION" in UNRESERVED + bare_label,
        DICTIONARY      => r"DICTIONARY" in UNRESERVED + bare_label,
        TEMPLATE        => r"TEMPLATE" in UNRESERVED + bare_label,
        PARSER          => r"PARSER" in UNRESERVED + bare_label,
        // `FORCE` is the only `DROP DATABASE ... (FORCE)` option keyword;
        // `unreserved_keyword` in Postgres' `gram.y` / `kwlist.h`.
        FORCE           => r"FORCE" in UNRESERVED + bare_label,
        // Cursor-declaration option keywords (`DECLARE ... CURSOR`). `HOLD`,
        // `SCROLL`, `ASENSITIVE`, `INSENSITIVE` are `unreserved_keyword` and
        // `BINARY` is `type_func_name_keyword` in Postgres' `gram.y` /
        // `kwlist.h`. Hard keywords here (like the FETCH/MOVE direction
        // keywords `NEXT`/`FORWARD`/…) since they only appear in fixed
        // cursor-grammar positions.
        HOLD            => r"HOLD" in UNRESERVED + bare_label,
        SCROLL          => r"SCROLL" in UNRESERVED + bare_label,
        BINARY          => r"BINARY" in TYPE_FUNC_NAME + bare_label,
        ASENSITIVE      => r"ASENSITIVE" in UNRESERVED + bare_label,
        INSENSITIVE     => r"INSENSITIVE" in UNRESERVED + bare_label,
        // `LOCK ... IN <lock_type> MODE` keywords. `MODE` and `EXCLUSIVE` are
        // `unreserved_keyword` in Postgres' `gram.y`, but kept hard here
        // (matching the sibling lock-mode keywords `ACCESS` / `SHARE` / `ROW`,
        // which are also hard) since they only appear in fixed lock-grammar
        // positions.
        EXCLUSIVE       => r"EXCLUSIVE" in UNRESERVED + bare_label,
        MODE            => r"MODE" in UNRESERVED + bare_label,
        // Maintenance-statement option keywords: `VACUUM FREEZE`,
        // `TRUNCATE { RESTART | CONTINUE } IDENTITY`. `FREEZE` is
        // `type_func_name_keyword` in Postgres' `gram.y` / `kwlist.h`;
        // `RESTART` and `CONTINUE` are `unreserved_keyword`. Soft here so
        // they remain reclaimable as identifiers outside maintenance
        // statements.
        FREEZE          => r"FREEZE" in TYPE_FUNC_NAME + bare_label,
        RESTART         => r"RESTART" in UNRESERVED + bare_label,
        CONTINUE        => r"CONTINUE" in UNRESERVED + bare_label,
        // GRANT/REVOKE/ALTER DEFAULT PRIVILEGES keywords. All are
        // `unreserved_keyword` in Postgres' `gram.y` / `kwlist.h`. Soft here so
        // they remain reclaimable as identifiers outside privilege grammar
        // positions. `SEQUENCES` is shared with the transaction-control block
        // above.
        ADMIN           => r"ADMIN" in UNRESERVED + bare_label,
        GRANTED         => r"GRANTED" in UNRESERVED + bare_label,
        TABLES          => r"TABLES" in UNRESERVED + bare_label,
        FUNCTIONS       => r"FUNCTIONS" in UNRESERVED + bare_label,
        PROCEDURES      => r"PROCEDURES" in UNRESERVED + bare_label,
        ROUTINES        => r"ROUTINES" in UNRESERVED + bare_label,
        SCHEMAS         => r"SCHEMAS" in UNRESERVED + bare_label,
        TYPES           => r"TYPES" in UNRESERVED + bare_label,
        // COPY-statement keywords. All `unreserved_keyword` in Postgres'
        // `gram.y` / `kwlist.h`. Soft here so they remain reclaimable as
        // identifiers outside COPY grammar positions. `DELIMITERS` (the
        // legacy `USING DELIMITERS 'c'` form) must precede `DELIMITER` so
        // longest-match-wins picks the longer spelling first.
        DELIMITERS      => r"DELIMITERS" in UNRESERVED + bare_label,
        DELIMITER       => r"DELIMITER" in UNRESERVED + bare_label,
        STDIN           => r"STDIN" in UNRESERVED + bare_label,
        STDOUT          => r"STDOUT" in UNRESERVED + bare_label,
        PROGRAM         => r"PROGRAM" in UNRESERVED + bare_label,
        QUOTE           => r"QUOTE" in UNRESERVED + bare_label,
        HEADER          => r"HEADER" in UNRESERVED + bare_label,
        CSV             => r"CSV" in UNRESERVED + bare_label,
        // CREATE/ALTER ROLE/USER/GROUP attribute keywords. None of these are
        // PG keywords — in `gram.y` they're parsed as `IDENT` and discriminated
        // by `strcmp` inside `AlterOptRoleElem`. We model each as a distinct
        // soft keyword so the AST has a unique type per attribute. Each must
        // remain reclaimable as an identifier outside role grammar positions
        // (e.g. as a role name like `regress_test_superuser`).
        //
        // `NO*` variants must precede their bare counterparts so longest-
        // match-wins picks the longer spelling first (otherwise `NOSUPERUSER`
        // would lex as `NO` followed by `SUPERUSER`).
        NOSUPERUSER     => r"NOSUPERUSER" in UNRESERVED + bare_label,
        SUPERUSER       => r"SUPERUSER" in UNRESERVED + bare_label,
        NOCREATEDB      => r"NOCREATEDB" in UNRESERVED + bare_label,
        CREATEDB        => r"CREATEDB" in UNRESERVED + bare_label,
        NOCREATEROLE    => r"NOCREATEROLE" in UNRESERVED + bare_label,
        CREATEROLE      => r"CREATEROLE" in UNRESERVED + bare_label,
        NOINHERIT       => r"NOINHERIT" in UNRESERVED + bare_label,
        NOLOGIN         => r"NOLOGIN" in UNRESERVED + bare_label,
        LOGIN           => r"LOGIN" in UNRESERVED + bare_label,
        NOREPLICATION   => r"NOREPLICATION" in UNRESERVED + bare_label,
        REPLICATION     => r"REPLICATION" in UNRESERVED + bare_label,
        NOBYPASSRLS     => r"NOBYPASSRLS" in UNRESERVED + bare_label,
        BYPASSRLS       => r"BYPASSRLS" in UNRESERVED + bare_label,
        // Role-attribute keywords that ARE Postgres `unreserved_keyword`s in
        // `kwlist.h` (CONNECTION, ENCRYPTED, PASSWORD, SYSID, UNTIL,
        // UNENCRYPTED). Soft here so they remain reclaimable as identifiers
        // outside role-attribute positions.
        CONNECTION      => r"CONNECTION" in UNRESERVED + bare_label,
        ENCRYPTED       => r"ENCRYPTED" in UNRESERVED + bare_label,
        UNENCRYPTED     => r"UNENCRYPTED" in UNRESERVED + bare_label,
        PASSWORD        => r"PASSWORD" in UNRESERVED + bare_label,
        SYSID           => r"SYSID" in UNRESERVED + bare_label,
        UNTIL           => r"UNTIL" in UNRESERVED + bare_label,
        // CREATE OBJECT keywords for CAST/TYPE forms. All `unreserved_keyword`
        // in Postgres' `gram.y` / `kwlist.h`. Soft so they remain reclaimable
        // as identifiers outside their grammar positions.
        // `ENUM` is `enum_P` in gram.y; appears in `CREATE TYPE name AS ENUM (...)`.
        // `IMPLICIT` and `ASSIGNMENT` are the two `cast_context` words after
        // `AS` in `CREATE CAST (...) WITH/WITHOUT FUNCTION AS {IMPLICIT|ASSIGNMENT}`.
        ENUM            => r"ENUM" in UNRESERVED + bare_label,
        IMPLICIT        => r"IMPLICIT" in UNRESERVED + bare_label,
        ASSIGNMENT      => r"ASSIGNMENT" in UNRESERVED + bare_label,
        // CREATE TRIGGER's transition-table clause: `REFERENCING {OLD|NEW}
        // TABLE [AS] name ...`. Postgres' `kwlist.h` marks REFERENCING as
        // `unreserved_keyword`, so it is soft here and remains reclaimable
        // as an identifier outside the trigger grammar.
        REFERENCING     => r"REFERENCING" in UNRESERVED + bare_label,
        // `ENABLE` / `DISABLE` — subscription on/off toggles in
        // `ALTER SUBSCRIPTION name { ENABLE | DISABLE }`. Both are
        // `unreserved_keyword` in `kwlist.h` (`ENABLE_P` / `DISABLE_P`).
        // Soft so they stay reclaimable as identifiers elsewhere.
        ENABLE          => r"ENABLE" in UNRESERVED + bare_label,
        DISABLE         => r"DISABLE" in UNRESERVED + bare_label,
        // `ATTRIBUTE` — used in ALTER TYPE name { ADD | DROP | ALTER | RENAME }
        // ATTRIBUTE column. `unreserved_keyword` in `kwlist.h`, so soft here
        // and remains reclaimable as an identifier elsewhere.
        ATTRIBUTE       => r"ATTRIBUTE" in UNRESERVED + bare_label,
        // ALTER {INDEX,VIEW,MATERIALIZED VIEW,SEQUENCE,TRIGGER,EVENT TRIGGER,
        // DOMAIN,DATABASE} object-level action keywords. All five are
        // `unreserved_keyword` in `kwlist.h`: `ATTACH PARTITION`,
        // `[NO] DEPENDS ON EXTENSION`, `SET LOGGED`/`SET UNLOGGED` (paired
        // with the existing hard `UNLOGGED`), `ENABLE REPLICA`, `VALIDATE
        // CONSTRAINT`. Soft so they stay reclaimable as identifiers outside
        // ALTER positions.
        ATTACH          => r"ATTACH" in UNRESERVED + bare_label,
        DEPENDS         => r"DEPENDS" in UNRESERVED + bare_label,
        LOGGED          => r"LOGGED" in UNRESERVED + bare_label,
        REPLICA         => r"REPLICA" in UNRESERVED + bare_label,
        VALIDATE        => r"VALIDATE" in UNRESERVED + bare_label,
        // ALTER TABLE-specific action keywords. All `unreserved_keyword` in
        // `kwlist.h`: `DETACH PARTITION` (+ optional FINALIZE), `SET EXPRESSION
        // AS` / `DROP EXPRESSION`. Soft so they remain reclaimable as
        // identifiers outside ALTER TABLE positions.
        DETACH          => r"DETACH" in UNRESERVED + bare_label,
        FINALIZE        => r"FINALIZE" in UNRESERVED + bare_label,
        EXPRESSION      => r"EXPRESSION" in UNRESERVED + bare_label,
        // `RECHECK` — accepted by `CREATE OPERATOR CLASS` opclass items
        // (`OPERATOR n any_op opclass_purpose [RECHECK]`). `unreserved_keyword`
        // in `kwlist.h`; deprecated but still parsed for legacy-dump
        // compatibility. Soft so it stays reclaimable as an identifier outside
        // opclass-item positions.
        RECHECK         => r"RECHECK" in UNRESERVED + bare_label,
    },
    punctuation {
        Semi      => ";",          ";",
        Comma     => ",",          ",",
        LParen    => r"\(",        "(",
        RParen    => r"\)",        ")",
        // Record comparison operators (`*=` etc.) — longest-match first so
        // they win over bare `Star`. Used only as Pratt infix operators.
        StarLte   => r"\*<=",      "*<=",
        StarGte   => r"\*>=",      "*>=",
        StarNeq   => r"\*<>",      "*<>",
        StarLt    => r"\*<",       "*<",
        StarGt    => r"\*>",       "*>",
        StarEq    => r"\*=",       "*=",
        Star      => r"\*",        "*",
        Dot       => r"\.",        ".",
        // 3-char `===` before 2-char `=>` and single-char `=`.
        TripleEq   => "===",       "===",
        Eq        => "=",          "=",
        FatArrow  => r"=>",        "=>",
        ColonEquals => r":=",      ":=",
        // 3-char `!==` and `!=-` before 2-char `!=`.
        BangEqEq   => "!==",       "!==",
        BangEqMinus => "!=-",      "!=-",
        BangEq    => "!=",         "!=",
        Neq       => "<>",         "<>",
        // 3-char `<`-prefixed operators must come before 2-char `<=`/`<>`/`<<`
        // and before the single-char `<`.
        // 3-char `<<<` before 2-char `<<`.
        LtLtLt     => r"<<<",      "<<<",
        LtLtEq     => r"<<=",      "<<=",
        LtLtPipe   => r"<<\|",     "<<|",
        LtMinusGt  => r"<->",      "<->",
        LtLt       => r"<<",       "<<",
        Lte       => "<=",         "<=",
        // Geometric "below" `<^` before single-char `<`.
        LtCaret    => r"<\^",      "<^",
        // 3-char `>>=` before `>>`, then `>=`, `>`.
        // 3-char `>>>` before 2-char `>>`.
        GtGtGt     => r">>>",      ">>>",
        GtGtEq     => r">>=",      ">>=",
        GtGt       => r">>",       ">>",
        Gte       => ">=",         ">=",
        // Geometric "above" `>^` before single-char `>`.
        GtCaret    => r">\^",      ">^",
        Lt        => "<",          "<",
        Gt        => ">",          ">",
        ColonColon => "::",        "::",
        Colon      => ":",          ":",
        // Psql meta-commands that can terminate a SQL statement in place of `;`.
        // Must be listed before plain BackSlash so longest-match-wins picks the
        // specific directive over the bare backslash.
        PsqlCrosstabview => r"\\crosstabview\b", "\\crosstabview",
        PsqlGexec  => r"\\gexec\b", "\\gexec",
        PsqlGset   => r"\\gset\b",  "\\gset",
        PsqlGx     => r"\\gx\b",    "\\gx",
        PsqlG      => r"\\g\b",     "\\g",
        // `\;` — psql batch separator: ends a statement without ending the
        // line. Listed before bare `BackSlash`.
        PsqlBatchSemi => r"\\;",   "\\;",
        BackSlash  => r"\\",       "\\",
        Plus       => r"\+",       "+",
        // 3-char `-|-` before 2-char `->>`/`->` before single-char `-`.
        MinusPipeMinus => r"-\|-", "-|-",
        Minus      => "-",         "-",
        // 3-char `|>>` and `|&>` before 2-char `||`.
        PipeGtGt       => r"\|>>", "|>>",
        PipeAmpGt      => r"\|&>", "|&>",
        // Cube-root `||/` (prefix operator). Must come before `||`.
        PipePipeSlash  => r"\|\|/", "||/",
        Concat     => r"\|\|",     "||",
        // Square-root `|/` (prefix operator). Must come after `||/`/`||`
        // and before bare `|`.
        PipeSlash      => r"\|/",  "|/",
        // Single-char `|` (bitwise OR). Must be declared after `||` and
        // other `|`-prefixed operators so longest-match picks the longer form.
        Pipe       => r"\|",       "|",
        Slash      => "/",         "/",
        Percent    => "%",         "%",
        LBracket   => r"\[",       "[",
        RBracket   => r"\]",       "]",
        // JSON/JSONB operators. Longer before shorter (longest-match-wins).
        HashArrowArrow => r"#>>",      "#>>",
        HashArrow      => r"#>",       "#>",
        // 2-char `##` (geometric closest-point / path intersection). After
        // `#>>` and `#>` but before single-char `#`.
        HashHash       => "##",        "##",
        // jsonb delete-path operator `#-`. Must precede single-char `Pound`
        // so longest-match-wins picks the 2-char form. Without this entry the
        // classifier splits `#-` into `Pound` + `Minus`, which the formatter
        // then re-emits as `# -` — re-parsed by PostgreSQL as two operators.
        HashMinus      => r"#-",       "#-",
        // Single-char `#` (bitwise XOR). Must come after all longer `#`-prefixed
        // tokens so longest-match-wins.
        Pound          => r"\#",       "#",
        ArrowArrow     => r"->>",      "->>",
        Arrow          => r"->",       "->",
        // 3-char `?||` and `?-|` before 2-char `?|`/`?-` (longest-match-wins).
        QuestionPipePipe => r"\?\|\|",  "?||",
        QuestionDashPipe => r"\?-\|",   "?-|",
        QuestionPipe   => r"\?\|",     "?|",
        QuestionAmp    => r"\?&",      "?&",
        QuestionHash   => r"\?#",      "?#",
        QuestionDash   => r"\?-",      "?-",
        // `@@@` before 3-char `@-@`/`@#@`/`@+@` before `@@` before `@?` / `@>`.
        AtAtAt         => r"@@@",      "@@@",
        // User-defined / geometric 3-char `@`-prefixed operators.
        AtMinusAt      => r"@-@",      "@-@",
        AtHashAt       => r"@#@",      "@#@",
        AtPlusAt       => r"@\+@",     "@+@",
        AtAt           => r"@@",       "@@",
        AtQuestion     => r"@\?",      "@?",
        AtGt           => r"@>",       "@>",
        // Single-char `@` (prefix absolute-value operator). Declared after
        // all longer `@`-prefixed operators so longest-match-wins.
        At             => r"@",        "@",
        LtAt           => r"<@",       "<@",
        Question       => r"\?",       "?",
        // `&`-prefixed range/geometric operators. 3-char `&<|` before 2-char.
        AmpLtPipe      => r"&<\|",     "&<|",
        AmpAmp         => r"&&",       "&&",
        AmpLt          => r"&<",       "&<",
        AmpGt          => r"&>",       "&>",
        // Single-char `&` (bitwise AND). Must follow all longer `&`-prefixed
        // operators so longest-match-wins.
        Amp            => r"&",        "&",
        // Locale-aware text comparison operators. 4-char before 3-char,
        // all before POSIX regex `~*`/`!~*`/`!~`/`~=`/`~`.
        TildeLeqTilde  => r"~<=~",     "~<=~",
        TildeGeqTilde  => r"~>=~",     "~>=~",
        TildeLtTilde   => r"~<~",      "~<~",
        TildeGtTilde   => r"~>~",      "~>~",
        // LIKE/ILIKE family operators. PG uses `~~`/`!~~`/`~~*`/`!~~*` as
        // the implementation operators for LIKE/NOT LIKE/ILIKE/NOT ILIKE.
        // Longer forms must precede shorter forms (and precede the POSIX
        // `~*`/`!~*`/`~`/`!~` family) so longest-match wins.
        BangTildeTildeStar => r"!~~\*", "!~~*",
        TildeTildeStar => r"~~\*",     "~~*",
        BangTildeTilde => r"!~~",      "!~~",
        TildeTilde     => r"~~",       "~~",
        // POSIX regex match operators. Longest-first.
        BangTildeStar  => r"!~\*",     "!~*",
        TildeStar      => r"~\*",      "~*",
        BangTilde      => r"!~",       "!~",
        // Geometric "same as" operator. Must precede bare `~`.
        TildeEq        => r"~=",       "~=",
        Tilde          => r"~",        "~",
        // Text "starts-with" operator `^@`. Must precede single-char `Caret`
        // so longest-match-wins picks the 2-char form. Without this entry the
        // classifier splits `^@` into `Caret` + `At`, which the formatter then
        // re-emits as `^ @` — re-parsed by PostgreSQL as two operators.
        CaretAt        => r"\^@",      "^@",
        // Exponentiation operator (Postgres).
        Caret          => r"\^",       "^",
    },
    literals {
        // `DollarStringLit` — PG dollar-quoted string (`$tag$ ... $tag$`,
        // `$$ ... $$`). The `#[regex]` matches only the opening `$tag$`; the
        // `scan_dollar_string` logos callback then scans for the matching
        // close tag (a back-reference no regex DFA can express) and bumps the
        // lexer past it.
        //
        // The tag follows unquoted-identifier rules — empty, or
        // `[A-Za-z_][A-Za-z0-9_]*` — so it cannot start with a digit. `$1$`
        // is therefore NOT a dollar-string opening; `$1` lexes as `DollarNum`
        // (a positional parameter), matching PostgreSQL.
        //
        // `#[arbitrary(custom)]`: the `#[regex]` pattern is only the opening
        // `$tag$`, so the auto-generated pattern-sampled `Arbitrary` would
        // yield an unterminated string. A hand-written `Arbitrary` (see the
        // `arbitrary_impls` module below) supplies a complete `$tag$…$tag$`.
        #[railroad(label = "<$ String Literal>")]
        #[arbitrary(custom)]
        DollarStringLit => r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$" with crate::tokens::scan_dollar_string,
        #[railroad(label = "<Unicode Quoter Identifier>")]
        UnicodeQuotedIdent => r#"(?i:U)&"[^"]*(?:""[^"]*)*""#,
        #[railroad(label = "<Quoted Identifier>")]
        QuotedIdent => r#""[^"]*(?:""[^"]*)*""#,
        #[railroad(label = "<Unicode String Literal>")]
        UnicodeStringLit => r"(?i:U)&'(?:[^']|'')*'",
        #[railroad(label = "<Escape String Literal>")]
        EscapeStringLit => r"(?i:E)'(?:[^'\\]|\\.|'')*'",
        // Bit-string and hex-string literals: `B'10'`, `X'1FF'`. Declared
        // before `StringLit` so longest-match-wins picks the prefixed form
        // when the `B`/`X` prefix is present — otherwise the lexer would
        // consume the prefix as an identifier and the quoted body as an
        // ordinary `StringLit`, which the formatter then separates with a
        // space (`B '10'`). Bodies use `[^']*` (no `''`-escape) because
        // Postgres bit/hex strings cannot contain quotes. Invalid body
        // contents (non-`0`/`1` inside `B'…'`, non-hex inside `X'…'`) are a
        // parse-time error in PG, not a lex-time one — mirror that and
        // accept any non-quote body at the lexer level.
        #[railroad(label = "<Bit String Literal>")]
        BitStringLit => r"(?i:B)'[^']*'",
        #[railroad(label = "<Hex String Literal>")]
        HexStringLit => r"(?i:X)'[^']*'",
        #[railroad(label = "<String Literal>")]
        StringLit  => r"'[^']*(?:''[^']*)*'",
        // NumericLit must require a decimal point OR an exponent so it does
        // not collide with bare integers (handled by IntegerLit). Forms:
        //   123.45    .5    123.    1e10    1.5e-5    .5e10
        // Declared before IntegerLit so longest-match-wins picks the longer
        // literal when an exponent is present. NumericLit doesn't recognise
        // the `0x`/`0o`/`0b` prefixes at all, so a hex/octal/binary literal
        // (caught by IntegerLit below) cannot match NumericLit — Postgres has
        // no hex-float literal form. The longest-match-wins tiebreak only
        // matters for the bare-decimal arm (e.g. `0.5` vs `0`); ordering
        // IntegerLit's non-decimal arms first inside its own alternation is
        // mostly defensive — the `0x`/`0o`/`0b` prefixes don't conflict with
        // its decimal arm either.
        // Digit groups allow `_` as a separator between digits (Postgres 16+).
        // A digit group is `[0-9](?:_?[0-9])*` — starts with a digit, and
        // every subsequent `_` must be followed by a digit.
        //
        // Trailing-junk rejection: mirror Postgres flex's "trailing junk
        // after numeric literal" rejection. Without it, `1.5abc` permissively
        // splits into `NumericLit("1.5")` + `Ident("abc")`, but PG raises a lex
        // error for that input.
        //
        // logos has no `\b`/`\B`, so the old trailing-boundary anchors are
        // replaced by the `reject_trailing_word` logos callback. The two arms
        // the old regex distinguished — digit-terminated forms (old `\b`) and
        // the bare trailing-dot form `1.` (old `\B` after `.`) — reject on
        // exactly the same condition: the next char being a word char
        // (`[A-Za-z0-9_]`). One callback covers both. A rejected literal makes
        // `lex` emit a `TOKEN_KIND_NONE` span, the statement falls to `Raw`,
        // and psql emits the canonical PG error matching the regress oracle.
        // See regress_numerology's trailing-junk fixtures.
        //
        // NumericLit requires a decimal point OR exponent so it does not
        // collide with bare integers; declared before IntegerLit so
        // longest-match-wins picks the longer literal when an exponent is
        // present. Digit groups allow `_` as a separator (Postgres 16+).
        #[railroad(label = "<Numeric Literal>")]
        NumericLit => r"(?:(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\.)" with crate::tokens::reject_trailing_word,
        // IntegerLit accepts decimal, hex (`0x…`/`0X…`), octal (`0o…`/`0O…`),
        // and binary (`0b…`/`0B…`) forms with optional `_` digit separators
        // (Postgres 16+). The non-decimal alternatives are listed first so
        // longest-match-wins picks them over the bare-decimal alternative
        // when the `0x`/`0o`/`0b` prefix is present. The `reject_trailing_word`
        // callback supplies the old trailing `\b` semantics — see NumericLit.
        #[railroad(label = "<Integer Literal>")]
        IntegerLit => r"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)" with crate::tokens::reject_trailing_word,
        // Positional parameter reference: `$1`, `$2`, … used in function
        // bodies and prepared statements. Declared as a *capturing* literal
        // (not punctuation) so the digits survive into the AST — a punctuation
        // token is a unit struct with a fixed display string and would
        // collapse every `$n` to a bare `$`. `\$[0-9]+` does not prefix-conflict
        // with the hand-rolled `DollarStringLit` (`$tag$…`), whose tag is
        // `[a-zA-Z_]*` and so cannot start with a digit.
        //
        // The `reject_trailing_word` callback rejects `$1a` / `$0_1` — a
        // parameter immediately followed by an identifier character. PG's
        // scanner flags this as "trailing junk after parameter", so without
        // the callback pg-sql would over-permissively split `$1a` into
        // `$1` + `a` and accept SQL that PG rejects (caught by the
        // differential `numerology` corpus).
        #[railroad(label = "<Positional Parameter>")]
        DollarNum => r"\$[0-9]+" with crate::tokens::reject_trailing_word,
        // psql client variable substitution: `:foo` or `:'foo'` or `:"foo"`.
        // Treated as an opaque expression atom so SELECTs that reference
        // psql-set variables parse structurally.
        #[railroad(label = "<psql var>")]
        PsqlVar => r#":(?:[A-Za-z_][A-Za-z0-9_]*|'[^']*'|"[^"]*")"#,
    },
    // Lexer-only tokens: each contributes a `TokenKind` variant + `#[regex]`
    // so logos lexes it, but the `tokens!` macro generates NO struct / `Parse`
    // / `FormatTokens` for them. Their token types are hand-written below
    // because their `Parse` impl does more than a plain kind-check.
    lexer_tokens {
        // `/* ... */` block comment (PostgreSQL allows nesting). The
        // `#[regex]` matches only the opening `/*`; the `skip_block_comment`
        // callback scans the nested body and returns `Skip`, so no
        // `BlockComment` token ever reaches the array — the variant exists
        // only to host the `#[regex]` + callback.
        //
        // Declared FIRST in this block so it outranks `CustomOp`: `/*` matches
        // both `BlockComment` (`/\*`, 2 chars) and `CustomOp`
        // (`[-+*/...]{2,}`, 2 chars). logos breaks an equal-length tie by
        // priority, and the `tokens!` macro assigns priorities descending in
        // declaration order — so `BlockComment` must precede `CustomOp` to win.
        BlockComment => r"/\*" with crate::tokens::skip_block_comment,
        // The single lexical "word" kind. In a real lexer one span yields one
        // token, so `UnquotedIdent`, `BareAliasName`, and reserved/soft
        // keywords all share this lexical kind; the hand-written `Parse` impls
        // below reclaim soft keywords / keywords per their acceptance rules.
        //
        // FUTURE: declare `accepts soft_keywords` here so the first-set
        // walker can include soft-keyword kinds in the `Unquoted` variant's
        // first set. The macro syntax landed (see preceding commits) but
        // two pieces of build-time codegen still need to honour
        // `accepts_kinds`:
        //   1. `compute_first_sets` (firstset.rs) — currently treats
        //      every `TypeKind::Token` as a single-kind leaf, so adding
        //      an `accepts` clause today inserts only the `UnquotedIdent`
        //      branch into `Ident`'s first-set tree and drops soft-keyword
        //      reclamation (verified by 102 broken pg-sql tests).
        //   2. `emit_pratt_enum_impl` (parse_emit.rs) — Pratt nud kind-
        //      match doesn't yet expand per accepted kind, so `Expr`
        //      falls through to its sequential atom path on soft-keyword
        //      input (e.g. `sum(c)` where `sum` lexes as `UnquotedIdent`
        //      — observed as 45 broken tests once (1) is addressed).
        // See docs/plans/2026-05-26-codegen-kind-match-universal.md
        // §"Phase 6 — accepts-clause infrastructure (deferred)".
        UnquotedIdent => r"[A-Za-z_][A-Za-z0-9_]*",
        // Multi-char user-defined operator. logos longest-match resolves
        // single-char punct (`+` → `Plus`) vs multi-char (`++` → `CustomOp`).
        //
        // Encodes PostgreSQL's operator-boundary rule from `scan.l`: a
        // multi-char operator that ends in `+` or `-` must contain at
        // least one of the "qualifying" chars `~ ! @ # ^ & | ? %`,
        // otherwise the trailing `+`/`-` is stripped. The classic case
        // is `=-`: PG lexes it as `Eq` + `Minus`, not as a single
        // 2-char Op. The two alternatives below encode that:
        //
        //   alt A: contains a qualifying char anywhere (length >= 2).
        //   alt B: only non-qualifying op chars and doesn't end in `+`/`-`.
        //
        // Alt B uses `[-+*/<>=]+[*/<>=]` — the final char is constrained
        // to `[*/<>=]`, so `=-`, `=+`, `<-`, `++`, `+-`, `-+`, `*+`,
        // `/-` and friends do NOT match `CustomOp` and instead lex as
        // their single-char tokens.
        CustomOp => r"([-+*/<>=~!@#%^&|?]*[~!@#%^&|?][-+*/<>=~!@#%^&|?]*|[-+*/<>=]+[*/<>=])",
    },
    // `bare_label_keywords` = every keyword carrying the `bare_label` flag,
    // regardless of category. Used by `BareColLabel` to admit any keyword
    // PG would accept as a bare (no-`AS`) column label.
    classes {
        bare_label_keywords = keywords where bare_label,
    }
    // Postgres identifier-position nonterminals. Each target generates a
    // restricted-admit keyword enum (`<Name>Keyword`) plus an outer
    // two-variant target enum (`<Name> = Ident | <Name>Keyword`). The
    // categories admitted by each target mirror the unions in PG's
    // `gram.y`:
    //
    //   ColId               = IDENT + U + C
    //   type_function_name  = IDENT + U + T
    //   NonReservedWord     = IDENT + U + C + T
    //   ColLabel            = IDENT + U + C + T + R   (all four categories)
    //   BareColLabel        = IDENT + keywords with bare_label set
    //
    // No AST site references these types yet — Phase 3 migrates pg-sql's
    // existing `Ident` AST sites to the appropriate target.
    targets {
        ColId              : literal::Ident admits UNRESERVED, COL_NAME,
        type_function_name : literal::Ident admits UNRESERVED, TYPE_FUNC_NAME,
        NonReservedWord    : literal::Ident admits UNRESERVED, COL_NAME, TYPE_FUNC_NAME,
        ColLabel           : literal::Ident admits UNRESERVED, COL_NAME, TYPE_FUNC_NAME, RESERVED,
        BareColLabel       : literal::Ident admits bare_label_keywords,
    }
}

/// Lex `src` for the pg-sql grammar — wraps the macro-generated `lex` and
/// post-processes a single ambiguity left over from the logos lex pass.
///
/// The `PsqlVar` regex `:(?:[A-Za-z_]\w*|'…'|"…")` greedily consumes
/// `:KEYWORD` (e.g. `:NULL`, `:TRUE`) as a single psql-variable token,
/// shadowing the legitimate SQL parse `':' <bool literal>` inside
/// array-slice bounds. Gram.y permits any `a_expr` in slice bounds, so this
/// pass splits any `PsqlVar` whose body is a SQL reserved keyword into a
/// `Colon` + keyword pair before the parser sees the cursor.
///
/// Only the bare-ident form of `PsqlVar` is split; quoted forms
/// (`:'name'` / `:"name"`) carry their own delimiters and are never SQL
/// keywords.
pub fn pg_lex(src: &str) -> ::recursa::LexResult {
    let mut lexed = lex(src);
    split_psql_var_keyword_tokens(src, &mut lexed.tokens);
    split_bang_eq_minus_before_dash_comment(src, &mut lexed.tokens);
    lexed
}

/// Compensate for the `!=` + `--` greedy lex when the source has `!=--`.
///
/// PG's scan.l, presented with `!=--`, lexes `!=` (BangEq) and then skips
/// the `-- …` line comment. logos can't model that with longest-match
/// alone — it greedily takes the 3-char `!=-` (BangEqMinus) or the 4-char
/// `!=--` (CustomOp) and treats the second `-` as part of the operator
/// rather than the start of a line comment.
///
/// This pass walks the records: when it sees a `BangEqMinus` whose byte
/// after `end` is `-`, OR a `CustomOp` / `BangEqMinus`-kind record whose
/// source text begins with `!=--`, it rewrites the record as `BangEq` (2
/// bytes) and drops every subsequent token that falls inside the
/// line-comment span up to (but not including) the next newline.
fn split_bang_eq_minus_before_dash_comment(
    src: &str,
    tokens: &mut Vec<::recursa_core::TokenRecord>,
) {
    let bang_eq_minus_kind = TokenKind::BangEqMinus as u16;
    let custom_op_kind = TokenKind::CustomOp as u16;
    let bang_eq_kind = TokenKind::BangEq as u16;
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < tokens.len() {
        let rec = tokens[i];
        // Two greedy-match cases:
        //
        //  - `BangEqMinus` (`!=-`, 3 bytes) followed in source by another
        //    `-` — the legitimate 3-char operator (e.g. `CREATE OPERATOR
        //    !=-` or `SELECT 10 !=-`) is unaffected because the next byte
        //    after `-` is whitespace / `;`, not another `-`.
        //
        //  - `CustomOp` whose text starts with `!=--` — logos's multi-char
        //    operator catch-all happily consumes all four chars even
        //    though the trailing `--` should have started a line comment.
        let start = rec.start as usize;
        let end = rec.end as usize;
        let split_at_2 = if rec.kind == bang_eq_minus_kind {
            bytes.get(end).copied() == Some(b'-')
        } else if rec.kind == custom_op_kind {
            let text = &bytes[start..end];
            text.len() >= 4 && text.starts_with(b"!=--")
        } else {
            false
        };
        if !split_at_2 {
            i += 1;
            continue;
        }
        // Rewrite the operator record into a BangEq covering the first two bytes.
        tokens[i] = ::recursa_core::TokenRecord {
            kind: bang_eq_kind,
            start: rec.start,
            end: rec.start + 2,
        };
        // The `-- …` line comment runs from `start + 2` up to the next
        // newline. Drop every token whose span lies inside that comment.
        let comment_start = start + 2;
        let comment_end = bytes[comment_start..]
            .iter()
            .position(|&b| b == b'\n')
            .map(|j| comment_start + j)
            .unwrap_or(bytes.len());
        let mut j = i + 1;
        while j < tokens.len() {
            let r = tokens[j];
            if (r.start as usize) >= comment_end {
                break;
            }
            if (r.end as usize) <= comment_end {
                tokens.remove(j);
                continue;
            }
            j += 1;
        }
        i += 1;
    }
}

fn split_psql_var_keyword_tokens(src: &str, tokens: &mut Vec<::recursa_core::TokenRecord>) {
    let psql_var_kind = TokenKind::PsqlVar as u16;
    let colon_kind = TokenKind::Colon as u16;
    let mut i = 0;
    while i < tokens.len() {
        let rec = tokens[i];
        if rec.kind != psql_var_kind {
            i += 1;
            continue;
        }
        let text = &src[rec.start as usize..rec.end as usize];
        // Only the `:ident` form (no quote after the colon) can name a
        // SQL keyword; `:'…'` and `:"…"` carry literal delimiters.
        let body = match text.as_bytes().get(1) {
            Some(&b) if b.is_ascii_alphabetic() || b == b'_' => &text[1..],
            _ => {
                i += 1;
                continue;
            }
        };
        let Some(body_kind) = psql_var_body_keyword_kind(body) else {
            i += 1;
            continue;
        };
        // Replace the single PsqlVar record with [Colon, keyword].
        let colon = ::recursa_core::TokenRecord {
            kind: colon_kind,
            start: rec.start,
            end: rec.start + 1,
        };
        let body_rec = ::recursa_core::TokenRecord {
            kind: body_kind,
            start: rec.start + 1,
            end: rec.end,
        };
        tokens[i] = colon;
        tokens.insert(i + 1, body_rec);
        i += 2;
    }
}

/// Reserved keywords that pg-sql refuses to lex as the body of a bare-ident
/// `PsqlVar`. Returns the matching `TokenKind` discriminant so the lex
/// post-processor can re-emit the body as the proper keyword token.
fn psql_var_body_keyword_kind(body: &str) -> Option<u16> {
    // Compare case-insensitively against the (small) reserved-keyword set
    // that legitimately appears after `:` in slice-bound positions. Extend
    // this list as the corpus surfaces more cases.
    if body.eq_ignore_ascii_case("NULL") {
        Some(TokenKind::NULL as u16)
    } else if body.eq_ignore_ascii_case("TRUE") {
        Some(TokenKind::TRUE as u16)
    } else if body.eq_ignore_ascii_case("FALSE") {
        Some(TokenKind::FALSE as u16)
    } else {
        None
    }
}

/// Lex `src` and build an `Input` borrowing a `'static`-leaked `LexResult`.
///
/// Test-only convenience. `Input::new` borrows a `LexResult`, so a caller
/// must hold the `LexResult` in a binding for the `Input`'s lifetime. In
/// tests that is noise: this helper lexes `src` and `Box::leak`s the result
/// so the returned `Input<'static>` needs no caller-side binding. Leaking is
/// acceptable here — test processes are short-lived.
#[cfg(test)]
pub fn test_input(src: &'static str) -> ::recursa::Input<'static> {
    let lexed: &'static ::recursa::LexResult =
        ::std::boxed::Box::leak(::std::boxed::Box::new(pg_lex(src)));
    ::recursa::Input::new(src, lexed)
}

// Re-exports preserving the legacy `keyword::` / `punct::` / `literal::` paths
// used throughout the rest of pg-sql. The `tokens!` macro emits every token
// struct at this module's top level; these sub-modules expose them under the
// names existing imports expect.

// `DollarStringLit` is now a `tokens!` `literals` entry (its
// `scan_dollar_string` logos callback handles the matched-close back-
// reference at lex time), so it is generated at this module's top level
// like every other literal — no extra re-export needed here.

#[allow(non_camel_case_types)]
pub mod keyword {
    pub use super::Keyword;
    pub use super::{
        ABORT,
        ABSOLUTE,
        ACCESS,
        ACTION,
        ADD,
        AFTER,
        AGGREGATE,
        ALL,
        ALSO,
        ALTER,
        ALWAYS,
        ANALYZE,
        AND,
        ANY,
        ARRAY,
        AS,
        ASC,
        ASENSITIVE,
        AT,
        ATOMIC,
        AUTHORIZATION,
        BACKWARD,
        BEFORE,
        BEGIN,
        BETWEEN,
        BINARY,
        BIT,
        BOOL,
        BOOLEAN,
        BOTH,
        BREADTH,
        BRIN,
        BTREE,
        BY,
        CACHE,
        CALL,
        CALLED,
        CASCADE,
        CASCADED,
        CASE,
        CAST,
        CHAIN,
        CHARACTER,
        CHARACTERISTICS,
        CHECK,
        CHECKPOINT,
        CLOSE,
        CLUSTER,
        COLLATE,
        COLLATION,
        COLUMN,
        COMMENT,
        COMMENTS,
        COMMIT,
        COMMITTED,
        COMPRESSION,
        CONCURRENTLY,
        CONFLICT,
        CONSTRAINT,
        CONSTRAINTS,
        // XML function family (soft).
        CONTENT,
        CONVERSION,
        COPY,
        COST,
        COSTS,
        CREATE,
        CROSS,
        CUBE,
        CURRENT,
        CURSOR,
        CYCLE,
        DATA,
        DATABASE,
        DAY,
        DEALLOCATE,
        DECLARE,
        DEFAULT,
        DEFAULTS,
        DEFERRABLE,
        DEFERRED,
        DEFINER,
        DELETE,
        DEPTH,
        DESC,
        DISCARD,
        DISTINCT,
        DO,
        DOCUMENT,
        DOMAIN,
        DOUBLE,
        DROP,
        EACH,
        ELSE,
        END,
        ESCAPE,
        EVENT,
        EXCEPT,
        EXCLUDE,
        EXCLUDED,
        EXCLUDING,
        EXCLUSIVE,
        EXECUTE,
        EXISTS,
        EXPLAIN,
        EXTENDED,
        EXTENSION,
        EXTERNAL,
        EXTRACT,
        FALSE,
        FETCH,
        FILTER,
        FIRST,
        FOLLOWING,
        FOR,
        FOREIGN,
        FORWARD,
        FROM,
        FULL,
        FUNCTION,
        GENERATED,
        GIN,
        GIST,
        GRANT,
        GROUP,
        GROUPING,
        GROUPS,
        HANDLER,
        HASH,
        HAVING,
        HOLD,
        HOUR,
        IDENTITY,
        IF,
        ILIKE,
        IMMEDIATE,
        IMMUTABLE,
        IMPORT,
        IN,
        INCLUDE,
        INCLUDING,
        INCREMENT,
        INDENT,
        INDEX,
        INDEXES,
        INHERIT,
        INHERITS,
        INITIALLY,
        INLINE,
        INNER,
        INOUT,
        INPUT,
        INSENSITIVE,
        INSERT,
        INSTEAD,
        INT,
        INTEGER,
        INTERSECT,
        INTERVAL,
        INTO,
        INVOKER,
        IS,
        ISNULL,
        ISOLATION,
        JOIN,
        KEY,
        LABEL,
        LANGUAGE,
        LARGE,
        LAST,
        LATERAL,
        LEADING,
        LEAKPROOF,
        LEFT,
        LEVEL,
        LIKE,
        LIMIT,
        LISTEN,
        LOAD,
        LOCAL,
        LOCATION,
        LOCK,
        LOCKED,
        MAIN,
        MAPPING,
        MATCH,
        MATCHED,
        MATERIALIZED,
        MAXVALUE,
        MERGE,
        METHOD,
        MINUTE,
        MINVALUE,
        MODE,
        MODULUS,
        MONTH,
        MOVE,
        NAME,
        NATURAL,
        NEW,
        NEXT,
        NFC,
        NFD,
        NFKC,
        NFKD,
        NO,
        NONE,
        NORMALIZED,
        NOT,
        NOTHING,
        NOTIFY,
        NOTNULL,
        NOWAIT,
        NULL,
        NULLS,
        NUMERIC,
        OF,
        OFF,
        OFFSET,
        OIDS,
        OLD,
        ON,
        ONLY,
        OPERATOR,
        OPTION,
        OPTIONS,
        OR,
        ORDER,
        ORDINALITY,
        OTHERS,
        OUT,
        OUTER,
        OVER,
        OVERLAPS,
        OVERLAY,
        OVERRIDING,
        OWNED,
        OWNER,
        PARALLEL,
        PARTIAL,
        PARTITION,
        PLACING,
        PLAIN,
        PLANS,
        POLICY,
        POSITION,
        PRECEDING,
        PRECISION,
        PREPARE,
        PREPARED,
        PRESERVE,
        PRIMARY,
        PRIOR,
        PRIVILEGES,
        PROCEDURAL,
        PROCEDURE,
        PUBLICATION,
        RANGE,
        READ,
        REASSIGN,
        RECURSIVE,
        REF,
        REFERENCES,
        REFRESH,
        REINDEX,
        RELATIVE,
        RELEASE,
        REMAINDER,
        RENAME,
        REPEATABLE,
        REPLACE,
        RESET,
        RESTRICT,
        RESTRICTED,
        RETURN,
        RETURNING,
        RETURNS,
        REVOKE,
        RIGHT,
        ROLE,
        ROLLBACK,
        ROLLUP,
        ROUTINE,
        ROW,
        ROWS,
        RULE,
        SAFE,
        SAVEPOINT,
        SCHEMA,
        SCROLL,
        SEARCH,
        SECOND,
        SECURITY,
        SELECT,
        SEQUENCE,
        SEQUENCES,
        SERIAL,
        SERIALIZABLE,
        SERVER,
        SESSION,
        SET,
        SETOF,
        SETS,
        SHARE,
        SHOW,
        SIMILAR,
        SIMPLE,
        SKIP,
        SNAPSHOT,
        SOME,
        SOURCE,
        SPGIST,
        SQL,
        STABLE,
        STANDALONE,
        START,
        STATEMENT,
        STATISTICS,
        STORAGE,
        STORED,
        STRICT,
        SUBSCRIPTION,
        SUBSTRING,
        SUMMARY,
        SUPPORT,
        SYSTEM,
        TABLE,
        TABLESAMPLE,
        TABLESPACE,
        TARGET,
        TEMP,
        TEMPORARY,
        TEXT,
        THEN,
        TIES,
        TIME,
        TIMESTAMP,
        TIMING,
        TO,
        TRAILING,
        TRANSACTION,
        TRANSFORM,
        TRIGGER,
        TRIM,
        TRUE,
        TRUNCATE,
        TRUSTED,
        TYPE,
        UESCAPE,
        UNBOUNDED,
        UNCOMMITTED,
        UNION,
        UNIQUE,
        UNKNOWN,
        UNLISTEN,
        UNLOGGED,
        UNSAFE,
        UPDATE,
        USER,
        USING,
        VACUUM,
        VALID,
        VALIDATOR,
        VALUE,
        VALUES,
        VARCHAR,
        VARIADIC,
        VARYING,
        VERBOSE,
        VERSION,
        VIEW,
        VOLATILE,
        WHEN,
        WHERE,
        WINDOW,
        WITH,
        WITHIN,
        WITHOUT,
        WORK,
        WRAPPER,
        WRITE,
        XML,
        XMLATTRIBUTES,
        XMLELEMENT,
        XMLEXISTS,
        XMLFOREST,
        XMLNAMESPACES,
        XMLPARSE,
        XMLPI,
        XMLROOT,
        XMLSERIALIZE,
        XMLTABLE,
        YEAR,
        YES,
        ZONE,
    };
}

/// Soft (non-reserved) keyword token types — the SQL/JSON function family.
/// Re-exported from the parent module's `tokens!` output so call-sites and
/// generated code can use `soft_keyword::FORMAT` etc.
pub mod soft_keyword {
    pub use super::SoftKeyword;
    pub use super::{
        ABSENT, ADMIN, ASSIGNMENT, ATTACH, ATTRIBUTE, BYPASSRLS, CLASS, COLUMNS, CONDITIONAL,
        CONFIGURATION, CONNECTION, CONTINUE, CREATEDB, CREATEROLE, CSV, DELIMITER, DELIMITERS,
        DEPENDS, DETACH, DICTIONARY, DISABLE, EMPTY, ENABLE, ENCODING, ENCRYPTED, ENUM, ERROR,
        EXPRESSION, FAMILY, FINALIZE, FORCE, FORMAT, FREEZE, FUNCTIONS, GRANTED, HEADER, IMPLICIT,
        JSON, JSON_ARRAY, JSON_ARRAYAGG, JSON_EXISTS, JSON_OBJECT, JSON_OBJECTAGG, JSON_QUERY,
        JSON_SCALAR, JSON_SERIALIZE, JSON_TABLE, JSON_VALUE, KEEP, KEYS, LOGGED, LOGIN, NESTED,
        NFC, NFD, NFKC, NFKD, NOBYPASSRLS, NOCREATEDB, NOCREATEROLE, NOINHERIT, NOLOGIN,
        NOREPLICATION, NORMALIZED, NOSUPERUSER, OBJECT, OMIT, PARSER, PASSING, PASSWORD, PATH,
        PROCEDURES, PROGRAM, QUOTE, QUOTES, RECHECK, REFERENCING, REPLICA, REPLICATION, RESTART,
        ROUTINES, SCALAR, SCHEMAS, SQL, STDIN, STDOUT, STRING, SUPERUSER, SYSID, TABLES, TEMPLATE,
        TYPES, UNCONDITIONAL, UNENCRYPTED, UNTIL, VALIDATE,
    };
}

pub mod punct {
    pub use super::Punctuation;
    pub use super::{
        Amp, AmpAmp, AmpGt, AmpLt, AmpLtPipe, Arrow, ArrowArrow, At, AtAt, AtAtAt, AtGt, AtHashAt,
        AtMinusAt, AtPlusAt, AtQuestion, BackSlash, BangEq, BangEqEq, BangEqMinus, BangTilde,
        BangTildeStar, BangTildeTilde, BangTildeTildeStar, Caret, CaretAt, Colon, ColonColon,
        ColonEquals, Comma, Concat, Dot, Eq, FatArrow, Gt, GtCaret, GtGt, GtGtEq, GtGtGt, Gte,
        HashArrow, HashArrowArrow, HashHash, HashMinus, LBracket, LParen, Lt, LtAt, LtCaret, LtLt,
        LtLtEq, LtLtLt, LtLtPipe, LtMinusGt, Lte, Minus, MinusPipeMinus, Neq, Percent, Pipe,
        PipeAmpGt, PipeGtGt, PipePipeSlash, PipeSlash, Plus, Pound, PsqlBatchSemi,
        PsqlCrosstabview, PsqlG, PsqlGexec, PsqlGset, PsqlGx, Question, QuestionAmp, QuestionDash,
        QuestionDashPipe, QuestionHash, QuestionPipe, QuestionPipePipe, RBracket, RParen, Semi,
        Slash, Star, StarEq, StarGt, StarGte, StarLt, StarLte, StarNeq, Tilde, TildeEq,
        TildeGeqTilde, TildeGtTilde, TildeLeqTilde, TildeLtTilde, TildeStar, TildeTilde,
        TildeTildeStar, TripleEq,
    };
}

// Literals
pub mod literal {
    use super::*;
    use recursa_diagram::railroad;

    // Re-export the literal token types that the `tokens` macro above generated
    // at the parent module's top level. Existing call-sites continue to use
    // `literal::DollarStringLit`, `literal::QuotedIdent`, etc.
    //
    // `DollarStringLit` is now a `tokens!` literal (its `scan_dollar_string`
    // logos callback handles the matched close-tag back-reference at lex
    // time), so it is generated like the rest and re-exported here.
    pub use super::Literal;
    pub use super::{
        BitStringLit, DollarNum, DollarStringLit, EscapeStringLit, HexStringLit, IntegerLit,
        NumericLit, PsqlVar, QuotedIdent, StringLit, UnicodeQuotedIdent, UnicodeStringLit,
    };

    use recursa::{Input, ParseError};
    use recursa_core::Parse;

    /// Recover the borrowed source text of the token under the cursor.
    /// Returns `None` at end-of-input.
    fn current_text<'input>(input: &Input<'input>) -> Option<&'input str> {
        let rec = input.current_record()?;
        Some(&input.source()[rec.start as usize..rec.end as usize])
    }

    /// Catch-all for Postgres user-defined operator names.
    ///
    /// Matches any sequence of the characters `+ - * / < > = ~ ! @ # % ^ & | ?`.
    /// In expression contexts this is the LAST-RESORT infix/prefix in the Pratt
    /// parser — known punct tokens are tried first. In DDL contexts (CREATE/ALTER/
    /// DROP OPERATOR) this is the primary scanner for the operator name.
    ///
    /// `CustomOp`'s lexical kind (`TokenKind::CustomOp`) is produced by the
    /// `tokens!` `lexer_tokens` block. The `Parse` impl is hand-written — a
    /// recursa gap: a `lexer_tokens` entry deliberately has no generated
    /// `Parse`, and a plain `tokens!` `literals` entry would generate the
    /// kind-check impl below, but `CustomOp` shares the `lexer_tokens` block
    /// with `UnquotedIdent` (whose `Parse` genuinely cannot be generated).
    #[derive(Visit, Transform, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
    #[visit(terminal)]
    #[transform(terminal)]
    #[railroad(label = "<operator>")]
    pub struct CustomOp<'input>(pub ::std::borrow::Cow<'input, str>);

    impl<'input> Parse<'input> for CustomOp<'input> {
        type Prefix = ();

        fn meta() -> &'static recursa_core::Meta {
            static META: recursa_core::Meta = recursa_core::Meta {
                name: "CustomOp",
                tags: &[],
            };
            &META
        }

        fn peek(input: &mut Input<'input>) -> bool {
            input.peek_kind(0) == Some(super::TokenKind::CustomOp as u16)
        }

        fn parse(input: &mut Input<'input>) -> Result<Self, ParseError> {
            if input.peek_kind(0) == Some(super::TokenKind::CustomOp as u16) {
                let text = current_text(input).expect("current_record present");
                input.advance();
                Ok(CustomOp(::std::borrow::Cow::Borrowed(text)))
            } else {
                Err(ParseError::new(
                    input.source_arc(),
                    input.byte_offset()..input.byte_offset(),
                    "CustomOp",
                ))
            }
        }
    }

    impl<'input> recursa::FormatTokens for CustomOp<'input> {
        fn format_tokens(&self, tokens: &mut Vec<recursa::fmt::Token>) {
            tokens.push(recursa::fmt::Token::String(self.0.as_ref().to_string()));
        }
    }

    // --- Identifier ---

    /// ALL SQL keywords (uppercase) for identifier exclusion.
    ///
    /// This is the set the grammar treats as reserved for the purpose of
    /// rejecting bare identifiers. Postgres has a much smaller reserved set
    /// than listed here historically — many words that the grammar matches as
    /// `keyword::X` in specific positions are still usable as identifiers in
    /// other positions. Those words should NOT appear here.
    ///
    /// In the logos token model, identifier/keyword disambiguation is done by
    /// `TokenKind` discriminant (`unquoted_ident_kind_ok`), so this set is no
    /// longer consulted by `Parse` — it is retained only for the
    /// `arbitrary`-feature generator (`arb_non_keyword_ident`).
    #[cfg_attr(not(feature = "arbitrary"), allow(dead_code))]
    const SQL_KEYWORDS: &[&str] = &[
        // Core reserved: expression, query, clause keywords.
        "SELECT",
        "FROM",
        "WHERE",
        "AS",
        "AND",
        "OR",
        "NOT",
        "TRUE",
        "FALSE",
        "NULL",
        "IS",
        "UNKNOWN",
        // STATEMENT leads.
        "CREATE",
        "TABLE",
        "INSERT",
        "INTO",
        "VALUES",
        "DROP",
        "DELETE",
        "UPDATE",
        "MERGE",
        "ALTER",
        // Ordering / limit.
        "ORDER",
        "BY",
        // PRIMARY and KEY are contextual: they appear in `PRIMARY KEY`
        // constraint positions but PostgreSQL allows them as ordinary column
        // and identifier names elsewhere (e.g., `CREATE INDEX i ON t(key)`).
        // Recognized as `keyword::PRIMARY` / `keyword::KEY` only where the
        // grammar explicitly looks for them.
        "ASC",
        "DESC",
        "NULLS",
        "UNIQUE",
        "USING",
        "OFFSET",
        "LIMIT",
        // Predicates / set ops.
        "LIKE",
        "ILIKE",
        "IN",
        "BETWEEN",
        "EXISTS",
        "WHEN",
        "THEN",
        "ELSE",
        "END",
        "CASE",
        "UNION",
        "INTERSECT",
        "EXCEPT",
        "DISTINCT",
        "ALL",
        "WITH",
        "RECURSIVE",
        "GROUP",
        "HAVING",
        "RETURNING",
        "IF",
        // Joins.
        "JOIN",
        "LEFT",
        "RIGHT",
        "FULL",
        "INNER",
        "CROSS",
        "ON",
        "OUTER",
        "NATURAL",
        // DDL structure.
        "PARTITION",
        "OF",
        "FOR",
        "INHERITS",
        "REFERENCES",
        "FOREIGN",
        // Grammar clauses that appear after identifier positions and must
        // be reserved to prevent being consumed as column/alias names.
        "SET",
        "WINDOW",
        "TABLESAMPLE",
        // NOTE: ROWS, RANGE, GROUPS are intentionally NOT listed here:
        // Postgres treats them as unreserved, so they are valid table /
        // column / alias names (`FROM rows`, `SELECT range FROM ...`).
        // Window frame parsing guards against them specifically via a
        // dedicated `WindowRefNameIdent` postcondition — see
        // `expr::InlineWindowSpec::ref_name`.
    ];

    /// `true` if `s` matches a SQL_KEYWORDS entry case-insensitively.
    ///
    /// Used only by the `arbitrary`-feature identifier generator now that
    /// `Parse` does keyword disambiguation by `TokenKind`. Uses a static
    /// `HashSet<&'static str>` (built once, ASCII-uppercase keys) plus an
    /// ASCII-uppercase stack buffer for short identifiers so the common case
    /// has no heap allocation.
    #[cfg_attr(not(feature = "arbitrary"), allow(dead_code))]
    fn is_keyword(s: &str) -> bool {
        use std::collections::HashSet;
        static SET: std::sync::OnceLock<HashSet<&'static str>> = std::sync::OnceLock::new();
        let set = SET.get_or_init(|| SQL_KEYWORDS.iter().copied().collect());

        // SQL_KEYWORDS keys are already uppercase. ASCII-uppercase the
        // input into a stack buffer to avoid allocation for normal-size
        // identifiers (the vast majority in practice).
        const STACK_BUF: usize = 64;
        let bytes = s.as_bytes();
        if bytes.len() <= STACK_BUF {
            let mut buf = [0u8; STACK_BUF];
            for (i, &b) in bytes.iter().enumerate() {
                buf[i] = b.to_ascii_uppercase();
            }
            // SAFETY: ASCII-uppercase preserves UTF-8 validity for ASCII
            // bytes; non-ASCII bytes pass through unchanged.
            let upper = unsafe { std::str::from_utf8_unchecked(&buf[..bytes.len()]) };
            set.contains(upper)
        } else {
            // Long identifier (rare). Fall back to allocation.
            set.contains(s.to_ascii_uppercase().as_str())
        }
    }

    /// `true` if `kind` is a token kind that `UnquotedIdent` accepts: its own
    /// `UnquotedIdent` lexical kind, or any *soft* (non-reserved) keyword.
    ///
    /// In the logos token model every lexed word produces exactly one
    /// `TokenKind`: a reserved keyword (`SELECT`), a soft keyword (`FORMAT`),
    /// or the catch-all `UnquotedIdent`. Soft keywords are reclaimable as
    /// ordinary identifiers wherever the grammar does not specifically ask
    /// for the keyword — preserving the old classifier-path behaviour exactly.
    /// Reserved keywords and all non-word kinds are rejected.
    fn unquoted_ident_kind_ok(kind: u16) -> bool {
        kind == super::TokenKind::UnquotedIdent as u16 || token_kind_is_soft(kind)
    }

    /// Unquoted SQL identifier: a lexed word that is not a reserved keyword.
    #[derive(Visit, Transform, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[visit(terminal)]
    #[transform(terminal)]
    #[railroad(label = "<Unquoted Identifier>")]
    pub struct UnquotedIdent<'input>(pub ::std::borrow::Cow<'input, str>);

    // Hand-written `Parse` impl — a genuine recursa gap. `UnquotedIdent`'s
    // lexical kind comes from a `tokens!` `lexer_tokens` entry, but its
    // `Parse` impl cannot be the plain kind-check a `literals` entry would
    // generate: it must ALSO accept every soft-keyword `TokenKind` (soft
    // keywords are reclaimable as identifiers) while rejecting reserved
    // keywords. The framework has no way to express "accept this kind plus
    // a predicate over other kinds", so the impl is written by hand. Filed
    // as a recursa limitation: a `literals`/`lexer_tokens` mode that accepts
    // a set of kinds, or a kind-predicate hook.
    impl<'input> Parse<'input> for UnquotedIdent<'input> {
        type Prefix = ();

        fn meta() -> &'static recursa_core::Meta {
            static META: recursa_core::Meta = recursa_core::Meta {
                name: "unquoted_ident",
                tags: &[],
            };
            &META
        }

        fn peek(input: &mut Input<'input>) -> bool {
            input.peek_kind(0).is_some_and(unquoted_ident_kind_ok)
        }

        fn parse(input: &mut Input<'input>) -> Result<Self, ParseError> {
            if input.peek_kind(0).is_some_and(unquoted_ident_kind_ok) {
                let text = current_text(input).expect("current_record present");
                input.advance();
                Ok(UnquotedIdent(::std::borrow::Cow::Borrowed(text)))
            } else {
                Err(ParseError::new(
                    input.source_arc(),
                    input.byte_offset()..input.byte_offset(),
                    "UnquotedIdent",
                ))
            }
        }
    }

    impl<'input> recursa::FormatTokens for UnquotedIdent<'input> {
        fn format_tokens(&self, tokens: &mut Vec<recursa::fmt::Token>) {
            tokens.push(recursa::fmt::Token::String(self.0.as_ref().to_string()));
        }
    }

    /// SQL identifier: unicode-quoted (`U&"Foo"`), double-quoted (`"Foo"`),
    /// or unquoted (`foo`).
    ///
    /// Variant ordering: `UnicodeQuoted` (`U&"`) first as the longest prefix,
    /// then `Quoted` (`"`), then `Unquoted` (letter).
    // No `postcondition` on Ident itself — `UnquotedIdent` already
    // enforces `not_keyword` via its own postcondition, and Quoted/
    // UnicodeQuoted variants are allowed to contain keywords (they're
    // explicit-quoted). Running the postcondition here only added a
    // second round of the keyword scan per Ident parse (profile pre-fix
    // showed Ident::parse at 40% of total parse time on numeric_big).
    #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
    #[derive(Visit, Transform, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[recursa::parser]
    #[visit(terminal)]
    #[transform(terminal)]
    #[railroad(label = "<Identifier>")]
    pub enum Ident<'input> {
        #[railroad(label = "<Unicode Quoted>")]
        UnicodeQuoted(UnicodeQuotedIdent<'input>),
        #[railroad(label = "<Quoted>")]
        Quoted(literal::QuotedIdent<'input>),
        #[railroad(label = "<Unquoted>")]
        Unquoted(UnquotedIdent<'input>),
    }

    impl<'input> recursa::FormatTokens for Ident<'input> {
        fn format_tokens(&self, tokens: &mut Vec<recursa::fmt::Token>) {
            match self {
                Ident::UnicodeQuoted(u) => u.format_tokens(tokens),
                Ident::Quoted(quoted) => quoted.format_tokens(tokens),
                Ident::Unquoted(unquoted) => unquoted.format_tokens(tokens),
            }
        }
    }

    impl<'input> Ident<'input> {
        /// The raw text of the identifier.
        pub fn text(&self) -> &str {
            match self {
                Ident::UnicodeQuoted(u) => &u.0,
                Ident::Quoted(q) => &q.0,
                Ident::Unquoted(u) => &u.0,
            }
        }
    }

    /// Frame-unit keywords that must be rejected when parsing a window
    /// `ref_name` so they are not silently consumed as an identifier and
    /// leave the frame clause stranded.
    const WINDOW_FRAME_UNIT_KEYWORDS: &[&str] = &["ROWS", "RANGE", "GROUPS"];

    fn is_frame_unit(s: &str) -> bool {
        WINDOW_FRAME_UNIT_KEYWORDS
            .iter()
            .any(|kw| kw.eq_ignore_ascii_case(s))
    }

    /// Postcondition: reject idents that are window frame-unit keywords.
    fn not_frame_unit<'input>(ident: &Ident<'input>) -> Result<(), ParseError> {
        if let Ident::Unquoted(u) = ident
            && is_frame_unit(&u.0)
        {
            return Err(ParseError::new(
                u.0.to_string(),
                0..u.0.len(),
                "identifier (not ROWS/RANGE/GROUPS)",
            ));
        }
        Ok(())
    }

    /// Identifier usable as a window `ref_name` (existing-window reference).
    /// Rejects `ROWS`/`RANGE`/`GROUPS` so the window frame clause after the
    /// optional `ref_name` parses correctly.
    ///
    /// Modeled as a single-variant enum so the derive macro threads the
    /// postcondition through peek+parse (tuple structs don't currently
    /// support `#[parse(postcondition = ...)]`).
    #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
    #[derive(Visit, Transform, FormatTokens, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[recursa::parser(postcondition = crate::tokens::literal::not_frame_unit_wrapper)]
    #[visit(terminal)]
    #[transform(terminal)]
    #[railroad(label = "<Window Ref Name>")]
    pub enum WindowRefNameIdent<'input> {
        Ident(Ident<'input>),
    }

    pub(crate) fn not_frame_unit_wrapper<'input>(
        wrapper: &WindowRefNameIdent<'input>,
    ) -> Result<(), ParseError> {
        let WindowRefNameIdent::Ident(id) = wrapper;
        not_frame_unit(id)
    }

    // --- Alias name (any SQL word — identifier or keyword) ---

    /// Bare-word alias name: any SQL word including keywords (`SELECT 1 AS true`).
    #[derive(Visit, Transform, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[visit(terminal)]
    #[transform(terminal)]
    #[railroad(label = "<Bare Alias Name>")]
    pub struct BareAliasName<'input>(pub ::std::borrow::Cow<'input, str>);

    // Hand-written `Parse` impl — a genuine recursa gap, same as
    // `UnquotedIdent`. `BareAliasName` matched the bare-word regex
    // `[a-zA-Z_][a-zA-Z0-9_]*` regardless of keyword status, so in the token
    // model it accepts ANY lexed *word* kind — `UnquotedIdent`, every
    // reserved keyword, and every soft keyword — and rejects punctuation,
    // operators, and string/numeric/dollar literals. "Word" is decided by
    // the token's first source byte being `[A-Za-z_]`: keyword and
    // `UnquotedIdent` text always starts there; `QuotedIdent` (`"`),
    // `DollarStringLit` (`$`), `PsqlVar` (`:`), numbers and operators do not.
    impl<'input> Parse<'input> for BareAliasName<'input> {
        type Prefix = ();

        fn meta() -> &'static recursa_core::Meta {
            static META: recursa_core::Meta = recursa_core::Meta {
                name: "bare_alias_name",
                tags: &[],
            };
            &META
        }

        fn peek(input: &mut Input<'input>) -> bool {
            current_text(input).is_some_and(starts_word)
        }

        fn parse(input: &mut Input<'input>) -> Result<Self, ParseError> {
            match current_text(input) {
                Some(text) if starts_word(text) => {
                    input.advance();
                    Ok(BareAliasName(::std::borrow::Cow::Borrowed(text)))
                }
                _ => Err(ParseError::new(
                    input.source_arc(),
                    input.byte_offset()..input.byte_offset(),
                    "BareAliasName",
                )),
            }
        }
    }

    /// `true` if `text`'s first byte is an SQL word-start char (`[A-Za-z_]`).
    fn starts_word(text: &str) -> bool {
        matches!(text.as_bytes().first(), Some(b) if b.is_ascii_alphabetic() || *b == b'_')
    }

    impl<'input> recursa::FormatTokens for BareAliasName<'input> {
        fn format_tokens(&self, tokens: &mut Vec<recursa::fmt::Token>) {
            tokens.push(recursa::fmt::Token::String(self.0.as_ref().to_string()));
        }
    }

    /// Alias name: unicode-quoted (`U&"..."`), double-quoted (`"Foo"`),
    /// or bare word (including keywords) (`true`, `myalias`).
    ///
    /// Variant ordering: `UnicodeQuoted` (`U&"`) first (longest prefix),
    /// then `Quoted` (`"`), then `Bare` (letter).
    #[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
    #[derive(Visit, Transform, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[recursa::parser]
    #[visit(terminal)]
    #[transform(terminal)]
    #[railroad(label = "<Alias Name>")]
    pub enum AliasName<'input> {
        #[railroad(label = "<Unicode Quoted>")]
        UnicodeQuoted(UnicodeQuotedIdent<'input>),
        #[railroad(label = "<Quoted>")]
        Quoted(QuotedIdent<'input>),
        #[railroad(label = "<Bare>")]
        Bare(BareAliasName<'input>),
    }

    impl<'input> AliasName<'input> {
        /// Raw text of the alias name (with quotes if quoted).
        pub fn text(&self) -> &str {
            match self {
                AliasName::UnicodeQuoted(u) => &u.0,
                AliasName::Quoted(q) => &q.0,
                AliasName::Bare(b) => &b.0,
            }
        }
    }

    impl<'input> recursa::FormatTokens for AliasName<'input> {
        fn format_tokens(&self, tokens: &mut Vec<recursa::fmt::Token>) {
            match self {
                AliasName::UnicodeQuoted(u) => u.format_tokens(tokens),
                AliasName::Quoted(q) => q.format_tokens(tokens),
                AliasName::Bare(b) => b.format_tokens(tokens),
            }
        }
    }

    // --- Rest of line ---

    /// Matches the remainder of text on the current line (up to newline or end of input).
    #[derive(Visit, Transform, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    #[visit(terminal)]
    #[transform(terminal)]
    pub struct RestOfLine<'input>(pub ::std::borrow::Cow<'input, str>);

    // Hand-written `Parse` impl — a genuine recursa gap. `RestOfLine` matches
    // raw source up to the next newline, content that is not lexable SQL
    // (psql `\directive` argument text). In the logos token model there is no
    // "rest of line" token kind: this impl recovers the raw slice from the
    // current token's byte offset to the next `\n` in `source`, then advances
    // the token cursor past every token whose span lies within that line.
    // Filed as a recursa limitation: raw-source-spanning tokens have no
    // first-class model in the token-array design.
    impl<'input> Parse<'input> for RestOfLine<'input> {
        type Prefix = ();

        fn meta() -> &'static recursa_core::Meta {
            static META: recursa_core::Meta = recursa_core::Meta {
                name: "rest_of_line",
                tags: &[],
            };
            &META
        }

        fn peek(_input: &mut Input<'input>) -> bool {
            // `RestOfLine` matches `[^\n]*` — it always succeeds (possibly
            // matching the empty string), so peek is unconditionally true.
            true
        }

        fn parse(input: &mut Input<'input>) -> Result<Self, ParseError> {
            let source = input.source();
            let start = input.byte_offset();
            // The line runs from the current byte offset to the next newline.
            let rel_end = source[start..].find('\n').unwrap_or(source.len() - start);
            let line_end = start + rel_end;
            let text = &source[start..line_end];
            // Advance the token cursor past every token starting before the
            // newline — those tokens belong to this line's raw text.
            while input
                .current_record()
                .is_some_and(|r| (r.start as usize) < line_end)
            {
                input.advance();
            }
            Ok(RestOfLine(::std::borrow::Cow::Borrowed(text)))
        }
    }

    impl<'input> recursa::FormatTokens for RestOfLine<'input> {
        fn format_tokens(&self, tokens: &mut Vec<recursa::fmt::Token>) {
            tokens.push(recursa::fmt::Token::String(self.0.as_ref().to_string()));
        }
    }

    // -- Manual Arbitrary impls for literal types --
    //
    // Literal types hold `Cow<str>` whose content must match the parse
    // regex (including delimiters). A blind derive would generate random
    // bytes. These impls produce syntactically valid SQL literals.

    #[cfg(feature = "arbitrary")]
    mod arbitrary_impls {
        use super::*;
        use arbitrary::{Arbitrary, Unstructured};
        use std::borrow::Cow;

        const IDENT_FIRST: &[u8] = b"abcdefghijklmnopqrstuvwxyz_";
        const IDENT_REST: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
        const SAFE_CHARS: &[u8] = b"abc123 ";

        fn arb_ident_str(u: &mut Unstructured<'_>) -> arbitrary::Result<String> {
            let first = *u.choose(IDENT_FIRST)? as char;
            let rest_len: usize = u.int_in_range(0..=8)?;
            let mut s = String::with_capacity(1 + rest_len);
            s.push(first);
            for _ in 0..rest_len {
                s.push(*u.choose(IDENT_REST)? as char);
            }
            Ok(s)
        }

        fn arb_non_keyword_ident(u: &mut Unstructured<'_>) -> arbitrary::Result<String> {
            for _ in 0..100 {
                let s = arb_ident_str(u)?;
                if !super::is_keyword(&s) {
                    return Ok(s);
                }
            }
            Ok("_arb_ident".to_string())
        }

        fn arb_safe_body(u: &mut Unstructured<'_>, max: usize) -> arbitrary::Result<String> {
            let len: usize = u.int_in_range(0..=max)?;
            let mut s = String::with_capacity(len);
            for _ in 0..len {
                s.push(*u.choose(SAFE_CHARS)? as char);
            }
            Ok(s)
        }

        impl<'a> Arbitrary<'a> for UnquotedIdent<'_> {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                let s = arb_non_keyword_ident(u)?;
                Ok(Self(Cow::Owned(s)))
            }
        }

        impl<'a> Arbitrary<'a> for BareAliasName<'_> {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                let s = arb_ident_str(u)?;
                Ok(Self(Cow::Owned(s)))
            }
        }

        impl<'a> Arbitrary<'a> for RestOfLine<'_> {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                let body = arb_safe_body(u, 20)?;
                Ok(Self(Cow::Owned(body)))
            }
        }

        impl<'a> Arbitrary<'a> for DollarStringLit<'_> {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                // Tag: empty (yielding `$$...$$`) or a short ident-like word.
                // The body must not contain the close sequence, so use the
                // safe-char alphabet (no `$`).
                let tag_len: usize = u.int_in_range(0..=4)?;
                let mut tag = String::with_capacity(tag_len);
                for _ in 0..tag_len {
                    tag.push(*u.choose(IDENT_REST)? as char);
                }
                let body = arb_safe_body(u, 20)?;
                let s = format!("${tag}${body}${tag}$");
                Ok(Self(Cow::Owned(s)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::keyword::*;
    use super::literal::*;
    use super::punct::*;

    // --- Keyword tests ---

    #[test]
    fn keyword_select_uppercase() {
        let mut input = crate::tokens::test_input("SELECT");
        assert!(SELECT::peek(&mut input));
    }

    #[test]
    fn keyword_select_lowercase() {
        let mut input = crate::tokens::test_input("select");
        assert!(SELECT::peek(&mut input));
    }

    #[test]
    fn keyword_select_mixed_case() {
        let mut input = crate::tokens::test_input("SeLeCt");
        assert!(SELECT::peek(&mut input));
    }

    #[test]
    fn keyword_select_not_prefix_of_identifier() {
        let mut input = crate::tokens::test_input("SELECTED");
        assert!(!SELECT::peek(&mut input));
    }

    #[test]
    fn keyword_bool_not_prefix_of_booleq() {
        let mut input = crate::tokens::test_input("booleq");
        assert!(!BOOL::peek(&mut input));
    }

    #[test]
    fn keyword_bool_matches_standalone() {
        let mut input = crate::tokens::test_input("bool");
        assert!(BOOL::peek(&mut input));
    }

    #[test]
    fn keyword_boolean_matches() {
        let mut input = crate::tokens::test_input("BOOLEAN");
        assert!(BOOLEAN::peek(&mut input));
    }

    #[test]
    fn keyword_not_matches() {
        let mut input = crate::tokens::test_input("NOT");
        assert!(NOT::peek(&mut input));
    }

    // --- Punctuation tests ---

    #[test]
    fn punctuation_semicolon() {
        let mut input = crate::tokens::test_input(";");
        let _ = Semi::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn punctuation_neq() {
        let mut input = crate::tokens::test_input("<>");
        assert!(Neq::peek(&mut input));
    }

    #[test]
    fn punctuation_colon_colon() {
        let mut input = crate::tokens::test_input("::");
        assert!(ColonColon::peek(&mut input));
    }

    #[test]
    fn punctuation_lte() {
        let mut input = crate::tokens::test_input("<=");
        assert!(Lte::peek(&mut input));
    }

    #[test]
    fn punctuation_gte() {
        let mut input = crate::tokens::test_input(">=");
        assert!(Gte::peek(&mut input));
    }

    // --- Custom/locale operator punct tests ---

    #[test]
    fn punctuation_tilde_leq_tilde() {
        let mut input = crate::tokens::test_input("~<=~");
        assert!(TildeLeqTilde::peek(&mut input));
    }

    #[test]
    fn punctuation_tilde_geq_tilde() {
        let mut input = crate::tokens::test_input("~>=~");
        assert!(TildeGeqTilde::peek(&mut input));
    }

    #[test]
    fn punctuation_tilde_lt_tilde() {
        let mut input = crate::tokens::test_input("~<~");
        assert!(TildeLtTilde::peek(&mut input));
    }

    #[test]
    fn punctuation_tilde_gt_tilde() {
        let mut input = crate::tokens::test_input("~>~");
        assert!(TildeGtTilde::peek(&mut input));
    }

    // --- LIKE/ILIKE family operator tests ---
    //
    // Postgres uses `~~`/`!~~`/`~~*`/`!~~*` as the implementation operators
    // for LIKE/NOT LIKE/ILIKE/NOT ILIKE. They must be distinct tokens so the
    // formatter does not emit them as two adjacent `~` characters with a
    // space between (which would produce `~ ~`, a different operator
    // sequence to PG).

    #[test]
    fn punctuation_tilde_tilde() {
        let mut input = crate::tokens::test_input("~~");
        assert!(TildeTilde::peek(&mut input));
    }

    #[test]
    fn punctuation_bang_tilde_tilde() {
        let mut input = crate::tokens::test_input("!~~");
        assert!(BangTildeTilde::peek(&mut input));
    }

    #[test]
    fn punctuation_tilde_tilde_star() {
        let mut input = crate::tokens::test_input("~~*");
        assert!(TildeTildeStar::peek(&mut input));
    }

    #[test]
    fn punctuation_bang_tilde_tilde_star() {
        let mut input = crate::tokens::test_input("!~~*");
        assert!(BangTildeTildeStar::peek(&mut input));
    }

    // Disambiguation tests — the longer LIKE/ILIKE operators must win over
    // their shorter prefixes (`~`, `!~`, `~*`, `!~*`).

    #[test]
    fn tilde_tilde_wins_over_tilde() {
        // `~~` should not be consumed as two `~` tokens.
        let mut input = crate::tokens::test_input("~~");
        let _tok = TildeTilde::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn tilde_tilde_star_wins_over_tilde_star() {
        // `~~*` should not be consumed as `~` + `~*`.
        let mut input = crate::tokens::test_input("~~*");
        let _tok = TildeTildeStar::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn bang_tilde_tilde_wins_over_bang_tilde() {
        // `!~~` should not be consumed as `!~` + `~`.
        let mut input = crate::tokens::test_input("!~~");
        let _tok = BangTildeTilde::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn bang_tilde_tilde_star_wins_over_bang_tilde_star() {
        // `!~~*` should not be consumed as `!~` + `~*` or `!~~` + `*`.
        let mut input = crate::tokens::test_input("!~~*");
        let _tok = BangTildeTildeStar::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn punctuation_triple_eq() {
        let mut input = crate::tokens::test_input("===");
        assert!(TripleEq::peek(&mut input));
    }

    #[test]
    fn punctuation_bang_eq_eq() {
        let mut input = crate::tokens::test_input("!==");
        assert!(BangEqEq::peek(&mut input));
    }

    #[test]
    fn punctuation_hash_hash() {
        let mut input = crate::tokens::test_input("##");
        assert!(HashHash::peek(&mut input));
    }

    #[test]
    fn punctuation_at_minus_at() {
        let mut input = crate::tokens::test_input("@-@");
        assert!(AtMinusAt::peek(&mut input));
    }

    #[test]
    fn punctuation_at_hash_at() {
        let mut input = crate::tokens::test_input("@#@");
        assert!(AtHashAt::peek(&mut input));
    }

    #[test]
    fn punctuation_at_plus_at() {
        let mut input = crate::tokens::test_input("@+@");
        assert!(AtPlusAt::peek(&mut input));
    }

    #[test]
    fn punctuation_bang_eq_minus() {
        let mut input = crate::tokens::test_input("!=-");
        assert!(BangEqMinus::peek(&mut input));
    }

    // Disambiguation: longer forms must win over shorter prefixes.

    #[test]
    fn tilde_leq_tilde_wins_over_tilde() {
        // ~<=~ should not be consumed as ~ then <=~
        let mut input = crate::tokens::test_input("~<=~");
        let _tok = TildeLeqTilde::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn triple_eq_wins_over_eq() {
        let mut input = crate::tokens::test_input("===");
        let _tok = TripleEq::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn bang_eq_eq_wins_over_bang_eq() {
        let mut input = crate::tokens::test_input("!==");
        let _tok = BangEqEq::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn hash_hash_wins_over_pound() {
        let mut input = crate::tokens::test_input("##");
        let _tok = HashHash::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn at_minus_at_wins_over_at() {
        let mut input = crate::tokens::test_input("@-@");
        let _tok = AtMinusAt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    // --- String literal tests ---

    #[test]
    fn string_literal_simple() {
        let mut input = crate::tokens::test_input("'hello world'");
        let lit = StringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "'hello world'");
        assert!(input.is_empty());
    }

    #[test]
    fn string_literal_with_escaped_quote() {
        let mut input = crate::tokens::test_input("'it''s'");
        let lit = StringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "'it''s'");
    }

    #[test]
    fn string_literal_empty() {
        let mut input = crate::tokens::test_input("''");
        let lit = StringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "''");
    }

    #[test]
    fn string_literal_with_spaces() {
        let mut input = crate::tokens::test_input("'   f           '");
        let lit = StringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "'   f           '");
    }

    // --- INTEGER literal tests ---

    #[test]
    fn integer_literal() {
        let mut input = crate::tokens::test_input("42");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "42");
    }

    #[test]
    fn integer_literal_zero() {
        let mut input = crate::tokens::test_input("0");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "0");
    }

    // --- NUMERIC literal tests (decimals + exponent) ---

    #[test]
    fn numeric_literal_simple_decimal() {
        let mut input = crate::tokens::test_input("4.5");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "4.5");
    }

    #[test]
    fn numeric_literal_leading_dot() {
        let mut input = crate::tokens::test_input(".5");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, ".5");
    }

    #[test]
    fn numeric_literal_exponent_int() {
        let mut input = crate::tokens::test_input("2e3");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "2e3");
    }

    #[test]
    fn numeric_literal_decimal_with_exponent() {
        let mut input = crate::tokens::test_input("4.5e10");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "4.5e10");
    }

    #[test]
    fn numeric_literal_negative_exponent() {
        let mut input = crate::tokens::test_input("1.5e-5");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "1.5e-5");
    }

    #[test]
    fn numeric_literal_large_exponent() {
        let mut input = crate::tokens::test_input("4.4e131071");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "4.4e131071");
    }

    #[test]
    fn integer_literal_with_underscores() {
        let mut input = crate::tokens::test_input("100_000_000_000_000");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "100_000_000_000_000");
    }

    // Postgres 16+ accepts non-decimal integer literal forms: `0x42F`, `0b101`,
    // `0o273` (plus uppercase prefixes and `_` digit separators). Without these,
    // `0x42F` lexes as `IntegerLit("0")` + `Ident("x42F")` — the bug this widening
    // closes.
    #[test]
    fn integer_literal_hex_lowercase_prefix() {
        let mut input = crate::tokens::test_input("0x42F");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "0x42F");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn integer_literal_hex_uppercase_prefix() {
        let mut input = crate::tokens::test_input("0X1A2b");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "0X1A2b");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn integer_literal_hex_with_underscores() {
        let mut input = crate::tokens::test_input("0xFF_FF");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "0xFF_FF");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn integer_literal_octal_prefix() {
        let mut input = crate::tokens::test_input("0o273");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "0o273");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn integer_literal_binary_prefix() {
        let mut input = crate::tokens::test_input("0b101");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "0b101");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    /// PG accepts `_` immediately after the radix prefix (`0b_…`, `0x_…`,
    /// `0o_…`) — gram.y `bininteger 0[bB](_?{bindigit})+`.
    #[test]
    fn integer_literal_radix_prefix_leading_underscore() {
        for src in ["0b_10_0101", "0x_FF", "0o_7"] {
            let mut input = crate::tokens::test_input(src);
            let lit =
                IntegerLit::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}"));
            assert_eq!(lit.0, src);
            assert!(
                input.is_empty(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    // ── lexer-arbitration test helper ─────────────────────────────────────
    //
    // The old classifier (`classify(src, 0) -> SlotState`) is gone; the logos
    // `lex` pass replaces it. These helpers expose the lexer's first-token
    // decision so the cross-token arbitration tests below — longest-match,
    // trailing-junk rejection, single-token classification — still verify the
    // same behaviour against the new model.

    /// The first lexed token of `src`, or `None` if `src` lexes to nothing.
    fn lex_first(src: &str) -> Option<recursa::TokenRecord> {
        super::lex(src).tokens.first().copied()
    }

    /// `true` if `src`'s first token has kind `kind` and spans exactly
    /// `expected_len` bytes — the new analogue of the old
    /// `SlotState::Token(slot)` + `slot.kind` / `slot.token_end` check.
    fn first_token_is(src: &str, kind: super::TokenKind, expected_len: usize) -> bool {
        matches!(
            lex_first(src),
            Some(rec) if rec.kind == kind as u16 && rec.end as usize == expected_len
        )
    }

    // Longest-match-wins must keep `NumericLit` ahead of `IntegerLit` whenever
    // a `.` or exponent is present. These tests route through `lex` so they
    // exercise cross-token arbitration — if a future change let `IntegerLit`'s
    // regex match `0.5`, the lexer (not `NumericLit::parse` in isolation) is
    // where the wrong choice would surface.
    #[test]
    fn numeric_literal_still_wins_over_integer_with_decimal() {
        assert!(first_token_is("0.5", super::TokenKind::NumericLit, 3));
    }

    #[test]
    fn numeric_literal_still_wins_over_integer_with_exponent() {
        assert!(first_token_is("1e10", super::TokenKind::NumericLit, 4));
    }

    #[test]
    fn numeric_literal_with_underscores() {
        let mut input = crate::tokens::test_input("1_234.567_89");
        let lit = NumericLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "1_234.567_89");
    }

    // The trailing-dot form (`1.` — empty fraction, no exponent) is a valid
    // PostgreSQL numeric literal when followed by a non-word char or EOF.
    // It must classify as a `NumericLit` spanning the whole `1.`.
    #[test]
    fn numeric_literal_trailing_dot_valid() {
        for src in ["1.", "1. ", "1.;", "1_000."] {
            let dot_end = src.find('.').unwrap() + 1;
            assert!(
                first_token_is(src, super::TokenKind::NumericLit, dot_end),
                "{src:?} should lex as NumericLit spanning up to the dot"
            );
        }
    }

    // A numeric literal immediately followed by an identifier-start char is a
    // PostgreSQL lex error ("trailing junk after numeric literal"). The
    // trailing-dot form must NOT match across the dot for `0.a` / `1_000._5` —
    // the classifier must not return a `NumericLit` spanning the dot.
    #[test]
    fn numeric_literal_trailing_dot_junk_rejected() {
        for src in ["0.a", "1_000._5"] {
            // The `reject_trailing_word` callback makes the trailing-dot
            // numeric form reject when a word char follows. `0` alone lexing
            // as IntegerLit is fine — the `.a` / `._5` trailing junk is what
            // PostgreSQL rejects.
            if let Some(rec) = lex_first(src) {
                assert_ne!(
                    rec.kind,
                    super::TokenKind::NumericLit as u16,
                    "{src:?} must not lex as NumericLit spanning the dot"
                );
            }
        }
    }

    #[test]
    fn integer_literal_does_not_match_decimal() {
        // Bare integer still works
        let mut input = crate::tokens::test_input("42");
        let lit = IntegerLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "42");
    }

    // Cross-token arbitration: with the classifier installed, the non-decimal
    // `0x…` / `0o…` / `0b…` forms (and their underscore-separated variants)
    // must classify as `IntegerLit` — NOT as `IntegerLit("0")` followed by an
    // `Ident("xNNN")`. This is the umbrella analogue of the bit/hex-string
    // arbitration test above; the per-token `IntegerLit::parse` tests verify
    // the regex in isolation, this test verifies the cross-regex arbitration
    // that is the actual code path the bug lived on.
    #[test]
    fn integer_literals_classify_as_single_token() {
        for src in ["0x42F", "0b101", "0o273", "0xFF_FF", "0X1A2b"] {
            assert!(
                first_token_is(src, super::TokenKind::IntegerLit, src.len()),
                "{src:?} must lex as a single IntegerLit token"
            );
        }
    }

    // Trailing-junk rejection: PG's flex lexer rejects a numeric literal that
    // is immediately butted up against an identifier character or a trailing
    // underscore (no whitespace separator). Examples PG rejects with
    // "trailing junk after numeric literal" or "invalid …integer at or near
    // …": `123abc`, `0xFFG`, `0b101z`, `0o7ab`, `100_`, `0xff_`, `1.5xyz`,
    // `1e10abc`, `.5xyz`, `1_000._5`.
    //
    // The fix anchors `IntegerLit` and `NumericLit` with `\b` at the end. The
    // regex crate's `\b` is a transition between a word character (`[A-Za-z_
    // 0-9]`) and a non-word character (or input boundary). Effect:
    //   - For literals ending in a digit (the common case), `\b` requires the
    //     following character to be a non-word character — `123abc` no longer
    //     matches `IntegerLit` at all (no `\b` between `3` and `a`).
    //   - For NumericLit forms ending in `.` (e.g. `1_000.`), the regex engine
    //     can backtrack to a shorter prefix when the digit-ending form fails
    //     `\b` (e.g. `1.5xyz` matches `1.`). That partial match is still
    //     enough to derail the parse: the residue (`5xyz` etc.) does not lex
    //     as any token after the `\b` anchor closes off `IntegerLit`, so the
    //     statement falls through to `Raw` and psql emits the PG error.
    //
    // These tests verify the IntegerLit case fully (no token at all) and the
    // NumericLit case as "no token that consumes the entire input" — which is
    // the property that defeats the permissive split. See
    // regress_numerology's trailing-junk fixtures.
    #[test]
    fn integer_lit_does_not_match_when_followed_by_ident_char() {
        for src in ["123abc", "0xFFG", "0b101z", "0o7ab", "100_", "0xff_"] {
            // `reject_trailing_word` rejects the numeric match; the lexer
            // emits a `TOKEN_KIND_NONE` span (or no token), never IntegerLit.
            if let Some(rec) = lex_first(src) {
                assert_ne!(
                    rec.kind,
                    super::TokenKind::IntegerLit as u16,
                    "{src:?} must not lex as IntegerLit",
                );
            }
        }
    }

    #[test]
    fn numeric_lit_does_not_match_when_followed_by_ident_char() {
        // The `\b` anchor eliminates these forms — none of them classify as
        // any NumericLit-bearing token. PG rejects each with "trailing junk
        // after numeric literal" / "invalid …" diagnostics. With our fix the
        // regex doesn't match at all, no token is produced, and the
        // statement falls to Raw so psql emits the canonical PG error.
        for src in ["1e10abc", ".5xyz"] {
            if let Some(rec) = lex_first(src) {
                assert_ne!(
                    rec.kind,
                    super::TokenKind::NumericLit as u16,
                    "{src:?} must not lex as NumericLit",
                );
            }
        }
    }

    #[test]
    fn numeric_lit_with_trailing_ident_does_not_consume_full_input() {
        // Forms where the digit-end alt fails `\b` but the regex engine
        // backtracks to the shorter dot-end alt — e.g. `1.5xyz` matches
        // `1.` instead of the previous `1.5`. The fix doesn't make these
        // forms unparseable at the regex level, but it leaves enough
        // residue (`5xyz`, etc.) that the residue does not lex as a
        // contiguous token (`5` would need `\b` to follow it for the
        // IntegerLit arm to match — but `5xyz` has no boundary between
        // `5` and `x`), so the statement still fails to parse cleanly
        // and falls to Raw. This test pins the "leaves residue" property
        // — i.e., the NumericLit match length is strictly less than the
        // PRE-fix length (which would have consumed everything up to but
        // not including the alphabetic suffix).
        // (src, max_acceptable_token_end)
        for (src, max_len) in [("1.5xyz", 2usize), ("1.5abc", 2usize)] {
            let Some(rec) = lex_first(src) else {
                continue; // no token at all is also acceptable
            };
            if rec.kind == super::TokenKind::NumericLit as u16 {
                assert!(
                    (rec.end as usize) <= max_len,
                    "{src:?}: NumericLit match length {} exceeds expected \
                     max {}. reject_trailing_word must reject the longer \
                     digit-ending match.",
                    rec.end,
                    max_len,
                );
            }
        }
    }

    // Regression guards: legitimate terminators (whitespace, `;`, `,`, EOF)
    // must still let the IntegerLit / NumericLit regex match.
    #[test]
    fn integer_lit_still_matches_with_legitimate_terminators() {
        for (src, expected_len) in [
            ("123", 3),
            ("123 ", 3),
            ("123;", 3),
            ("123,", 3),
            ("0xFF", 4),
            ("0b101", 5),
            ("100", 3),
        ] {
            assert!(
                first_token_is(src, super::TokenKind::IntegerLit, expected_len),
                "{src:?} should lex as IntegerLit of length {expected_len}"
            );
        }
    }

    #[test]
    fn numeric_lit_still_matches_with_legitimate_terminators() {
        for (src, expected_len) in [
            ("1.5", 3),
            ("1.5;", 3),
            ("1.5,", 3),
            ("1e10", 4),
            (".5", 2),
            ("1.5 ", 3),
        ] {
            assert!(
                first_token_is(src, super::TokenKind::NumericLit, expected_len),
                "{src:?} should lex as NumericLit of length {expected_len}"
            );
        }
    }

    // --- Bit-string / hex-string literal tests ---
    //
    // Postgres recognises `B'10'` (bit-string) and `X'1FF'` (hex-string) as
    // single literal tokens. The body content is validated at parse-time, not
    // lex-time — the lexer accepts any non-quote characters. Quotes cannot
    // appear inside (no `''`-escape, unlike `StringLit`).

    #[test]
    fn bit_string_literal_uppercase_prefix() {
        let mut input = crate::tokens::test_input("B'10'");
        let lit = BitStringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "B'10'");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn bit_string_literal_lowercase_prefix() {
        let mut input = crate::tokens::test_input("b'001'");
        let lit = BitStringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "b'001'");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn bit_string_literal_empty() {
        let mut input = crate::tokens::test_input("B''");
        let lit = BitStringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "B''");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn hex_string_literal_uppercase_prefix() {
        let mut input = crate::tokens::test_input("X'1FF'");
        let lit = HexStringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "X'1FF'");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    #[test]
    fn hex_string_literal_lowercase_prefix() {
        let mut input = crate::tokens::test_input("x'42f'");
        let lit = HexStringLit::parse(&mut input).unwrap();
        assert_eq!(lit.0, "x'42f'");
        assert!(
            input.is_empty(),
            "leftover: {:?}",
            &input.source()[input.byte_offset()..]
        );
    }

    // Cross-token arbitration: with the classifier installed, `B'…'` and
    // `X'…'` must lex as the dedicated `BitStringLit` / `HexStringLit` kinds —
    // NOT as an `Ident` followed by a `StringLit`. This is the actual code
    // path where the formatter-inserted-space bug manifested (the previous
    // behaviour was two distinct tokens which the formatter then separated
    // with a space).
    #[test]
    fn bit_and_hex_string_literals_classify_as_single_token() {
        for (src, expected) in [
            ("B'10'", super::TokenKind::BitStringLit),
            ("b'001'", super::TokenKind::BitStringLit),
            ("B''", super::TokenKind::BitStringLit),
            ("X'1FF'", super::TokenKind::HexStringLit),
            ("x'42f'", super::TokenKind::HexStringLit),
        ] {
            assert!(
                first_token_is(src, expected, src.len()),
                "{src:?} must lex as a single {expected:?} token"
            );
        }
    }

    // --- Dollar-string literal tests ---
    //
    // Postgres dollar-quoted strings (`$tag$ ... $tag$`, `$$ ... $$`) close
    // ONLY at a matching tag. The previous regex-based scanner used a non-
    // greedy regex (`\$[a-zA-Z_]*\$[\s\S]*?\$[a-zA-Z_]*\$`), but the lexer's
    // `MatchKind::All` configuration picks the LONGEST alternative — defeating
    // `*?` and causing two distinct dollar-strings in the same input to
    // collapse into one over-matched token. These tests pin the matched-tag
    // semantics through `DollarStringLit::parse` directly and through the
    // cross-token classifier path.

    #[test]
    fn dollar_string_lit_matches_only_matching_close_tag() {
        // Two distinct `$$ ... $$` strings around a `SELECT 1;` — the FIRST
        // must end at the first `$$`, not over-match to the last `$$`.
        let src = "$$ A $$ SELECT 1; $$ B $$";
        let mut input = crate::tokens::test_input(src);
        let first = DollarStringLit::parse(&mut input).expect("first $$...$$ must parse");
        assert_eq!(
            first.0.as_ref(),
            "$$ A $$",
            "first dollar-string must end at the first matching $$; got {:?}",
            first.0.as_ref(),
        );
    }

    #[test]
    fn dollar_string_lit_named_tag_closes_only_on_matching_tag() {
        // `$foo$ body $bar$ more $foo$` — the close inside (`$bar$`) does NOT
        // match the open (`$foo$`) so scanning must continue until the real
        // `$foo$` close. This is the key "back-reference" behaviour that the
        // NFA-based regex can't express.
        let src = "$foo$ body $bar$ more $foo$";
        let mut input = crate::tokens::test_input(src);
        let lit = DollarStringLit::parse(&mut input).expect("named-tag dollar-string must parse");
        assert_eq!(
            lit.0.as_ref(),
            "$foo$ body $bar$ more $foo$",
            "scanning must continue past the non-matching $bar$ close",
        );
    }

    #[test]
    fn dollar_string_lit_named_tag_two_distinct_strings() {
        // Two separate `$foo$...$foo$` literals — the first must end at the
        // first matching `$foo$`, not over-match into the second.
        let src = "$foo$ A $foo$ X $foo$ B $foo$";
        let mut input = crate::tokens::test_input(src);
        let first = DollarStringLit::parse(&mut input).expect("first $foo$...$foo$ must parse");
        assert_eq!(
            first.0.as_ref(),
            "$foo$ A $foo$",
            "first named dollar-string must end at the first matching close",
        );
    }

    #[test]
    fn dollar_string_lit_with_classifier_ends_at_first_matching_close() {
        // Cross-classifier check: with the classifier installed, parsing the
        // FIRST `$$ A $$` must consume only that token, leaving ` SELECT 1;
        // $$ B $$` for the next parse. The previous regex-based scanner with
        // `MatchKind::All` over-matched, consuming both dollar-strings in one
        // 25-byte token.
        let src = "$$ A $$ SELECT 1; $$ B $$";
        let mut input = crate::tokens::test_input(src);
        let first =
            DollarStringLit::parse(&mut input).expect("first $$...$$ must parse with classifier");
        assert_eq!(
            first.0.as_ref(),
            "$$ A $$",
            "first dollar-string must end at the first matching $$ under the classifier",
        );
    }

    #[test]
    fn dollar_string_lit_empty_body() {
        // `$$$$` is an empty dollar-quoted string with empty tag.
        let src = "$$$$";
        let mut input = crate::tokens::test_input(src);
        let lit = DollarStringLit::parse(&mut input).expect("empty $$$$ must parse");
        assert_eq!(lit.0.as_ref(), "$$$$");
    }

    #[test]
    fn dollar_string_lit_rejects_digit_leading_tag() {
        // A dollar-quote tag follows unquoted-identifier rules: it cannot
        // start with a digit. `$1$...$1$` is therefore NOT a dollar-quoted
        // string — `$1` is a positional parameter (`DollarNum`).
        let src = "$1$x$1$";
        let mut input = crate::tokens::test_input(src);
        assert!(
            DollarStringLit::parse(&mut input).is_err(),
            "$1$...$1$ must NOT parse as a dollar-string (tag cannot start with a digit)",
        );
        let mut input = crate::tokens::test_input(src);
        let num = DollarNum::parse(&mut input).expect("$1 must parse as DollarNum");
        assert_eq!(num.0.as_ref(), "$1");
    }

    // --- Soft keyword tests ---

    /// A soft (non-reserved) keyword is reclaimable as an identifier when a
    /// classifier is installed: `format`, `path`, `json`, etc. classify as
    /// their keyword token, but `UnquotedIdent` still accepts them. A
    /// reserved keyword (`select`) must stay rejected.
    #[test]
    fn soft_keyword_parses_as_identifier_with_classifier() {
        // Non-reserved Postgres keywords — usable as ordinary identifiers.
        for word in [
            "format", "path", "json", "empty", "scalar", // SQL/JSON family
            "target", "source", "key", "name", "value", "data", "update", "insert", "type",
            "method", "owner", "action", "level", "off",
        ] {
            let mut input = crate::tokens::test_input(word);
            let id = super::literal::UnquotedIdent::parse(&mut input)
                .unwrap_or_else(|e| panic!("soft keyword {word:?} should parse as ident: {e}"));
            assert_eq!(id.0.as_ref(), word);
            assert!(input.is_empty(), "leftover after {word:?}");
        }
    }

    #[test]
    fn reserved_keyword_rejected_as_identifier_with_classifier() {
        // Reserved keywords, plus the clause keywords kept hard because an
        // optional-identifier slot precedes them: `SET` (UPDATE target
        // alias), `NULLS` (index opclass), `PARTITION` (window spec — the
        // only window keyword not guarded by `not_frame_unit`).
        for word in ["select", "from", "where", "set", "nulls", "partition"] {
            let mut input = crate::tokens::test_input(word);
            assert!(
                super::literal::UnquotedIdent::parse(&mut input).is_err(),
                "reserved/clause keyword {word:?} must not parse as an identifier"
            );
        }
    }

    #[test]
    fn token_kind_is_soft_classifies_correctly() {
        assert!(super::token_kind_is_soft(super::TokenKind::FORMAT as u16));
        assert!(super::token_kind_is_soft(
            super::TokenKind::JSON_TABLE as u16
        ));
        assert!(!super::token_kind_is_soft(super::TokenKind::SELECT as u16));
    }

    // --- Identifier tests ---

    #[test]
    fn identifier_simple() {
        let mut input = crate::tokens::test_input("my_table");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "my_table");
    }

    #[test]
    fn identifier_with_digits() {
        let mut input = crate::tokens::test_input("f1");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "f1");
    }

    #[test]
    fn identifier_uppercase() {
        let mut input = crate::tokens::test_input("BOOLTBL1");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "BOOLTBL1");
    }

    #[test]
    fn unquoted_rejects_keyword_select() {
        let mut input = crate::tokens::test_input("SELECT");
        assert!(!UnquotedIdent::peek(&mut input));
    }

    #[test]
    fn unquoted_rejects_keyword_true() {
        let mut input = crate::tokens::test_input("true");
        assert!(!UnquotedIdent::peek(&mut input));
    }

    #[test]
    fn unquoted_rejects_keyword_null() {
        let mut input = crate::tokens::test_input("NULL");
        assert!(!UnquotedIdent::peek(&mut input));
    }

    #[test]
    fn ident_enum_rejects_keyword() {
        // Under the new Parse semantics, postcondition on enum wraps both peek and parse:
        // peek forks+runs parse, so peek returns false for a keyword input.
        let mut input = crate::tokens::test_input("SELECT");
        assert!(!Ident::peek(&mut input));
        let mut input2 = crate::tokens::test_input("SELECT");
        assert!(Ident::parse(&mut input2).is_err());
    }

    #[test]
    fn ident_accepts_rows_as_identifier() {
        // ROWS is unreserved in PostgreSQL and must be usable as a plain
        // identifier (e.g. `FROM rows`, `SELECT range FROM t`).
        for w in ["rows", "ROWS", "range", "groups"] {
            let mut input = crate::tokens::test_input(w);
            let id =
                Ident::parse(&mut input).unwrap_or_else(|_| panic!("{w} should parse as Ident"));
            assert_eq!(id.text(), w);
        }
    }

    #[test]
    fn window_ref_name_rejects_frame_units() {
        // Window `ref_name` must reject ROWS/RANGE/GROUPS so the frame
        // clause after the (optional) ref_name still parses.
        for w in ["rows", "ROWS", "range", "RANGE", "groups", "GROUPS"] {
            let mut input = crate::tokens::test_input(w);
            assert!(
                !WindowRefNameIdent::peek(&mut input),
                "{w} must not peek as a window ref_name"
            );
        }
    }

    #[test]
    fn window_ref_name_accepts_plain_ident() {
        let mut input = crate::tokens::test_input("w1");
        let id = WindowRefNameIdent::parse(&mut input).unwrap();
        let WindowRefNameIdent::Ident(inner) = &id;
        assert_eq!(inner.text(), "w1");
    }

    #[test]
    fn ident_enum_parses_quoted() {
        let mut input = crate::tokens::test_input("\"SELECT\"");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "\"SELECT\"");
        assert!(input.is_empty());
    }

    #[test]
    fn identifier_accepts_keyword_prefix() {
        let mut input = crate::tokens::test_input("isfalse");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "isfalse");
    }

    #[test]
    fn identifier_accepts_booleq() {
        let mut input = crate::tokens::test_input("booleq");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "booleq");
    }

    #[test]
    fn identifier_accepts_boolne() {
        let mut input = crate::tokens::test_input("boolne");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "boolne");
    }

    #[test]
    fn identifier_accepts_isnul() {
        let mut input = crate::tokens::test_input("isnul");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "isnul");
    }

    #[test]
    fn identifier_accepts_istrue() {
        let mut input = crate::tokens::test_input("istrue");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "istrue");
    }

    #[test]
    fn identifier_accepts_pg_input_is_valid() {
        let mut input = crate::tokens::test_input("pg_input_is_valid");
        let id = Ident::parse(&mut input).unwrap();
        assert_eq!(id.text(), "pg_input_is_valid");
    }
}

#[cfg(test)]
mod ident_enum_tests {
    use super::literal::*;
    use recursa::Parse;

    #[test]
    fn ident_peek_rejects_from_keyword() {
        let mut input = crate::tokens::test_input("FROM");
        eprintln!("Ident::peek(FROM) = {}", Ident::peek(&mut input));
        assert!(
            !Ident::peek(&mut input),
            "Ident should not peek true for FROM"
        );
    }
}
