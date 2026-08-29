/// CREATE TABLESPACE / DROP TABLESPACE statement AST.
use recursa::seq::Seq0;

use crate::ast::ddl::index::{StorageParam, WithStorage};
use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `OWNER role` optional clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct OwnerClause<'input> {
    #[tok(OWNER, this)]
    pub role: crate::tokens::NonReservedWord<'input>,
}

/// `LOCATION 'path'` clause.
#[derive(recursa::Node, Debug, Clone)]
pub struct LocationClause<'input> {
    #[tok(LOCATION, this)]
    pub path: literal::StringLit<'input>,
}

/// `CREATE TABLESPACE name [OWNER role] LOCATION 'path' [WITH (params)]`
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateTablespaceStmt<'input> {
    #[tok(CREATE, TABLESPACE, this)]
    pub name: crate::tokens::ColId<'input>,
    pub owner: Option<OwnerClause<'input>>,
    pub location: LocationClause<'input>,
    pub with_options: Option<WithStorage<'input>>,
}

/// `RENAME TO new_name` action on ALTER TABLESPACE.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTablespaceRename<'input> {
    #[tok(RENAME, TO, this)]
    pub new_name: crate::tokens::ColId<'input>,
}

/// `OWNER TO new_owner` action on ALTER TABLESPACE.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTablespaceOwner<'input> {
    #[tok(OWNER, TO, this)]
    pub new_owner: crate::tokens::NonReservedWord<'input>,
}

/// `SET (param = value, ...)` action on ALTER TABLESPACE.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTablespaceSetAction<'input> {
    #[tok(SET, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params:  Vec<StorageParam<'input> > ,
}

/// `RESET (param [= value] [, ...])` action on ALTER TABLESPACE.
///
/// Postgres accepts the same `reloptions` payload here as for `SET`, even
/// though the `= value` half is ignored: `gram.y`'s `AlterTblSpcStmt` uses
/// the `reloptions` rule for both branches.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTablespaceResetAction<'input> {
    #[tok(RESET, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub params:  Vec<StorageParam<'input> > ,
}

/// One of the supported ALTER TABLESPACE actions.
///
/// Variant ordering: all variants start with distinct keywords (SET, RESET,
/// RENAME, OWNER), so order is for clarity only.
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterTablespaceAction<'input> {
    Set(AlterTablespaceSetAction<'input>),
    Reset(AlterTablespaceResetAction<'input>),
    Rename(AlterTablespaceRename<'input>),
    Owner(AlterTablespaceOwner<'input>),
}

/// `ALTER TABLESPACE name { RENAME TO new_name | OWNER TO new_owner
///                         | SET (params) | RESET (params) }`
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterTablespaceStmt<'input> {
    #[tok(ALTER, TABLESPACE, this)]
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterTablespaceAction<'input>,
}

/// `DROP TABLESPACE [IF EXISTS] name`
#[derive(recursa::Node, Debug, Clone)]
pub struct DropTablespaceStmt<'input> {
    #[tok(DROP, TABLESPACE, this)]
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    pub name: crate::tokens::ColId<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_tablespace_basic() {
        let lexed = crate::tokens::lex("CREATE TABLESPACE ts1 LOCATION '/tmp'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_tablespace_with_options() {
        let lexed = crate::tokens::lex("CREATE TABLESPACE ts1 LOCATION '' WITH (random_page_cost = 3.0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_create_tablespace_owner() {
        let lexed = crate::tokens::lex("CREATE TABLESPACE ts1 OWNER foo LOCATION '/tmp'");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_tablespace() {
        let lexed = crate::tokens::lex("DROP TABLESPACE ts1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_tablespace_if_exists() {
        let lexed = crate::tokens::lex("DROP TABLESPACE IF EXISTS ts1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_set() {
        let lexed = crate::tokens::lex("ALTER TABLESPACE ts SET (random_page_cost = 1.0)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_reset() {
        let lexed = crate::tokens::lex("ALTER TABLESPACE ts RESET (random_page_cost, effective_io_concurrency)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_rename() {
        let lexed = crate::tokens::lex("ALTER TABLESPACE ts RENAME TO ts2");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_alter_tablespace_owner() {
        let lexed = crate::tokens::lex("ALTER TABLESPACE ts OWNER TO foo");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterTablespaceStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }
}
