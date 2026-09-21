/// CREATE PROCEDURE / DROP PROCEDURE / CALL statement AST.
use crate::ast::ddl::function::{FuncOption, FunctionParameters, RoutineBody};
// ---------------------------------------------------------------------------
// Additional imports for the ALTER/DROP types appended to this file as part
// of the DDL physical-extraction migration. Glob imports keep cross-batch
// type references resolvable regardless of migration order; a polish pass
// will tighten these once the migration completes.
use crate::ast::ddl::function::AlterFuncAction;
#[allow(unused_imports)]
use crate::ast::shared::expr::*;
#[allow(unused_imports)]
use crate::ast::shared::flags::*;
#[allow(unused_imports)]
use crate::ast::shared::names::*;
#[allow(unused_imports)]
use crate::ast::shared::numbers::*;
// ---------------------------------------------------------------------------

recursa::ast_node! {
    /// `CREATE [OR REPLACE] PROCEDURE name ( [ parameters ] ) options...`
    ///
    /// `name` is a `QualifiedName` (gram.y `CreateFunctionStmt: … PROCEDURE
    /// func_name`, where `func_name: type_function_name | ColId indirection`
    /// — accepting schema-qualified names like `testns.bar`).
    #[derive(Debug)]
    #[tok(CREATE, this)]
    pub struct CreateProcedureStmt {
        #[presence(OR, REPLACE)]
        pub or_replace: bool,
        #[tok(PROCEDURE, this)]
        pub name: crate::ast::shared::names::QualifiedName,
        pub args: FunctionParameters,
        pub options: zero_or_many!(FuncOption),
        /// gram.y `opt_routine_body`, after `opt_createfunc_opt_list`.
        pub body: Option<RoutineBody>,
    }
}

recursa::ast_node! {
    /// One target of `DROP PROCEDURE`: `name [(args)]`.
    #[derive(Debug)]
    pub struct DropProcedureTarget {
        pub name: crate::ast::shared::names::QualifiedName,
        pub args: Option<FunctionParameters>,
    }
}

recursa::ast_node! {
    /// DROP PROCEDURE `name [(args)] [, name [(args)] ...] [CASCADE | RESTRICT]`.
    ///
    /// Per gram.y `RemoveFuncStmt`: the target is a `function_with_argtypes_list`
    /// (one or more `name [(args)]` entries separated by commas).
    #[derive(Debug)]
    #[tok(DROP, PROCEDURE, this)]
    pub struct DropProcedureStmt {
        #[presence(IF, EXISTS)]
        pub if_exists: bool,
        #[sep(COMMA)]
        /// gram.y `function_with_argtypes_list`: one or more targets.
        pub targets: one_or_many!(DropProcedureTarget),
        pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
    }
}

recursa::ast_node! {
    /// gram.y:1159 `CallStmt: CALL func_application`.
    ///
    /// `func_application` is the production a function call uses, so the name
    /// is a `func_name`, which may be schema-qualified (`CALL s.p(1)`), and
    /// the arguments are a function call's: named, `VARIADIC`, and the rest.
    /// It is the node a function table in `FROM` holds.
    #[derive(Debug)]
    pub struct CallStmt {
        #[tok(CALL, this)]
        pub call: crate::ast::shared::expr::FunctionApplicationExpr,
    }
}

// =========================================================================
// ALTER/DROP PROCEDURE — appended from simple_stmts.rs during physical extraction.
// =========================================================================

recursa::ast_node! {
    /// `ALTER PROCEDURE function_with_argtypes action` — same action shape
    /// as [`AlterFunctionStmt`](crate::ast::ddl::function::AlterFunctionStmt);
    /// gram.y treats `OBJECT_PROCEDURE` as a tag on
    /// the same `AlterFunctionStmt` node and runs the same
    /// `alterfunc_opt_list` rule. Semantic analysis (not parsing) rejects
    /// option items that don't apply to procedures.
    #[derive(Debug)]
    pub struct AlterProcedureStmt {
        #[tok(ALTER, PROCEDURE, this)]
        pub target: crate::ast::ddl::function::DropFunctionTarget,
        pub action: AlterFuncAction,
    }
}
