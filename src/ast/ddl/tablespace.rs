/// CREATE TABLESPACE / DROP TABLESPACE statement AST.
use crate::ast::ddl::index::{StorageParam, WithStorage};
use crate::tokens::literal;

recursa::ast_node! {
    /// `OWNER role` optional clause.
    #[derive(Debug)]
    pub struct OwnerClause {
        #[tok(OWNER, this)]
        pub role: crate::tokens::NonReservedWord,
    }
}

recursa::ast_node! {
    /// `LOCATION 'path'` clause.
    #[derive(Debug)]
    pub struct LocationClause {
        #[tok(LOCATION, this)]
        pub path: literal::StringLit,
    }
}

recursa::ast_node! {
    /// `CREATE TABLESPACE name [OWNER role] LOCATION 'path' [WITH (params)]`
    #[derive(Debug)]
    pub struct CreateTablespaceStmt {
        #[tok(CREATE, TABLESPACE, this)]
        pub name: crate::tokens::ColId,
        pub owner: Option<OwnerClause>,
        pub location: LocationClause,
        pub with_options: Option<WithStorage>,
    }
}

recursa::ast_node! {
    /// `RENAME TO new_name` action on ALTER TABLESPACE.
    #[derive(Debug)]
    pub struct AlterTablespaceRename {
        #[tok(RENAME, TO, this)]
        pub new_name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `OWNER TO new_owner` action on ALTER TABLESPACE.
    #[derive(Debug)]
    pub struct AlterTablespaceOwner {
        #[tok(OWNER, TO, this)]
        pub new_owner: crate::tokens::NonReservedWord,
    }
}

recursa::ast_node! {
    /// `SET (param = value, ...)` action on ALTER TABLESPACE.
    #[derive(Debug)]
    #[tok(SET, LPAREN, this, RPAREN)]
    pub struct AlterTablespaceSetAction {
        #[sep(COMMA)]
        pub params: zero_or_many!(StorageParam),
    }
}

recursa::ast_node! {
    /// `RESET (param [= value] [, ...])` action on ALTER TABLESPACE.
    ///
    /// Postgres accepts the same `reloptions` payload here as for `SET`, even
    /// though the `= value` half is ignored: `gram.y`'s `AlterTblSpcStmt` uses
    /// the `reloptions` rule for both branches.
    #[derive(Debug)]
    #[tok(RESET, LPAREN, this, RPAREN)]
    pub struct AlterTablespaceResetAction {
        #[sep(COMMA)]
        pub params: zero_or_many!(StorageParam),
    }
}

recursa::ast_node! {
    /// One of the supported ALTER TABLESPACE actions.
    ///
    /// Variant ordering: all variants start with distinct keywords (SET, RESET,
    /// RENAME, OWNER), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterTablespaceAction {
        Set(AlterTablespaceSetAction),
        Reset(AlterTablespaceResetAction),
        Rename(AlterTablespaceRename),
        Owner(AlterTablespaceOwner),
    }
}

recursa::ast_node! {
    /// `ALTER TABLESPACE name { RENAME TO new_name | OWNER TO new_owner
    ///                         | SET (params) | RESET (params) }`
    #[derive(Debug)]
    pub struct AlterTablespaceStmt {
        #[tok(ALTER, TABLESPACE, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterTablespaceAction,
    }
}

recursa::ast_node! {
    /// `DROP TABLESPACE [IF EXISTS] name`
    #[derive(Debug)]
    #[tok(DROP, TABLESPACE, this)]
    pub struct DropTablespaceStmt {
        #[presence(IF, EXISTS)]
        pub if_exists: bool,
        pub name: crate::tokens::ColId,
    }
}
