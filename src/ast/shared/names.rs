/// Name-shaped AST primitives: qualified names, role names, type names,
/// operator names, and the rename/owner/schema action clauses that bundle them.
use crate::tokens::literal;

/// A comma-separated list of qualified (dotted) names — Postgres'
/// `any_name_list` / `name_list` in DROP-family statements.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash)]
pub struct NameList<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<QualifiedName<'input>>,
}

impl<'input> NameList<'input> {
    /// Number of names in the list.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the list is empty (always false — `Seq1` requires one entry).
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// A single role reference — Postgres' `RoleSpec`.
///
/// Only the `NonReservedWord` form is modelled: every role reference in the
/// differential corpus is a plain (possibly quoted) identifier. The reserved
/// pseudo-roles `CURRENT_ROLE` / `CURRENT_USER` / `SESSION_USER` are not yet
/// modelled — when a corpus statement needs one, add reserved-keyword tokens
/// and extend this enum to a tuple variant per form.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleSpec<'input> {
    pub name: crate::tokens::NonReservedWord<'input>,
}

/// A comma-separated list of roles — Postgres' `role_list`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleList<'input> {
    #[sep(COMMA)]
    pub roles: recursa::Vec1<RoleSpec<'input>>,
}

impl<'input> RoleList<'input> {
    /// Number of roles in the list.
    pub fn len(&self) -> usize {
        self.roles.len()
    }

    /// Whether the list is empty (always false — `Seq1` requires one entry).
    pub fn is_empty(&self) -> bool {
        self.roles.is_empty()
    }
}

/// A type-name reference — Postgres' `Typename` as it appears in
/// `DROP TYPE` / `DROP DOMAIN` / `DROP CAST`.
///
/// The corpus only exercises simple (possibly qualified) type names and
/// keyword-spelled built-in types in these positions, so this delegates to
/// the expression-level `TypeName`. Array suffixes and `%TYPE` are not used
/// by any DROP corpus statement.
pub use crate::ast::shared::expr::TypeName;

/// A comma-separated list of type names — Postgres' `type_name_list`.
///
/// Items are `CastType` rather than bare `TypeName` so the array suffix
/// (`int[]`, `text[]`) survives. PG's `type_name_list` is built from
/// `Typename`, which includes the `[]`/`[N]` array suffix(es) — the bare
/// `TypeName` enum in pg-sql models only `SimpleTypename`.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub struct TypeNameList<'input> {
    #[sep(COMMA)]
    pub types: recursa::Vec1<crate::ast::shared::expr::CastType<'input>>,
}

/// The `(...)` argument signature on `DROP AGGREGATE name(...)`.
///
/// The corpus only exercises `(*)` (zero-argument aggregate) and a plain
/// comma-separated type list. The ordered-set `(... ORDER BY ...)` forms and
/// named/moded `aggr_arg`s are not used by any DROP corpus statement.
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq)]
pub enum AggregateArgs<'input> {
    #[tok(LPAREN, STAR, RPAREN)]
    /// `(*)` — the zero-argument aggregate (spelled like `COUNT(*)`).
    Star,
    /// `(type, ...)` — explicit argument type list.
    Types(
        #[tok(LPAREN, this, RPAREN)]
        #[sep(COMMA)]
        recursa::Vec1<TypeName<'input>>,
    ),
}

/// A dotted name: `name`, `schema.name`, or `catalog.schema.name`.
///
/// This is the usual shape for table/view/sequence/type references in SQL.
/// Must NOT collide with `Expr::QualRef` at the Pratt level because
/// `QualifiedName` is only used in non-expression positions (FROM targets,
/// DROP targets, ALTER targets, etc.).
#[derive(recursa::Node, Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedName<'input> {
    #[sep(DOT)]
    pub parts: recursa::Vec1<literal::Ident<'input>>,
}

impl<'input> QualifiedName<'input> {
    /// Returns the final (object) name part.
    pub fn object(&self) -> &str {
        self.parts
            .last()
            .expect("qualified names contain at least one part")
            .text()
    }
}

