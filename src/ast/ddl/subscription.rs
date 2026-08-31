//! SUBSCRIPTION DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::publication::{SetDefinitionClause, WithDefinition};
use crate::ast::ddl::role::DefElem;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::ast::utility::copy::CopySconst;
use crate::tokens::{literal, punct};

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
#[tok(PUBLICATION, this)]
pub struct SubscriptionPublicationClause<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input>>,
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
#[tok(DROP, SUBSCRIPTION, this)]
pub struct DropSubscriptionStmt<'input> {
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
#[tok(REFRESH, PUBLICATION, this)]
pub struct AlterSubscriptionRefresh<'input> {
    pub with: Option<WithDefinition<'input>>,
}

/// `ADD PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... ADD PUBLICATION ...` form.
#[derive(recursa::Node, Debug, Clone)]
#[tok(ADD, PUBLICATION, this)]
pub struct AlterSubscriptionAddPublication<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input>>,
    pub with: Option<WithDefinition<'input>>,
}

/// `DROP PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... DROP PUBLICATION ...` form.
#[derive(recursa::Node, Debug, Clone)]
#[tok(DROP, PUBLICATION, this)]
pub struct AlterSubscriptionDropPublication<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input>>,
    pub with: Option<WithDefinition<'input>>,
}

/// `SET PUBLICATION name_list [WITH (def_list)]` — Postgres'
/// `ALTER SUBSCRIPTION ... SET PUBLICATION ...` form. Distinct from
/// `SET CONNECTION sconst` (kept separate variant) and from
/// `SET (def_list)` (modelled via [`SetDefinitionClause`]).
#[derive(recursa::Node, Debug, Clone)]
#[tok(SET, PUBLICATION, this)]
pub struct AlterSubscriptionSetPublication<'input> {
    #[sep(COMMA)]
    pub names: recursa::Vec1<crate::tokens::ColId<'input>>,
    pub with: Option<WithDefinition<'input>>,
}

/// `SKIP (def_list)` — Postgres' `ALTER SUBSCRIPTION ... SKIP definition`.
#[derive(recursa::Node, Debug, Clone)]
#[tok(SKIP, LPAREN, this, RPAREN)]
pub struct AlterSubscriptionSkip<'input> {
    #[sep(COMMA)]
    pub items: recursa::Vec1<DefElem<'input>>,
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
    #[tok(ENABLE)]
    Enable,
    #[tok(DISABLE)]
    Disable,
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

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/embedded-tests/src/ast/ddl/subscription.tests.rs"
));
