pub mod cursor;
pub mod ddl;
pub mod dml;
pub mod file;
pub mod session;
pub mod shared;
pub mod tcl;
pub mod utility;

pub use self::file::{StatementTerminator, TerminatedStatement};

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
    dml::values::QueryBody,
    session::discard::*,
    session::notify::*,
    session::set_reset::{LoadStmt, ResetStmt, ShowStmt, VariableSetStmt},
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
#[derive(recursa::Node, Debug)]
pub enum Statement<'input> {
    // --- Multi-keyword statements (longest first_pattern first) ---
    Explain(recursa::ArenaBox<'input, ExplainStmt<'input>>),
    // CREATE variants: multi-keyword before single-keyword
    CreateFunction(recursa::ArenaBox<'input, CreateFunctionStmt<'input>>),
    CreateProcedure(recursa::ArenaBox<'input, CreateProcedureStmt<'input>>),
    CreateTablespace(recursa::ArenaBox<'input, CreateTablespaceStmt<'input>>),
    ImportForeignSchema(recursa::ArenaBox<'input, ImportForeignSchemaStmt<'input>>),
    CreateConstraintTrigger(recursa::ArenaBox<'input, CreateConstraintTriggerStmt<'input>>),
    CreateTrigger(recursa::ArenaBox<'input, CreateTriggerStmt<'input>>),
    CreateEventTrigger(recursa::ArenaBox<'input, CreateEventTriggerStmt<'input>>),
    CreateAccessMethod(CreateAccessMethodStmt<'input>),
    CreateMaterializedView(recursa::ArenaBox<'input, CreateMaterializedViewStmt<'input>>),
    CreateTextSearch(recursa::ArenaBox<'input, CreateTextSearchStmt<'input>>),
    CreateForeign(recursa::ArenaBox<'input, CreateForeignStmt<'input>>),
    CreateIndex(recursa::ArenaBox<'input, CreateIndexStmt<'input>>),
    CreateView(recursa::ArenaBox<'input, CreateViewStmt<'input>>),
    CreateRule(recursa::ArenaBox<'input, CreateRuleStmt<'input>>),
    CreateGroup(CreateGroupStmt<'input>),
    CreateRole(CreateRoleStmt<'input>),
    // `CREATE USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `CREATE USER ...` variant so longest-match-wins picks the
    // specific path.
    CreateUserMapping(recursa::ArenaBox<'input, CreateUserMappingStmt<'input>>),
    CreateUser(CreateUserStmt<'input>),
    CreateSchema(CreateSchemaStmt<'input>),
    CreateSequence(recursa::ArenaBox<'input, CreateSequenceStmt<'input>>),
    CreateType(recursa::ArenaBox<'input, CreateTypeStmt<'input>>),
    CreateDomain(recursa::ArenaBox<'input, CreateDomainStmt<'input>>),
    CreateAggregate(recursa::ArenaBox<'input, CreateAggregateStmt<'input>>),
    // `CREATE OPERATOR CLASS ...` and `CREATE OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `CREATE OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    CreateOperatorClass(recursa::ArenaBox<'input, CreateOperatorClassStmt<'input>>),
    CreateOperatorFamily(CreateOperatorFamilyStmt<'input>),
    CreateOperator(recursa::ArenaBox<'input, CreateOperatorStmt<'input>>),
    CreateCast(recursa::ArenaBox<'input, CreateCastStmt<'input>>),
    CreateTransform(recursa::ArenaBox<'input, CreateTransformStmt<'input>>),
    CreateCollation(recursa::ArenaBox<'input, CreateCollationStmt<'input>>),
    CreateExtension(CreateExtensionStmt<'input>),
    CreatePolicy(recursa::ArenaBox<'input, CreatePolicyStmt<'input>>),
    CreateStatistics(recursa::ArenaBox<'input, CreateStatisticsStmt<'input>>),
    CreatePublication(recursa::ArenaBox<'input, CreatePublicationStmt<'input>>),
    CreateSubscription(recursa::ArenaBox<'input, CreateSubscriptionStmt<'input>>),
    CreateConversion(recursa::ArenaBox<'input, CreateConversionStmt<'input>>),
    CreateServer(recursa::ArenaBox<'input, CreateServerStmt<'input>>),
    CreateLanguage(recursa::ArenaBox<'input, CreateLanguageStmt<'input>>),
    CreateDatabase(CreateDatabaseStmt<'input>),
    CreateTable(recursa::ArenaBox<'input, CreateTableStmt<'input>>),
    // DROP variants
    DropFunction(recursa::ArenaBox<'input, DropFunctionStmt<'input>>),
    DropProcedure(recursa::ArenaBox<'input, DropProcedureStmt<'input>>),
    DropRoutine(recursa::ArenaBox<'input, DropRoutineStmt<'input>>),
    DropTablespace(recursa::ArenaBox<'input, DropTablespaceStmt<'input>>),
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
    DropOperator(recursa::ArenaBox<'input, DropOperatorStmt<'input>>),
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
    DropTable(recursa::ArenaBox<'input, DropTableStmt<'input>>),
    // ALTER variants: multi-keyword before single-keyword
    AlterDefaultPrivileges(recursa::ArenaBox<'input, AlterDefaultPrivilegesStmt<'input>>),
    AlterForeign(recursa::ArenaBox<'input, AlterForeignStmt<'input>>),
    AlterEventTrigger(AlterEventTriggerStmt<'input>),
    AlterTrigger(recursa::ArenaBox<'input, AlterTriggerStmt<'input>>),
    AlterMaterializedView(recursa::ArenaBox<'input, AlterMaterializedViewStmt<'input>>),
    AlterTextSearch(recursa::ArenaBox<'input, AlterTextSearchStmt<'input>>),
    AlterLargeObject(AlterLargeObjectStmt<'input>),
    AlterTablespace(AlterTablespaceStmt<'input>),
    AlterTable(recursa::ArenaBox<'input, AlterTableStmt<'input>>),
    AlterRule(AlterRuleStmt<'input>),
    AlterGroup(AlterGroupStmt<'input>),
    AlterRole(recursa::ArenaBox<'input, AlterRoleStmt<'input>>),
    // `ALTER USER MAPPING ...` is a three-keyword lead and must precede
    // the bare `ALTER USER ...` variant so longest-match-wins picks the
    // specific path.
    AlterUserMapping(recursa::ArenaBox<'input, AlterUserMappingStmt<'input>>),
    AlterUser(recursa::ArenaBox<'input, AlterUserStmt<'input>>),
    AlterSchema(AlterSchemaStmt<'input>),
    AlterSequence(recursa::ArenaBox<'input, AlterSequenceStmt<'input>>),
    AlterType(recursa::ArenaBox<'input, AlterTypeStmt<'input>>),
    AlterDomain(recursa::ArenaBox<'input, AlterDomainStmt<'input>>),
    AlterAggregate(recursa::ArenaBox<'input, AlterAggregateStmt<'input>>),
    // `ALTER OPERATOR CLASS ...` and `ALTER OPERATOR FAMILY ...` are
    // three-keyword leads and must precede the bare `ALTER OPERATOR ...`
    // variant so longest-match-wins picks the specific path.
    AlterOperatorClass(recursa::ArenaBox<'input, AlterOperatorClassStmt<'input>>),
    AlterOperatorFamily(recursa::ArenaBox<'input, AlterOperatorFamilyStmt<'input>>),
    AlterOperator(recursa::ArenaBox<'input, AlterOperatorStmt<'input>>),
    AlterCollation(AlterCollationStmt<'input>),
    AlterExtension(AlterExtensionStmt<'input>),
    AlterPolicy(recursa::ArenaBox<'input, AlterPolicyStmt<'input>>),
    AlterStatistics(AlterStatisticsStmt<'input>),
    AlterPublication(AlterPublicationStmt<'input>),
    AlterSubscription(AlterSubscriptionStmt<'input>),
    AlterConversion(AlterConversionStmt<'input>),
    AlterServer(AlterServerStmt<'input>),
    AlterLanguage(AlterLanguageStmt<'input>),
    AlterDatabase(recursa::ArenaBox<'input, AlterDatabaseStmt<'input>>),
    AlterIndex(recursa::ArenaBox<'input, AlterIndexStmt<'input>>),
    AlterView(recursa::ArenaBox<'input, AlterViewStmt<'input>>),
    AlterFunction(recursa::ArenaBox<'input, AlterFunctionStmt<'input>>),
    AlterProcedure(recursa::ArenaBox<'input, AlterProcedureStmt<'input>>),
    AlterRoutine(recursa::ArenaBox<'input, AlterRoutineStmt<'input>>),
    // CALL stored procedure
    Call(CallStmt<'input>),
    // DML
    Insert(recursa::ArenaBox<'input, InsertStmt<'input>>),
    Update(recursa::ArenaBox<'input, UpdateStmt<'input>>),
    Merge(recursa::ArenaBox<'input, MergeStmt<'input>>),
    Delete(recursa::ArenaBox<'input, DeleteStmt<'input>>),
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
    Grant(recursa::ArenaBox<'input, GrantStmt<'input>>),
    Revoke(recursa::ArenaBox<'input, RevokeStmt<'input>>),
    // Utility
    SecurityLabel(recursa::ArenaBox<'input, SecurityLabelStmt<'input>>),
    Comment(recursa::ArenaBox<'input, CommentStmt<'input>>),
    Copy(recursa::ArenaBox<'input, CopyStmt<'input>>),
    Truncate(TruncateStmt<'input>),
    Reindex(recursa::ArenaBox<'input, ReindexStmt<'input>>),
    Refresh(RefreshStmt<'input>),
    Cluster(recursa::ArenaBox<'input, ClusterStmt<'input>>),
    Checkpoint(CheckpointStmt),
    Vacuum(recursa::ArenaBox<'input, VacuumStmt<'input>>),
    Lock(LockStmt<'input>),
    Notify(NotifyStmt<'input>),
    Listen(ListenStmt<'input>),
    Unlisten(UnlistenStmt<'input>),
    Discard(DiscardStmt),
    Reassign(ReassignStmt<'input>),
    Do(recursa::ArenaBox<'input, DoStmt<'input>>),
    // Cursor
    Declare(DeclareStmt<'input>),
    Fetch(FetchStmt<'input>),
    Close(CloseStmt<'input>),
    Move(MoveStmt<'input>),
    // Configuration
    // VariableSetStmt owns PostgreSQL's complete `SET set_rest` family,
    // including literal `LOCAL` / `SESSION` prefixes.
    SetConstraints(SetConstraintsStmt<'input>),
    Set(VariableSetStmt<'input>),
    Reset(ResetStmt<'input>),
    Show(ShowStmt<'input>),
    Load(LoadStmt<'input>),
    Analyze(AnalyzeStmt<'input>),
    /// A query without a `WITH` prefix: gram.y `SelectStmt` less its
    /// `with_clause` forms, which `With` owns.
    Query(recursa::ArenaBox<'input, QueryBody<'input>>),
    /// `WITH ... { query | INSERT | UPDATE | DELETE | MERGE }`: the CTE list
    /// is the shared prefix of five statements, so it is factored once.
    With(recursa::ArenaBox<'input, crate::ast::shared::with_clause::WithStatement<'input>>),
}
