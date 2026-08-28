//! DOMAIN DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::trigger::ConstraintAttributeElem;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `COLLATE name` clause on a domain.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainCollate<'input> {
    pub collate: COLLATE,
    pub name: QualifiedName<'input>,
}

/// `[CONSTRAINT name]` prefix on a domain constraint.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainConstraintName<'input> {
    pub constraint: CONSTRAINT,
    pub name: crate::tokens::ColId<'input>,
}

/// `NOT NULL` domain constraint body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainNotNull {
    pub not: NOT,
    pub null: NULL,
}

/// `CHECK (expr)` domain constraint body.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainCheckBody<'input> {
    pub check: CHECK,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
}

/// `DEFAULT expr` clause — domain default value.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainDefault<'input> {
    pub default: DEFAULT,
    pub expr: Box<Expr<'input>>,
}

/// Body of a domain constraint — Postgres' `DomainConstraintElem` plus the
/// `DEFAULT expr` form (which is split out from `ColConstraintElem` by
/// `SplitColQualList` in gram.y).
///
/// Variant ordering: `NotNull` (`NOT NULL`, 2 tokens) before `Null`; `Check`
/// and `Default` are keyword-led and unambiguous.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum DomainConstraintBody<'input> {
    NotNull(DomainNotNull),
    Null(NULL),
    Check(DomainCheckBody<'input>),
    Default(DomainDefault<'input>),
}

/// A single domain constraint — `[CONSTRAINT name] body`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct DomainConstraint<'input> {
    pub name: Option<DomainConstraintName<'input>>,
    pub body: DomainConstraintBody<'input>,
}

/// `CREATE DOMAIN name [AS] Typename [COLLATE name] [constraint_list]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateDomainStmt<'input> {
    pub create: CREATE,
    pub domain: DOMAIN,
    pub name: QualifiedName<'input>,
    pub r#as: Option<AS>,
    pub type_name: CastType<'input>,
    pub collate: Option<DomainCollate<'input>>,
    pub constraints: Vec<DomainConstraint<'input>>,
}

/// `DROP DOMAIN [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropDomainStmt<'input> {
    pub drop: DROP,
    pub domain: DOMAIN,
    pub if_exists: Option<IfExists>,
    pub types: TypeNameList<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `CHECK (expr) ConstraintAttributeSpec` — Postgres' CHECK arm of
/// `DomainConstraintElem` (the ALTER DOMAIN-specific form). Differs from
/// CREATE DOMAIN's `DomainCheckBody` by carrying the optional trailing
/// `ConstraintAttributeSpec` (e.g. `NOT VALID`).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainCheckConstraint<'input> {
    pub check: CHECK,
    pub expr: Surrounded<punct::LParen, Box<Expr<'input>>, punct::RParen>,
    pub attrs: Vec<ConstraintAttributeElem>,
}

/// `NOT NULL ConstraintAttributeSpec` — Postgres' NOT NULL arm of
/// `DomainConstraintElem` (ALTER DOMAIN-specific form). The corpus
/// exercises only the bare `NOT NULL` form, but the grammar allows the
/// optional `ConstraintAttributeSpec` trailer.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainNotNullConstraint {
    pub not: NOT,
    pub null: NULL,
    pub attrs: Vec<ConstraintAttributeElem>,
}

/// One body of an ALTER DOMAIN ADD constraint — Postgres'
/// `DomainConstraintElem`.
///
/// Variant ordering: variants begin with distinct keywords (`CHECK` /
/// `NOT`), so order is for clarity only.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterDomainConstraintElem<'input> {
    Check(AlterDomainCheckConstraint<'input>),
    NotNull(AlterDomainNotNullConstraint),
}

/// `[CONSTRAINT name] DomainConstraintElem` on ALTER DOMAIN ADD —
/// reuses the shared `DomainConstraintName` prefix from CREATE DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainConstraint<'input> {
    pub name: Option<DomainConstraintName<'input>>,
    pub elem: AlterDomainConstraintElem<'input>,
}

/// `ADD [CONSTRAINT name] DomainConstraintElem` — ADD action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainAdd<'input> {
    pub add: ADD,
    pub constraint: AlterDomainConstraint<'input>,
}

/// `DROP CONSTRAINT [IF EXISTS] name [CASCADE | RESTRICT]` — DROP CONSTRAINT
/// action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainDropConstraint<'input> {
    pub drop: DROP,
    pub constraint: CONSTRAINT,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `VALIDATE CONSTRAINT name` — VALIDATE action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainValidate<'input> {
    pub validate: VALIDATE,
    pub constraint: CONSTRAINT,
    pub name: crate::tokens::ColId<'input>,
}

