//! COMMENT ON / SECURITY LABEL ON — share the same `CommentObject`
//! object-kind grammar, so they live in one file. Per §6 Q5 of the
//! destination map, SECURITY LABEL is placed here rather than in a
//! dedicated file because its grammar mirrors COMMENT.

use recursa::seq::Seq0;
use recursa_diagram::railroad;

use crate::ast::shared::names::{AggregateArgs, QualifiedName};
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::{CONFIGURATION, DICTIONARY, PARSER, TEMPLATE};
use crate::tokens::{literal, punct};

// --- COMMENT / SECURITY LABEL shared object grammar ---

/// The object kind plus name in a `COMMENT ON` / `SECURITY LABEL ON` clause.
///
/// This models Postgres' `object_type_any_name any_name`,
/// `object_type_name name`, `COLUMN any_name`, `TYPE/DOMAIN Typename`,
/// `AGGREGATE/FUNCTION/PROCEDURE/ROUTINE …_with_argtypes`, and the
/// `CONSTRAINT … ON …` / `POLICY|RULE|TRIGGER … ON …` forms.
///
/// Variant ordering matters: multi-keyword kinds (`MATERIALIZED VIEW`,
/// `FOREIGN TABLE`, `TEXT SEARCH …`, `ACCESS METHOD`, …) precede any single
/// keyword that shares their prefix so longest-match-wins picks the most
/// specific spelling.
///
/// Deferred kinds: `OPERATOR CLASS` / `OPERATOR FAMILY` (we model bare
/// `OPERATOR operator_with_argtypes` but not the `CLASS` / `FAMILY` forms,
/// which take `any_name USING method`), `LARGE OBJECT` (numeric/`:var`
/// object id), `CAST` and `TRANSFORM`. A `COMMENT ON` / `SECURITY LABEL ON`
/// of a deferred kind fails this enum and the whole statement surfaces as a
/// [`crate::ast::FileItem::ParseError`] in the file-level parse output.
#[derive(recursa::Node, Debug, Clone)]
pub enum CommentObject<'input> {
    // CONSTRAINT and object_type_name_on_any_name `name ON any_name` forms —
    // listed first since their leading keyword is unambiguous.
    Constraint(CommentConstraintObject<'input>),
    Policy(CommentPolicyObject<'input>),
    Rule(CommentRuleObject<'input>),
    Trigger(CommentTriggerObject<'input>),
    // object_type_any_name `any_name` — multi-word kinds before single-word.
    MaterializedView(CommentMatViewObject<'input>),
    ForeignTable(CommentForeignTableObject<'input>),
    TextSearchParser(CommentTsParserObject<'input>),
    TextSearchDictionary(CommentTsDictionaryObject<'input>),
    TextSearchTemplate(CommentTsTemplateObject<'input>),
    TextSearchConfiguration(CommentTsConfigObject<'input>),
    Table(CommentTableObject<'input>),
    Sequence(CommentSequenceObject<'input>),
    View(CommentViewObject<'input>),
    Index(CommentIndexObject<'input>),
    Collation(CommentCollationObject<'input>),
    Conversion(CommentConversionObject<'input>),
    Statistics(CommentStatisticsObject<'input>),
    Column(CommentColumnObject<'input>),
    // object_type_name `name` — multi-word kinds before single-word.
    AccessMethod(CommentAccessMethodObject<'input>),
    EventTrigger(CommentEventTriggerObject<'input>),
    ForeignDataWrapper(CommentForeignDataWrapperObject<'input>),
    ProceduralLanguage(CommentProceduralLanguageObject<'input>),
    Language(CommentLanguageObject<'input>),
    Database(CommentDatabaseObject<'input>),
    Role(CommentRoleObject<'input>),
    Subscription(CommentSubscriptionObject<'input>),
    Tablespace(CommentTablespaceObject<'input>),
    Extension(CommentExtensionObject<'input>),
    Publication(CommentPublicationObject<'input>),
    Schema(CommentSchemaObject<'input>),
    Server(CommentServerObject<'input>),
    // TYPE/DOMAIN take a Typename.
    Type(CommentTypeObject<'input>),
    Domain(CommentDomainObject<'input>),
    // Callable objects take a parenthesized argument signature.
    Aggregate(CommentAggregateObject<'input>),
    Function(CommentFunctionObject<'input>),
    Procedure(CommentProcedureObject<'input>),
    Routine(CommentRoutineObject<'input>),
    // `OPERATOR op(args)` — same `operator_with_argtypes` grammar as
    // `DROP OPERATOR`.
    Operator(CommentOperatorObject<'input>),
    // `LARGE OBJECT NumericOnly` — gram.y's `COMMENT ON LARGE_P OBJECT_P
    // NumericOnly` arm. Two-keyword lead but disjoint from the other
    // variants once the discriminator (`LARGE`) is reached.
    LargeObject(CommentLargeObjectObject<'input>),
}

/// `LARGE OBJECT NumericOnly` comment object (gram.y `COMMENT ON LARGE_P
/// OBJECT_P NumericOnly`). The OID is a numeric literal — corpus uses only
/// positive `IntegerLit`s.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentLargeObjectObject<'input> {
    #[tok(LARGE, OBJECT, this)]
    pub oid: literal::IntegerLit<'input>,
}

/// `OPERATOR op(args)` comment object — Postgres' `operator_with_argtypes`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentOperatorObject<'input> {
    #[tok(OPERATOR, this)]
    pub target: crate::ast::shared::names::OperatorWithArgtypes<'input>,
}

/// `TABLE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTableObject<'input> {
    #[tok(TABLE, this)]
    pub name: QualifiedName<'input>,
}

/// `SEQUENCE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentSequenceObject<'input> {
    #[tok(SEQUENCE, this)]
    pub name: QualifiedName<'input>,
}

/// `VIEW name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentViewObject<'input> {
    #[tok(VIEW, this)]
    pub name: QualifiedName<'input>,
}

/// `INDEX name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentIndexObject<'input> {
    #[tok(INDEX, this)]
    pub name: QualifiedName<'input>,
}

/// `COLLATION name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentCollationObject<'input> {
    #[tok(COLLATION, this)]
    pub name: QualifiedName<'input>,
}

/// `CONVERSION name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentConversionObject<'input> {
    #[tok(CONVERSION, this)]
    pub name: QualifiedName<'input>,
}

/// `STATISTICS name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentStatisticsObject<'input> {
    #[tok(STATISTICS, this)]
    pub name: QualifiedName<'input>,
}

/// `COLUMN any_name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentColumnObject<'input> {
    #[tok(COLUMN, this)]
    pub name: QualifiedName<'input>,
}

/// `MATERIALIZED VIEW name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentMatViewObject<'input> {
    #[tok(MATERIALIZED, VIEW, this)]
    pub name: QualifiedName<'input>,
}

/// `FOREIGN TABLE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentForeignTableObject<'input> {
    #[tok(FOREIGN, TABLE, this)]
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH PARSER name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTsParserObject<'input> {
    #[tok(TEXT, SEARCH, PARSER, this)]
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH DICTIONARY name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTsDictionaryObject<'input> {
    #[tok(TEXT, SEARCH, DICTIONARY, this)]
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH TEMPLATE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTsTemplateObject<'input> {
    #[tok(TEXT, SEARCH, TEMPLATE, this)]
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH CONFIGURATION name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTsConfigObject<'input> {
    #[tok(TEXT, SEARCH, CONFIGURATION, this)]
    pub name: QualifiedName<'input>,
}

/// `DATABASE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentDatabaseObject<'input> {
    #[tok(DATABASE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `ROLE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentRoleObject<'input> {
    #[tok(ROLE, this)]
    pub name: crate::tokens::NonReservedWord<'input>,
}

/// `SUBSCRIPTION name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentSubscriptionObject<'input> {
    #[tok(SUBSCRIPTION, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `TABLESPACE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTablespaceObject<'input> {
    #[tok(TABLESPACE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `EXTENSION name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentExtensionObject<'input> {
    #[tok(EXTENSION, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `PUBLICATION name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentPublicationObject<'input> {
    #[tok(PUBLICATION, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `SCHEMA name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentSchemaObject<'input> {
    #[tok(SCHEMA, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `SERVER name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentServerObject<'input> {
    #[tok(SERVER, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `LANGUAGE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentLanguageObject<'input> {
    #[tok(LANGUAGE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `PROCEDURAL LANGUAGE name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentProceduralLanguageObject<'input> {
    #[tok(PROCEDURAL, LANGUAGE, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `ACCESS METHOD name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentAccessMethodObject<'input> {
    #[tok(ACCESS, METHOD, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `EVENT TRIGGER name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentEventTriggerObject<'input> {
    #[tok(EVENT, TRIGGER, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `FOREIGN DATA WRAPPER name` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentForeignDataWrapperObject<'input> {
    #[tok(FOREIGN, DATA, WRAPPER, this)]
    pub name: crate::tokens::ColId<'input>,
}

/// `TYPE Typename` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTypeObject<'input> {
    #[tok(TYPE, this)]
    pub type_name: crate::ast::shared::names::TypeName<'input>,
}

/// `DOMAIN Typename` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentDomainObject<'input> {
    #[tok(DOMAIN, this)]
    pub type_name: crate::ast::shared::names::TypeName<'input>,
}

/// `POLICY name ON table` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentPolicyObject<'input> {
    #[tok(POLICY, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
}

/// `RULE name ON table` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentRuleObject<'input> {
    #[tok(RULE, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
}

/// `TRIGGER name ON table` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentTriggerObject<'input> {
    #[tok(TRIGGER, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    pub table: QualifiedName<'input>,
}

/// `CONSTRAINT name ON [DOMAIN] any_name` — the constraint object forms.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentConstraintObject<'input> {
    #[tok(CONSTRAINT, this)]
    pub name: crate::tokens::ColId<'input>,
    #[tok(ON, this)]
    #[presence(DOMAIN)]
    pub domain: bool,
    pub container: QualifiedName<'input>,
}

/// `FUNCTION name(args)` comment object — Postgres' `function_with_argtypes`.
///
/// Only the parenthesized-signature form is modelled; every corpus example
/// carries an explicit argument list. The bare-name (`args_unspecified`) form
/// is not exercised by any corpus statement.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentFunctionObject<'input> {
    #[tok(FUNCTION, this)]
    pub name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:

        Vec<crate::ast::ddl::function::FuncParam<'input> >

    ,
}

/// `PROCEDURE name(args)` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentProcedureObject<'input> {
    #[tok(PROCEDURE, this)]
    pub name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:

        Vec<crate::ast::ddl::function::FuncParam<'input> >

    ,
}

/// `ROUTINE name(args)` comment object.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentRoutineObject<'input> {
    #[tok(ROUTINE, this)]
    pub name: QualifiedName<'input>,
    #[tok(LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub args:

        Vec<crate::ast::ddl::function::FuncParam<'input> >

    ,
}

/// `AGGREGATE name(args)` — Postgres' `aggregate_with_argtypes`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentAggregateObject<'input> {
    #[tok(AGGREGATE, this)]
    pub name: QualifiedName<'input>,
    pub args: AggregateArgs<'input>,
}

/// The comment/label text — Postgres' `comment_text` / `security_label`: a
/// string literal or the keyword `NULL` (drop the comment/label).
#[derive(recursa::Node, Debug, Clone)]
pub enum CommentText<'input> {
    #[tok(NULL)] Null,
    Text(literal::StringLit<'input>),
}

// --- COMMENT ---

/// `COMMENT ON object IS { 'text' | NULL }`
#[derive(recursa::Node, Debug, Clone)]
pub struct CommentStmt<'input> {
    #[tok(COMMENT, ON, this)]
    pub object: CommentObject<'input>,
    #[tok(IS, this)]
    pub text: CommentText<'input>,
}

// -----------------------------------------------------------------------
// SECURITY LABEL — same shared object grammar as COMMENT.
// -----------------------------------------------------------------------

// --- SECURITY LABEL ---

/// A security-label provider name — Postgres' `NonReservedWord_or_Sconst`.
///
/// Variant ordering: `String` before `Word` is irrelevant (disjoint
/// first-sets — a quoted string vs an identifier), but the string form is the
/// one the corpus exercises (`FOR 'dummy'`).
#[derive(recursa::Node, Debug, Clone)]
pub enum SecurityLabelProviderName<'input> {
    String(literal::StringLit<'input>),
    Word(literal::Ident<'input>),
}

/// The `FOR provider` clause on a `SECURITY LABEL` statement — Postgres'
/// `opt_provider`.
#[derive(recursa::Node, Debug, Clone)]
pub struct SecurityLabelProvider<'input> {
    #[tok(FOR, this)]
    pub name: SecurityLabelProviderName<'input>,
}

/// `SECURITY LABEL [FOR provider] ON object IS { 'label' | NULL }`
///
/// The object grammar is shared verbatim with `COMMENT ON` ([`CommentObject`]).
/// Postgres' `SecLabelStmt` accepts a subset of object kinds; the wider
/// `CommentObject` enum is reused since SECURITY LABEL of an unsupported kind
/// is rejected by PostgreSQL anyway and never appears in the corpus.
#[derive(recursa::Node, Debug, Clone)]
pub struct SecurityLabelStmt<'input> {
    #[tok(SECURITY, LABEL, this)]
    pub provider: Option<SecurityLabelProvider<'input>>,
    #[tok(ON, this)]
    pub object: CommentObject<'input>,
    #[tok(IS, this)]
    pub text: CommentText<'input>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_comment_on_operator_custom_op() {
        // Regression: `COMMENT ON OPERATOR === (a, b)` shares the
        // `operator_with_argtypes` grammar with DROP/ALTER OPERATOR and must
        // accept non-standard operator names.
        let stmt: CommentStmt =
            parse_stmt("COMMENT ON OPERATOR === (int4, int4) IS 'custom equality'");
        assert!(matches!(stmt.object, CommentObject::Operator(_)));
    }

    #[test]
    fn comment_on_table_is_modelled() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON TABLE attmp IS 'table comment'");
        assert!(matches!(stmt.text, CommentText::Text(_)));
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TABLE attmp IS 'table comment'"),
            "COMMENT ON TABLE attmp IS 'table comment'"
        );
    }

    #[test]
    fn comment_on_table_null_is_modelled() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON TABLE attmp IS NULL");
        assert!(matches!(stmt.text, CommentText::Null(_)));
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TABLE attmp IS NULL"),
            "COMMENT ON TABLE attmp IS NULL"
        );
    }

    #[test]
    fn comment_on_column_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON COLUMN ctlt1.a IS 'A'");
        assert!(matches!(stmt.object, CommentObject::Column(_)));
        reparse_stable::<CommentStmt>("COMMENT ON COLUMN ctlt1.a IS 'A'");
    }

    #[test]
    fn comment_on_materialized_view_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON MATERIALIZED VIEW mv IS 'm'"),
            "COMMENT ON MATERIALIZED VIEW mv IS 'm'"
        );
    }

    #[test]
    fn comment_on_text_search_parser_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TEXT SEARCH PARSER p IS 'x'"),
            "COMMENT ON TEXT SEARCH PARSER p IS 'x'"
        );
    }

    #[test]
    fn comment_on_database_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON DATABASE db IS 'x'"),
            "COMMENT ON DATABASE db IS 'x'"
        );
    }

    #[test]
    fn comment_on_access_method_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON ACCESS METHOD am IS 'x'"),
            "COMMENT ON ACCESS METHOD am IS 'x'"
        );
    }

    #[test]
    fn comment_on_constraint_on_table_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON CONSTRAINT c ON t IS 'x'"),
            "COMMENT ON CONSTRAINT c ON t IS 'x'"
        );
    }

    #[test]
    fn comment_on_constraint_on_domain_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON CONSTRAINT c ON DOMAIN d IS 'x'"),
            "COMMENT ON CONSTRAINT c ON DOMAIN d IS 'x'"
        );
    }

    #[test]
    fn comment_on_trigger_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TRIGGER tg ON t IS 'x'"),
            "COMMENT ON TRIGGER tg ON t IS 'x'"
        );
    }

    #[test]
    fn comment_on_type_roundtrips() {
        assert_eq!(
            roundtrip::<CommentStmt>("COMMENT ON TYPE default_test_row IS 'x'"),
            "COMMENT ON TYPE default_test_row IS 'x'"
        );
    }

    #[test]
    fn comment_on_function_with_args_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON FUNCTION f(int, text) IS 'x'");
        assert!(matches!(stmt.object, CommentObject::Function(_)));
        reparse_stable::<CommentStmt>("COMMENT ON FUNCTION f(int, text) IS 'x'");
    }

    #[test]
    fn comment_on_aggregate_star_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON AGGREGATE newcnt(*) IS 'x'");
        assert!(matches!(stmt.object, CommentObject::Aggregate(_)));
        reparse_stable::<CommentStmt>("COMMENT ON AGGREGATE newcnt(*) IS 'x'");
    }

    #[test]
    fn comment_on_aggregate_types_roundtrips() {
        let stmt: CommentStmt = parse_stmt("COMMENT ON AGGREGATE newavg(int4) IS 'x'");
        assert!(matches!(stmt.object, CommentObject::Aggregate(_)));
        reparse_stable::<CommentStmt>("COMMENT ON AGGREGATE newavg(int4) IS 'x'");
    }

    #[test]
    fn security_label_on_table_is_modelled() {
        let stmt: SecurityLabelStmt = parse_stmt("SECURITY LABEL ON TABLE t IS 'classified'");
        assert!(stmt.provider.is_none());
        assert_eq!(
            roundtrip::<SecurityLabelStmt>("SECURITY LABEL ON TABLE t IS 'classified'"),
            "SECURITY LABEL ON TABLE t IS 'classified'"
        );
    }

    #[test]
    fn security_label_with_provider_keeps_provider() {
        let stmt: SecurityLabelStmt =
            parse_stmt("SECURITY LABEL FOR 'dummy' ON TABLE t IS 'classified'");
        assert!(stmt.provider.is_some());
        assert_eq!(
            roundtrip::<SecurityLabelStmt>("SECURITY LABEL FOR 'dummy' ON TABLE t IS 'classified'"),
            "SECURITY LABEL FOR 'dummy' ON TABLE t IS 'classified'"
        );
    }

    #[test]
    fn security_label_on_role_null_roundtrips() {
        assert_eq!(
            roundtrip::<SecurityLabelStmt>("SECURITY LABEL ON ROLE r IS NULL"),
            "SECURITY LABEL ON ROLE r IS NULL"
        );
    }
}
