//! SUBSCRIPTION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};
use recursa::surrounded::Surrounded;
use recursa::{FormatTokens, Transform, Visit};

use crate::ast::ddl::publication::{SetDefinitionClause, WithDefinition};
use crate::ast::ddl::role::DefElem;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::keyword::*;
use crate::tokens::soft_keyword::*;
use crate::tokens::{literal, punct};
use recursa_diagram::railroad;

/// `CONNECTION sconst` clause on CREATE SUBSCRIPTION.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubscriptionConnectionClause<'input> {
    pub connection: crate::tokens::soft_keyword::CONNECTION,
    pub conninfo: CopySconst<'input>,
}

/// `PUBLICATION name_list` clause on CREATE SUBSCRIPTION — Postgres'
/// `PUBLICATION name_list`. Each name is an identifier (publication
/// names are not qualified).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct SubscriptionPublicationClause<'input> {
    pub publication: PUBLICATION,
    pub names: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
}

/// `CREATE SUBSCRIPTION name CONNECTION sconst PUBLICATION name_list
/// [WITH (def_list)]` — Postgres' `CreateSubscriptionStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct CreateSubscriptionStmt<'input> {
    pub create: CREATE,
    pub subscription: SUBSCRIPTION,
    pub name: crate::tokens::ColId<'input>,
    pub connection: SubscriptionConnectionClause<'input>,
    pub publication_clause: SubscriptionPublicationClause<'input>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP SUBSCRIPTION [IF EXISTS] name [CASCADE | RESTRICT]`.
///
/// Postgres' `DropSubscriptionStmt` rule takes a single `name`, not a list.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct DropSubscriptionStmt<'input> {
    pub drop: DROP,
    pub subscription: SUBSCRIPTION,
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `CONNECTION sconst` — Postgres' `ALTER SUBSCRIPTION name CONNECTION
/// sconst` and also the (already-modelled, in `SubscriptionConnectionClause`)
/// `CREATE SUBSCRIPTION ... CONNECTION ...` form.
///
/// pg-sql reuses [`SubscriptionConnectionClause`] for this branch.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterSubscriptionRefresh<'input> {
    pub refresh: REFRESH,
    pub publication: PUBLICATION,
    pub with: Option<WithDefinition<'input>>,
}

/// `ADD PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... ADD PUBLICATION ...` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterSubscriptionAddPublication<'input> {
    pub add: ADD,
    pub publication: PUBLICATION,
    pub names: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... DROP PUBLICATION ...` form.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterSubscriptionDropPublication<'input> {
    pub drop: DROP,
    pub publication: PUBLICATION,
    pub names: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
    pub with: Option<WithDefinition<'input>>,
}

/// `SET PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... SET PUBLICATION ...` form. Distinct from
/// `SET CONNECTION sconst` (kept separate variant) and from
/// `SET (def_list)` (modelled via [`SetDefinitionClause`]).
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterSubscriptionSetPublication<'input> {
    pub set: SET,
    pub publication: PUBLICATION,
    pub names: Seq1<crate::tokens::ColId<'input>, punct::Comma>,
    pub with: Option<WithDefinition<'input>>,
}

/// `SKIP (def_list)` — Postgres' `ALTER SUBSCRIPTION ... SKIP definition`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub struct AlterSubscriptionSkip<'input> {
    pub skip: crate::tokens::keyword::SKIP,
    pub items: Surrounded<punct::LParen, Seq1<DefElem<'input>, punct::Comma>, punct::RParen>,
}

/// One action on `ALTER SUBSCRIPTION name action` — covers Postgres'
/// `AlterSubscriptionStmt` (CONNECTION, REFRESH PUBLICATION, ADD/DROP/SET
/// PUBLICATION, SET (...), SKIP (...), ENABLE, DISABLE) plus the
/// `RENAME TO` / `OWNER TO` branches from `RenameStmt` / `AlterOwnerStmt`.
///
/// Variant ordering: variants beginning with unique keywords
/// (`RENAME`, `OWNER`, `CONNECTION`, `REFRESH`, `ADD`, `DROP`, `SKIP`,
/// `ENABLE`, `DISABLE`) are listed before the two `SET ...` variants.
/// The two `SET ...` variants share the `SET` token; lists
/// `SetPublication` (`SET PUBLICATION`, two tokens) before `SetDef`
/// (`SET (`, two tokens). Each disambiguates on the second token.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules)]
pub enum AlterSubscriptionAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    Connection(SubscriptionConnectionClause<'input>),
    Refresh(AlterSubscriptionRefresh<'input>),
    AddPublication(AlterSubscriptionAddPublication<'input>),
    DropPublication(AlterSubscriptionDropPublication<'input>),
    Skip(AlterSubscriptionSkip<'input>),
    Enable(ENABLE),
    Disable(DISABLE),
    SetPublication(AlterSubscriptionSetPublication<'input>),
    SetDef(SetDefinitionClause<'input>),
}

/// `ALTER SUBSCRIPTION name action` — Postgres' `AlterSubscriptionStmt`
/// plus the subscription branches of `RenameStmt` / `AlterOwnerStmt`.
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["ddl"])]
pub struct AlterSubscriptionStmt<'input> {
    pub alter: ALTER,
    pub subscription: SUBSCRIPTION,
    pub name: crate::tokens::ColId<'input>,
    pub action: AlterSubscriptionAction<'input>,
}

#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn parse_drop_subscription() {
        let mut input = crate::tokens::test_input("DROP SUBSCRIPTION sub1 CASCADE");
        let stmt = DropSubscriptionStmt::parse(&mut input).unwrap();
        assert_eq!(stmt.name.text(), "sub1");
        assert!(stmt.behavior.is_some());
        assert!(input.is_empty());
    }

    /// `ALTER SUBSCRIPTION name SET (origin = 'value')` — string-valued
    /// def_arg, sanity test for the SetDef path.
    #[test]
    fn parse_alter_subscription_set_origin_string() {
        let mut input =
            crate::tokens::test_input("ALTER SUBSCRIPTION regress_testsub4 SET (origin = 'none')");
        let _stmt = AlterSubscriptionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    /// `ALTER SUBSCRIPTION name SET (origin = any)` — `any` is a reserved
    /// keyword used as a `def_arg` value (gram.y `def_arg` accepts
    /// `reserved_keyword`). subscription.sql corpus uses this.
    #[test]
    fn parse_alter_subscription_set_origin_any() {
        let mut input =
            crate::tokens::test_input("ALTER SUBSCRIPTION regress_testsub4 SET (origin = any)");
        let _stmt = AlterSubscriptionStmt::parse(&mut input).unwrap();
        assert!(input.is_empty());
    }

    #[test]
    fn create_subscription_connection_publication_roundtrips() {
        let stmt: CreateSubscriptionStmt = parse_stmt(
            "CREATE SUBSCRIPTION regress_testsub CONNECTION 'testconn' PUBLICATION testpub WITH (connect = false)",
        );
        assert_eq!(stmt.name.text(), "regress_testsub");
        assert_eq!(stmt.publication_clause.names.len(), 1);
        assert!(stmt.with.is_some());
        reparse_stable::<CreateSubscriptionStmt>(
            "CREATE SUBSCRIPTION regress_testsub CONNECTION 'testconn' PUBLICATION testpub WITH (connect = false)",
        );
    }

    #[test]
    fn create_subscription_multi_publication_roundtrips() {
        reparse_stable::<CreateSubscriptionStmt>(
            "CREATE SUBSCRIPTION s CONNECTION 'dbname=x' PUBLICATION p1, p2, p3 WITH (connect = false)",
        );
    }
}