/// Function definition name (CREATE FUNCTION / DROP FUNCTION / DROP ROUTINE).
///
/// PG's `func_name: type_function_name | ColId indirection` admits
/// unreserved keywords such as `SET`. [`QualifiedName`] already uses the
/// generated identifier admission set containing those keywords, so one
/// canonical variant covers both ordinary and keyword-spelled names.
#[derive(recursa::Node, Debug, Clone)]
pub enum FuncDefName<'input> {
    Name(QualifiedName<'input>),
}

impl<'input> FuncDefName<'input> {
    /// Returns the final (object) name part as text.
    pub fn object(&self) -> &str {
        match self {
            FuncDefName::Name(q) => q.object(),
        }
    }
}

/// `RENAME TO new_name` — the rename action shared by many ALTER
/// statements. Postgres routes most of these through `RenameStmt`, but
/// pg-sql's dispatcher commits on the leading `ALTER objtype ...`
/// keywords, so each `Alter*Stmt` re-models its own rename branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct RenameTo<'input> {
    #[tok(RENAME, TO, this)]
    pub new_name: literal::Ident<'input>,
}

/// `OWNER TO RoleSpec` — the owner-change action shared by many ALTER
/// statements. Postgres routes most of these through `AlterOwnerStmt`,
/// but pg-sql's dispatcher commits on the leading `ALTER objtype ...`
/// keywords, so each `Alter*Stmt` re-models its own owner branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct OwnerTo<'input> {
    #[tok(OWNER, TO, this)]
    pub new_owner: RoleSpec<'input>,
}

/// `SET SCHEMA name` — the set-schema action shared by ALTER FOREIGN
/// TABLE, ALTER TABLE, ALTER VIEW, ALTER MATERIALIZED VIEW, etc.
/// Postgres routes most of these through `AlterObjectSchemaStmt`, but
/// pg-sql's dispatcher commits on the leading `ALTER objtype ...`
/// keywords, so each `Alter*Stmt` re-models its own set-schema branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct SetSchemaClause<'input> {
    #[tok(SET, SCHEMA, this)]
    pub new_schema: literal::Ident<'input>,
}

