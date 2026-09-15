//! COMMENT ON / SECURITY LABEL ON — share the same `CommentObject`
//! object-kind grammar, so they live in one file. Per §6 Q5 of the
//! destination map, SECURITY LABEL is placed here rather than in a
//! dedicated file because its grammar mirrors COMMENT.

use crate::ast::ddl::function::FunctionParameters;
use crate::ast::shared::names::{AggregateArgs, QualifiedName};
use crate::tokens::literal;

// --- COMMENT / SECURITY LABEL shared object grammar ---

recursa::ast_node! {
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
    /// a file-level parse error.
    #[derive(Debug)]
    pub enum CommentObject {
        // CONSTRAINT and object_type_name_on_any_name `name ON any_name` forms —
        // listed first since their leading keyword is unambiguous.
        Constraint(CommentConstraintObject),
        Policy(CommentPolicyObject),
        Rule(CommentRuleObject),
        Trigger(CommentTriggerObject),
        // object_type_any_name `any_name` — multi-word kinds before single-word.
        MaterializedView(CommentMatViewObject),
        ForeignTable(CommentForeignTableObject),
        TextSearchParser(CommentTsParserObject),
        TextSearchDictionary(CommentTsDictionaryObject),
        TextSearchTemplate(CommentTsTemplateObject),
        TextSearchConfiguration(CommentTsConfigObject),
        Table(CommentTableObject),
        Sequence(CommentSequenceObject),
        View(CommentViewObject),
        Index(CommentIndexObject),
        Collation(CommentCollationObject),
        Conversion(CommentConversionObject),
        Statistics(CommentStatisticsObject),
        Column(CommentColumnObject),
        // object_type_name `name` — multi-word kinds before single-word.
        AccessMethod(CommentAccessMethodObject),
        EventTrigger(CommentEventTriggerObject),
        ForeignDataWrapper(CommentForeignDataWrapperObject),
        ProceduralLanguage(CommentProceduralLanguageObject),
        Language(CommentLanguageObject),
        Database(CommentDatabaseObject),
        Role(CommentRoleObject),
        Subscription(CommentSubscriptionObject),
        Tablespace(CommentTablespaceObject),
        Extension(CommentExtensionObject),
        Publication(CommentPublicationObject),
        Schema(CommentSchemaObject),
        Server(CommentServerObject),
        // TYPE/DOMAIN take a Typename.
        Type(CommentTypeObject),
        Domain(CommentDomainObject),
        // Callable objects take a parenthesized argument signature.
        Aggregate(CommentAggregateObject),
        Function(CommentFunctionObject),
        Procedure(CommentProcedureObject),
        Routine(CommentRoutineObject),
        // `OPERATOR op(args)` — same `operator_with_argtypes` grammar as
        // `DROP OPERATOR`.
        Operator(CommentOperatorObject),
        // `LARGE OBJECT NumericOnly` — gram.y's `COMMENT ON LARGE_P OBJECT_P
        // NumericOnly` arm. Two-keyword lead but disjoint from the other
        // variants once the discriminator (`LARGE`) is reached.
        LargeObject(CommentLargeObjectObject),
    }
}

recursa::ast_node! {
    /// `LARGE OBJECT NumericOnly` comment object (gram.y `COMMENT ON LARGE_P
    /// OBJECT_P NumericOnly`). The OID is a numeric literal — corpus uses only
    /// positive `IntegerLit`s.
    #[derive(Debug)]
    pub struct CommentLargeObjectObject {
        #[tok(LARGE, OBJECT, this)]
        pub oid: literal::IntegerLit,
    }
}

recursa::ast_node! {
    /// `OPERATOR op(args)` comment object — Postgres' `operator_with_argtypes`.
    #[derive(Debug)]
    pub struct CommentOperatorObject {
        #[tok(OPERATOR, this)]
        pub target: crate::ast::shared::names::OperatorWithArgtypes,
    }
}

