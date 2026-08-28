/// CREATE PROCEDURE / DROP PROCEDURE / CALL statement AST.
use recursa::seq::{OptionalTrailing, Seq0};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateProcedureStmt<'input> {
    pub create: CREATE,
    pub or_replace: Option<(OR, REPLACE)>,
    pub procedure: PROCEDURE,
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    pub args: Surrounded<punct::LParen, Seq0<FuncParam<'input>, punct::Comma>, punct::RParen>,
    pub options: Seq0<FuncOption<'input>, (), OptionalTrailing>,
}

/// One target of `DROP PROCEDURE`: `name [(args)]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DropProcedureTarget<'input> {
    pub name: crate::ast::shared::names::QualifiedName<'input>,
    pub args:
        Option<Surrounded<punct::LParen, Seq0<FuncParam<'input>, punct::Comma>, punct::RParen>>,
}

/// DROP PROCEDURE `name [(args)] [, name [(args)] ...] [CASCADE | RESTRICT]`.
///
/// Per gram.y `RemoveFuncStmt`: the target is a `function_with_argtypes_list`
/// (one or more `name [(args)]` entries separated by commas).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropProcedureStmt<'input> {
    pub drop: DROP,
    pub procedure: PROCEDURE,
    pub if_exists: Option<(IF, EXISTS)>,
    pub targets: Seq0<DropProcedureTarget<'input>, punct::Comma>,
    pub behavior: Option<crate::ast::shared::flags::DropBehavior>,
}

/// CALL name ( [ argument ] [, ...] )
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["procedural"])]
pub struct CallStmt<'input> {
    pub call: CALL,
    pub name: crate::tokens::type_function_name<'input>,
    pub args: Surrounded<punct::LParen, Seq0<FuncArg<'input>, punct::Comma>, punct::RParen>,
}

#[cfg(test)]
mod tests {
    use crate::ast::test_support::*;
    use recursa::Parse;

    use super::*;

    #[test]
    fn parse_create_procedure_basic() {
        let mut input = crate::tokens::test_input(
            "CREATE PROCEDURE ptest1(x text) LANGUAGE SQL AS $$ INSERT INTO cp_test VALUES (1, x); $$",
        );
        let _stmt = CreateProcedureStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_call_basic() {
        let mut input = crate::tokens::test_input("CALL ptest1('a')");
        let _stmt = CallStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_call_concat_arg() {
        let mut input = crate::tokens::test_input("CALL ptest1('xy' || 'zzy')");
        let _stmt = CallStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_call_no_args() {
        let mut input = crate::tokens::test_input("CALL nonexistent()");
        let _stmt = CallStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn parse_drop_procedure() {
        let mut input = crate::tokens::test_input("DROP PROCEDURE ptest1");
        let _stmt = DropProcedureStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// CREATE PROCEDURE with a schema-qualified name (gram.y
    /// `CreateFunctionStmt: … PROCEDURE func_name`, where `func_name` is
    /// `type_function_name` accepting `schema.name`). privileges.sql
    /// corpus uses `CREATE PROCEDURE testns.bar()`.
    #[test]
    fn parse_create_procedure_qualified_name() {
        let mut input =
            crate::tokens::test_input("CREATE PROCEDURE testns.bar() AS 'select 1' LANGUAGE sql");
        let stmt = CreateProcedureStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "bar");
        assert!(input.is_empty());
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterProcedureStmt<'input> {
    pub alter: ALTER,
    pub procedure: PROCEDURE,
    pub target: crate::ast::ddl::function::DropFunctionTarget<'input>,
    pub action: AlterFuncAction<'input>,
}
