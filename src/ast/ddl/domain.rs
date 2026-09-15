//! DOMAIN DDL statements (CREATE/ALTER/DROP).
#![allow(unused_imports)]

use crate::ast::ddl::trigger::ConstraintAttributeElem;
use crate::ast::shared::expr::*;
use crate::ast::shared::flags::*;
use crate::ast::shared::names::*;
use crate::ast::shared::numbers::*;
use crate::tokens::{literal, punct};

recursa::ast_node! {
    /// `COLLATE name` clause on a domain.
    #[derive(Debug)]
    pub struct DomainCollate {
        #[tok(COLLATE, this)]
        pub name: QualifiedName,
    }
}

recursa::ast_node! {
    /// `[CONSTRAINT name]` prefix on a domain constraint.
    #[derive(Debug)]
    pub struct DomainConstraintName {
        #[tok(CONSTRAINT, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `NOT NULL` domain constraint body.
    #[derive(Debug)]
    pub enum DomainNotNull {
        #[tok(NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
    /// `CHECK (expr)` domain constraint body.
    #[derive(Debug)]
    pub struct DomainCheckBody {
        #[tok(CHECK, LPAREN, this, RPAREN)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `DEFAULT expr` clause — domain default value.
    #[derive(Debug)]
    pub struct DomainDefault {
        /// gram.y `ColConstraintElem: DEFAULT b_expr` (the restricted expression
        /// grammar; exclusions as in `PositionInner`), so `DEFAULT 1 NOT NULL`
        /// ends the default at `NOT`.
        #[parse(pratt(exclude(
            Default,
            Collate,
            QuantifiedComparisonCmp,
            QuantifiedComparisonLike,
            QuantifiedComparisonOp,
            QuantifiedComparisonAdd,
            QuantifiedComparisonMul,
            QuantifiedComparisonPow,
            IsJson,
            IsNormalized,
            BoolTest,
            Notnull,
            Isnull,
            AtLocal,
            AtTimeZone,
            NotInExpr,
            NotIlike,
            NotSimilarTo,
            NotLike,
            SimilarTo,
            Ilike,
            Like,
            Overlaps,
            InExpr,
            NotBetweenExpr,
            BetweenExpr,
            Or,
            And
        )))]
        #[tok(DEFAULT, this)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// Body of a domain constraint — Postgres' `DomainConstraintElem` plus the
    /// `DEFAULT expr` form (which is split out from `ColConstraintElem` by
    /// `SplitColQualList` in gram.y).
    ///
    /// Variant ordering: `NotNull` (`NOT NULL`, 2 tokens) before `Null`; `Check`
    /// and `Default` are keyword-led and unambiguous.
    #[derive(Debug)]
    pub enum DomainConstraintBody {
        NotNull(DomainNotNull),
        #[tok(NULL)]
        Null,
        Check(DomainCheckBody),
        Default(DomainDefault),
    }
}

recursa::ast_node! {
    /// A single domain constraint — `[CONSTRAINT name] body`.
    #[derive(Debug)]
    pub struct DomainConstraint {
        pub name: Option<DomainConstraintName>,
        pub body: DomainConstraintBody,
    }
}

recursa::ast_node! {
    /// `CREATE DOMAIN name [AS] Typename [COLLATE name] [constraint_list]`.
    #[derive(Debug)]
    pub struct CreateDomainStmt {
        #[tok(CREATE, DOMAIN, this)]
        pub name: QualifiedName,
        #[tok(optional(AS), this)]
        pub type_name: CastType,
        pub collate: Option<DomainCollate>,
        pub constraints: zero_or_many!(DomainConstraint),
    }
}

