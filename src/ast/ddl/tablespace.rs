/// CREATE TABLESPACE / DROP TABLESPACE statement AST.
use crate::ast::ddl::index::{StorageParam, WithStorage};
use crate::tokens::literal;

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
#[tok(SET, LPAREN, this, RPAREN)]
pub struct AlterTablespaceSetAction<'input> {
    #[sep(COMMA)]
    pub params: Vec<StorageParam<'input>>,
}

/// `RESET (param [= value] [, ...])` action on ALTER TABLESPACE.
///
/// Postgres accepts the same `reloptions` payload here as for `SET`, even
/// though the `= value` half is ignored: `gram.y`'s `AlterTblSpcStmt` uses
/// the `reloptions` rule for both branches.
#[derive(recursa::Node, Debug, Clone)]
#[tok(RESET, LPAREN, this, RPAREN)]
pub struct AlterTablespaceResetAction<'input> {
    #[sep(COMMA)]
    pub params: Vec<StorageParam<'input>>,
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
