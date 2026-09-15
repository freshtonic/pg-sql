//! TYPE  DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::publication::SetDefinitionClause;
use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// A single column in `CREATE TYPE name AS (col_list)` — Postgres'
    /// `TableFuncElement`: `ColId Typename [COLLATE name]`.
    #[derive(Debug)]
    pub struct CompositeTypeColumn {
        pub name: crate::tokens::ColId,
        pub type_name: CastType,
        pub collate: Option<CompositeTypeCollate>,
    }
}

recursa::ast_node! {
    /// `COLLATE name` clause on a composite-type column.
    #[derive(Debug)]
    pub struct CompositeTypeCollate {
        #[tok(COLLATE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `AS (col_list)` — composite-type definition body.
    #[derive(Debug)]
    #[tok(AS, LPAREN, this, RPAREN)]
    pub struct CreateTypeComposite {
        #[sep(COMMA)]
        pub columns: zero_or_many!(CompositeTypeColumn),
    }
}

recursa::ast_node! {
    /// `AS ENUM ('label', ...)` — enum-type definition body. The label list may
    /// be empty (Postgres allows `AS ENUM ()` to create a shell-only enum).
    #[derive(Debug)]
    #[tok(AS, ENUM, LPAREN, this, RPAREN)]
    pub struct CreateTypeEnum {
        #[sep(COMMA)]
        pub labels: zero_or_many!(literal::StringLit),
    }
}

recursa::ast_node! {
    /// `AS RANGE (def_list)` — range-type definition body.
    #[derive(Debug)]
    pub struct CreateTypeRange {
        #[tok(AS, RANGE, this)]
        pub definition: DefList,
    }
}

recursa::ast_node! {
    /// The body of a `CREATE TYPE name ‹body›` statement.
    ///
    /// Variant ordering: multi-keyword forms (`AS ENUM`, `AS RANGE`) before
    /// `Composite` (`AS` + paren list) so the longer match wins. `Base` is the
    /// `(def_list)` form (no `AS`); it begins with `(` and so cannot collide
    /// with the `AS …` variants.
    #[derive(Debug)]
    pub enum CreateTypeBody {
        Enum(CreateTypeEnum),
        Range(CreateTypeRange),
        Composite(CreateTypeComposite),
        Base(DefList),
    }
}

recursa::ast_node! {
    /// `CREATE TYPE name [body]`.
    ///
    /// - `CREATE TYPE name` — shell type
    /// - `CREATE TYPE name AS (col_list)` — composite
    /// - `CREATE TYPE name AS ENUM (labels)` — enum
    /// - `CREATE TYPE name AS RANGE (def_list)` — range
    /// - `CREATE TYPE name (def_list)` — base type
    #[derive(Debug)]
    pub struct CreateTypeStmt {
        #[tok(CREATE, TYPE, this)]
        pub name: QualifiedName,
        pub body: Option<CreateTypeBody>,
    }
}

recursa::ast_node! {
    /// `DROP TYPE [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, TYPE, this)]
    pub struct DropTypeStmt {
        pub if_exists: Option<IfExists>,
        pub types: TypeNameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `RENAME ATTRIBUTE old TO new [CASCADE | RESTRICT]` — Postgres'
    /// `RenameStmt` branch for composite-type attribute renames.
    #[derive(Debug)]
    pub struct AlterTypeRenameAttribute {
        #[tok(RENAME, ATTRIBUTE, this)]
        pub old_name: crate::tokens::ColId,
        #[tok(TO, this)]
        pub new_name: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `RENAME VALUE old_value TO new_value` — Postgres' `AlterEnumStmt`
    /// branch for renaming enum values. Both literals are string literals.
    #[derive(Debug)]
    pub struct AlterTypeRenameValue {
        #[tok(RENAME, VALUE, this)]
        pub old_value: literal::StringLit,
        #[tok(TO, this)]
        pub new_value: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `BEFORE 'value'` or `AFTER 'value'` — neighbor anchor on
    /// `ALTER TYPE name ADD VALUE`.
    #[derive(Debug)]
    pub enum AlterEnumValuePosition {
        Before(AlterEnumValueBefore),
        After(AlterEnumValueAfter),
    }
}

recursa::ast_node! {
    /// `BEFORE 'neighbor'` — neighbour anchor on
    /// `ALTER TYPE name ADD VALUE ... BEFORE 'neighbor'`.
    #[derive(Debug)]
    pub struct AlterEnumValueBefore {
        #[tok(BEFORE, this)]
        pub neighbor: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `AFTER 'neighbor'` — neighbour anchor on
    /// `ALTER TYPE name ADD VALUE ... AFTER 'neighbor'`.
    #[derive(Debug)]
    pub struct AlterEnumValueAfter {
        #[tok(AFTER, this)]
        pub neighbor: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `ADD VALUE [IF NOT EXISTS] 'val' [{BEFORE|AFTER} 'neighbour']` —
    /// Postgres' `AlterEnumStmt` ADD VALUE branch.
    #[derive(Debug)]
    #[tok(ADD, VALUE, this)]
    pub struct AlterTypeAddValue {
        pub if_not_exists: Option<IfNotExists>,
        pub new_value: literal::StringLit,
        pub position: Option<AlterEnumValuePosition>,
    }
}

recursa::ast_node! {
    /// `ADD ATTRIBUTE column_def [CASCADE | RESTRICT]` — one `alter_type_cmd`
    /// in Postgres. `column_def` is modelled as the same `CompositeTypeColumn`
    /// used by `CREATE TYPE name AS (...)` (Postgres' `TableFuncElement`):
    /// `name typename [COLLATE qualified_name]`.
    #[derive(Debug)]
    pub struct AlterTypeAddAttribute {
        #[tok(ADD, ATTRIBUTE, this)]
        pub column: CompositeTypeColumn,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `DROP ATTRIBUTE [IF EXISTS] name [CASCADE | RESTRICT]` — one
    /// `alter_type_cmd` in Postgres.
    #[derive(Debug)]
    #[tok(DROP, ATTRIBUTE, this)]
    pub struct AlterTypeDropAttribute {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `[SET DATA]` modifier preceding `TYPE` in
    /// `ALTER ATTRIBUTE name [SET DATA] TYPE typename`. Postgres'
    /// `opt_set_data`.
    #[derive(Debug)]
    pub enum SetDataClause {
        #[tok(SET, DATA)]
        Value,
    }
}

recursa::ast_node! {
    /// `ALTER ATTRIBUTE name [SET DATA] TYPE typename [COLLATE qual] [CASCADE
    /// | RESTRICT]` — one `alter_type_cmd` in Postgres. The typename uses the
    /// same `CastType` enum as `CREATE TYPE name AS (col_list)` column types.
    /// The optional `COLLATE` clause reuses [`CompositeTypeCollate`] (Postgres'
    /// `opt_collate_clause` — `COLLATE any_name`).
    #[derive(Debug)]
    pub struct AlterTypeAlterAttribute {
        #[tok(ALTER, ATTRIBUTE, this)]
        pub name: crate::tokens::ColId,
        pub set_data: Option<SetDataClause>,
        #[tok(TYPE, this)]
        pub type_name: CastType,
        pub collate: Option<CompositeTypeCollate>,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// One `alter_type_cmd` in Postgres — an `ADD ATTRIBUTE`, `DROP ATTRIBUTE`,
    /// or `ALTER ATTRIBUTE` action on `ALTER TYPE name action [, action ...]`.
    ///
    /// Variant ordering: each variant has a distinct leading keyword (`ADD`,
    /// `DROP`, `ALTER`) followed by `ATTRIBUTE`. Order is for clarity.
    #[derive(Debug)]
    pub enum AlterTypeCmd {
        AddAttribute(AlterTypeAddAttribute),
        DropAttribute(AlterTypeDropAttribute),
        AlterAttribute(AlterTypeAlterAttribute),
    }
}

recursa::ast_node! {
    /// One or more comma-separated `alter_type_cmd`s — Postgres'
    /// `alter_type_cmds` non-terminal.
    #[derive(Debug)]
    pub struct AlterTypeCmdList {
        #[sep(COMMA)]
        pub cmds: one_or_many!(AlterTypeCmd),
    }
}

recursa::ast_node! {
    /// One action on `ALTER TYPE any_name action` — covers Postgres'
    /// `RenameStmt` (RENAME TO, RENAME ATTRIBUTE), `AlterOwnerStmt`
    /// (OWNER TO), `AlterObjectSchemaStmt` (SET SCHEMA), `AlterTypeStmt`
    /// (SET (...)), `AlterEnumStmt` (ADD VALUE, RENAME VALUE), and
    /// `alter_type_cmds` (ADD/DROP/ALTER ATTRIBUTE, comma-separated).
    ///
    /// Variant ordering: variants with two-keyword prefixes go before
    /// single-keyword variants that share the same first token.
    /// `RENAME ATTRIBUTE` / `RENAME VALUE` (two tokens) before `RENAME TO`
    /// (also two tokens — distinct second). `SET SCHEMA` / `SET (` (the
    /// def-list form starts with `SET LPAREN`) — distinct second tokens.
    /// `ADD VALUE` (two tokens) before `Cmds` (which can start with `ADD
    /// ATTRIBUTE`).
    #[derive(Debug)]
    pub enum AlterTypeAction {
        RenameAttribute(AlterTypeRenameAttribute),
        RenameValue(AlterTypeRenameValue),
        Rename(RenameTo),
        Owner(OwnerTo),
        SetSchema(SetSchemaClause),
        SetDef(SetDefinitionClause),
        AddValue(AlterTypeAddValue),
        Cmds(AlterTypeCmdList),
    }
}

recursa::ast_node! {
    /// `ALTER TYPE any_name action` — Postgres' `AlterTypeStmt` /
    /// `AlterEnumStmt` / `RenameStmt` / `AlterOwnerStmt` /
    /// `AlterObjectSchemaStmt` branches for types, plus the composite-type
    /// `alter_type_cmds` set (ADD/DROP/ALTER ATTRIBUTE, comma-separated).
    #[derive(Debug)]
    pub struct AlterTypeStmt {
        #[tok(ALTER, TYPE, this)]
        pub name: QualifiedName,
        pub action: AlterTypeAction,
    }
}
