//! Compiles the embedded test bodies under `embedded-tests/src` as one
//! integration-test target instead of inside the library.
//!
//! Each test body was written at its original module position and names
//! items through `super::*` and `crate::...` paths. The module tree below
//! mirrors those positions: every mirrored module re-exports the matching
//! `pg_sql` module, and the crate root re-exports `pg_sql` itself, so the
//! bodies compile unchanged against the library's public interface.
#![allow(unused_imports)]

pub use pg_sql::*;

pub mod ast {
    pub use pg_sql::ast::*;
    pub mod test_support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/embedded-tests/src/ast/test_support.body.rs"
        ));
    }
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded-tests/src/ast/mod.tests.rs"
    ));
    pub mod cursor {
        pub use pg_sql::ast::cursor::*;
        pub mod declare {
            pub use pg_sql::ast::cursor::declare::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/cursor/declare.tests.rs"
            ));
        }
        pub mod fetch {
            pub use pg_sql::ast::cursor::fetch::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/cursor/fetch.tests.rs"
            ));
        }
    }
    pub mod ddl {
        pub use pg_sql::ast::ddl::*;
        pub mod access_method {
            pub use pg_sql::ast::ddl::access_method::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/access_method.tests.rs"
            ));
        }
        pub mod aggregate {
            pub use pg_sql::ast::ddl::aggregate::*;
            pub use pg_sql::ast::shared::names::AggregateArgs;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/aggregate.tests.rs"
            ));
        }
        pub mod cast {
            pub use pg_sql::ast::ddl::cast::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/cast.tests.rs"
            ));
        }
        pub mod collation {
            pub use pg_sql::ast::ddl::collation::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/collation.tests.rs"
            ));
        }
        pub mod conversion {
            pub use pg_sql::ast::ddl::conversion::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/conversion.tests.rs"
            ));
        }
        pub mod database {
            pub use pg_sql::ast::ddl::database::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/database.tests.rs"
            ));
        }
        pub mod domain {
            pub use pg_sql::ast::ddl::domain::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/domain.tests.rs"
            ));
        }
        pub mod extension {
            pub use pg_sql::ast::ddl::extension::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/extension.tests.rs"
            ));
        }
        pub mod foreign {
            pub use pg_sql::ast::ddl::foreign::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/foreign.tests.rs"
            ));
        }
        pub mod function {
            pub use pg_sql::ast::ddl::function::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/function.tests.rs"
            ));
        }
        pub mod index {
            pub use pg_sql::ast::ddl::index::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/index.tests.rs"
            ));
        }
        pub mod language {
            pub use pg_sql::ast::ddl::language::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/language.tests.rs"
            ));
        }
        pub mod large_object {
            pub use pg_sql::ast::ddl::large_object::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/large_object.tests.rs"
            ));
        }
        pub mod materialized_view {
            pub use pg_sql::ast::ddl::materialized_view::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/materialized_view.tests.rs"
            ));
        }
        pub mod operator {
            pub use pg_sql::ast::ddl::operator::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/operator.tests.rs"
            ));
        }
        pub mod policy {
            pub use pg_sql::ast::ddl::policy::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/policy.tests.rs"
            ));
        }
        pub mod procedure {
            pub use pg_sql::ast::ddl::function::AlterFuncAction;
            pub use pg_sql::ast::ddl::procedure::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/procedure.tests.rs"
            ));
        }
        pub mod publication {
            pub use pg_sql::ast::ddl::publication::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/publication.tests.rs"
            ));
        }
        pub mod role {
            pub use pg_sql::ast::ddl::role::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/role.tests.rs"
            ));
        }
        pub mod rule {
            pub use pg_sql::ast::ddl::rule::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/rule.tests.rs"
            ));
        }
        pub mod schema {
            pub use pg_sql::ast::ddl::schema::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/schema.tests.rs"
            ));
        }
        pub mod sequence {
            pub use pg_sql::ast::ddl::sequence::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/sequence.tests.rs"
            ));
        }
        pub mod statistics {
            pub use pg_sql::ast::ddl::statistics::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/statistics.tests.rs"
            ));
        }
        pub mod subscription {
            pub use pg_sql::ast::ddl::subscription::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/subscription.tests.rs"
            ));
        }
        pub mod table {
            pub use pg_sql::ast::ddl::table::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/table.tests.rs"
            ));
        }
        pub mod tablespace {
            pub use pg_sql::ast::ddl::tablespace::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/tablespace.tests.rs"
            ));
        }
        pub mod text_search {
            pub use pg_sql::ast::ddl::text_search::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/text_search.tests.rs"
            ));
        }
        pub mod transform {
            pub use pg_sql::ast::ddl::transform::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/transform.tests.rs"
            ));
        }
        pub mod trigger {
            pub use pg_sql::ast::ddl::trigger::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/trigger.tests.rs"
            ));
        }
        pub mod r#type {
            pub use pg_sql::ast::ddl::r#type::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/type.tests.rs"
            ));
        }
        pub mod view {
            pub use pg_sql::ast::ddl::view::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/ddl/view.tests.rs"
            ));
        }
    }
    pub mod dml {
        pub use pg_sql::ast::dml::*;
        pub mod delete {
            pub use pg_sql::ast::dml::delete::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/dml/delete.tests.rs"
            ));
        }
        pub mod insert {
            pub use pg_sql::ast::dml::insert::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/dml/insert.tests.rs"
            ));
        }
        pub mod merge {
            pub use pg_sql::ast::dml::merge::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/dml/merge.tests.rs"
            ));
        }
        pub mod select {
            pub use pg_sql::ast::dml::select::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/dml/select.tests.rs"
            ));
        }
        pub mod update {
            pub use pg_sql::ast::dml::update::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/dml/update.tests.rs"
            ));
        }
        pub mod values {
            pub use pg_sql::ast::dml::values::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/dml/values.tests.rs"
            ));
        }
    }
    pub mod session {
        pub use pg_sql::ast::session::*;
        pub mod discard {
            pub use pg_sql::ast::session::discard::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/session/discard.tests.rs"
            ));
        }
        pub mod notify {
            pub use pg_sql::ast::session::notify::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/session/notify.tests.rs"
            ));
        }
        pub mod set_reset {
            pub use pg_sql::ast::session::set_reset::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/session/set_reset.tests.rs"
            ));
        }
    }
    pub mod shared {
        pub use pg_sql::ast::shared::*;
        pub mod expr {
            pub use pg_sql::ast::shared::expr::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/shared/expr.tests.rs"
            ));
        }
        pub mod with_clause {
            pub use pg_sql::ast::shared::with_clause::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/shared/with_clause.tests.rs"
            ));
        }
    }
    pub mod tcl {
        pub use pg_sql::ast::tcl::*;
        pub mod prepared {
            pub use pg_sql::ast::tcl::prepared::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/tcl/prepared.tests.rs"
            ));
        }
        pub mod savepoint {
            pub use pg_sql::ast::tcl::savepoint::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/tcl/savepoint.tests.rs"
            ));
        }
        pub mod transaction {
            pub use pg_sql::ast::tcl::transaction::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/tcl/transaction.tests.rs"
            ));
        }
    }
    pub mod utility {
        pub use pg_sql::ast::utility::*;
        pub mod analyze {
            pub use pg_sql::ast::utility::analyze::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/analyze.tests.rs"
            ));
        }
        pub mod cluster {
            pub use pg_sql::ast::utility::cluster::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/cluster.tests.rs"
            ));
        }
        pub mod comment {
            pub use pg_sql::ast::utility::comment::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/comment.tests.rs"
            ));
        }
        pub mod copy {
            pub use pg_sql::ast::utility::copy::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/copy.tests.rs"
            ));
        }
        pub mod explain {
            pub use pg_sql::ast::utility::explain::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/explain.tests.rs"
            ));
        }
        pub mod grant {
            pub use pg_sql::ast::utility::grant::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/grant.tests.rs"
            ));
        }
        pub mod lock {
            pub use pg_sql::ast::utility::lock::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/lock.tests.rs"
            ));
        }
        pub mod ownership {
            pub use pg_sql::ast::utility::ownership::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/ownership.tests.rs"
            ));
        }
        pub mod refresh {
            pub use pg_sql::ast::utility::refresh::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/refresh.tests.rs"
            ));
        }
        pub mod reindex {
            pub use pg_sql::ast::utility::reindex::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/reindex.tests.rs"
            ));
        }
        pub mod truncate {
            pub use pg_sql::ast::utility::truncate::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/truncate.tests.rs"
            ));
        }
        pub mod vacuum {
            pub use pg_sql::ast::utility::vacuum::*;
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/embedded-tests/src/ast/utility/vacuum.tests.rs"
            ));
        }
    }
}

pub mod bench_data {
    pub use pg_sql::bench_data::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded-tests/src/bench_data.tests.rs"
    ));
}

pub mod tokens {
    pub use pg_sql::tokens::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded-tests/src/tokens.ident_enum_tests.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded-tests/src/tokens.test_input.rs"
    ));
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded-tests/src/tokens.tests.rs"
    ));
}
