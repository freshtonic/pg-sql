//! COMMENT ON / SECURITY LABEL ON — share the same `CommentObject`
//! object-kind grammar, so they live in one file. Per §6 Q5 of the
//! destination map, SECURITY LABEL is placed here rather than in a
//! dedicated file because its grammar mirrors COMMENT.

use recursa::seq::Seq0;
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentLargeObjectObject<'input> {
    pub large: LARGE,
    pub object: crate::tokens::soft_keyword::OBJECT,
    pub oid: literal::IntegerLit<'input>,
}

/// `OPERATOR op(args)` comment object — Postgres' `operator_with_argtypes`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentOperatorObject<'input> {
    pub kind: OPERATOR,
    pub target: crate::ast::shared::names::OperatorWithArgtypes<'input>,
}

/// `TABLE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTableObject<'input> {
    pub kind: TABLE,
    pub name: QualifiedName<'input>,
}

/// `SEQUENCE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentSequenceObject<'input> {
    pub kind: SEQUENCE,
    pub name: QualifiedName<'input>,
}

/// `VIEW name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentViewObject<'input> {
    pub kind: VIEW,
    pub name: QualifiedName<'input>,
}

/// `INDEX name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentIndexObject<'input> {
    pub kind: INDEX,
    pub name: QualifiedName<'input>,
}

/// `COLLATION name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentCollationObject<'input> {
    pub kind: COLLATION,
    pub name: QualifiedName<'input>,
}

/// `CONVERSION name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentConversionObject<'input> {
    pub kind: CONVERSION,
    pub name: QualifiedName<'input>,
}

/// `STATISTICS name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentStatisticsObject<'input> {
    pub kind: STATISTICS,
    pub name: QualifiedName<'input>,
}

/// `COLUMN any_name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentColumnObject<'input> {
    pub kind: COLUMN,
    pub name: QualifiedName<'input>,
}

/// `MATERIALIZED VIEW name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentMatViewObject<'input> {
    pub kind: (MATERIALIZED, VIEW),
    pub name: QualifiedName<'input>,
}

/// `FOREIGN TABLE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentForeignTableObject<'input> {
    pub kind: (FOREIGN, TABLE),
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH PARSER name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTsParserObject<'input> {
    pub kind: (TEXT, SEARCH, PARSER),
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH DICTIONARY name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTsDictionaryObject<'input> {
    pub kind: (TEXT, SEARCH, DICTIONARY),
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH TEMPLATE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTsTemplateObject<'input> {
    pub kind: (TEXT, SEARCH, TEMPLATE),
    pub name: QualifiedName<'input>,
}

/// `TEXT SEARCH CONFIGURATION name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTsConfigObject<'input> {
    pub kind: (TEXT, SEARCH, CONFIGURATION),
    pub name: QualifiedName<'input>,
}

/// `DATABASE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentDatabaseObject<'input> {
    pub kind: DATABASE,
    pub name: crate::tokens::ColId<'input>,
}

/// `ROLE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentRoleObject<'input> {
    pub kind: ROLE,
    pub name: crate::tokens::NonReservedWord<'input>,
}

/// `SUBSCRIPTION name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentSubscriptionObject<'input> {
    pub kind: SUBSCRIPTION,
    pub name: crate::tokens::ColId<'input>,
}

/// `TABLESPACE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTablespaceObject<'input> {
    pub kind: TABLESPACE,
    pub name: crate::tokens::ColId<'input>,
}

/// `EXTENSION name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentExtensionObject<'input> {
    pub kind: EXTENSION,
    pub name: crate::tokens::ColId<'input>,
}

/// `PUBLICATION name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentPublicationObject<'input> {
    pub kind: PUBLICATION,
    pub name: crate::tokens::ColId<'input>,
}

/// `SCHEMA name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentSchemaObject<'input> {
    pub kind: SCHEMA,
    pub name: crate::tokens::ColId<'input>,
}

/// `SERVER name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentServerObject<'input> {
    pub kind: SERVER,
    pub name: crate::tokens::ColId<'input>,
}

/// `LANGUAGE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentLanguageObject<'input> {
    pub kind: LANGUAGE,
    pub name: crate::tokens::ColId<'input>,
}

/// `PROCEDURAL LANGUAGE name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentProceduralLanguageObject<'input> {
    pub kind: (PROCEDURAL, LANGUAGE),
    pub name: crate::tokens::ColId<'input>,
}

/// `ACCESS METHOD name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentAccessMethodObject<'input> {
    pub kind: (ACCESS, METHOD),
    pub name: crate::tokens::ColId<'input>,
}

