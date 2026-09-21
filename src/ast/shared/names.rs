/// Name-shaped AST primitives: qualified names, role names, type names,
/// operator names, and the rename/owner/schema action clauses that bundle them.
use crate::tokens::literal;

recursa::ast_node! {
    /// A comma-separated list of qualified (dotted) names — Postgres'
    /// `any_name_list` / `name_list` in DROP-family statements.
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct NameList {
        #[sep(COMMA)]
        pub names: one_or_many!(QualifiedName),
    }
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

recursa::ast_node! {
    /// A single role reference — Postgres' `RoleSpec`.
    ///
    /// Only the `NonReservedWord` form is modelled: every role reference in the
    /// differential corpus is a plain (possibly quoted) identifier. The reserved
    /// pseudo-roles `CURRENT_ROLE` / `CURRENT_USER` / `SESSION_USER` are not yet
    /// modelled — when a corpus statement needs one, add reserved-keyword tokens
    /// and extend this enum to a tuple variant per form.
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct RoleSpec {
        pub name: crate::tokens::NonReservedWord,
    }
}

recursa::ast_node! {
    /// A comma-separated list of roles — Postgres' `role_list`.
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct RoleList {
        #[sep(COMMA)]
        pub roles: one_or_many!(RoleSpec),
    }
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

recursa::ast_node! {
    /// A comma-separated list of type names — Postgres' `type_name_list`.
    ///
    /// Items are `CastType` rather than bare `TypeName` so the array suffix
    /// (`int[]`, `text[]`) survives. PG's `type_name_list` is built from
    /// `Typename`, which includes the `[]`/`[N]` array suffix(es) — the bare
    /// `TypeName` enum in pg-sql models only `SimpleTypename`.
    #[derive(Debug, PartialEq, Eq)]
    pub struct TypeNameList {
        #[sep(COMMA)]
        pub types: one_or_many!(crate::ast::shared::expr::CastType),
    }
}

recursa::ast_node! {
    /// The `(...)` argument signature on `DROP AGGREGATE name(...)`.
    ///
    /// The corpus only exercises `(*)` (zero-argument aggregate) and a plain
    /// comma-separated type list. The ordered-set `(... ORDER BY ...)` forms and
    /// named/moded `aggr_arg`s are not used by any DROP corpus statement.
    #[derive(Debug, PartialEq, Eq)]
    pub enum AggregateArgs {
        #[tok(LPAREN, STAR, RPAREN)]
        /// `(*)` — the zero-argument aggregate (spelled like `COUNT(*)`).
        Star,
        /// `(type, ...)` — explicit argument type list.
        Types(AggregateArgTypeList),
    }
}

recursa::ast_node! {
    /// The parenthesized type list of `aggregate_with_argtypes`.
    ///
    /// The parentheses surround the whole list; a field-level attachment would
    /// bind to each element and declare `(int), (text)`.
    #[derive(Debug, PartialEq, Eq, derive_more :: Deref)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct AggregateArgTypeList(
        #[sep(COMMA)]
        #[deref]
        pub one_or_many!(TypeName),
    );
}

recursa::ast_node! {
    /// A dotted name: `name`, `schema.name`, or `catalog.schema.name`.
    ///
    /// This is the usual shape for table/view/sequence/type references in SQL.
    /// Must NOT collide with `Expr::QualRef` because
    /// `QualifiedName` is only used in non-expression positions (FROM targets,
    /// DROP targets, ALTER targets, etc.).
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct QualifiedName {
        /// gram.y `qualified_name: ColId | ColId indirection`: the first part is
        /// a `ColId`, so `verbose`, `full` and the other `type_func_name`
        /// keywords are not object names.
        pub first: crate::tokens::ColId,
        /// gram.y `indirection`: each part after a dot is `attr_name`, a
        /// `ColLabel` (any keyword class).
        pub rest: zero_or_many!(QualifiedNamePart),
    }
}

recursa::ast_node! {
    /// One `'.' attr_name` of gram.y `indirection` inside a `qualified_name`.
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct QualifiedNamePart {
        #[tok(DOT, this)]
        pub name: crate::tokens::ColLabel,
    }
}

impl<'input> QualifiedName<'input> {
    /// Returns the final (object) name part.
    pub fn object(&self) -> &str {
        match self.rest.last() {
            Some(part) => part.name.text(),
            None => self.first.text(),
        }
    }

    /// Number of dotted parts, `catalog.schema.name` being three.
    pub fn len(&self) -> usize {
        1 + self.rest.len()
    }

    /// A qualified name always has one part.
    pub fn is_empty(&self) -> bool {
        false
    }
}

recursa::ast_node! {
    /// Function definition name (CREATE FUNCTION / DROP FUNCTION / DROP ROUTINE).
    ///
    /// PG's `func_name: type_function_name | ColId indirection` admits
    /// unreserved keywords such as `SET`. [`QualifiedName`] already uses the
    /// generated identifier admission set containing those keywords, so one
    /// canonical variant covers both ordinary and keyword-spelled names.
    #[derive(Debug)]
    pub enum FuncDefName {
        Name(QualifiedName),
    }
}

impl<'input> FuncDefName<'input> {
    /// Returns the final (object) name part as text.
    pub fn object(&self) -> &str {
        match self {
            FuncDefName::Name(q) => q.object(),
        }
    }
}

recursa::ast_node! {
    /// `RENAME TO new_name` — the rename action shared by many ALTER
    /// statements. Postgres routes most of these through `RenameStmt`, but
    /// pg-sql keeps one LR production family per leading `ALTER objtype ...`
    /// form, so each `Alter*Stmt` re-models its own rename branch.
    #[derive(Debug)]
    pub struct RenameTo {
        #[tok(RENAME, TO, this)]
        pub new_name: literal::Ident,
    }
}

recursa::ast_node! {
    /// `OWNER TO RoleSpec` — the owner-change action shared by many ALTER
    /// statements. Postgres routes most of these through `AlterOwnerStmt`,
    /// but pg-sql keeps one LR production family per leading
    /// `ALTER objtype ...` form, so each `Alter*Stmt` re-models its own owner
    /// branch.
    #[derive(Debug)]
    pub struct OwnerTo {
        #[tok(OWNER, TO, this)]
        pub new_owner: RoleSpec,
    }
}

recursa::ast_node! {
    /// `SET SCHEMA name` — the set-schema action shared by ALTER FOREIGN
    /// TABLE, ALTER TABLE, ALTER VIEW, ALTER MATERIALIZED VIEW, etc.
    /// Postgres routes most of these through `AlterObjectSchemaStmt`, but
    /// pg-sql keeps one LR production family per leading `ALTER objtype ...`
    /// form, so each `Alter*Stmt` re-models its own set-schema branch.
    #[derive(Debug)]
    pub struct SetSchemaClause {
        #[tok(SET, SCHEMA, this)]
        pub new_schema: literal::Ident,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum OperatorName {
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
        Custom(literal::CustomOp),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum QualifiedOperatorName {
        /// `[schema.]op` — at least one `Ident.` segment followed by an
        /// `OperatorName`.
        Qualified(QualifiedOperatorPath),
        /// Bare operator name with no schema qualifier.
        Plain(OperatorName),
    }
}

recursa::ast_node! {
    /// A schema-qualified operator name: one or more `Ident.` segments followed
    /// by an `OperatorName`.
    #[derive(Debug)]
    pub struct QualifiedOperatorPath {
        pub first: QualifiedOperatorPrefix,
        pub rest: zero_or_many!(QualifiedOperatorPrefix),
        pub name: OperatorName,
    }
}

recursa::ast_node! {
    /// One `Ident.` segment of a qualified operator name's schema prefix.
    #[derive(Debug)]
    pub struct QualifiedOperatorPrefix {
        #[tok(this, DOT)]
        pub name: literal::Ident,
    }
}

recursa::ast_node! {
    /// One side of an `oper_argtypes` pair: a type name, or `NONE` —
    /// PostgreSQL's missing-operand marker on a unary operator.
    ///
    /// `NONE` is a `COL_NAME` keyword, and `Typename`'s `type_function_name`
    /// path admits only `UNRESERVED` and `TYPE_FUNC_NAME` keywords, so
    /// [`TypeName`] does not reach it. gram.y likewise spells
    /// `'(' NONE ',' Typename ')'` as its own `oper_argtypes` alternative rather
    /// than widening the type name.
    ///
    /// Variant ordering: `None` first, because the literal `NONE` keyword is the
    /// specific match and `Type` would otherwise have to reject it.
    #[derive(Debug)]
    pub enum OperatorArgType {
        #[tok(NONE)]
        None,
        Type(TypeName),
    }
}

impl<'input> OperatorArgType<'input> {
    /// Returns the named type, or `None` for the `NONE` marker.
    pub fn type_name(&self) -> Option<&TypeName<'input>> {
        match self {
            OperatorArgType::None => Option::None,
            OperatorArgType::Type(type_name) => Some(type_name),
        }
    }
}

recursa::ast_node! {
    /// `(left, right)` argument-type signature on `operator_with_argtypes` —
    /// Postgres' `oper_argtypes`.
    ///
    /// gram.y gives the unary spellings their own alternatives (`'(' NONE ','
    /// Typename ')'` and `'(' Typename ',' NONE ')'`), which duplicates the same
    /// token language three times. This shared-prefix type parses the pair once
    /// and lets [`OperatorArgType`] carry the `NONE` marker; [`Self::left`] and
    /// [`Self::right`] read it back.
    ///
    /// The shared prefix also admits `(NONE, NONE)`, which gram.y has no
    /// alternative for. PostgreSQL rejects that pair semantically ("an operator
    /// must have at least one operand"), so the over-acceptance costs nothing a
    /// parser can decide.
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct OperatorArgtypes {
        pub left_type: OperatorArgType,
        #[tok(COMMA, this)]
        pub right_type: OperatorArgType,
    }
}

impl<'input> OperatorArgtypes<'input> {
    /// Returns the left operand type, or `None` for PostgreSQL's unary
    /// `(NONE, right)` spelling.
    pub fn left(&self) -> Option<&TypeName<'input>> {
        self.left_type.type_name()
    }

    /// Returns the right operand type, or `None` for PostgreSQL's unary
    /// `(left, NONE)` spelling.
    pub fn right(&self) -> Option<&TypeName<'input>> {
        self.right_type.type_name()
    }
}

