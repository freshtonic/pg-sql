#![allow(non_camel_case_types)]

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

recursa::tokens! {
    // Ignored content: ASCII whitespace and `-- line comments`. Both are
    // regular, so the generated lexer handles them directly. Nested `/* block
    // comments */` use the closed matcher declared with the trivia kinds below.
    ignore = r"[ \t\r\n\f]+",
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
        // `AS` always renders with a space on both sides. Without this the
        // Auto word policy leaves the following `(` tight (`AS(SELECT 1)`),
        // because an opening delimiter does not consume a pending word
        // separator.
        AS           => r"AS" in RESERVED, spacing = around,
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
    // Soft (non-reserved) keywords: recognised as keywords only where the
    // grammar asks for them; otherwise reclaimable as ordinary identifiers
    // (see `UnquotedIdent`'s `token_kind_is_soft` check). Covers Postgres
    // non-reserved / col-name / type-name keywords plus the SQL/JSON
    // function family — all common identifiers, so none may be reserved.
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
        SEMI      => ";",
        COMMA     => ",",
        LPAREN    => "(",
        RPAREN    => ")",
        // Record comparison operators (`*=` etc.) — longest-match first so
        // they win over bare `Star`. Used only as Pratt infix operators.
        STARLTE   => "*<=",
        STARGTE   => "*>=",
        STARNEQ   => "*<>",
        STARLT    => "*<",
        STARGT    => "*>",
        STAREQ    => "*=",
        STAR      => "*",
        DOT       => ".", spacing = tight,
        // 3-char `===` before 2-char `=>` and single-char `=`.
        TRIPLEEQ   => "===",
        EQ        => "=", spacing = around,
        FATARROW  => "=>",
        COLONEQUALS => ":=",
        // 3-char `!==` and `!=-` before 2-char `!=`.
        BANGEQEQ   => "!==",
        BANGEQMINUS => "!=-",
        BANGEQ    => "!=", spacing = around,
        NEQ       => "<>", spacing = around,
        // 3-char `<`-prefixed operators must come before 2-char `<=`/`<>`/`<<`
        // and before the single-char `<`.
        // 3-char `<<<` before 2-char `<<`.
        LTLTLT     => "<<<",
        LTLTEQ     => "<<=",
        LTLTPIPE   => "<<|",
        LTMINUSGT  => "<->",
        LTLT       => "<<",
        LTE       => "<=", spacing = around,
        // Geometric "below" `<^` before single-char `<`.
        LTCARET    => "<^",
        // 3-char `>>=` before `>>`, then `>=`, `>`.
        // 3-char `>>>` before 2-char `>>`.
        GTGTGT     => ">>>",
        GTGTEQ     => ">>=",
        GTGT       => ">>",
        GTE       => ">=", spacing = around,
        // Geometric "above" `>^` before single-char `>`.
        GTCARET    => ">^",
        LT        => "<", spacing = around,
        GT        => ">", spacing = around,
        COLONCOLON => "::",
        COLON      => ":",
        // Psql meta-commands that can terminate a SQL statement in place of `;`.
        // Must be listed before plain BackSlash so longest-match-wins picks the
        // specific directive over the bare backslash.
        PSQLCROSSTABVIEW => "\\crosstabview",
        PSQLGEXEC  => "\\gexec",
        PSQLGSET   => "\\gset",
        PSQLGX     => "\\gx",
        PSQLG      => "\\g",
        // `\;` — psql batch separator: ends a statement without ending the
        // line. Listed before bare `BackSlash`.
        PSQLBATCHSEMI => "\\;",
        BACKSLASH  => "\\",
        PLUS       => "+",
        // 3-char `-|-` before 2-char `->>`/`->` before single-char `-`.
        MINUSPIPEMINUS => "-|-",
        MINUS      => "-",
        // 3-char `|>>` and `|&>` before 2-char `||`.
        PIPEGTGT       => "|>>",
        PIPEAMPGT      => "|&>",
        // Cube-root `||/` (prefix operator). Must come before `||`.
        PIPEPIPESLASH  => "||/",
        CONCAT     => "||",
        // Square-root `|/` (prefix operator). Must come after `||/`/`||`
        // and before bare `|`.
        PIPESLASH      => "|/",
        // Single-char `|` (bitwise OR). Must be declared after `||` and
        // other `|`-prefixed operators so longest-match picks the longer form.
        PIPE       => "|",
        SLASH      => "/",
        PERCENT    => "%",
        LBRACKET   => "[",
        RBRACKET   => "]",
        // JSON/JSONB operators. Longer before shorter (longest-match-wins).
        HASHARROWARROW => "#>>",
        HASHARROW      => "#>",
        // 2-char `##` (geometric closest-point / path intersection). After
        // `#>>` and `#>` but before single-char `#`.
        HASHHASH       => "##",
        // jsonb delete-path operator `#-`. Must precede single-char `Pound`
        // so longest-match-wins picks the 2-char form. Without this entry the
        // classifier splits `#-` into `Pound` + `Minus`, which the formatter
        // then re-emits as `# -` — re-parsed by PostgreSQL as two operators.
        HASHMINUS      => "#-",
        // Single-char `#` (bitwise XOR). Must come after all longer `#`-prefixed
        // tokens so longest-match-wins.
        POUND          => "#",
        ARROWARROW     => "->>",
        ARROW          => "->",
        // 3-char `?||` and `?-|` before 2-char `?|`/`?-` (longest-match-wins).
        QUESTIONPIPEPIPE => "?||",
        QUESTIONDASHPIPE => "?-|",
        QUESTIONPIPE   => "?|",
        QUESTIONAMP    => "?&",
        QUESTIONHASH   => "?#",
        QUESTIONDASH   => "?-",
        // `@@@` before 3-char `@-@`/`@#@`/`@+@` before `@@` before `@?` / `@>`.
        ATATAT         => "@@@",
        // User-defined / geometric 3-char `@`-prefixed operators.
        ATMINUSAT      => "@-@",
        ATHASHAT       => "@#@",
        ATPLUSAT       => "@+@",
        ATAT           => "@@",
        ATQUESTION     => "@?",
        ATGT           => "@>",
        // Single-char `@` (prefix absolute-value operator). Declared after
        // all longer `@`-prefixed operators so longest-match-wins.
        ATSIGN         => "@",
        LTAT           => "<@",
        QUESTION       => "?",
        // `&`-prefixed range/geometric operators. 3-char `&<|` before 2-char.
        AMPLTPIPE      => "&<|",
        AMPAMP         => "&&",
        AMPLT          => "&<",
        AMPGT          => "&>",
        // Single-char `&` (bitwise AND). Must follow all longer `&`-prefixed
        // operators so longest-match-wins.
        AMP            => "&",
        // Locale-aware text comparison operators. 4-char before 3-char,
        // all before POSIX regex `~*`/`!~*`/`!~`/`~=`/`~`.
        TILDELEQTILDE  => "~<=~",
        TILDEGEQTILDE  => "~>=~",
        TILDELTTILDE   => "~<~",
        TILDEGTTILDE   => "~>~",
        // LIKE/ILIKE family operators. PG uses `~~`/`!~~`/`~~*`/`!~~*` as
        // the implementation operators for LIKE/NOT LIKE/ILIKE/NOT ILIKE.
        // Longer forms must precede shorter forms (and precede the POSIX
        // `~*`/`!~*`/`~`/`!~` family) so longest-match wins.
        BANGTILDETILDESTAR => "!~~*",
        TILDETILDESTAR => "~~*",
        BANGTILDETILDE => "!~~",
        TILDETILDE     => "~~",
        // POSIX regex match operators. Longest-first.
        BANGTILDESTAR  => "!~*",
        TILDESTAR      => "~*",
        BANGTILDE      => "!~",
        // Geometric "same as" operator. Must precede bare `~`.
        TILDEEQ        => "~=",
        TILDE          => "~",
        // Text "starts-with" operator `^@`. Must precede single-char `Caret`
        // so longest-match-wins picks the 2-char form. Without this entry the
        // classifier splits `^@` into `Caret` + `At`, which the formatter then
        // re-emits as `^ @` — re-parsed by PostgreSQL as two operators.
        CARETAT        => "^@",
        // Exponentiation operator (Postgres).
        CARET          => "^",
    },
    matchers {
        DollarStringLit => same_delimiter(opener = r"\$(?:[A-Za-z_][A-Za-z0-9_]*)?\$"),
        NumericLit => next_exclusion(pattern = r"(?:(?:[0-9](?:_?[0-9])*\.[0-9](?:_?[0-9])*|\.[0-9](?:_?[0-9])*)(?:[eE][+-]?[0-9](?:_?[0-9])*)?|[0-9](?:_?[0-9])*\.[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*[eE][+-]?[0-9](?:_?[0-9])*|[0-9](?:_?[0-9])*\.)", excluded = r"[A-Za-z0-9_]"),
        IntegerLit => next_exclusion(pattern = r"(?:0[xX](?:_?[0-9a-fA-F])+|0[oO](?:_?[0-7])+|0[bB](?:_?[01])+|[0-9](?:_?[0-9])*)", excluded = r"[A-Za-z0-9_]"),
        DollarNum => next_exclusion(pattern = r"\$[0-9]+", excluded = r"[A-Za-z0-9_]"),
        CustomOp => operator_run(
            characters = "-+*/<>=~!@#%^&|?",
            fences = ["/*", "--"],
            trailing = "+-",
            qualifying = "~!@#%^&|?",
            priority = 1
        ),
    }
    ignore {
        // A physical-line matcher avoids making the global regular-pattern
        // ambiguity DFA carry an unbounded `--[^\n]*` search state while
        // preserving PostgreSQL's line-comment extent through the newline.
        LineComment => physical_line(prefix = r"--", priority = 2),
        // Nested comments remain non-emitting until classified trivia lands in #93.
        BlockComment => nested(opener = "/*", closer = "*/", priority = 2),
    }
    admissions {
        AllWordKinds = keywords,
        ColId = UNRESERVED + COL_NAME,
        type_function_name = UNRESERVED + TYPE_FUNC_NAME,
        TypeNameIdent = UNRESERVED + TYPE_FUNC_NAME + { JSON }
            - { BOOL, TEXT, SERIAL, UNKNOWN },
        NonReservedWord = UNRESERVED + COL_NAME + TYPE_FUNC_NAME,
        ColLabel = UNRESERVED + COL_NAME + TYPE_FUNC_NAME + RESERVED,
        BareColLabel = bare_label,
        WindowRefName = ColId - { PARTITION, ORDER, ROWS, RANGE, GROUPS },
        TableFunctionName = type_function_name - { COLLATION },
        UpdateAliasName = ColId - { SET },
        SelectBareAliasName = bare_label
            - { AND, OR, NOT, IS, IN, LIKE, ILIKE, COLLATE, SIMILAR, BETWEEN, OPERATOR, AT },
        PsqlVariableName = AllWordKinds - { NULL, TRUE, FALSE },
        UnquotedIdent = NonReservedWord,
        BareAliasName = AllWordKinds,
    }
}

