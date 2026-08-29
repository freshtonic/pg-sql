/// CREATE PROCEDURE / DROP PROCEDURE / CALL statement AST.
use recursa::seq::{OptionalTrailing, Seq0};

use crate::ast::ddl::function::{FuncOption, FuncParam};
use crate::ast::shared::expr::FuncArg;
use crate::tokens::keyword::*;
use crate::tokens::punct;
use recursa_diagram::railroad;
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
#[allow(unused_imports)]
use crate::tokens::soft_keyword::*;
// ---------------------------------------------------------------------------

/// CREATE [OR REPLACE] PROCEDURE name ( [ parameters ] ) options...
///
/// `name` is a `QualifiedName` (gram.y `CreateFunctionStmt: … PROCEDURE
/// func_name`, where `func_name: type_function_name | ColId indirection`
/// — accepting schema-qualified names like `testns.bar`).
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateProcedureStmt<'input> {
    #[tok(CREATE, this, PROCEDURE)]
    #[presence(OR, REPLACE)]
    pub or_replace: bool,
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:  Vec<FuncParam<'input> > ,
    pub options: Vec<FuncOption<'input>  >,
}

/// One target of `DROP PROCEDURE`: `name [(args)]`.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropProcedureTarget<'input> {
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:
        Option< Vec<FuncParam<'input> > >,
}

/// DROP PROCEDURE `name [(args)] [, name [(args)] ...] [CASCADE | RESTRICT]`.
///
/// Per gram.y `RemoveFuncStmt`: the target is a `function_with_argtypes_list`
/// (one or more `name [(args)]` entries separated by commas).
#[derive(recursa::Node, Debug, Clone)]
pub struct DropProcedureStmt<'input> {
    #[tok(DROP, PROCEDURE, this)]
    #[presence(IF, EXISTS)]
    pub if_exists: bool,
    #[sep(COMMA)]
    pub targets: Vec<DropProcedureTarget<'input> >,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

/// CALL name ( [ argument ] [, ...] )
#[derive(recursa::Node, Debug, Clone)]
pub struct CallStmt<'input> {
    #[tok(CALL, this)]
    pub name: crate::tokens::type_function_name<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:  Vec<FuncArg<'input> > ,
}

#[cfg(test)]
mod tests {
    use crate::ast::test_support::*;
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_procedure_basic() {
        let lexed = crate::tokens::lex("CREATE PROCEDURE ptest1(x text) LANGUAGE SQL AS $$ INSERT INTO cp_test VALUES (1, x); $$");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CreateProcedureStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_basic() {
        let lexed = crate::tokens::lex("CALL ptest1('a')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CallStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_concat_arg() {
        let lexed = crate::tokens::lex("CALL ptest1('xy' || 'zzy')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CallStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_call_no_args() {
        let lexed = crate::tokens::lex("CALL nonexistent()");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = CallStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    #[test]
    fn parse_drop_procedure() {
        let lexed = crate::tokens::lex("DROP PROCEDURE ptest1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = DropProcedureStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// CREATE PROCEDURE with a schema-qualified name (gram.y
    /// `CreateFunctionStmt: … PROCEDURE func_name`, where `func_name` is
    /// `type_function_name` accepting `schema.name`). privileges.sql
    /// corpus uses `CREATE PROCEDURE testns.bar()`.
    #[test]
    fn parse_create_procedure_qualified_name() {
        let lexed = crate::tokens::lex("CREATE PROCEDURE testns.bar() AS 'select 1' LANGUAGE sql");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = CreateProcedureStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.object(), "bar");
        assert!(input.is_eof());
    }
    #[test]
    fn alter_procedure_strict() {
        let stmt: AlterProcedureStmt = parse_stmt("ALTER PROCEDURE ptest1(text) STRICT");
        assert!(matches!(stmt.action, AlterFuncAction::Options(_)));
        reparse_stable::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) STRICT");
    }

    #[test]
    fn alter_procedure_rename() {
        reparse_stable::<AlterProcedureStmt>("ALTER PROCEDURE ptest1(text) RENAME TO ptest1a");
    }
}

// =========================================================================
// ALTER/DROP PROCEDURE — appended from simple_stmts.rs during physical extraction.
// =========================================================================

/// `ALTER PROCEDURE function_with_argtypes action` — same action shape
/// as [`AlterFunctionStmt`]; gram.y treats `OBJECT_PROCEDURE` as a tag on
/// the same `AlterFunctionStmt` node and runs the same
/// `alterfunc_opt_list` rule. Semantic analysis (not parsing) rejects
/// option items that don't apply to procedures.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterProcedureStmt<'input> {
    #[tok(ALTER, PROCEDURE, this)]
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}
