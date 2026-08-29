pub mod cursor;
pub mod ddl;
pub mod dml;
pub mod file;
pub mod session;
pub mod shared;
pub mod tcl;
pub mod utility;

// Shared test helpers (`parse_stmt`, `reparse_stable`, `roundtrip`) used by
// the per-file `mod tests` blocks. Gated on `cfg(test)` so the module is
// elided from release builds.
#[cfg(test)]
pub(crate) mod test_support;

pub use self::file::{PsqlTerminator, StatementTerminator, TerminatedStatement};

use recursa_diagram::railroad;

// The `Statement` enum references ~170 *Stmt types defined across every
// sub-module of `ast::{ddl,dml,tcl,cursor,session,utility,shared}`.
// Glob-import each sub-module so variant bodies can spell the types by
// short name; this replaces the now-removed `simple_stmts::*` shim.
use self::{
    cursor::declare::*,
    cursor::fetch::*,
    ddl::access_method::*,
    ddl::aggregate::*,
    ddl::cast::*,
    ddl::collation::*,
    ddl::conversion::*,
    ddl::database::*,
    ddl::domain::*,
    ddl::extension::*,
    ddl::foreign::*,
    ddl::function::*,
    ddl::index::*,
    ddl::language::*,
    ddl::large_object::*,
    ddl::materialized_view::*,
    ddl::operator::*,
    ddl::policy::*,
    ddl::procedure::*,
    ddl::publication::*,
    ddl::role::*,
    ddl::rule::*,
    ddl::schema::*,
    ddl::sequence::*,
    ddl::statistics::*,
    ddl::subscription::*,
    ddl::table::*,
    ddl::tablespace::*,
    ddl::text_search::*,
    ddl::transform::*,
    ddl::trigger::*,
    ddl::r#type::*,
    ddl::view::*,
    dml::delete::DeleteStmt,
    dml::insert::InsertStmt,
    dml::merge::MergeStmt,
    dml::select::SelectStmt,
    dml::update::UpdateStmt,
    dml::values::{Subquery, TableStmt},
    session::discard::*,
    session::notify::*,
    session::set_reset::{
        LoadStmt, ResetStmt, SetRoleStmt, SetSessionAuthStmt, SetStmt, SetTimeZoneStmt,
        SetXmlOptionStmt, ShowStmt,
    },
    shared::with_clause::WithStatement,
    tcl::prepared::*,
    tcl::savepoint::*,
    tcl::transaction::*,
    utility::analyze::AnalyzeStmt,
    utility::checkpoint::*,
    utility::cluster::*,
    utility::comment::*,
    utility::copy::*,
    utility::r#do::*,
    utility::explain::ExplainStmt,
    utility::grant::*,
    utility::lock::*,
    utility::ownership::*,
    utility::refresh::*,
    utility::reindex::*,
    utility::truncate::*,
    utility::vacuum::*,
};