// PostgreSQL's scanner produces one identifier token for quoted and unquoted
// spellings. Keep that as one canonical content base; the generated admission
// types below differ only in which fixed keyword kinds each grammar position
// may reclaim.
#[derive(recursa::Node, Debug, Clone)]
pub enum ColId<'input> {
    Text(
        #[lex(
            pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
            admits(ColId)
        )]
        ColIdText<'input>,
    ),
}

impl ColId<'_> {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(text) => text.text(),
        }
    }
}

#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash)]
pub enum NonReservedWord<'input> {
    Text(
        #[lex(
            pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
            admits(NonReservedWord)
        )]
        NonReservedWordText<'input>,
    ),
}

impl NonReservedWord<'_> {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(text) => text.text(),
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(recursa::Node, Debug, Clone)]
pub enum type_function_name<'input> {
    Text(
        #[lex(
            pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
            admits(type_function_name)
        )]
        TypeFunctionNameText<'input>,
    ),
}

/// Unqualified function/table name after reserving `COLLATION FOR (...)` for
/// its dedicated SQL-standard table-expression production.
#[allow(non_camel_case_types)]
#[derive(recursa::Node, Debug, Clone)]
pub enum table_function_name<'input> {
    Text(
        #[lex(
            pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
            admits(TableFunctionName)
        )]
        TableFunctionNameText<'input>,
    ),
}

