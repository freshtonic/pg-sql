pub mod ast;
pub mod bench_data;
pub mod flame;
pub mod flame_report;
pub mod formatter;
pub mod tokens;

#[cfg(feature = "arbitrary")]
pub use arbitrary;

// Generated first-set dispatch helpers and parse_with_prefix impls.
// Included inside a private module so that the generated code can reference
// types by simple name (via the glob imports below) rather than requiring
// every type to be fully qualified. The helper *functions* are re-exported
// to the crate root so they are callable from any module in this crate.
pub mod __firstset {
    #[allow(unused_imports)]
    use crate::tokens::keyword::*;
    #[allow(unused_imports)]
    use crate::tokens::literal::*;
    #[allow(unused_imports)]
    use crate::tokens::punct::*;
    #[allow(unused_imports)]
    use crate::tokens::soft_keyword::*;
    // Module aliases needed because generated code preserves the original
    // source's unqualified module paths (e.g. `punct::LParen`, `literal::Ident`).
    #[allow(unused_imports)]
    use crate::ast::cursor::declare::*;
    #[allow(unused_imports)]
    use crate::ast::cursor::fetch::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::access_method::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::aggregate::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::cast::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::collation::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::conversion::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::database::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::domain::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::extension::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::foreign::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::function::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::index::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::language::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::large_object::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::materialized_view::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::operator::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::policy::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::procedure::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::publication::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::role::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::rule::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::schema::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::sequence::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::statistics::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::subscription::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::table::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::tablespace::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::text_search::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::transform::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::trigger::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::r#type::*;
    #[allow(unused_imports)]
    use crate::ast::ddl::view::*;
    #[allow(unused_imports)]
    use crate::ast::dml::delete::*;
    #[allow(unused_imports)]
    use crate::ast::dml::insert::*;
    #[allow(unused_imports)]
    use crate::ast::dml::merge::*;
    #[allow(unused_imports)]
    use crate::ast::dml::select::*;
    #[allow(unused_imports)]
    use crate::ast::dml::update::*;
    #[allow(unused_imports)]
    use crate::ast::dml::values::*;
    #[allow(unused_imports)]
    use crate::ast::session::discard::*;
    #[allow(unused_imports)]
    use crate::ast::session::notify::*;
    #[allow(unused_imports)]
    use crate::ast::session::set_reset::*;
    #[allow(unused_imports)]
    use crate::ast::shared::expr::*;
    #[allow(unused_imports)]
    use crate::ast::shared::flags::*;
    #[allow(unused_imports)]
    use crate::ast::shared::names::*;
    #[allow(unused_imports)]
    use crate::ast::shared::numbers::*;
    #[allow(unused_imports)]
    use crate::ast::shared::with_clause::*;
    #[allow(unused_imports)]
    use crate::ast::tcl::prepared::*;
    #[allow(unused_imports)]
    use crate::ast::tcl::savepoint::*;
    #[allow(unused_imports)]
    use crate::ast::tcl::transaction::*;
    #[allow(unused_imports)]
    use crate::ast::utility::analyze::*;
    #[allow(unused_imports)]
    use crate::ast::utility::checkpoint::*;
    #[allow(unused_imports)]
    use crate::ast::utility::cluster::*;
    #[allow(unused_imports)]
    use crate::ast::utility::comment::*;
    #[allow(unused_imports)]
    use crate::ast::utility::copy::*;
    #[allow(unused_imports)]
    use crate::ast::utility::r#do::*;
    #[allow(unused_imports)]
    use crate::ast::utility::explain::*;
    #[allow(unused_imports)]
    use crate::ast::utility::grant::*;
    #[allow(unused_imports)]
    use crate::ast::utility::lock::*;
    #[allow(unused_imports)]
    use crate::ast::utility::ownership::*;
    #[allow(unused_imports)]
    use crate::ast::utility::refresh::*;
    #[allow(unused_imports)]
    use crate::ast::utility::reindex::*;
    #[allow(unused_imports)]
    use crate::ast::utility::truncate::*;
    #[allow(unused_imports)]
    use crate::ast::utility::vacuum::*;
    #[allow(unused_imports)]
    use crate::tokens::{keyword, literal, punct, soft_keyword};
    // Target types generated by the `targets { ... }` block. The codegen
    // emits dispatch code that references these by simple name.
    #[allow(unused_imports)]
    use crate::tokens::{
        BareColLabel, BareColLabelKeyword, ColId, ColIdKeyword, ColLabel, ColLabelKeyword,
        NonReservedWord, NonReservedWordKeyword, type_function_name, type_function_nameKeyword,
    };
    // Types defined directly in ast/mod.rs
    #[allow(unused_imports)]
    use crate::ast::{
        FileItem, PsqlCommand, PsqlDirective, PsqlTerminator, Statement, StatementTerminator,
        TerminatedStatement,
    };
    #[allow(unused_imports)]
    use ::recursa::*;
    #[allow(unused_imports)]
    use ::recursa_core::seq::*;
    #[allow(unused_imports)]
    use ::recursa_core::surrounded::Surrounded;
    #[allow(unused_imports)]
    use ::recursa_core::*;

    include!("generated/first_set.rs");
}
#[allow(unused_imports)]
pub(crate) use __firstset::*;
