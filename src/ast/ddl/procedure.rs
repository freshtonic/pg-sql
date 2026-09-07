/// CREATE PROCEDURE / DROP PROCEDURE / CALL statement AST.
use crate::ast::ddl::function::{FuncOption, FunctionParameters, RoutineBody};
use crate::ast::shared::expr::FuncArg;
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

/// `CREATE [OR REPLACE] PROCEDURE name ( [ parameters ] ) options...`
///
/// `name` is a `QualifiedName` (gram.y `CreateFunctionStmt: … PROCEDURE
/// func_name`, where `func_name: type_function_name | ColId indirection`
/// — accepting schema-qualified names like `testns.bar`).
#[derive(recursa::Node, Debug)]
#[tok(CREATE, this)]
pub struct CreateProcedureStmt<'input> {
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    #[tok(PROCEDURE, this)]
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    pub args: FunctionParameters<'input>,
    pub options: recursa::ArenaVec<'input, FuncOption<'input>>,
    /// gram.y `opt_routine_body`, after `opt_createfunc_opt_list`.
    pub body: Option<RoutineBody<'input>>,
}

/// One target of `DROP PROCEDURE`: `name [(args)]`.
#[derive(recursa::Node, Debug)]
pub struct DropProcedureTarget<'input> {
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    pub args: Option<FunctionParameters<'input>>,
}

/// DROP PROCEDURE `name [(args)] [, name [(args)] ...] [CASCADE | RESTRICT]`.
///
/// Per gram.y `RemoveFuncStmt`: the target is a `function_with_argtypes_list`
/// (one or more `name [(args)]` entries separated by commas).
#[derive(recursa::Node, Debug)]
#[tok(DROP, PROCEDURE, this)]
pub struct DropProcedureStmt<'input> {
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    /// gram.y `function_with_argtypes_list`: one or more targets.
    pub targets: recursa::ArenaVec1<'input, DropProcedureTarget<'input>>,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

/// Parenthesized argument list of a `CALL` statement.
#[derive(recursa::Node, Debug, derive_more::Deref)]
#[tok(LPAREN, this, RPAREN)]
pub struct CallArguments<'input>(
    #[sep(COMMA)]
    #[deref]
    pub recursa::ArenaVec<'input, FuncArg<'input>>,
);

/// `CALL name ( [ argument ] [, ...] )`
#[derive(recursa::Node, Debug)]
pub struct CallStmt<'input> {
    #[tok(CALL, this)]
    pub name: crate::tokens::type_function_name<'input>,
    pub args: CallArguments<'input>,
}

// =========================================================================
// ALTER/DROP PROCEDURE — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `ALTER PROCEDURE function_with_argtypes action` — same action shape
/// as [`AlterFunctionStmt`](crate::ast::ddl::function::AlterFunctionStmt);
/// gram.y treats `OBJECT_PROCEDURE` as a tag on
/// the same `AlterFunctionStmt` node and runs the same
/// `alterfunc_opt_list` rule. Semantic analysis (not parsing) rejects
/// option items that don't apply to procedures.
#[derive(recursa::Node, Debug)]
pub struct AlterProcedureStmt<'input> {
    #[tok(ALTER, PROCEDURE, this)]
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}