/// A single (unqualified) operator name — Postgres' `all_Op` rule
/// (`Op | MathOp`).
///
/// `all_Op` is a lexer class in PG that absorbs any operator-character
/// sequence, plus the single-char `MathOp`s (`+ - * / % ^ < > =`) and the
/// 2-char comparisons `<= >= <>`. In recursa's logos token model every
/// distinct multi-char operator gets its own punct token (`Lte`, `Gte`,
/// `Neq`, `TripleEq`, `BangEqEq`, `BangEqMinus`, `LtLtLt`, …), so this enum
/// must enumerate every punct token whose spelling is made of operator
/// characters (`+ - * / % ^ < > = ~ ! @ # & | ?`). Anything else falls into
/// the multi-char catch-all `CustomOp`.
///
/// Variant ordering: peek regexes are exact per-variant (each variant maps
/// to exactly one token kind), so disambiguation is unambiguous regardless
/// of order. Variants are grouped by leading char for readability.
///
/// `FatArrow` (`=>`) is deliberately omitted: PG explicitly rejects `=>` as
/// an operator name, and excluding it lets the few corpus `CREATE OPERATOR
/// =>` lines surface as file-level parse errors, matching
/// PG's rejection on both sides of the differential oracle.
#[derive(recursa::Node, Debug, Clone)]
pub enum OperatorName<'input> {
    // Multi-char tokens whose spelling is purely operator chars. Each is
    // a single logos token kind so their peek regexes are disjoint.
    #[tok(STARLTE)]
    StarLte,
    #[tok(STARGTE)]
    StarGte,
    #[tok(STARNEQ)]
    StarNeq,
    #[tok(STARLT)]
    StarLt,
    #[tok(STARGT)]
    StarGt,
    #[tok(STAREQ)]
    StarEq,
    #[tok(TRIPLEEQ)]
    TripleEq,
    #[tok(BANGEQEQ)]
    BangEqEq,
    #[tok(BANGEQMINUS)]
    BangEqMinus,
    #[tok(BANGEQ)]
    BangEq,
    #[tok(LTLTLT)]
    LtLtLt,
    #[tok(LTLTEQ)]
    LtLtEq,
    #[tok(LTLTPIPE)]
    LtLtPipe,
    #[tok(LTMINUSGT)]
    LtMinusGt,
    #[tok(LTLT)]
    LtLt,
    #[tok(LTCARET)]
    LtCaret,
    #[tok(LTAT)]
    LtAt,
    #[tok(GTGTGT)]
    GtGtGt,
    #[tok(GTGTEQ)]
    GtGtEq,
    #[tok(GTGT)]
    GtGt,
    #[tok(GTCARET)]
    GtCaret,
    #[tok(HASHARROWARROW)]
    HashArrowArrow,
    #[tok(HASHARROW)]
    HashArrow,
    #[tok(HASHHASH)]
    HashHash,
    #[tok(HASHMINUS)]
    HashMinus,
    #[tok(ARROWARROW)]
    ArrowArrow,
    #[tok(ARROW)]
    Arrow,
    #[tok(MINUSPIPEMINUS)]
    MinusPipeMinus,
    #[tok(PIPEGTGT)]
    PipeGtGt,
    #[tok(PIPEAMPGT)]
    PipeAmpGt,
    #[tok(PIPEPIPESLASH)]
    PipePipeSlash,
    #[tok(CONCAT)]
    Concat,
    #[tok(PIPESLASH)]
    PipeSlash,
    #[tok(QUESTIONPIPEPIPE)]
    QuestionPipePipe,
    #[tok(QUESTIONDASHPIPE)]
    QuestionDashPipe,
    #[tok(QUESTIONPIPE)]
    QuestionPipe,
    #[tok(QUESTIONAMP)]
    QuestionAmp,
    #[tok(QUESTIONHASH)]
    QuestionHash,
    #[tok(QUESTIONDASH)]
    QuestionDash,
    #[tok(ATATAT)]
    AtAtAt,
    #[tok(ATMINUSAT)]
    AtMinusAt,
    #[tok(ATHASHAT)]
    AtHashAt,
    #[tok(ATPLUSAT)]
    AtPlusAt,
    #[tok(ATAT)]
    AtAt,
    #[tok(ATQUESTION)]
    AtQuestion,
    #[tok(ATGT)]
    AtGt,
    #[tok(AMPLTPIPE)]
    AmpLtPipe,
    #[tok(AMPAMP)]
    AmpAmp,
    #[tok(AMPLT)]
    AmpLt,
    #[tok(AMPGT)]
    AmpGt,
    #[tok(TILDELEQTILDE)]
    TildeLeqTilde,
    #[tok(TILDEGEQTILDE)]
    TildeGeqTilde,
    #[tok(TILDELTTILDE)]
    TildeLtTilde,
    #[tok(TILDEGTTILDE)]
    TildeGtTilde,
    #[tok(BANGTILDETILDESTAR)]
    BangTildeTildeStar,
    #[tok(TILDETILDESTAR)]
    TildeTildeStar,
    #[tok(BANGTILDETILDE)]
    BangTildeTilde,
    #[tok(TILDETILDE)]
    TildeTilde,
    #[tok(BANGTILDESTAR)]
    BangTildeStar,
    #[tok(TILDESTAR)]
    TildeStar,
    #[tok(BANGTILDE)]
    BangTilde,
    #[tok(TILDEEQ)]
    TildeEq,
    #[tok(CARETAT)]
    CaretAt,
    // Single-char punct tokens (the `MathOp` set plus the bare operator
    // characters PG treats as operator chars).
    #[tok(LTE)]
    Lte,
    #[tok(GTE)]
    Gte,
    #[tok(NEQ)]
    Neq,
    #[tok(PLUS)]
    Plus,
    #[tok(MINUS)]
    Minus,
    #[tok(STAR)]
    Star,
    #[tok(SLASH)]
    Slash,
    #[tok(PERCENT)]
    Percent,
    #[tok(CARET)]
    Caret,
    #[tok(LT)]
    Lt,
    #[tok(GT)]
    Gt,
    #[tok(EQ)]
    Eq,
    #[tok(TILDE)]
    Tilde,
    #[tok(ATSIGN)]
    At,
    #[tok(POUND)]
    Pound,
    #[tok(AMP)]
    Amp,
    #[tok(PIPE)]
    Pipe,
    #[tok(QUESTION)]
    Question,
    // Multi-char catch-all. Listed last because each of the specific punct
    // tokens above wins at the lexer level (logos longest-match-wins with
    // declaration order tiebreaker); only operator names that don't match
    // any specific token end up as `CustomOp`.
    Custom(literal::CustomOp<'input>),
}

