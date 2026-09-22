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

recursa::ast_node! {
    /// `CONNECTION sconst` clause on CREATE SUBSCRIPTION.
    #[derive(Debug)]
    pub struct SubscriptionConnectionClause {
        #[tok(CONNECTION, this)]
        pub conninfo: CopySconst,
    }
}

recursa::ast_node! {
    /// `PUBLICATION name_list` clause on CREATE SUBSCRIPTION — Postgres'
    /// `PUBLICATION name_list`. Each name is an identifier (publication
    /// names are not qualified).
    #[derive(Debug)]
    #[tok(PUBLICATION, this)]
    pub struct SubscriptionPublicationClause {
        #[sep(COMMA)]
        pub names: one_or_many!(crate::tokens::ColId),
    }
}

#[cfg(feature = "since-pg19")]
recursa::ast_node! {
    /// `SERVER name` — the foreign server of a subscription, added in 19
    /// (gram.y b73d13c:11045, 11084; research PostgreSQL 19, "Changes to
    /// existing statements", commit 8185bb534).
    #[derive(Debug)]
    pub struct SubscriptionServerClause {
        #[tok(SERVER, this)]
        pub server: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Where a subscription connects: `CONNECTION sconst`, or from 19 a
    /// foreign `SERVER name`.
    #[derive(Debug)]
    pub enum SubscriptionSource {
        Connection(SubscriptionConnectionClause),
        /// Added in 19: `CREATE SUBSCRIPTION name SERVER name PUBLICATION
        /// name_list opt_definition` (gram.y b73d13c:11045).
        #[cfg(feature = "since-pg19")]
        Server(SubscriptionServerClause),
    }
}

recursa::ast_node! {
    /// `CREATE SUBSCRIPTION name { CONNECTION sconst | SERVER name }
    /// PUBLICATION name_list [WITH (def_list)]` — Postgres'
    /// `CreateSubscriptionStmt`. The `SERVER` form is 19 only.
    #[derive(Debug)]
    pub struct CreateSubscriptionStmt {
        #[tok(CREATE, SUBSCRIPTION, this)]
        pub name: crate::tokens::ColId,
        pub source: SubscriptionSource,
        pub publication_clause: SubscriptionPublicationClause,
        pub with: Option<WithDefinition>,
    }
}

recursa::ast_node! {
    /// `DROP SUBSCRIPTION [IF EXISTS] name [CASCADE | RESTRICT]`.
    ///
    /// Postgres' `DropSubscriptionStmt` rule takes a single `name`, not a list.
    #[derive(Debug)]
    #[tok(DROP, SUBSCRIPTION, this)]
    pub struct DropSubscriptionStmt {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `CONNECTION sconst` — Postgres' `ALTER SUBSCRIPTION name CONNECTION
    /// sconst` and also the (already-modelled, in `SubscriptionConnectionClause`)
    /// `CREATE SUBSCRIPTION ... CONNECTION ...` form.
    ///
    /// pg-sql reuses [`SubscriptionConnectionClause`] for this branch.
    #[derive(Debug)]
    #[tok(REFRESH, PUBLICATION, this)]
    pub struct AlterSubscriptionRefresh {
        pub with: Option<WithDefinition>,
    }
}

recursa::ast_node! {
    /// `ADD PUBLICATION name_list [WITH (def_list)]` — Postgres'
    /// `ALTER SUBSCRIPTION ... ADD PUBLICATION ...` form.
    #[derive(Debug)]
    #[tok(ADD, PUBLICATION, this)]
    pub struct AlterSubscriptionAddPublication {
        #[sep(COMMA)]
        pub names: one_or_many!(crate::tokens::ColId),
        pub with: Option<WithDefinition>,
    }
}

recursa::ast_node! {
    /// `DROP PUBLICATION name_list [WITH (def_list)]` — Postgres'
    /// `ALTER SUBSCRIPTION ... DROP PUBLICATION ...` form.
    #[derive(Debug)]
    #[tok(DROP, PUBLICATION, this)]
    pub struct AlterSubscriptionDropPublication {
        #[sep(COMMA)]
        pub names: one_or_many!(crate::tokens::ColId),
        pub with: Option<WithDefinition>,
    }
}

recursa::ast_node! {
    /// `SET PUBLICATION name_list [WITH (def_list)]` — Postgres'
    /// `ALTER SUBSCRIPTION ... SET PUBLICATION ...` form. Distinct from
    /// `SET CONNECTION sconst` (kept separate variant) and from
    /// `SET (def_list)` (modelled via [`SetDefinitionClause`]).
    #[derive(Debug)]
    #[tok(SET, PUBLICATION, this)]
    pub struct AlterSubscriptionSetPublication {
        #[sep(COMMA)]
        pub names: one_or_many!(crate::tokens::ColId),
        pub with: Option<WithDefinition>,
    }
}

recursa::ast_node! {
    /// `SKIP (def_list)` — Postgres' `ALTER SUBSCRIPTION ... SKIP definition`.
    #[derive(Debug)]
    #[tok(SKIP, LPAREN, this, RPAREN)]
    pub struct AlterSubscriptionSkip {
        #[sep(COMMA)]
        pub items: one_or_many!(DefElem),
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterSubscriptionAction {
        Rename(RenameTo),
        Owner(OwnerTo),
        Connection(SubscriptionConnectionClause),
        /// Added in 19: `ALTER SUBSCRIPTION name SERVER name` (gram.y
        /// b73d13c:11084; research PostgreSQL 19, "Changes to existing
        /// statements").
        #[cfg(feature = "since-pg19")]
        Server(SubscriptionServerClause),
        Refresh(AlterSubscriptionRefresh),
        /// Added in 19: `ALTER SUBSCRIPTION name REFRESH SEQUENCES` (gram.y
        /// b73d13c:11104, commit f0b3573c3).
        #[cfg(feature = "since-pg19")]
        #[tok(REFRESH, SEQUENCES)]
        RefreshSequences,
        AddPublication(AlterSubscriptionAddPublication),
        DropPublication(AlterSubscriptionDropPublication),
        Skip(AlterSubscriptionSkip),
        #[tok(ENABLE)]
        Enable,
        #[tok(DISABLE)]
        Disable,
        SetPublication(AlterSubscriptionSetPublication),
        SetDef(SetDefinitionClause),
    }
}

recursa::ast_node! {
    /// `ALTER SUBSCRIPTION name action` — Postgres' `AlterSubscriptionStmt`
    /// plus the subscription branches of `RenameStmt` / `AlterOwnerStmt`.
    #[derive(Debug)]
    pub struct AlterSubscriptionStmt {
        #[tok(ALTER, SUBSCRIPTION, this)]
        pub name: crate::tokens::ColId,
        pub action: AlterSubscriptionAction,
    }
}