/// `RENAME CONSTRAINT old TO new` — RenameStmt branch for domain constraints.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainRenameConstraint<'input> {
    pub rename: RENAME,
    pub constraint: CONSTRAINT,
    pub old_name: crate::tokens::ColId<'input>,
    pub to: TO,
    pub new_name: crate::tokens::ColId<'input>,
}

/// `SET DEFAULT expr` — SET DEFAULT action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainSetDefault<'input> {
    pub set: SET,
    pub default: DEFAULT,
    pub expr: Box<Expr<'input>>,
}

/// `DROP DEFAULT` — DROP DEFAULT action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainDropDefault {
    pub drop: DROP,
    pub default: DEFAULT,
}

/// `SET NOT NULL` — SET NOT NULL action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainSetNotNull {
    pub set: SET,
    pub not: NOT,
    pub null: NULL,
}

/// `DROP NOT NULL` — DROP NOT NULL action on ALTER DOMAIN.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterDomainDropNotNull {
    pub drop: DROP,
    pub not: NOT,
    pub null: NULL,
}

/// One action on `ALTER DOMAIN any_name action` — Postgres' `AlterDomainStmt`,
/// `RenameStmt`, `AlterOwnerStmt` and `AlterObjectSchemaStmt` branches for
/// domains.
///
/// Variant ordering:
/// - `SetNotNull` / `SetDefault` / `SetSchema` all begin with `SET`; the
///   two-token forms (`SetNotNull` = `SET NOT NULL`, `SetSchema` = `SET
///   SCHEMA`, `SetDefault` = `SET DEFAULT`) have distinct second tokens.
/// - `DropNotNull` / `DropDefault` / `DropConstraint` all begin with
///   `DROP`; their second tokens (`NOT`, `DEFAULT`, `CONSTRAINT`) are
///   distinct.
/// - `RenameConstraint` (two-token `RENAME CONSTRAINT`) must precede the
///   single-keyword `Rename` (`RENAME TO`) since both start with `RENAME`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterDomainAction<'input> {
    Add(AlterDomainAdd<'input>),
    DropConstraint(AlterDomainDropConstraint<'input>),
    DropNotNull(AlterDomainDropNotNull),
    DropDefault(AlterDomainDropDefault),
    SetNotNull(AlterDomainSetNotNull),
    SetDefault(AlterDomainSetDefault<'input>),
    SetSchema(SetSchemaClause<'input>),
    Validate(AlterDomainValidate<'input>),
    RenameConstraint(AlterDomainRenameConstraint<'input>),
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
}

/// `ALTER DOMAIN any_name action` — Postgres' `AlterDomainStmt`,
/// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterDomainStmt<'input> {
    pub alter: ALTER,
    pub domain: DOMAIN,
    pub name: QualifiedName<'input>,
    pub action: AlterDomainAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_create_domain_simple() {
        let mut input = crate::tokens::test_input("CREATE DOMAIN domaintext text");
        let stmt = CreateDomainStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.object(), "domaintext");
        assert!(stmt.r#as.is_none());
        assert!(stmt.constraints.is_empty());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_domain_check_default_notnull() {
        let mut input = crate::tokens::test_input(
            "CREATE DOMAIN dcheck varchar(15) NOT NULL DEFAULT 'a' CHECK (VALUE = 'a')",
        );
        let stmt = CreateDomainStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.constraints.len(), 3);
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_domain_named_constraint() {
        let mut input = crate::tokens::test_input(
            "CREATE DOMAIN testdomain1 AS int CONSTRAINT unsigned CHECK (value > 0)",
        );
        let stmt = CreateDomainStmt::parse(&mut input).unwrap();
        assert!(stmt.r#as.is_some());
        assert_eq!(stmt.constraints.len(), 1);
        assert!(stmt.constraints[0].name.is_some());
        assert!(input.is_empty());
    }

    #[test]
    fn parse_create_domain_array_with_size() {
        // `int4[1]` — `[N]` array bound, exercised by domain.sql.
        let mut input = crate::tokens::test_input("CREATE DOMAIN domainint4arr int4[1]");
        let stmt = CreateDomainStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.type_name.array_suffixes.len(), 1);
        assert!(input.is_empty());
    }
}
