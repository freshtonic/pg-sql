//! COLLATION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

use crate::ast::ddl::role::DefList;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// Body of `CREATE COLLATION` after the name: either a `def_list` of options
/// (`LOCALE`/`LC_COLLATE`/`PROVIDER`/...), or `FROM existing_collation_name`.
///
/// Variant ordering: `From` (keyword-led) before `Options` (paren-led) — they
/// begin with different tokens so peek disambiguation is unambiguous.
#[derive(recursa::Node, Debug, Clone)]
pub enum CreateCollationBody<'input> {
    From(CollationFromClause<'input>),
    Options(DefList<'input>),
}

/// `FROM existing_collation_name` — copy an existing collation.
#[derive(recursa::Node, Debug, Clone)]
pub struct CollationFromClause<'input> {
    #[tok(FROM, this)]
    pub name: QualifiedName<'input>,
}

#[derive(recursa::Node, Debug, Clone)]
pub struct CreateCollationStmt<'input> {
    #[tok(CREATE, COLLATION, this)]
    pub if_not_exists: Option<IfNotExists>,
    pub name: QualifiedName<'input>,
    pub body: CreateCollationBody<'input>,
}

/// `DROP COLLATION [IF EXISTS] name [, ...] [CASCADE | RESTRICT]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropCollationStmt<'input> {
    #[tok(DROP, COLLATION, this)]
    pub if_exists: Option<IfExists>,
    pub names: NameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `REFRESH VERSION` — Postgres' `AlterCollationStmt` action.
#[derive(recursa::Node, Debug, Clone)]
pub enum CollationRefreshVersion { #[tok(REFRESH, VERSION)] Value, }

/// One action on `ALTER COLLATION any_name action` — Postgres'
/// `RenameStmt`, `AlterOwnerStmt`, `AlterObjectSchemaStmt`, and
/// `AlterCollationStmt` branches for collations.
///
/// Variant ordering: each variant has a distinct leading keyword
/// (`RENAME`, `OWNER`, `SET`, `REFRESH`), so order is for clarity.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterCollationAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    SetSchema(SetSchemaClause<'input>),
    RefreshVersion(CollationRefreshVersion),
}

/// `ALTER COLLATION any_name action` — Postgres' `AlterCollationStmt`
/// (REFRESH VERSION) plus the collation branches of `RenameStmt` /
/// `AlterOwnerStmt` / `AlterObjectSchemaStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterCollationStmt<'input> {
    #[tok(ALTER, COLLATION, this)]
    pub name: QualifiedName<'input>,
    pub action: AlterCollationAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_alter_collation_rename() {
        let lexed = crate::tokens::lex("ALTER COLLATION test1 RENAME TO test11");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_collation_refresh_version() {
        let lexed = crate::tokens::lex("ALTER COLLATION en_us REFRESH VERSION");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_collation_def_list() {
        let lexed = crate::tokens::lex("CREATE COLLATION mycoll (LC_COLLATE = \"POSIX\", LC_CTYPE = \"POSIX\")");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCollationStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "mycoll");
        assert!(matches!(stmt.body, CreateCollationBody::Options(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_collation_from() {
        let lexed = crate::tokens::lex("CREATE COLLATION mycoll FROM \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt.body, CreateCollationBody::From(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_collation_if_not_exists() {
        let lexed = crate::tokens::lex("CREATE COLLATION IF NOT EXISTS mycoll FROM \"C\"");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateCollationStmt::parse(&mut input).unwrap().into_ast();
        assert!(stmt.if_not_exists.is_some());
        assert!(input.is_eof());
    }
}