/// Identifier spelling admitted by an expression-level type name.
///
/// PostgreSQL treats the legacy built-ins modeled as fixed `TypeName`
/// variants separately, while `json` remains an identifier-spelled type even
/// though the lexer classifies it as a `COL_NAME` keyword.
#[allow(non_camel_case_types)]
#[derive(recursa::Node, Debug, Clone)]
pub enum type_name_ident<'input> {
    Text(
        #[lex(
            pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
            admits(TypeNameIdent)
        )]
        TypeNameIdentText<'input>,
    ),
}

impl type_name_ident<'_> {
    pub fn text(&self) -> &str {
        match self {
            Self::Text(text) => text.text(),
        }
    }
}

#[derive(recursa::Node, Debug, Clone)]
pub enum BareColLabel<'input> {
    Text(
        #[lex(
            pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
            admits(BareColLabel)
        )]
        BareColLabelText<'input>,
    ),
}

// Re-exports preserving the legacy `keyword::` / `punct::` / `literal::` paths
// used throughout the rest of pg-sql. The `tokens!` macro emits every token
// struct at this module's top level; these sub-modules expose them under the
// names existing imports expect.

// `DollarStringLit` is now a `tokens!` `literals` entry whose closed
// same-delimiter matcher handles the closing-tag back-reference. It is generated
// at this module's top level like every other literal, so no extra re-export is
// needed here.