/// Top-level SQL statement.
///
/// Variant ordering matters for disambiguation. More specific (longer leading
/// keyword sequences) must come before less specific:
/// - `With` must come before `Select` so `WITH ... SELECT` matches before bare `SELECT`.
/// - `Explain` wraps a Statement, so it must come before `Select`.
/// - `CreateFunction` and `CreateIndex` come before `CreateTable` because they
///   have `CREATE FUNCTION` / `CREATE INDEX` which are longer than `CREATE TABLE`.
///   `CreateView` likewise comes before `CreateTable`.
///   `CreateTable` handles regular, partitioned, and partition-of forms internally.
/// - `DropFunction` and `DropIndex` come before `DropTable` for the same reason.
/// - `Values` (CompoundQuery) starts with VALUES/TABLE/SELECT so it could
///   conflict. It must come after Explain but before bare Select to handle
///   `VALUES ... UNION ALL ...` and `TABLE tablename`.
#[derive(recursa::Node, Debug, Clone)]
pub enum Statement<'input> {
    // --- Multi-keyword statements (longest first_pattern first) ---
    #[railroad(label = "WITH ..")]
    With(Box<WithStatement<'input>>),
    #[railroad(label = "EXPLAIN ..")]
    Explain(Box<ExplainStmt<'input>>),
    // CREATE variants: multi-keyword before single-keyword
    #[railroad(label = "CREATE .. FUNCTION ..")]
    CreateFunction(Box<CreateFunctionStmt<'input>>),
    #[railroad(label = "CREATE .. PROCEDURE ..")]
    CreateProcedure(Box<CreateProcedureStmt<'input>>),
    #[railroad(label = "CREATE TABLESPACE ..")]
    CreateTablespace(Box<CreateTablespaceStmt<'input>>),
    #[railroad(label = "IMPORT FOREIGN SCHEMA ..")]
    ImportForeignSchema(Box<ImportForeignSchemaStmt<'input>>),
    #[railroad(label = "CREATE CONSTRAINT ..")]
    CreateConstraintTrigger(Box<CreateConstraintTriggerStmt<'input>>),
    #[railroad(label = "CREATE TRIGGER ..")]
    CreateTrigger(Box<CreateTriggerStmt<'input>>),
    #[railroad(label = "CREATE EVENT TRIGGER ..")]
    CreateEventTrigger(Box<CreateEventTriggerStmt<'input>>),
    #[railroad(label = "CREATE ACCESS METHOD ..")]
    CreateAccessMethod(CreateAccessMethodStmt<'input>),
    #[railroad(label = "CREATE MATERIALIZED VIEW ..")]
    CreateMaterializedView(Box<CreateMaterializedViewStmt<'input>>),
    #[railroad(label = "CREATE TEXT SEARCH ..")]
    CreateTextSearch(Box<CreateTextSearchStmt<'input>>),
    #[railroad(label = "CREATE FOREIGN ..")]
    CreateForeign(Box<CreateForeignStmt<'input>>),
    #[railroad(label = "CREATE INDEX ..")]
    CreateIndex(Box<CreateIndexStmt<'input>>),
    #[railroad(label = "CREATE VIEW ..")]
    CreateView(Box<CreateViewStmt<'input>>),
    #[railroad(label = "CREATE .. RULE ..")]
    CreateRule(Box<CreateRuleStmt<'input>>),
    #[railroad(label = "CREATE .. GROUP ..")]
    CreateGroup(CreateGroupStmt<'input>),
    #[railroad(label = "CREATE .. ROLE ..")]
    CreateRole(CreateRoleStmt<'input>),
    // `CREATE USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `CREATE USER ...` variant so longest-match-wins picks the
    // specific path.
    #[railroad(label = "CREATE USER MAPPING ..")]
    CreateUserMapping(Box<CreateUserMappingStmt<'input>>),
    #[railroad(label = "CREATE .. USER ..")]
    CreateUser(CreateUserStmt<'input>),
    #[railroad(label = "CREATE .. SCHEMA ..")]
    CreateSchema(CreateSchemaStmt<'input>),
    #[railroad(label = "CREATE .. SEQUENCE ..")]
    CreateSequence(Box<CreateSequenceStmt<'input>>),
    #[railroad(label = "CREATE .. TYPE ..")]
    CreateType(Box<CreateTypeStmt<'input>>),
    #[railroad(label = "CREATE .. DOMAIN ..")]
    CreateDomain(Box<CreateDomainStmt<'input>>),
    #[railroad(label = "CREATE .. AGGREGATE ..")]
    CreateAggregate(Box<CreateAggregateStmt<'input>>),
    // `CREATE OPERATOR CLASS ...` and `CREATE OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `CREATE OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    #[railroad(label = "CREATE OPERATOR CLASS ..")]
    CreateOperatorClass(Box<CreateOperatorClassStmt<'input>>),
    #[railroad(label = "CREATE OPERATOR FAMILY ..")]
    CreateOperatorFamily(CreateOperatorFamilyStmt<'input>),
    #[railroad(label = "CREATE .. OPERATOR ..")]
    CreateOperator(Box<CreateOperatorStmt<'input>>),
    #[railroad(label = "CREATE .. CAST ..")]
    CreateCast(Box<CreateCastStmt<'input>>),
    #[railroad(label = "CREATE .. TRANSFORM ..")]
    CreateTransform(Box<CreateTransformStmt<'input>>),
    #[railroad(label = "CREATE .. COLLATION ..")]
    CreateCollation(Box<CreateCollationStmt<'input>>),
    #[railroad(label = "CREATE .. EXTENSION ..")]
    CreateExtension(CreateExtensionStmt<'input>),
    #[railroad(label = "CREATE .. POLICY ..")]
    CreatePolicy(Box<CreatePolicyStmt<'input>>),
    #[railroad(label = "CREATE .. STATISTICS ..")]
    CreateStatistics(Box<CreateStatisticsStmt<'input>>),
    #[railroad(label = "CREATE .. PUBLICATION ..")]
    CreatePublication(Box<CreatePublicationStmt<'input>>),
    #[railroad(label = "CREATE .. SUBSCRIPTION ..")]
    CreateSubscription(Box<CreateSubscriptionStmt<'input>>),
    #[railroad(label = "CREATE .. CONVERSION ..")]
    CreateConversion(Box<CreateConversionStmt<'input>>),
    #[railroad(label = "CREATE .. SERVER ..")]
    CreateServer(Box<CreateServerStmt<'input>>),
    #[railroad(label = "CREATE .. LANGUAGE ..")]
    CreateLanguage(CreateLanguageStmt<'input>),
    #[railroad(label = "CREATE .. DATABASE ..")]
    CreateDatabase(CreateDatabaseStmt<'input>),
    #[railroad(label = "CREATE .. TABLE ..")]
    CreateTable(Box<CreateTableStmt<'input>>),
    // DROP variants
    #[railroad(label = "DROP FUNCTION ..")]
    DropFunction(Box<DropFunctionStmt<'input>>),
    #[railroad(label = "DROP PROCEDURE ..")]
    DropProcedure(Box<DropProcedureStmt<'input>>),
    #[railroad(label = "DROP ROUTINE ..")]
    DropRoutine(Box<DropRoutineStmt<'input>>),
    #[railroad(label = "DROP TABLESPACE ..")]
    DropTablespace(Box<DropTablespaceStmt<'input>>),
    #[railroad(label = "DROP TRIGGER ..")]
    DropTrigger(DropTriggerStmt<'input>),
    #[railroad(label = "DROP EVENT TRIGGER ..")]
    DropEventTrigger(DropEventTriggerStmt<'input>),
    #[railroad(label = "DROP ACCESS METHOD ..")]
    DropAccessMethod(DropAccessMethodStmt<'input>),
    #[railroad(label = "DROP MATERIALIZED VIEW ..")]
    DropMaterializedView(DropMaterializedViewStmt<'input>),
    #[railroad(label = "DROP TEXT SEARCH ..")]
    DropTextSearch(DropTextSearchStmt<'input>),
    #[railroad(label = "DROP FOREIGN ..")]
    DropForeign(DropForeignStmt<'input>),
    #[railroad(label = "DROP OWNED ..")]
    DropOwned(DropOwnedStmt<'input>),
    #[railroad(label = "DROP INDEX ..")]
    DropIndex(DropIndexStmt<'input>),
    #[railroad(label = "DROP VIEW ..")]
    DropView(DropViewStmt<'input>),
    #[railroad(label = "DROP RULE ..")]
    DropRule(DropRuleStmt<'input>),
    #[railroad(label = "DROP GROUP ..")]
    DropGroup(DropGroupStmt<'input>),
    #[railroad(label = "DROP ROLE ..")]
    DropRole(DropRoleStmt<'input>),
    // `DROP USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `DROP USER ...` variant so longest-match-wins picks the
    // specific path.
    #[railroad(label = "DROP USER MAPPING ..")]
    DropUserMapping(DropUserMappingStmt<'input>),
    #[railroad(label = "DROP USER ..")]
    DropUser(DropUserStmt<'input>),
    #[railroad(label = "DROP SCHEMA ..")]
    DropSchema(DropSchemaStmt<'input>),
    #[railroad(label = "DROP SEQUENCE ..")]
    DropSequence(DropSequenceStmt<'input>),
    #[railroad(label = "DROP TYPE ..")]
    DropType(DropTypeStmt<'input>),
    #[railroad(label = "DROP DOMAIN ..")]
    DropDomain(DropDomainStmt<'input>),
    #[railroad(label = "DROP AGGREGATE ..")]
    DropAggregate(DropAggregateStmt<'input>),
    // `DROP OPERATOR CLASS ...` and `DROP OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `DROP OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    #[railroad(label = "DROP OPERATOR CLASS ..")]
    DropOperatorClass(DropOperatorClassStmt<'input>),
    #[railroad(label = "DROP OPERATOR FAMILY ..")]
    DropOperatorFamily(DropOperatorFamilyStmt<'input>),
    #[railroad(label = "DROP OPERATOR ..")]
    DropOperator(Box<DropOperatorStmt<'input>>),
    #[railroad(label = "DROP CAST ..")]
    DropCast(DropCastStmt<'input>),
    #[railroad(label = "DROP TRANSFORM ..")]
    DropTransform(DropTransformStmt<'input>),
    #[railroad(label = "DROP COLLATION ..")]
    DropCollation(DropCollationStmt<'input>),
    #[railroad(label = "DROP EXTENSION ..")]
    DropExtension(DropExtensionStmt<'input>),
    #[railroad(label = "DROP POLICY ..")]
    DropPolicy(DropPolicyStmt<'input>),
    #[railroad(label = "DROP STATISTICS ..")]
    DropStatistics(DropStatisticsStmt<'input>),
    #[railroad(label = "DROP PUBLICATION ..")]
    DropPublication(DropPublicationStmt<'input>),
    #[railroad(label = "DROP SUBSCRIPTION ..")]
    DropSubscription(DropSubscriptionStmt<'input>),
    #[railroad(label = "DROP CONVERSION ..")]
    DropConversion(DropConversionStmt<'input>),
    #[railroad(label = "DROP SERVER ..")]
    DropServer(DropServerStmt<'input>),
    #[railroad(label = "DROP LANGUAGE ..")]
    DropLanguage(DropLanguageStmt<'input>),
    #[railroad(label = "DROP DATABASE ..")]
    DropDatabase(DropDatabaseStmt<'input>),
    #[railroad(label = "DROP TABLE ..")]
    DropTable(Box<DropTableStmt<'input>>),
    // ALTER variants: multi-keyword before single-keyword
    #[railroad(label = "ALTER DEFAULT PRIVILEGES ..")]
    AlterDefaultPrivileges(Box<AlterDefaultPrivilegesStmt<'input>>),
    #[railroad(label = "ALTER FOREIGN ..")]
    AlterForeign(AlterForeignStmt<'input>),
    #[railroad(label = "ALTER EVENT TRIGGER ..")]
    AlterEventTrigger(AlterEventTriggerStmt<'input>),
    #[railroad(label = "ALTER TRIGGER ..")]
    AlterTrigger(AlterTriggerStmt<'input>),
    #[railroad(label = "ALTER MATERIALIZED VIEW ..")]
    AlterMaterializedView(Box<AlterMaterializedViewStmt<'input>>),
    #[railroad(label = "ALTER TEXT SEARCH ..")]
    AlterTextSearch(Box<AlterTextSearchStmt<'input>>),
    #[railroad(label = "ALTER LARGE OBJECT ..")]
    AlterLargeObject(AlterLargeObjectStmt<'input>),
    #[railroad(label = "ALTER TABLESPACE ..")]
    AlterTablespace(AlterTablespaceStmt<'input>),
    #[railroad(label = "ALTER TABLE ..")]
    AlterTable(AlterTableStmt<'input>),
    #[railroad(label = "ALTER RULE ..")]
    AlterRule(AlterRuleStmt<'input>),
    #[railroad(label = "ALTER GROUP ..")]
    AlterGroup(AlterGroupStmt<'input>),
    #[railroad(label = "ALTER ROLE ..")]
    AlterRole(Box<AlterRoleStmt<'input>>),
    // `ALTER USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `ALTER USER ...` variant so longest-match-wins picks the
    // specific path.
    #[railroad(label = "ALTER USER MAPPING ..")]
    AlterUserMapping(Box<AlterUserMappingStmt<'input>>),
    #[railroad(label = "ALTER USER ..")]
    AlterUser(Box<AlterUserStmt<'input>>),
    #[railroad(label = "ALTER SCHEMA ..")]
    AlterSchema(AlterSchemaStmt<'input>),
    #[railroad(label = "ALTER SEQUENCE ..")]
    AlterSequence(Box<AlterSequenceStmt<'input>>),
    #[railroad(label = "ALTER TYPE ..")]
    AlterType(AlterTypeStmt<'input>),
    #[railroad(label = "ALTER DOMAIN ..")]
    AlterDomain(AlterDomainStmt<'input>),
    #[railroad(label = "ALTER AGGREGATE ..")]
    AlterAggregate(AlterAggregateStmt<'input>),
    // `ALTER OPERATOR CLASS ...` and `ALTER OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `ALTER OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    #[railroad(label = "ALTER OPERATOR CLASS ..")]
    AlterOperatorClass(Box<AlterOperatorClassStmt<'input>>),
    #[railroad(label = "ALTER OPERATOR FAMILY ..")]
    AlterOperatorFamily(Box<AlterOperatorFamilyStmt<'input>>),
    #[railroad(label = "ALTER OPERATOR ..")]
    AlterOperator(Box<AlterOperatorStmt<'input>>),
    #[railroad(label = "ALTER COLLATION ..")]
    AlterCollation(AlterCollationStmt<'input>),
    #[railroad(label = "ALTER EXTENSION ..")]
    AlterExtension(AlterExtensionStmt<'input>),
    #[railroad(label = "ALTER POLICY ..")]
    AlterPolicy(AlterPolicyStmt<'input>),
    #[railroad(label = "ALTER STATISTICS ..")]
    AlterStatistics(AlterStatisticsStmt<'input>),
    #[railroad(label = "ALTER PUBLICATION ..")]
    AlterPublication(AlterPublicationStmt<'input>),
    #[railroad(label = "ALTER SUBSCRIPTION ..")]
    AlterSubscription(AlterSubscriptionStmt<'input>),
    #[railroad(label = "ALTER CONVERSION ..")]
    AlterConversion(AlterConversionStmt<'input>),
    #[railroad(label = "ALTER SERVER ..")]
    AlterServer(AlterServerStmt<'input>),
    #[railroad(label = "ALTER LANGUAGE ..")]
    AlterLanguage(AlterLanguageStmt<'input>),
    #[railroad(label = "ALTER DATABASE ..")]
    AlterDatabase(Box<AlterDatabaseStmt<'input>>),
    #[railroad(label = "ALTER INDEX ..")]
    AlterIndex(Box<AlterIndexStmt<'input>>),
    #[railroad(label = "ALTER VIEW ..")]
    AlterView(Box<AlterViewStmt<'input>>),
    #[railroad(label = "ALTER FUNCTION ..")]
    AlterFunction(AlterFunctionStmt<'input>),
    #[railroad(label = "ALTER PROCEDURE ..")]
    AlterProcedure(AlterProcedureStmt<'input>),
    #[railroad(label = "ALTER ROUTINE ..")]
    AlterRoutine(AlterRoutineStmt<'input>),
    // CALL stored procedure
    #[railroad(label = "CALL ..")]
    Call(CallStmt<'input>),
    // DML
    #[railroad(label = "INSERT ..")]
    Insert(Box<InsertStmt<'input>>),
    #[railroad(label = "UPDATE ..")]
    Update(Box<UpdateStmt<'input>>),
    #[railroad(label = "MERGE ..")]
    Merge(Box<MergeStmt<'input>>),
    #[railroad(label = "DELETE ..")]
    Delete(Box<DeleteStmt<'input>>),
    // Transaction control
    #[railroad(label = "ROLLBACK ..")]
    Rollback(RollbackStmt<'input>),
    #[railroad(label = "SAVEPOINT ..")]
    Savepoint(SavepointStmt<'input>),
    #[railroad(label = "RELEASE ..")]
    Release(ReleaseStmt<'input>),
    #[railroad(label = "START TRANSACTION ..")]
    StartTransaction(StartTransactionStmt<'input>),
    #[railroad(label = "BEGIN ..")]
    Begin(BeginStmt<'input>),
    #[railroad(label = "COMMIT ..")]
    Commit(CommitStmt<'input>),
    #[railroad(label = "END ..")]
    End(EndStmt),
    #[railroad(label = "ABORT ..")]
    Abort(AbortStmt),
    // PREPARE / EXECUTE / DEALLOCATE
    #[railroad(label = "DEALLOCATE ..")]
    Deallocate(DeallocateStmt<'input>),
    #[railroad(label = "PREPARE ..")]
    Prepare(PrepareStmt<'input>),
    #[railroad(label = "EXECUTE ..")]
    Execute(ExecuteStmt<'input>),
    // Permissions
    #[railroad(label = "GRANT ..")]
    Grant(Box<GrantStmt<'input>>),
    #[railroad(label = "REVOKE ..")]
    Revoke(Box<RevokeStmt<'input>>),
    // Utility
    #[railroad(label = "SECURITY LABEL ..")]
    SecurityLabel(Box<SecurityLabelStmt<'input>>),
    #[railroad(label = "COMMENT ..")]
    Comment(Box<CommentStmt<'input>>),
    #[railroad(label = "COPY ..")]
    Copy(Box<CopyStmt<'input>>),
    #[railroad(label = "TRUNCATE ..")]
    Truncate(TruncateStmt<'input>),
    #[railroad(label = "REINDEX ..")]
    Reindex(Box<ReindexStmt<'input>>),
    #[railroad(label = "REFRESH ..")]
    Refresh(RefreshStmt<'input>),
    #[railroad(label = "CLUSTER ..")]
    Cluster(ClusterStmt<'input>),
    #[railroad(label = "CHECKPOINT")]
    Checkpoint(CheckpointStmt),
    #[railroad(label = "VACUUM ..")]
    Vacuum(Box<VacuumStmt<'input>>),
    #[railroad(label = "LOCK ..")]
    Lock(LockStmt<'input>),
    #[railroad(label = "NOTIFY ..")]
    Notify(NotifyStmt<'input>),
    #[railroad(label = "LISTEN ..")]
    Listen(ListenStmt<'input>),
    #[railroad(label = "UNLISTEN ..")]
    Unlisten(UnlistenStmt<'input>),
    #[railroad(label = "DISCARD ..")]
    Discard(DiscardStmt),
    #[railroad(label = "REASSIGN ..")]
    Reassign(ReassignStmt<'input>),
    #[railroad(label = "DO ..")]
    Do(Box<DoStmt<'input>>),
    // Cursor
    #[railroad(label = "DECLARE ..")]
    Declare(DeclareStmt<'input>),
    #[railroad(label = "FETCH ..")]
    Fetch(FetchStmt<'input>),
    #[railroad(label = "CLOSE ..")]
    Close(CloseStmt<'input>),
    #[railroad(label = "MOVE ..")]
    Move(MoveStmt<'input>),
    // Configuration
    // Multi-keyword SET variants must come before plain Set so
    // longest-match-wins picks the more specific form.
    #[railroad(label = "SET CONSTRAINTS ..")]
    SetConstraints(SetConstraintsStmt<'input>),
    #[railroad(label = "SET TRANSACTION ..")]
    SetTransaction(SetTransactionStmt<'input>),
    #[railroad(label = "SET SESSION AUTHORIZATION ..")]
    SetSessionAuth(SetSessionAuthStmt<'input>),
    #[railroad(label = "SET TIME ZONE ..")]
    SetTimeZone(SetTimeZoneStmt<'input>),
    #[railroad(label = "SET XML OPTION ..")]
    SetXmlOption(SetXmlOptionStmt),
    #[railroad(label = "SET ROLE ..")]
    SetRole(SetRoleStmt<'input>),
    #[railroad(label = "SET ..")]
    Set(SetStmt<'input>),
    #[railroad(label = "RESET ..")]
    Reset(ResetStmt<'input>),
    #[railroad(label = "SHOW ..")]
    Show(ShowStmt<'input>),
    #[railroad(label = "LOAD ..")]
    Load(LoadStmt<'input>),
    #[railroad(label = "ANALYZE ..")]
    Analyze(AnalyzeStmt<'input>),
    // Query
    #[railroad(label = "VALUES ..")]
    Values(Box<Subquery<'input>>),
    #[railroad(label = "SELECT ..")]
    Select(Box<SelectStmt<'input>>),
    #[railroad(label = "TABLE ..")]
    Table(TableStmt<'input>),
}
#[cfg(test)]
mod tests {
    use recursa::Parse;

    use super::*;

    /// `2 !=-- comment` (create_operator.sql) — PG's scan.l splits `!=--`
    /// into the `!=` comparison and a `-- …` line comment. logos would
    /// otherwise greedily take the 4-char `!=--` (CustomOp) operator and
    /// leave the comment body as stray identifier tokens; `pg_lex`'s
    /// `split_bang_eq_minus_before_dash_comment` pass undoes that.
    #[test]
    fn parse_bang_eq_minus_line_comment_split() {
        let src = "SELECT 2 !=-- comment to be removed by psql\n  1";
        let lexed = crate::tokens::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
        let leftover = &input.source()[input.byte_offset()..];
        assert!(input.is_eof(), "leftover: {leftover:?}");
    }

    /// Operator-form `LIKE` / `NOT LIKE` / `ILIKE` / `NOT ILIKE` — PG's
    /// `~~` / `!~~` / `~~*` / `!~~*` (gram.y 14860/14874/14888/14897) are
    /// the operator-equivalent spellings of the LIKE family. Used as
    /// ordinary infix Pratt operators on any a_expr.
    #[test]
    fn parse_like_operator_aliases() {
        for src in [
            "SELECT ROW('a','b') ~~ ROW('a','b') AS like_op",
            "SELECT 'foo' !~~ 'bar'",
            "SELECT 'foo' ~~* 'bar'",
            "SELECT 'foo' !~~* 'bar'",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt =
                Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    /// `UPDATE arrtest SET c[1:NULL] = '{…}'` — slice with a SQL keyword
    /// (NULL) as the upper bound. Relies on the `pg_lex` post-processor
    /// splitting the `:NULL` PsqlVar into a `Colon` + `NULL` pair.
    #[test]
    fn parse_subscript_assign_slice_null_bound() {
        let src = "UPDATE arrtest SET c[1:NULL] = '{16,25}' WHERE array_dims(c) is null";
        let lexed = crate::tokens::lex(src);
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let _stmt = Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
        assert!(input.is_eof());
    }

    /// `IS [NFC|NFD|NFKC|NFKD] NORMALIZED` and `IS NOT [NFC|NFD|NFKC|NFKD]
    /// NORMALIZED` — gram.y `a_expr IS [NOT] [unicode_normal_form] NORMALIZED`.
    /// The bare form (no NF-prefix) tests for default-form NFC normalization.
    #[test]
    fn parse_is_normalized() {
        for src in [
            "SELECT 'abc' IS NORMALIZED",
            "SELECT 'abc' IS NOT NORMALIZED",
            "SELECT 'abc' IS NFC NORMALIZED",
            "SELECT 'abc' IS NFD NORMALIZED",
            "SELECT 'abc' IS NFKC NORMALIZED",
            "SELECT 'abc' IS NFKD NORMALIZED",
            "SELECT 'abc' IS NOT NFC NORMALIZED",
            "SELECT U&'\\00E4\\24D1c' IS NFC NORMALIZED AS test_nfc",
            "SELECT U&'\\00E4\\24D1c' IS NORMALIZED AS test_default",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt =
                Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    /// `SET` is `unreserved_keyword` per kwlist.h — PG accepts a function
    /// named `set`, both as a call site (`SELECT set('t')`) and at function
    /// definition / drop sites. pg-sql keeps `SET` as a hard keyword to
    /// preserve `UPDATE … SET …` disambiguation, but reclaims it explicitly
    /// in function-name positions (see `FuncCallName::Set`).
    #[test]
    fn parse_set_as_function_name() {
        for src in [
            "SELECT set('t')",
            "CREATE FUNCTION set(tabname name) RETURNS VOID AS $$ BEGIN END; $$ LANGUAGE plpgsql",
            "DROP FUNCTION set(name)",
        ] {
            let lexed = crate::tokens::lex(src);
            assert_eq!(lexed.errors().count(), 0, "lex errors in input");
            let mut input = lexed.input();
            let _stmt =
                Statement::parse(&mut input).unwrap_or_else(|e| panic!("parse {src:?}: {e}")).into_ast();
            assert!(
                input.is_eof(),
                "leftover for {src:?}: {:?}",
                &input.source()[input.byte_offset()..]
            );
        }
    }

    /// Regression guard: keep the top-level statement enums small enough that the
    /// recursive descent parser fits in the default test thread stack.
    /// Prior to boxing the largest variants, `Statement` was 1480 bytes and
    /// fixture-parsing tests required `RUST_MIN_STACK=16777216`.
    #[test]
    fn statement_size_is_bounded() {
        use std::mem::size_of;
        let stmt = size_of::<Statement<'_>>();
        let item = size_of::<FileItem<'_>>();
        assert!(
            stmt <= 128,
            "Statement grew to {stmt} bytes — Box the largest variants",
        );
        assert!(
            item <= 128,
            "FileItem grew to {item} bytes — Box the largest variants",
        );
    }

    /// Print sizes of major AST node types. Run with `--nocapture` to see output.
    /// `#[ignore]` so it doesn't run by default but stays available for diagnosis.
    #[test]
    #[ignore]
    fn report_ast_sizes() {
        use std::mem::size_of;
        let mut sizes: Vec<(&'static str, usize)> = vec![
            ("FileItem", size_of::<FileItem<'_>>()),
            ("PsqlCommand", size_of::<PsqlCommand<'_>>()),
            ("TerminatedStatement", size_of::<TerminatedStatement<'_>>()),
            ("Statement", size_of::<Statement<'_>>()),
            ("Expr", size_of::<crate::ast::shared::expr::Expr<'_>>()),
            (
                "CaseSearched",
                size_of::<crate::ast::shared::expr::CaseSearched<'_>>(),
            ),
            (
                "CaseSimple",
                size_of::<crate::ast::shared::expr::CaseSimple<'_>>(),
            ),
            (
                "IntervalLit",
                size_of::<crate::ast::shared::expr::IntervalLit<'_>>(),
            ),
            (
                "TimestampLit",
                size_of::<crate::ast::shared::expr::TimestampLit<'_>>(),
            ),
            (
                "TypeCastFunc",
                size_of::<crate::ast::shared::expr::TypeCastFunc<'_>>(),
            ),
            (
                "XmlElement",
                size_of::<crate::ast::shared::expr::XmlElement<'_>>(),
            ),
            (
                "XmlForest",
                size_of::<crate::ast::shared::expr::XmlForest<'_>>(),
            ),
            (
                "XmlAttributes",
                size_of::<crate::ast::shared::expr::XmlAttributes<'_>>(),
            ),
            ("XmlPi", size_of::<crate::ast::shared::expr::XmlPi<'_>>()),
            (
                "ArrayExpr",
                size_of::<crate::ast::shared::expr::ArrayExpr<'_>>(),
            ),
            (
                "QualifiedRef",
                size_of::<crate::ast::shared::expr::QualifiedRef<'_>>(),
            ),
            (
                "QualifiedWildcard",
                size_of::<crate::ast::shared::expr::QualifiedWildcard<'_>>(),
            ),
            (
                "ParenExpr",
                size_of::<crate::ast::shared::expr::ParenExpr<'_>>(),
            ),
            (
                "ExistsExpr",
                size_of::<crate::ast::shared::expr::ExistsExpr<'_>>(),
            ),
            (
                "ArrayBracket",
                size_of::<crate::ast::shared::expr::ArrayBracket<'_>>(),
            ),
            (
                "RowExpr",
                size_of::<crate::ast::shared::expr::RowExpr<'_>>(),
            ),
            (
                "CastType",
                size_of::<crate::ast::shared::expr::CastType<'_>>(),
            ),
            (
                "ExtractCall",
                size_of::<crate::ast::shared::expr::ExtractCall<'_>>(),
            ),
            (
                "NotInSuffix",
                size_of::<crate::ast::shared::expr::NotInSuffix<'_>>(),
            ),
            (
                "InContent",
                size_of::<crate::ast::shared::expr::InContent<'_>>(),
            ),
            ("InList", size_of::<crate::ast::shared::expr::InList<'_>>()),
            (
                "SubstringCall",
                size_of::<crate::ast::shared::expr::SubstringCall<'_>>(),
            ),
            (
                "OverlayCall",
                size_of::<crate::ast::shared::expr::OverlayCall<'_>>(),
            ),
            (
                "TrimCall",
                size_of::<crate::ast::shared::expr::TrimCall<'_>>(),
            ),
            (
                "PositionCall",
                size_of::<crate::ast::shared::expr::PositionCall<'_>>(),
            ),
            (
                "SelectStmt",
                size_of::<crate::ast::dml::select::SelectStmt<'_>>(),
            ),
            (
                "CreateTableStmt",
                size_of::<crate::ast::ddl::table::CreateTableStmt<'_>>(),
            ),
            (
                "CreateFunctionStmt",
                size_of::<crate::ast::ddl::function::CreateFunctionStmt<'_>>(),
            ),
            (
                "InsertStmt",
                size_of::<crate::ast::dml::insert::InsertStmt<'_>>(),
            ),
            (
                "UpdateStmt",
                size_of::<crate::ast::dml::update::UpdateStmt<'_>>(),
            ),
            (
                "DeleteStmt",
                size_of::<crate::ast::dml::delete::DeleteStmt<'_>>(),
            ),
            (
                "MergeStmt",
                size_of::<crate::ast::dml::merge::MergeStmt<'_>>(),
            ),
            (
                "ExplainStmt",
                size_of::<crate::ast::utility::explain::ExplainStmt<'_>>(),
            ),
            (
                "CompoundQuery",
                size_of::<crate::ast::dml::values::Subquery<'_>>(),
            ),
            (
                "WithStatement",
                size_of::<crate::ast::shared::with_clause::WithStatement<'_>>(),
            ),
            (
                "FuncCall",
                size_of::<crate::ast::shared::expr::FuncCall<'_>>(),
            ),
            (
                "ColumnDef",
                size_of::<crate::ast::ddl::table::ColumnDef<'_>>(),
            ),
            (
                "ConflictAction",
                size_of::<crate::ast::dml::insert::ConflictAction<'_>>(),
            ),
            (
                "DoUpdateAction",
                size_of::<crate::ast::dml::insert::DoUpdateAction<'_>>(),
            ),
            (
                "GroupByItem",
                size_of::<crate::ast::dml::select::GroupByItem<'_>>(),
            ),
            (
                "FuncArg",
                size_of::<crate::ast::shared::expr::FuncArg<'_>>(),
            ),
            (
                "AlterTableStmt",
                size_of::<crate::ast::ddl::table::AlterTableStmt<'_>>(),
            ),
            (
                "CreateTriggerStmt",
                size_of::<crate::ast::ddl::trigger::CreateTriggerStmt<'_>>(),
            ),
            (
                "CreateRuleStmt",
                size_of::<crate::ast::ddl::rule::CreateRuleStmt<'_>>(),
            ),
            (
                "CreateForeignStmt",
                size_of::<crate::ast::ddl::foreign::CreateForeignStmt<'_>>(),
            ),
            (
                "CreateMaterializedViewStmt",
                size_of::<crate::ast::ddl::materialized_view::CreateMaterializedViewStmt<'_>>(),
            ),
            (
                "AlterMaterializedViewStmt",
                size_of::<crate::ast::ddl::materialized_view::AlterMaterializedViewStmt<'_>>(),
            ),
            (
                "CopyStmt",
                size_of::<crate::ast::utility::copy::CopyStmt<'_>>(),
            ),
            (
                "VacuumStmt",
                size_of::<crate::ast::utility::vacuum::VacuumStmt<'_>>(),
            ),
            (
                "ReindexStmt",
                size_of::<crate::ast::utility::reindex::ReindexStmt<'_>>(),
            ),
            (
                "ClusterStmt",
                size_of::<crate::ast::utility::cluster::ClusterStmt<'_>>(),
            ),
            (
                "GrantStmt",
                size_of::<crate::ast::utility::grant::GrantStmt<'_>>(),
            ),
            (
                "RevokeStmt",
                size_of::<crate::ast::utility::grant::RevokeStmt<'_>>(),
            ),
            ("DoStmt", size_of::<crate::ast::utility::r#do::DoStmt<'_>>()),
            (
                "CreateRoleStmt",
                size_of::<crate::ast::ddl::role::CreateRoleStmt<'_>>(),
            ),
            (
                "CreateAggregateStmt",
                size_of::<crate::ast::ddl::aggregate::CreateAggregateStmt<'_>>(),
            ),
            (
                "CreateOperatorStmt",
                size_of::<crate::ast::ddl::operator::CreateOperatorStmt<'_>>(),
            ),
            (
                "AnalyzeStmt",
                size_of::<crate::ast::utility::analyze::AnalyzeStmt<'_>>(),
            ),
            (
                "CreateIndexStmt",
                size_of::<crate::ast::ddl::index::CreateIndexStmt<'_>>(),
            ),
            (
                "CreateViewStmt",
                size_of::<crate::ast::ddl::view::CreateViewStmt<'_>>(),
            ),
            (
                "DropTableStmt",
                size_of::<crate::ast::ddl::table::DropTableStmt<'_>>(),
            ),
            (
                "CreateUserMappingStmt",
                size_of::<crate::ast::ddl::role::CreateUserMappingStmt<'_>>(),
            ),
            (
                "AlterUserMappingStmt",
                size_of::<crate::ast::ddl::role::AlterUserMappingStmt<'_>>(),
            ),
            (
                "DropUserMappingStmt",
                size_of::<crate::ast::ddl::role::DropUserMappingStmt<'_>>(),
            ),
            (
                "AlterOperatorClassStmt",
                size_of::<crate::ast::ddl::operator::AlterOperatorClassStmt<'_>>(),
            ),
            (
                "CreateProcedureStmt",
                size_of::<crate::ast::ddl::procedure::CreateProcedureStmt<'_>>(),
            ),
            (
                "CreateTablespaceStmt",
                size_of::<crate::ast::ddl::tablespace::CreateTablespaceStmt<'_>>(),
            ),
            (
                "DropFunctionStmt",
                size_of::<crate::ast::ddl::function::DropFunctionStmt<'_>>(),
            ),
            (
                "CreateEventTriggerStmt",
                size_of::<crate::ast::ddl::trigger::CreateEventTriggerStmt<'_>>(),
            ),
            (
                "CreateAccessMethodStmt",
                size_of::<crate::ast::ddl::access_method::CreateAccessMethodStmt<'_>>(),
            ),
            (
                "CreateLanguageStmt",
                size_of::<crate::ast::ddl::language::CreateLanguageStmt<'_>>(),
            ),
            (
                "CreateDatabaseStmt",
                size_of::<crate::ast::ddl::database::CreateDatabaseStmt<'_>>(),
            ),
            (
                "CreateUserStmt",
                size_of::<crate::ast::ddl::role::CreateUserStmt<'_>>(),
            ),
            (
                "CreateSchemaStmt",
                size_of::<crate::ast::ddl::schema::CreateSchemaStmt<'_>>(),
            ),
            (
                "CreateSequenceStmt",
                size_of::<crate::ast::ddl::sequence::CreateSequenceStmt<'_>>(),
            ),
            (
                "CreateTypeStmt",
                size_of::<crate::ast::ddl::r#type::CreateTypeStmt<'_>>(),
            ),
            (
                "CreateDomainStmt",
                size_of::<crate::ast::ddl::domain::CreateDomainStmt<'_>>(),
            ),
            (
                "CreateCastStmt",
                size_of::<crate::ast::ddl::cast::CreateCastStmt<'_>>(),
            ),
            (
                "CreateCollationStmt",
                size_of::<crate::ast::ddl::collation::CreateCollationStmt<'_>>(),
            ),
            (
                "CreateExtensionStmt",
                size_of::<crate::ast::ddl::extension::CreateExtensionStmt<'_>>(),
            ),
            (
                "CreatePolicyStmt",
                size_of::<crate::ast::ddl::policy::CreatePolicyStmt<'_>>(),
            ),
            (
                "CreateStatisticsStmt",
                size_of::<crate::ast::ddl::statistics::CreateStatisticsStmt<'_>>(),
            ),
            (
                "CreatePublicationStmt",
                size_of::<crate::ast::ddl::publication::CreatePublicationStmt<'_>>(),
            ),
            (
                "CreateSubscriptionStmt",
                size_of::<crate::ast::ddl::subscription::CreateSubscriptionStmt<'_>>(),
            ),
            (
                "CreateConversionStmt",
                size_of::<crate::ast::ddl::conversion::CreateConversionStmt<'_>>(),
            ),
            (
                "CreateServerStmt",
                size_of::<crate::ast::ddl::foreign::CreateServerStmt<'_>>(),
            ),
            (
                "CreateGroupStmt",
                size_of::<crate::ast::ddl::role::CreateGroupStmt<'_>>(),
            ),
            (
                "AlterIndexStmt",
                size_of::<crate::ast::ddl::index::AlterIndexStmt<'_>>(),
            ),
            (
                "AlterViewStmt",
                size_of::<crate::ast::ddl::view::AlterViewStmt<'_>>(),
            ),
            (
                "AlterFunctionStmt",
                size_of::<crate::ast::ddl::function::AlterFunctionStmt<'_>>(),
            ),
            (
                "AlterDatabaseStmt",
                size_of::<crate::ast::ddl::database::AlterDatabaseStmt<'_>>(),
            ),
            (
                "AlterDomainStmt",
                size_of::<crate::ast::ddl::domain::AlterDomainStmt<'_>>(),
            ),
            (
                "AlterEventTriggerStmt",
                size_of::<crate::ast::ddl::trigger::AlterEventTriggerStmt<'_>>(),
            ),
            (
                "AlterTriggerStmt",
                size_of::<crate::ast::ddl::trigger::AlterTriggerStmt<'_>>(),
            ),
            (
                "AlterSequenceStmt",
                size_of::<crate::ast::ddl::sequence::AlterSequenceStmt<'_>>(),
            ),
            (
                "ImportForeignSchemaStmt",
                size_of::<crate::ast::ddl::foreign::ImportForeignSchemaStmt<'_>>(),
            ),
            (
                "CommentStmt",
                size_of::<crate::ast::utility::comment::CommentStmt<'_>>(),
            ),
            (
                "SecurityLabelStmt",
                size_of::<crate::ast::utility::comment::SecurityLabelStmt<'_>>(),
            ),
            (
                "PrepareStmt",
                size_of::<crate::ast::tcl::prepared::PrepareStmt<'_>>(),
            ),
            (
                "TableRef",
                size_of::<crate::ast::dml::select::TableRef<'_>>(),
            ),
            (
                "SimpleTableRef",
                size_of::<crate::ast::dml::select::SimpleTableRef<'_>>(),
            ),
            (
                "CompoundQuery (if any)",
                size_of::<crate::ast::dml::values::Subquery<'_>>(),
            ),
        ];
        sizes.sort_by_key(|b| std::cmp::Reverse(b.1));
        eprintln!("\n=== AST sizes (bytes) ===");
        for (name, size) in &sizes {
            eprintln!("{size:>6}  {name}");
        }
        eprintln!();
    }

    #[test]
    fn parse_statement_select() {
        let lexed = crate::tokens::lex("SELECT 1 AS one");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        // Bare SELECT now matches via CompoundQuery path since Values variant
        // precedes Select for compound query (UNION etc.) support.
        assert!(matches!(stmt, Statement::Values(_)));
    }

    #[test]
    fn parse_statement_create_table() {
        let lexed = crate::tokens::lex("CREATE TABLE t (f1 bool)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::CreateTable(_)));
    }

    #[test]
    fn parse_statement_insert() {
        let lexed = crate::tokens::lex("INSERT INTO t (f1) VALUES (true)");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::Insert(_)));
    }

    #[test]
    fn parse_statement_delete() {
        let lexed = crate::tokens::lex("DELETE FROM t WHERE a > 1");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::Delete(_)));
    }

    #[test]
    fn parse_statement_drop_table() {
        let lexed = crate::tokens::lex("DROP TABLE t");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let stmt = Statement::parse(&mut input).unwrap().into_ast();
        assert!(matches!(stmt, Statement::DropTable(_)));
    }

    #[test]
    fn parse_psql_command_statement() {
        let lexed = crate::tokens::lex("SELECT 1;");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let cmd = PsqlCommand::parse(&mut input).unwrap().into_ast();
        assert!(matches!(cmd, PsqlCommand::Statement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_psql_command_directive() {
        let lexed = crate::tokens::lex("\\pset null '(null)'\n");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let cmd = PsqlCommand::parse(&mut input).unwrap().into_ast();
        match cmd {
            PsqlCommand::Directive(d) => assert_eq!(d.rest.0, "pset null '(null)'"),
            _ => panic!("expected directive"),
        }
    }

    #[test]
    fn parse_select_with_where_and_bool_test() {
        let lexed = crate::tokens::lex("SELECT f1 FROM BOOLTBL1 WHERE f1 IS TRUE;");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let cmd = PsqlCommand::parse(&mut input).unwrap().into_ast();
        assert!(matches!(cmd, PsqlCommand::Statement(_)));
        assert!(input.is_eof());
    }

    #[test]
    fn parse_full_insert_with_type_cast() {
        let lexed = crate::tokens::lex("INSERT INTO BOOLTBL1 (f1) VALUES (bool 't');");
        assert_eq!(lexed.errors().count(), 0, "lex errors in input");
        let mut input = lexed.input();
        let cmd = PsqlCommand::parse(&mut input).unwrap().into_ast();
        assert!(matches!(cmd, PsqlCommand::Statement(_)));
    }
}