recursa::ast_node! {
    /// `TABLE name` comment object.
    #[derive(Debug)]
    pub struct CommentTableObject {
        #[tok(TABLE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `SEQUENCE name` comment object.
    #[derive(Debug)]
    pub struct CommentSequenceObject {
        #[tok(SEQUENCE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `VIEW name` comment object.
    #[derive(Debug)]
    pub struct CommentViewObject {
        #[tok(VIEW, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `INDEX name` comment object.
    #[derive(Debug)]
    pub struct CommentIndexObject {
        #[tok(INDEX, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `COLLATION name` comment object.
    #[derive(Debug)]
    pub struct CommentCollationObject {
        #[tok(COLLATION, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `CONVERSION name` comment object.
    #[derive(Debug)]
    pub struct CommentConversionObject {
        #[tok(CONVERSION, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `STATISTICS name` comment object.
    #[derive(Debug)]
    pub struct CommentStatisticsObject {
        #[tok(STATISTICS, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `COLUMN any_name` comment object.
    #[derive(Debug)]
    pub struct CommentColumnObject {
        #[tok(COLUMN, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `MATERIALIZED VIEW name` comment object.
    #[derive(Debug)]
    pub struct CommentMatViewObject {
        #[tok(MATERIALIZED, VIEW, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `FOREIGN TABLE name` comment object.
    #[derive(Debug)]
    pub struct CommentForeignTableObject {
        #[tok(FOREIGN, TABLE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `TEXT SEARCH PARSER name` comment object.
    #[derive(Debug)]
    pub struct CommentTsParserObject {
        #[tok(TEXT, SEARCH, PARSER, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `TEXT SEARCH DICTIONARY name` comment object.
    #[derive(Debug)]
    pub struct CommentTsDictionaryObject {
        #[tok(TEXT, SEARCH, DICTIONARY, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `TEXT SEARCH TEMPLATE name` comment object.
    #[derive(Debug)]
    pub struct CommentTsTemplateObject {
        #[tok(TEXT, SEARCH, TEMPLATE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `TEXT SEARCH CONFIGURATION name` comment object.
    #[derive(Debug)]
    pub struct CommentTsConfigObject {
        #[tok(TEXT, SEARCH, CONFIGURATION, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `DATABASE name` comment object.
    #[derive(Debug)]
    pub struct CommentDatabaseObject {
        #[tok(DATABASE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `ROLE name` comment object.
    #[derive(Debug)]
    pub struct CommentRoleObject {
        #[tok(ROLE, this)]
        pub name: crate::tokens::NonReservedWord,
    }
}

recursa::ast_node! {
    /// `SUBSCRIPTION name` comment object.
    #[derive(Debug)]
    pub struct CommentSubscriptionObject {
        #[tok(SUBSCRIPTION, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `TABLESPACE name` comment object.
    #[derive(Debug)]
    pub struct CommentTablespaceObject {
        #[tok(TABLESPACE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `EXTENSION name` comment object.
    #[derive(Debug)]
    pub struct CommentExtensionObject {
        #[tok(EXTENSION, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `PUBLICATION name` comment object.
    #[derive(Debug)]
    pub struct CommentPublicationObject {
        #[tok(PUBLICATION, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `SCHEMA name` comment object.
    #[derive(Debug)]
    pub struct CommentSchemaObject {
        #[tok(SCHEMA, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `SERVER name` comment object.
    #[derive(Debug)]
    pub struct CommentServerObject {
        #[tok(SERVER, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `LANGUAGE name` comment object.
    #[derive(Debug)]
    pub struct CommentLanguageObject {
        #[tok(LANGUAGE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `PROCEDURAL LANGUAGE name` comment object.
    #[derive(Debug)]
    pub struct CommentProceduralLanguageObject {
        #[tok(PROCEDURAL, LANGUAGE, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `ACCESS METHOD name` comment object.
    #[derive(Debug)]
    pub struct CommentAccessMethodObject {
        #[tok(ACCESS, METHOD, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `EVENT TRIGGER name` comment object.
    #[derive(Debug)]
    pub struct CommentEventTriggerObject {
        #[tok(EVENT, TRIGGER, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `FOREIGN DATA WRAPPER name` comment object.
    #[derive(Debug)]
    pub struct CommentForeignDataWrapperObject {
        #[tok(FOREIGN, DATA, WRAPPER, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `TYPE Typename` comment object.
    #[derive(Debug)]
    pub struct CommentTypeObject {
        #[tok(TYPE, this)]
        pub type_name: crate::ast::shared::names::TypeName,
    }
}

recursa::ast_node! {
    /// `DOMAIN Typename` comment object.
    #[derive(Debug)]
    pub struct CommentDomainObject {
        #[tok(DOMAIN, this)]
        pub type_name: crate::ast::shared::names::TypeName,
    }
}

recursa::ast_node! {
    /// `POLICY name ON table` comment object.
    #[derive(Debug)]
    pub struct CommentPolicyObject {
        #[tok(POLICY, this)]
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
    }
}

recursa::ast_node! {
    /// `RULE name ON table` comment object.
    #[derive(Debug)]
    pub struct CommentRuleObject {
        #[tok(RULE, this)]
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
    }
}

recursa::ast_node! {
    /// `TRIGGER name ON table` comment object.
    #[derive(Debug)]
    pub struct CommentTriggerObject {
        #[tok(TRIGGER, this)]
        pub name: crate::tokens::ColId,
        #[tok(ON, this)]
        pub table: QualifiedName,
    }
}

recursa::ast_node! {
    /// `CONSTRAINT name ON [DOMAIN] any_name` — the constraint object forms.
    #[derive(Debug)]
    pub struct CommentConstraintObject {
        #[tok(CONSTRAINT, this, ON)]
        pub name: crate::tokens::ColId,
        #[presence(DOMAIN)]
        pub domain: bool,
        pub container: QualifiedName,
    }
}

recursa::ast_node! {
    /// `FUNCTION name(args)` comment object — Postgres' `function_with_argtypes`.
    ///
    /// Only the parenthesized-signature form is modelled; every corpus example
    /// carries an explicit argument list. The bare-name (`args_unspecified`) form
    /// is not exercised by any corpus statement.
    #[derive(Debug)]
    pub struct CommentFunctionObject {
        #[tok(FUNCTION, this)]
        pub name: QualifiedName,
        pub args: FunctionParameters,
    }
}

recursa::ast_node! {
    /// `PROCEDURE name(args)` comment object.
    #[derive(Debug)]
    pub struct CommentProcedureObject {
        #[tok(PROCEDURE, this)]
        pub name: QualifiedName,
        pub args: FunctionParameters,
    }
}

recursa::ast_node! {
    /// `ROUTINE name(args)` comment object.
    #[derive(Debug)]
    pub struct CommentRoutineObject {
        #[tok(ROUTINE, this)]
        pub name: QualifiedName,
        pub args: FunctionParameters,
    }
}

recursa::ast_node! {
    /// `AGGREGATE name(args)` — Postgres' `aggregate_with_argtypes`.
    #[derive(Debug)]
    pub struct CommentAggregateObject {
        #[tok(AGGREGATE, this)]
        pub name: QualifiedName,
        pub args: AggregateArgs,
    }
}

recursa::ast_node! {
    /// The comment/label text — Postgres' `comment_text` / `security_label`: a
    /// string literal or the keyword `NULL` (drop the comment/label).
    #[derive(Debug)]
    pub enum CommentText {
        #[tok(NULL)]
        Null,
        Text(literal::StringLit),
    }
}

// --- COMMENT ---

recursa::ast_node! {
    /// `COMMENT ON object IS { 'text' | NULL }`
    #[derive(Debug)]
    pub struct CommentStmt {
        #[tok(COMMENT, ON, this)]
        pub object: CommentObject,
        #[tok(IS, this)]
        pub text: CommentText,
    }
}

// -----------------------------------------------------------------------
// SECURITY LABEL — same shared object grammar as COMMENT.
// -----------------------------------------------------------------------

// --- SECURITY LABEL ---

recursa::ast_node! {
    /// A security-label provider name — Postgres' `NonReservedWord_or_Sconst`.
    ///
    /// Variant ordering: `String` before `Word` is irrelevant (disjoint
    /// first-sets — a quoted string vs an identifier), but the string form is the
    /// one the corpus exercises (`FOR 'dummy'`).
    #[derive(Debug)]
    pub enum SecurityLabelProviderName {
        String(literal::StringLit),
        Word(literal::Ident),
    }
}

recursa::ast_node! {
    /// The `FOR provider` clause on a `SECURITY LABEL` statement — Postgres'
    /// `opt_provider`.
    #[derive(Debug)]
    pub struct SecurityLabelProvider {
        #[tok(FOR, this)]
        pub name: SecurityLabelProviderName,
    }
}

recursa::ast_node! {
    /// `SECURITY LABEL [FOR provider] ON object IS { 'label' | NULL }`
    ///
    /// The object grammar is shared verbatim with `COMMENT ON` ([`CommentObject`]).
    /// Postgres' `SecLabelStmt` accepts a subset of object kinds; the wider
    /// `CommentObject` enum is reused since SECURITY LABEL of an unsupported kind
    /// is rejected by PostgreSQL anyway and never appears in the corpus.
    #[derive(Debug)]
    #[tok(SECURITY, LABEL, this)]
    pub struct SecurityLabelStmt {
        pub provider: Option<SecurityLabelProvider>,
        #[tok(ON, this)]
        pub object: CommentObject,
        #[tok(IS, this)]
        pub text: CommentText,
    }
}
