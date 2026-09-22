//! REPACK statement, added in 19: gram.y `RepackStmt` (b73d13c:12080;
//! research PostgreSQL 19, "New statements", commits ac58465e0, 28d534e2a and
//! f23de46e15b). The whole module has a `since-pg19` gate.
//!
//! ```sql
//! REPACK [(options)] qualified_name [(col [, ...])] [USING INDEX [name]]
//! REPACK [(options)] [USING INDEX]
//! ```

use crate::ast::shared::names::QualifiedName;
use crate::ast::utility::explain::UtilityOptionList;
use crate::ast::utility::vacuum::VacuumColumnList;

recursa::ast_node! {
    /// `USING INDEX [name]` after a table: gram.y `USING INDEX name` and
    /// `opt_usingindex` (b73d13c:1148).
    #[derive(Debug)]
    #[tok(USING, INDEX, this)]
    pub struct RepackUsingIndex {
        pub index: Option<crate::tokens::ColId>,
    }
}

recursa::ast_node! {
    /// `qualified_name opt_name_list [USING INDEX [name]]`. The table is a
    /// `qualified_name`, not a `relation_expr`, so `ONLY` and `*` are syntax
    /// errors (f23de46e15b).
    #[derive(Debug)]
    pub struct RepackTable {
        pub table: QualifiedName,
        pub columns: Option<VacuumColumnList>,
        pub using_index: Option<RepackUsingIndex>,
    }
}

recursa::ast_node! {
    /// What follows the options: a table, or `USING INDEX` with no table and
    /// no index name (the third arm of `RepackStmt`).
    #[derive(Debug)]
    pub enum RepackTarget {
        Table(RepackTable),
        #[tok(USING, INDEX)]
        UsingIndex,
    }
}

recursa::ast_node! {
    /// gram.y `RepackStmt` (b73d13c:12080), the `REPACK` arms. `CLUSTER`
    /// keeps its own node: 19 builds a `RepackStmt` for it, with the same
    /// syntax as 18.
    #[derive(Debug)]
    #[tok(REPACK, this)]
    pub struct RepackStmt {
        pub options: Option<UtilityOptionList>,
        pub target: Option<RepackTarget>,
    }
}