#[allow(non_camel_case_types)]
pub mod keyword {}

pub mod soft_keyword {}

pub mod punct {}

// Literals
pub mod literal {
    /// Canonical source module for the generated content-bearing literal
    /// types used by the migrated PostgreSQL grammar.
    #[allow(dead_code)]
    #[derive(recursa::Node, Debug, Clone)]
    pub struct LiteralBindings<'input> {
        #[lex(pattern = r"'[^']*(?:''[^']*)*'")]
        pub string: StringLit<'input>,
        #[lex(pattern = r"(?i:U)&'(?:[^']|'')*'")]
        pub unicode_string: UnicodeStringLit<'input>,
        #[lex(pattern = r"(?i:E)'(?:[^'\\]|\\.|'')*'")]
        pub escape_string: EscapeStringLit<'input>,
        #[lex(pattern = r"(?i:B)'[^']*'")]
        pub bit_string: BitStringLit<'input>,
        #[lex(pattern = r"(?i:X)'[^']*'")]
        pub hex_string: HexStringLit<'input>,
        #[lex(matcher)]
        pub dollar_string: DollarStringLit<'input>,
        #[lex(matcher)]
        pub dollar_number: DollarNum<'input>,
        #[lex(matcher)]
        pub integer: IntegerLit<'input>,
        #[lex(matcher)]
        pub numeric: NumericLit<'input>,
        #[lex(matcher)]
        pub custom_operator: CustomOp<'input>,
    }

    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct PsqlVariable<'input> {
        #[tok(COLON)]
        pub name: PsqlVariableValue<'input>,
    }

    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum PsqlVariableValue<'input> {
        Name(
            #[lex(
                pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
                admits(PsqlVariableName)
            )]
            PsqlVariableNameText<'input>,
        ),
        String(StringLit<'input>),
    }

    // Catch-all for Postgres user-defined operator names.
    //
    // Matches any sequence of the characters `+ - * / < > = ~ ! @ # % ^ & | ?`.
    // In expression contexts this is the LAST-RESORT infix/prefix in the Pratt
    // parser — known punct tokens are tried first. In DDL contexts (CREATE/ALTER/
    // DROP OPERATOR) this is the primary scanner for the operator name.
    //
    // `CustomOp` is produced by the declarative `operator_run` matcher and
    // receives its source-backed parse type from Recursa's generated token
    // machinery.
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
        // Window-reference positions exclude them through the dedicated
        // `WindowRefName` admission set below.
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

    /// SQL identifier admitted by PostgreSQL's `IDENT` / non-reserved-word
    /// production. Quoted and unquoted spellings share one lexical base; the
    /// named admission set controls which fixed keywords may occupy it.
    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum Ident<'input> {
        Text(
            #[lex(
                pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
                admits(UnquotedIdent)
            )]
            IdentText<'input>,
        ),
    }

    impl<'input> Ident<'input> {
        /// The raw text of the identifier.
        pub fn text(&self) -> &str {
            match self {
                Ident::Text(text) => text.text(),
            }
        }
    }

    /// Bare alias on an UPDATE target relation.
    ///
    /// PostgreSQL's `relation_expr_opt_alias` gives the following `SET`
    /// keyword precedence over interpreting it as a bare alias. Quoted
    /// identifiers and every other `ColId` spelling remain available.
    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum UpdateAliasName<'input> {
        Text(
            #[lex(
                pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
                admits(UpdateAliasName)
            )]
            UpdateAliasNameText<'input>,
        ),
    }

    impl<'input> UpdateAliasName<'input> {
        /// Raw alias text, including quotes when present.
        pub fn text(&self) -> &str {
            match self {
                UpdateAliasName::Text(text) => text.text(),
            }
        }
    }

    /// Bare alias on a SELECT target.
    ///
    /// `IS` starts several expression continuations. PostgreSQL's generated
    /// parser shifts those continuations before reducing a bare alias; this
    /// admission set encodes that precedence while explicit `AS is` and quoted
    /// `"is"` aliases remain available.
    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum SelectBareAliasName<'input> {
        Text(
            #[lex(
                pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
                admits(SelectBareAliasName)
            )]
            SelectBareAliasNameText<'input>,
        ),
    }

    impl<'input> SelectBareAliasName<'input> {
        /// Raw alias text, including quotes when present.
        pub fn text(&self) -> &str {
            match self {
                SelectBareAliasName::Text(text) => text.text(),
            }
        }
    }

    /// Identifier usable as a window `ref_name` (existing-window reference).
    /// Rejects clause heads (`PARTITION`, `ORDER`, `ROWS`, `RANGE`, `GROUPS`)
    /// so clauses after the optional `ref_name` parse deterministically.
    ///
    /// Modeled as a single-variant enum to preserve the legacy wrapper while
    /// the generated content matcher enforces the named admission set.
    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum WindowRefNameIdent<'input> {
        Text(
            #[lex(
                pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
                admits(WindowRefName)
            )]
            WindowRefNameText<'input>,
        ),
    }

    // --- Alias name (any SQL word — identifier or keyword) ---

    /// Alias name, including PostgreSQL keywords admitted in bare-label
    /// positions. Quoted and unquoted spellings share the identifier base.
    #[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub enum AliasName<'input> {
        Text(
            #[lex(
                pattern = r#"[Uu]&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#,
                admits(BareAliasName)
            )]
            AliasNameText<'input>,
        ),
    }

    impl<'input> AliasName<'input> {
        /// Raw text of the alias name (with quotes if quoted).
        pub fn text(&self) -> &str {
            match self {
                AliasName::Text(text) => text.text(),
            }
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

        impl<'a> Arbitrary<'a> for IdentText<'_> {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                let s = arb_non_keyword_ident(u)?;
                Ok(Self::new(s))
            }
        }

        impl<'a> Arbitrary<'a> for AliasNameText<'_> {
            fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
                let s = arb_ident_str(u)?;
                Ok(Self::new(s))
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
                Ok(Self::new(s))
            }
        }
    }
}