/// A possibly schema-qualified operator name — Postgres' `any_operator`.
///
/// Postgres allows arbitrary prefixes of `ColId.` parts (e.g., `pg_catalog.+`,
/// `schema_op1.#*#`). Modelled as an enum so the peek set covers both the
/// `Ident.` qualified path and every bare-operator first-token from
/// [`OperatorName`].
///
/// Variant ordering: `Qualified` starts with `Ident`, `Plain` starts with a
/// punct/operator token. Their first sets are disjoint, so order is for
/// clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum QualifiedOperatorName<'input> {
    /// `[schema.]op` — at least one `Ident.` segment followed by an
    /// `OperatorName`.
    Qualified(QualifiedOperatorPath<'input>),
    /// Bare operator name with no schema qualifier.
    Plain(OperatorName<'input>),
}

/// A schema-qualified operator name: one or more `Ident.` segments followed
/// by an `OperatorName`.
#[derive(recursa::Node, Debug, Clone)]
pub struct QualifiedOperatorPath<'input> {
    pub first: QualifiedOperatorPrefix<'input>,
    pub rest: Vec<QualifiedOperatorPrefix<'input>>,
    pub name: OperatorName<'input>,
}

/// One `Ident.` segment of a qualified operator name's schema prefix.
#[derive(recursa::Node, Debug, Clone)]
pub struct QualifiedOperatorPrefix<'input> {
    #[tok(this, DOT)]
    pub name: literal::Ident<'input>,
}

/// `(left, right)` argument-type signature on `operator_with_argtypes` —
/// Postgres' `oper_argtypes`.
///
/// `NONE` is a `COL_NAME` keyword and is therefore also accepted by
/// [`TypeName`]. Modelling unary and binary spellings as separate grammar
/// alternatives would duplicate the same token language. This shared-prefix
/// type parses the pair once; [`Self::left`] and [`Self::right`] interpret a
/// single unqualified `NONE` as PostgreSQL's missing operand marker.
#[derive(recursa::Node, Debug, Clone)]
#[tok(LPAREN, this, RPAREN)]
pub struct OperatorArgtypes<'input> {
    pub left_type: TypeName<'input>,
    #[tok(COMMA, this)]
    pub right_type: TypeName<'input>,
}

impl<'input> OperatorArgtypes<'input> {
    /// Returns the left operand type, or `None` for PostgreSQL's unary
    /// `(NONE, right)` spelling.
    pub fn left(&self) -> Option<&TypeName<'input>> {
        (!is_operator_none(&self.left_type)).then_some(&self.left_type)
    }

    /// Returns the right operand type, or `None` for PostgreSQL's unary
    /// `(left, NONE)` spelling.
    pub fn right(&self) -> Option<&TypeName<'input>> {
        (!is_operator_none(&self.right_type)).then_some(&self.right_type)
    }
}

fn is_operator_none(type_name: &TypeName<'_>) -> bool {
    let TypeName::Ident(name) = type_name else {
        return false;
    };
    name.parts.len() == 1 && name.object().eq_ignore_ascii_case("none")
}

/// `any_operator oper_argtypes` — Postgres' `operator_with_argtypes`. The
/// full reference to a specific operator (including overload signature)
/// used by `DROP OPERATOR`, `ALTER OPERATOR`, `COMMENT ON OPERATOR`,
/// `SECURITY LABEL ON OPERATOR`, etc.
#[derive(recursa::Node, Debug, Clone)]
pub struct OperatorWithArgtypes<'input> {
    pub name: QualifiedOperatorName<'input>,
    pub args: OperatorArgtypes<'input>,
}