recursa::ast_node! {
    /// `any_operator oper_argtypes` — Postgres' `operator_with_argtypes`. The
    /// full reference to a specific operator (including overload signature)
    /// used by `DROP OPERATOR`, `ALTER OPERATOR`, `COMMENT ON OPERATOR`,
    /// `SECURITY LABEL ON OPERATOR`, etc.
    #[derive(Debug)]
    pub struct OperatorWithArgtypes {
        pub name: QualifiedOperatorName,
        pub args: OperatorArgtypes,
    }
}

recursa::ast_node! {
    /// gram.y:13757 `relation_expr: qualified_name | extended_relation_expr`
    /// with gram.y:13771 `extended_relation_expr: qualified_name '*' | ONLY
    /// qualified_name | ONLY '(' qualified_name ')'`: the table a statement
    /// names, with its inheritance marker.
    ///
    /// `ONLY` excludes inheritance children, and a trailing `*` states the
    /// default, that they are included. The two never combine: `ONLY t *` is a
    /// syntax error, which a pair of flags cannot say. Every statement that
    /// gram.y gives a `relation_expr` holds this one node: `LOCK`, `TRUNCATE`,
    /// `ALTER TABLE`, `ALTER FOREIGN TABLE`, `CREATE INDEX`, `UPDATE`,
    /// `DELETE`, `MERGE`, `TABLE` and the publication table list. A `FROM`
    /// item has the same forms, written with its alias and sample in
    /// `crate::ast::dml::select`.
    ///
    /// Variant ordering: `Only` leads with the keyword, `Named` with a name.
    #[derive(Debug)]
    pub enum RelationExpr {
        /// `ONLY name` or `ONLY ( name )`.
        Only(OnlyRelation),
        /// `name` or `name *`.
        Named(InheritedRelation),
    }
}

