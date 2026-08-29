//! SUBSCRIPTION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use recursa::seq::{Seq0, Seq1};

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
#[derive(recursa::Node, Debug, Clone)]
pub struct SubscriptionConnectionClause<'input> {
    #[tok(CONNECTION, this)]
    pub conninfo: CopySconst<'input>,
}

/// `PUBLICATION name_list` clause on CREATE SUBSCRIPTION — Postgres'
/// `PUBLICATION name_list`. Each name is an identifier (publication
/// names are not qualified).
#[derive(recursa::Node, Debug, Clone)]
pub struct SubscriptionPublicationClause<'input> {
    #[tok(PUBLICATION, this)]
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input> >,
}

/// `CREATE SUBSCRIPTION name CONNECTION sconst PUBLICATION name_list
/// [WITH (def_list)]` — Postgres' `CreateSubscriptionStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct CreateSubscriptionStmt<'input> {
    #[tok(CREATE, SUBSCRIPTION, this)]
    pub name: crate::tokens::ColId<'input>,
    pub connection: SubscriptionConnectionClause<'input>,
    pub publication_clause: SubscriptionPublicationClause<'input>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP SUBSCRIPTION [IF EXISTS] name [CASCADE | RESTRICT]`.
///
/// Postgres' `DropSubscriptionStmt` rule takes a single `name`, not a list.
#[derive(recursa::Node, Debug, Clone)]
pub struct DropSubscriptionStmt<'input> {
    #[tok(DROP, SUBSCRIPTION, this)]
    pub if_exists: Option<IfExists>,
    pub name: crate::tokens::ColId<'input>,
    pub behavior: Option<DropBehavior>,
}

/// `CONNECTION sconst` — Postgres' `ALTER SUBSCRIPTION name CONNECTION
/// sconst` and also the (already-modelled, in `SubscriptionConnectionClause`)
/// `CREATE SUBSCRIPTION ... CONNECTION ...` form.
///
/// pg-sql reuses [`SubscriptionConnectionClause`] for this branch.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSubscriptionRefresh<'input> {
    #[tok(REFRESH, PUBLICATION, this)]
    pub with: Option<WithDefinition<'input>>,
}

/// `ADD PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... ADD PUBLICATION ...` form.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSubscriptionAddPublication<'input> {
    #[tok(ADD, PUBLICATION, this)]
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input> >,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... DROP PUBLICATION ...` form.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSubscriptionDropPublication<'input> {
    #[tok(DROP, PUBLICATION, this)]
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input> >,
    pub with: Option<WithDefinition<'input>>,
}

/// `SET PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... SET PUBLICATION ...` form. Distinct from
/// `SET CONNECTION sconst` (kept separate variant) and from
/// `SET (def_list)` (modelled via [`SetDefinitionClause`]).
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSubscriptionSetPublication<'input> {
    #[tok(SET, PUBLICATION, this)]
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input> >,
    pub with: Option<WithDefinition<'input>>,
}

/// `SKIP (def_list)` — Postgres' `ALTER SUBSCRIPTION ... SKIP definition`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSubscriptionSkip<'input> {
    #[tok(SKIP, LPAREN, this, RPAREN)]
    #[sep(COMMA)]
    pub items:  recursa::Vec1<DefElem<'input> > ,
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
#[derive(recursa::Node, Debug, Clone)]
pub enum AlterSubscriptionAction<'input> {
    Rename(RenameTo<'input>),
    Owner(OwnerTo<'input>),
    Connection(SubscriptionConnectionClause<'input>),
    Refresh(AlterSubscriptionRefresh<'input>),
    AddPublication(AlterSubscriptionAddPublication<'input>),
    DropPublication(AlterSubscriptionDropPublication<'input>),
    Skip(AlterSubscriptionSkip<'input>),
    #[tok(ENABLE)] Enable,
    #[tok(DISABLE)] Disable,
    SetPublication(AlterSubscriptionSetPublication<'input>),
    SetDef(SetDefinitionClause<'input>),
}

/// `ALTER SUBSCRIPTION name action` — Postgres' `AlterSubscriptionStmt`
/// plus the subscription branches of `RenameStmt` / `AlterOwnerStmt`.
#[derive(recursa::Node, Debug, Clone)]
pub struct AlterSubscriptionStmt<'input> {
    #[tok(ALTER, SUBSCRIPTION, this)]
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
        let lexed = crate::tokens::lex("DROP SUBSCRIPTION sub1 CASCADE");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = DropSubscriptionStmt::parse(&mut input).unwrap().into_ast();
        assert_eq!(stmt.name.text(), "sub1");
        assert!(stmt.behavior.is_some());
        assert!(input.is_eof());
    }

    /// `ALTER SUBSCRIPTION name SET (origin = 'value')` — string-valued
    /// def_arg, sanity test for the SetDef path.
    #[test]
    fn parse_alter_subscription_set_origin_string() {
        let lexed = crate::tokens::lex("ALTER SUBSCRIPTION regress_testsub4 SET (origin = 'none')");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterSubscriptionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
    }

    /// `ALTER SUBSCRIPTION name SET (origin = any)` — `any` is a reserved
    /// keyword used as a `def_arg` value (gram.y `def_arg` accepts
    /// `reserved_keyword`). subscription.sql corpus uses this.
    #[test]
    fn parse_alter_subscription_set_origin_any() {
        let lexed = crate::tokens::lex("ALTER SUBSCRIPTION regress_testsub4 SET (origin = any)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = AlterSubscriptionStmt::parse(&mut input).unwrap().into_ast();
        assert!(input.is_eof());
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