recursa::ast_node! {
    /// `DROP DOMAIN [IF EXISTS] type [, ...] [CASCADE | RESTRICT]`.
    #[derive(Debug)]
    #[tok(DROP, DOMAIN, this)]
    pub struct DropDomainStmt {
        pub if_exists: Option<IfExists>,
        pub types: TypeNameList,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `CHECK (expr) ConstraintAttributeSpec` — Postgres' CHECK arm of
    /// `DomainConstraintElem` (the ALTER DOMAIN-specific form). Differs from
    /// CREATE DOMAIN's `DomainCheckBody` by carrying the optional trailing
    /// `ConstraintAttributeSpec` (e.g. `NOT VALID`).
    #[derive(Debug)]
    pub struct AlterDomainCheckConstraint {
        #[tok(CHECK, LPAREN, this, RPAREN)]
        pub expr: boxed!(Expr),
        pub attrs: zero_or_many!(ConstraintAttributeElem),
    }
}

recursa::ast_node! {
    /// `NOT NULL ConstraintAttributeSpec` — Postgres' NOT NULL arm of
    /// `DomainConstraintElem` (ALTER DOMAIN-specific form). The corpus
    /// exercises the bare `NOT NULL` form as well as the optional
    /// `ConstraintAttributeSpec` trailer the grammar allows.
    ///
    /// The `NOT NULL` keywords sit on the struct, not on the `attrs` field: a
    /// field-level `#[tok(...)]` on a repeated field binds to each element, so
    /// it would demand one `NOT NULL` per attribute and reject the bare form
    /// (which has no attributes at all).
    #[derive(Debug)]
    #[tok(NOT, NULL, this)]
    pub struct AlterDomainNotNullConstraint {
        pub attrs: zero_or_many!(ConstraintAttributeElem),
    }
}

recursa::ast_node! {
    /// One body of an ALTER DOMAIN ADD constraint — Postgres'
    /// `DomainConstraintElem`.
    ///
    /// Variant ordering: variants begin with distinct keywords (`CHECK` /
    /// `NOT`), so order is for clarity only.
    #[derive(Debug)]
    pub enum AlterDomainConstraintElem {
        Check(AlterDomainCheckConstraint),
        NotNull(AlterDomainNotNullConstraint),
    }
}

recursa::ast_node! {
    /// `[CONSTRAINT name] DomainConstraintElem` on ALTER DOMAIN ADD —
    /// reuses the shared `DomainConstraintName` prefix from CREATE DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainConstraint {
        pub name: Option<DomainConstraintName>,
        pub elem: AlterDomainConstraintElem,
    }
}

recursa::ast_node! {
    /// `ADD [CONSTRAINT name] DomainConstraintElem` — ADD action on ALTER DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainAdd {
        #[tok(ADD, this)]
        pub constraint: AlterDomainConstraint,
    }
}

recursa::ast_node! {
    /// `DROP CONSTRAINT [IF EXISTS] name [CASCADE | RESTRICT]` — DROP CONSTRAINT
    /// action on ALTER DOMAIN.
    #[derive(Debug)]
    #[tok(DROP, CONSTRAINT, this)]
    pub struct AlterDomainDropConstraint {
        pub if_exists: Option<IfExists>,
        pub name: crate::tokens::ColId,
        pub behavior: Option<DropBehavior>,
    }
}

recursa::ast_node! {
    /// `VALIDATE CONSTRAINT name` — VALIDATE action on ALTER DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainValidate {
        #[tok(VALIDATE, CONSTRAINT, this)]
        pub name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `RENAME CONSTRAINT old TO new` — RenameStmt branch for domain constraints.
    #[derive(Debug)]
    pub struct AlterDomainRenameConstraint {
        #[tok(RENAME, CONSTRAINT, this)]
        pub old_name: crate::tokens::ColId,
        #[tok(TO, this)]
        pub new_name: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// `SET DEFAULT expr` — SET DEFAULT action on ALTER DOMAIN.
    #[derive(Debug)]
    pub struct AlterDomainSetDefault {
        #[tok(SET, DEFAULT, this)]
        pub expr: boxed!(Expr),
    }
}

recursa::ast_node! {
    /// `DROP DEFAULT` — DROP DEFAULT action on ALTER DOMAIN.
    #[derive(Debug)]
    pub enum AlterDomainDropDefault {
        #[tok(DROP, DEFAULT)]
        Value,
    }
}

recursa::ast_node! {
    /// `SET NOT NULL` — SET NOT NULL action on ALTER DOMAIN.
    #[derive(Debug)]
    pub enum AlterDomainSetNotNull {
        #[tok(SET, NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
    /// `DROP NOT NULL` — DROP NOT NULL action on ALTER DOMAIN.
    #[derive(Debug)]
    pub enum AlterDomainDropNotNull {
        #[tok(DROP, NOT, NULL)]
        Value,
    }
}

recursa::ast_node! {
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
    #[derive(Debug)]
    pub enum AlterDomainAction {
        Add(AlterDomainAdd),
        DropConstraint(AlterDomainDropConstraint),
        DropNotNull(AlterDomainDropNotNull),
        DropDefault(AlterDomainDropDefault),
        SetNotNull(AlterDomainSetNotNull),
        SetDefault(AlterDomainSetDefault),
        SetSchema(SetSchemaClause),
        Validate(AlterDomainValidate),
        RenameConstraint(AlterDomainRenameConstraint),
        Rename(RenameTo),
        Owner(OwnerTo),
    }
}

recursa::ast_node! {
    /// `ALTER DOMAIN any_name action` — Postgres' `AlterDomainStmt`,
    /// `RenameStmt`, `AlterOwnerStmt`, and `AlterObjectSchemaStmt` branches.
    #[derive(Debug)]
    pub struct AlterDomainStmt {
        #[tok(ALTER, DOMAIN, this)]
        pub name: QualifiedName,
        pub action: AlterDomainAction,
    }
}