recursa::ast_node! {
    /// gram.y `extended_relation_expr`'s two `ONLY` forms: `ONLY
    /// qualified_name | ONLY '(' qualified_name ')'`. Neither takes a `*`.
    ///
    /// Variant ordering: the forms part on the `(` after `ONLY`.
    #[derive(Debug)]
    pub enum OnlyRelation {
        Parens(#[tok(ONLY, LPAREN, this, RPAREN)] QualifiedName),
        Plain(#[tok(ONLY, this)] QualifiedName),
    }
}

recursa::ast_node! {
    /// `name [*]`: a relation with its inheritance children, the default.
    #[derive(Debug)]
    pub struct InheritedRelation {
        pub name: QualifiedName,
        #[presence(STAR)]
        #[pretty(break_before = soft)]
        pub star: bool,
    }
}

impl<'input> RelationExpr<'input> {
    /// The relation's name, whichever form wraps it.
    pub fn name(&self) -> &QualifiedName<'input> {
        match self {
            Self::Only(only) => only.name(),
            Self::Named(relation) => &relation.name,
        }
    }

    /// Whether `ONLY` excludes the inheritance children.
    pub fn is_only(&self) -> bool {
        matches!(self, Self::Only(_))
    }
}

impl<'input> OnlyRelation<'input> {
    /// The relation's name, with or without its parentheses.
    pub fn name(&self) -> &QualifiedName<'input> {
        match self {
            Self::Parens(name) | Self::Plain(name) => name,
        }
    }
}