/// `EVENT TRIGGER name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentEventTriggerObject<'input> {
    pub kind: (EVENT, TRIGGER),
    pub name: crate::tokens::ColId<'input>,
}

/// `FOREIGN DATA WRAPPER name` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentForeignDataWrapperObject<'input> {
    pub kind: (FOREIGN, DATA, WRAPPER),
    pub name: crate::tokens::ColId<'input>,
}

/// `TYPE Typename` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTypeObject<'input> {
    pub kind: TYPE,
    pub type_name: crate::ast::shared::names::TypeName<'input>,
}

/// `DOMAIN Typename` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentDomainObject<'input> {
    pub kind: DOMAIN,
    pub type_name: crate::ast::shared::names::TypeName<'input>,
}

/// `POLICY name ON table` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentPolicyObject<'input> {
    pub kind: POLICY,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
}

/// `RULE name ON table` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentRuleObject<'input> {
    pub kind: RULE,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
}

/// `TRIGGER name ON table` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentTriggerObject<'input> {
    pub kind: TRIGGER,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub table: QualifiedName<'input>,
}

/// `CONSTRAINT name ON [DOMAIN] any_name` — the constraint object forms.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentConstraintObject<'input> {
    pub constraint: CONSTRAINT,
    pub name: crate::tokens::ColId<'input>,
    pub on: ON,
    pub domain: Option<DOMAIN>,
    pub container: QualifiedName<'input>,
}

/// `FUNCTION name(args)` comment object — Postgres' `function_with_argtypes`.
///
/// Only the parenthesized-signature form is modelled; every corpus example
/// carries an explicit argument list. The bare-name (`args_unspecified`) form
/// is not exercised by any corpus statement.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentFunctionObject<'input> {
    pub kind: FUNCTION,
    pub name: QualifiedName<'input>,
    pub args: Surrounded<
        punct::LParen,
        Seq0<crate::ast::ddl::function::FuncParam<'input>, punct::Comma>,
        punct::RParen,
    >,
}

/// `PROCEDURE name(args)` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentProcedureObject<'input> {
    pub kind: PROCEDURE,
    pub name: QualifiedName<'input>,
    pub args: Surrounded<
        punct::LParen,
        Seq0<crate::ast::ddl::function::FuncParam<'input>, punct::Comma>,
        punct::RParen,
    >,
}

/// `ROUTINE name(args)` comment object.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentRoutineObject<'input> {
    pub kind: ROUTINE,
    pub name: QualifiedName<'input>,
    pub args: Surrounded<
        punct::LParen,
        Seq0<crate::ast::ddl::function::FuncParam<'input>, punct::Comma>,
        punct::RParen,
    >,
}

/// `AGGREGATE name(args)` — Postgres' `aggregate_with_argtypes`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct CommentAggregateObject<'input> {
    pub aggregate: AGGREGATE,
    pub name: QualifiedName<'input>,
    pub args: AggregateArgs<'input>,
}

/// The comment/label text — Postgres' `comment_text` / `security_label`: a
/// string literal or the keyword `NULL` (drop the comment/label).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum CommentText<'input> {
    Null(NULL),
    Text(literal::StringLit<'input>),
}

// --- COMMENT ---

/// `COMMENT ON object IS { 'text' | NULL }`
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct CommentStmt<'input> {
    pub comment: COMMENT,
    pub on: ON,
    pub object: CommentObject<'input>,
    pub is: IS,
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
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum SecurityLabelProviderName<'input> {
    String(literal::StringLit<'input>),
    Word(literal::Ident<'input>),
}

/// The `FOR provider` clause on a `SECURITY LABEL` statement — Postgres'
/// `opt_provider`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SecurityLabelProvider<'input> {
    pub for_kw: FOR,
    pub name: SecurityLabelProviderName<'input>,
}

/// `SECURITY LABEL [FOR provider] ON object IS { 'label' | NULL }`
///
/// The object grammar is shared verbatim with `COMMENT ON` ([`CommentObject`]).
/// Postgres' `SecLabelStmt` accepts a subset of object kinds; the wider
/// `CommentObject` enum is reused since SECURITY LABEL of an unsupported kind
/// is rejected by PostgreSQL anyway and never appears in the corpus.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["utility"])]
pub struct SecurityLabelStmt<'input> {
    pub security: SECURITY,
    pub label: LABEL,
    pub provider: Option<SecurityLabelProvider<'input>>,
    pub on: ON,
    pub object: CommentObject<'input>,
    pub is: IS,
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
