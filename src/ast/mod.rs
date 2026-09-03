pub mod cursor;
pub mod ddl;
pub mod dml;
pub mod file;
pub mod session;
pub mod shared;
pub mod tcl;
pub mod utility;

pub use self::file::{PsqlTerminator, StatementTerminator, TerminatedStatement};

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
    dml::update::UpdateStmt,
    dml::values::Subquery,
    session::discard::*,
    session::notify::*,
    session::set_reset::{
        LoadStmt, ResetStmt, SetRoleStmt, SetSessionAuthStmt, SetStmt, SetTimeZoneStmt,
        SetXmlOptionStmt, ShowStmt,
    },
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
/// - `Explain` wraps a Statement, so it must come before `Query`.
/// - `CreateFunction` and `CreateIndex` come before `CreateTable` because they
///   have `CREATE FUNCTION` / `CREATE INDEX` which are longer than `CREATE TABLE`.
///   `CreateView` likewise comes before `CreateTable`.
///   `CreateTable` handles regular, partitioned, and partition-of forms internally.
/// - `DropFunction` and `DropIndex` come before `DropTable` for the same reason.
/// - `Query` is the single shared-prefix type for `WITH`, `SELECT`, `VALUES`,
///   `TABLE`, parenthesized queries, and their set-operation continuations.
#[derive(recursa::Node, Debug, Clone)]
pub enum Statement<'input> {
    // --- Multi-keyword statements (longest first_pattern first) ---
    Explain(Box<ExplainStmt<'input>>),
    // CREATE variants: multi-keyword before single-keyword
    CreateFunction(Box<CreateFunctionStmt<'input>>),
    CreateProcedure(Box<CreateProcedureStmt<'input>>),
    CreateTablespace(Box<CreateTablespaceStmt<'input>>),
    ImportForeignSchema(Box<ImportForeignSchemaStmt<'input>>),
    CreateConstraintTrigger(Box<CreateConstraintTriggerStmt<'input>>),
    CreateTrigger(Box<CreateTriggerStmt<'input>>),
    CreateEventTrigger(Box<CreateEventTriggerStmt<'input>>),
    CreateAccessMethod(CreateAccessMethodStmt<'input>),
    CreateMaterializedView(Box<CreateMaterializedViewStmt<'input>>),
    CreateTextSearch(Box<CreateTextSearchStmt<'input>>),
    CreateForeign(Box<CreateForeignStmt<'input>>),
    CreateIndex(Box<CreateIndexStmt<'input>>),
    CreateView(Box<CreateViewStmt<'input>>),
    CreateRule(Box<CreateRuleStmt<'input>>),
    CreateGroup(CreateGroupStmt<'input>),
    CreateRole(CreateRoleStmt<'input>),
    // `CREATE USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `CREATE USER ...` variant so longest-match-wins picks the
    // specific path.
    CreateUserMapping(Box<CreateUserMappingStmt<'input>>),
    CreateUser(CreateUserStmt<'input>),
    CreateSchema(CreateSchemaStmt<'input>),
    CreateSequence(Box<CreateSequenceStmt<'input>>),
    CreateType(Box<CreateTypeStmt<'input>>),
    CreateDomain(Box<CreateDomainStmt<'input>>),
    CreateAggregate(Box<CreateAggregateStmt<'input>>),
    // `CREATE OPERATOR CLASS ...` and `CREATE OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `CREATE OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    CreateOperatorClass(Box<CreateOperatorClassStmt<'input>>),
    CreateOperatorFamily(CreateOperatorFamilyStmt<'input>),
    CreateOperator(Box<CreateOperatorStmt<'input>>),
    CreateCast(Box<CreateCastStmt<'input>>),
    CreateTransform(Box<CreateTransformStmt<'input>>),
    CreateCollation(Box<CreateCollationStmt<'input>>),
    CreateExtension(CreateExtensionStmt<'input>),
    CreatePolicy(Box<CreatePolicyStmt<'input>>),
    CreateStatistics(Box<CreateStatisticsStmt<'input>>),
    CreatePublication(Box<CreatePublicationStmt<'input>>),
    CreateSubscription(Box<CreateSubscriptionStmt<'input>>),
    CreateConversion(Box<CreateConversionStmt<'input>>),
    CreateServer(Box<CreateServerStmt<'input>>),
    CreateLanguage(CreateLanguageStmt<'input>),
    CreateDatabase(CreateDatabaseStmt<'input>),
    CreateTable(Box<CreateTableStmt<'input>>),
    // DROP variants
    DropFunction(Box<DropFunctionStmt<'input>>),
    DropProcedure(Box<DropProcedureStmt<'input>>),
    DropRoutine(Box<DropRoutineStmt<'input>>),
    DropTablespace(Box<DropTablespaceStmt<'input>>),
    DropTrigger(DropTriggerStmt<'input>),
    DropEventTrigger(DropEventTriggerStmt<'input>),
    DropAccessMethod(DropAccessMethodStmt<'input>),
    DropMaterializedView(DropMaterializedViewStmt<'input>),
    DropTextSearch(DropTextSearchStmt<'input>),
    DropForeign(DropForeignStmt<'input>),
    DropOwned(DropOwnedStmt<'input>),
    DropIndex(DropIndexStmt<'input>),
    DropView(DropViewStmt<'input>),
    DropRule(DropRuleStmt<'input>),
    DropGroup(DropGroupStmt<'input>),
    DropRole(DropRoleStmt<'input>),
    // `DROP USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `DROP USER ...` variant so longest-match-wins picks the
    // specific path.
    DropUserMapping(DropUserMappingStmt<'input>),
    DropUser(DropUserStmt<'input>),
    DropSchema(DropSchemaStmt<'input>),
    DropSequence(DropSequenceStmt<'input>),
    DropType(DropTypeStmt<'input>),
    DropDomain(DropDomainStmt<'input>),
    DropAggregate(DropAggregateStmt<'input>),
    // `DROP OPERATOR CLASS ...` and `DROP OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `DROP OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    DropOperatorClass(DropOperatorClassStmt<'input>),
    DropOperatorFamily(DropOperatorFamilyStmt<'input>),
    DropOperator(Box<DropOperatorStmt<'input>>),
    DropCast(DropCastStmt<'input>),
    DropTransform(DropTransformStmt<'input>),
    DropCollation(DropCollationStmt<'input>),
    DropExtension(DropExtensionStmt<'input>),
    DropPolicy(DropPolicyStmt<'input>),
    DropStatistics(DropStatisticsStmt<'input>),
    DropPublication(DropPublicationStmt<'input>),
    DropSubscription(DropSubscriptionStmt<'input>),
    DropConversion(DropConversionStmt<'input>),
    DropServer(DropServerStmt<'input>),
    DropLanguage(DropLanguageStmt<'input>),
    DropDatabase(DropDatabaseStmt<'input>),
    DropTable(Box<DropTableStmt<'input>>),
    // ALTER variants: multi-keyword before single-keyword
    AlterDefaultPrivileges(Box<AlterDefaultPrivilegesStmt<'input>>),
    AlterForeign(AlterForeignStmt<'input>),
    AlterEventTrigger(AlterEventTriggerStmt<'input>),
    AlterTrigger(AlterTriggerStmt<'input>),
    AlterMaterializedView(Box<AlterMaterializedViewStmt<'input>>),
    AlterTextSearch(Box<AlterTextSearchStmt<'input>>),
    AlterLargeObject(AlterLargeObjectStmt<'input>),
    AlterTablespace(AlterTablespaceStmt<'input>),
    AlterTable(AlterTableStmt<'input>),
    AlterRule(AlterRuleStmt<'input>),
    AlterGroup(AlterGroupStmt<'input>),
    AlterRole(Box<AlterRoleStmt<'input>>),
    // `ALTER USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `ALTER USER ...` variant so longest-match-wins picks the
    // specific path.
    AlterUserMapping(Box<AlterUserMappingStmt<'input>>),
    AlterUser(Box<AlterUserStmt<'input>>),
    AlterSchema(AlterSchemaStmt<'input>),
    AlterSequence(Box<AlterSequenceStmt<'input>>),
    AlterType(AlterTypeStmt<'input>),
    AlterDomain(AlterDomainStmt<'input>),
    AlterAggregate(AlterAggregateStmt<'input>),
    // `ALTER OPERATOR CLASS ...` and `ALTER OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `ALTER OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    AlterOperatorClass(Box<AlterOperatorClassStmt<'input>>),
    AlterOperatorFamily(Box<AlterOperatorFamilyStmt<'input>>),
    AlterOperator(Box<AlterOperatorStmt<'input>>),
    AlterCollation(AlterCollationStmt<'input>),
    AlterExtension(AlterExtensionStmt<'input>),
    AlterPolicy(AlterPolicyStmt<'input>),
    AlterStatistics(AlterStatisticsStmt<'input>),
    AlterPublication(AlterPublicationStmt<'input>),
    AlterSubscription(AlterSubscriptionStmt<'input>),
    AlterConversion(AlterConversionStmt<'input>),
    AlterServer(AlterServerStmt<'input>),
    AlterLanguage(AlterLanguageStmt<'input>),
    AlterDatabase(Box<AlterDatabaseStmt<'input>>),
    AlterIndex(Box<AlterIndexStmt<'input>>),
    AlterView(Box<AlterViewStmt<'input>>),
    AlterFunction(AlterFunctionStmt<'input>),
    AlterProcedure(AlterProcedureStmt<'input>),
    AlterRoutine(AlterRoutineStmt<'input>),
    // CALL stored procedure
    Call(CallStmt<'input>),
    // DML
    Insert(Box<InsertStmt<'input>>),
    Update(Box<UpdateStmt<'input>>),
    Merge(Box<MergeStmt<'input>>),
    Delete(Box<DeleteStmt<'input>>),
    // Transaction control
    Rollback(RollbackStmt<'input>),
    Savepoint(SavepointStmt<'input>),
    Release(ReleaseStmt<'input>),
    StartTransaction(StartTransactionStmt<'input>),
    Begin(BeginStmt<'input>),
    Commit(CommitStmt<'input>),
    End(EndStmt),
    Abort(AbortStmt),
    // PREPARE / EXECUTE / DEALLOCATE
    Deallocate(DeallocateStmt<'input>),
    Prepare(PrepareStmt<'input>),
    Execute(ExecuteStmt<'input>),
    // Permissions
    Grant(Box<GrantStmt<'input>>),
    Revoke(Box<RevokeStmt<'input>>),
    // Utility
    SecurityLabel(Box<SecurityLabelStmt<'input>>),
    Comment(Box<CommentStmt<'input>>),
    Copy(Box<CopyStmt<'input>>),
    Truncate(TruncateStmt<'input>),
    Reindex(Box<ReindexStmt<'input>>),
    Refresh(RefreshStmt<'input>),
    Cluster(ClusterStmt<'input>),
    Checkpoint(CheckpointStmt),
    Vacuum(Box<VacuumStmt<'input>>),
    Lock(LockStmt<'input>),
    Notify(NotifyStmt<'input>),
    Listen(ListenStmt<'input>),
    Unlisten(UnlistenStmt<'input>),
    Discard(DiscardStmt),
    Reassign(ReassignStmt<'input>),
    Do(Box<DoStmt<'input>>),
    // Cursor
    Declare(DeclareStmt<'input>),
    Fetch(FetchStmt<'input>),
    Close(CloseStmt<'input>),
    Move(MoveStmt<'input>),
    // Configuration
    // Multi-keyword SET variants must come before plain Set so
    // longest-match-wins picks the more specific form.
    SetConstraints(SetConstraintsStmt<'input>),
    SetTransaction(SetTransactionStmt<'input>),
    SetSessionAuth(SetSessionAuthStmt<'input>),
    SetTimeZone(SetTimeZoneStmt<'input>),
    SetXmlOption(SetXmlOptionStmt),
    SetRole(SetRoleStmt<'input>),
    Set(SetStmt<'input>),
    Reset(ResetStmt<'input>),
    Show(ShowStmt<'input>),
    Load(LoadStmt<'input>),
    Analyze(AnalyzeStmt<'input>),
    // Query. `Subquery` owns the common prefix so the top-level dispatcher
    // does not compare duplicate languages for WITH/SELECT/VALUES/TABLE.
    Query(Box<Subquery<'input>>),
}
