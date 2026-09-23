//! PREPARE / EXECUTE / DEALLOCATE statements (including the two-phase-commit
//! `PREPARE TRANSACTION 'gid'` form).

use crate::ast::shared::expr::Expr;
use crate::ast::shared::names::TypeNameList;
use crate::tokens::literal;

// --- PREPARE / EXECUTE / DEALLOCATE ---

recursa::ast_node! {
    /// A statement that can be the body of `PREPARE name AS ...` — Postgres'
    /// `PreparableStmt`: `SELECT | INSERT | UPDATE | DELETE | MERGE`.
    ///
    /// `Query` uses `Subquery`, which already models Postgres' full `SelectStmt`
    /// grammar (`SELECT`, set operations, `VALUES`, `TABLE`, and `WITH`). The
    /// other four variants have disjoint leading keywords, so variant order does
    /// not affect disambiguation.
    #[derive(Debug)]
    pub enum PreparableStmt {
        Query(boxed!(crate::ast::dml::values::QueryBody)),
        /// `WITH ...` before a query or a DML statement, factored as in `Statement`.
        With(boxed!(crate::ast::shared::with_clause::WithStatement)),
        Insert(boxed!(crate::ast::dml::insert::InsertStmt)),
        Update(boxed!(crate::ast::dml::update::UpdateStmt)),
        Delete(boxed!(crate::ast::dml::delete::DeleteStmt)),
        /// Added in 15: research, PostgreSQL 15, "New statements" (REL_15_19 gram.y
        /// `PreparableStmt`, `ExplainableStmt`, `MergeStmt: opt_with_clause MERGE`).
        #[config(since = pg15)]
        Merge(boxed!(crate::ast::dml::merge::MergeStmt)),
    }
}

recursa::ast_node! {
    /// `( typename [, ...] )` parameter-type list on a `PREPARE` statement
    /// (`prep_type_clause` in `gram.y`).
    #[derive(Debug)]
    pub struct PrepareTypes {
        #[tok(LPAREN, this, RPAREN)]
        pub types: TypeNameList,
    }
}

recursa::ast_node! {
    /// Body of a standard `PREPARE name [(types)] AS stmt` statement.
    #[derive(Debug)]
    pub struct PrepareStandardBody {
        pub name: literal::AliasName,
        pub types: Option<PrepareTypes>,
        #[tok(AS, this)]
        pub body: PreparableStmt,
    }
}

recursa::ast_node! {
    /// `PREPARE TRANSACTION 'gid'` — the two-phase-commit transaction-prepare
    /// form (`gram.y::TransactionStmt: PREPARE TRANSACTION Sconst`). Distinct
    /// from the `PREPARE name … AS stmt` form modelled by
    /// [`PrepareStandardBody`].
    #[derive(Debug)]
    pub struct PrepareTransactionBody {
        #[tok(TRANSACTION, this)]
        pub gid: literal::StringLit,
    }
}

recursa::ast_node! {
    /// Body of a `PREPARE` statement.
    ///
    /// `Transaction` matches the dedicated `TRANSACTION` form. `Standard` accepts
    /// an ordinary prepared-statement name; the grammar keeps these surface forms
    /// separate so the LR table can distinguish the following string from `AS` or
    /// a type list.
    #[derive(Debug)]
    pub enum PrepareStmtBody {
        Transaction(PrepareTransactionBody),
        Standard(PrepareStandardBody),
    }
}

recursa::ast_node! {
    /// ```sql
    /// PREPARE name [ (typename [, ...]) ] AS PreparableStmt
    /// PREPARE TRANSACTION 'gid'
    /// ```
    #[derive(Debug)]
    pub struct PrepareStmt {
        #[tok(PREPARE, this)]
        pub body: PrepareStmtBody,
    }
}

recursa::ast_node! {
    /// `( expr [, ...] )` argument list on an `EXECUTE` statement
    /// (`execute_param_clause` in `gram.y`).
    #[derive(Debug)]
    #[tok(LPAREN, this, RPAREN)]
    pub struct ExecuteParams {
        #[sep(COMMA)]
        pub params: zero_or_many!(Expr),
    }
}

recursa::ast_node! {
    /// ```sql
    /// EXECUTE name [ (expr [, ...]) ]
    /// ```
    #[derive(Debug)]
    pub struct ExecuteStmt {
        #[tok(EXECUTE, this)]
        pub name: literal::AliasName,
        pub params: Option<ExecuteParams>,
    }
}

recursa::ast_node! {
    /// Target of a `DEALLOCATE` statement: a named prepared statement or `ALL`.
    ///
    /// Variant ordering: `All` (the `ALL` keyword) before `Name` so the reserved
    /// word is not swallowed as a statement name.
    #[derive(Debug)]
    pub enum DeallocateTarget {
        #[tok(ALL)]
        All,
        Name(literal::Ident),
    }
}

recursa::ast_node! {
    /// ```sql
    /// DEALLOCATE [PREPARE] { name | ALL }
    /// ```
    #[derive(Debug)]
    #[tok(DEALLOCATE, this)]
    pub struct DeallocateStmt {
        #[presence(PREPARE)]
        pub prepare: bool,
        pub target: DeallocateTarget,
    }
}
