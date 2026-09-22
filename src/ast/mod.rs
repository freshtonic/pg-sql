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

recursa::ast_node! {
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
    #[derive(Debug)]
    #[flat(pool)]
    pub enum Statement {
        // --- Multi-keyword statements (longest first_pattern first) ---
        Explain(boxed!(ExplainStmt)),
        // CREATE variants: multi-keyword before single-keyword
        CreateFunction(boxed!(CreateFunctionStmt)),
        CreateProcedure(boxed!(CreateProcedureStmt)),
        CreateTablespace(boxed!(CreateTablespaceStmt)),
        ImportForeignSchema(boxed!(ImportForeignSchemaStmt)),
        CreateConstraintTrigger(boxed!(CreateConstraintTriggerStmt)),
        CreateTrigger(boxed!(CreateTriggerStmt)),
        CreateEventTrigger(boxed!(CreateEventTriggerStmt)),
        CreateAccessMethod(CreateAccessMethodStmt),
        CreateMaterializedView(boxed!(CreateMaterializedViewStmt)),
        CreateTextSearch(boxed!(CreateTextSearchStmt)),
        CreateForeign(boxed!(CreateForeignStmt)),
        CreateIndex(boxed!(CreateIndexStmt)),
        CreateView(boxed!(CreateViewStmt)),
        CreateRule(boxed!(CreateRuleStmt)),
        CreateGroup(CreateGroupStmt),
        CreateRole(CreateRoleStmt),
        // `CREATE USER MAPPING ...` is a three-keyword lead and must precede
        // the bare `CREATE USER ...` variant so longest-match-wins picks the
        // specific path.
        CreateUserMapping(boxed!(CreateUserMappingStmt)),
        CreateUser(CreateUserStmt),
        CreateSchema(CreateSchemaStmt),
        CreateSequence(boxed!(CreateSequenceStmt)),
        CreateType(boxed!(CreateTypeStmt)),
        CreateDomain(boxed!(CreateDomainStmt)),
        CreateAggregate(boxed!(CreateAggregateStmt)),
        // `CREATE OPERATOR CLASS ...` and `CREATE OPERATOR FAMILY ...` are
        // three-keyword leads and must precede the bare `CREATE OPERATOR ...`
        // variant so longest-match-wins picks the specific path.
        CreateOperatorClass(boxed!(CreateOperatorClassStmt)),
        CreateOperatorFamily(CreateOperatorFamilyStmt),
        CreateOperator(boxed!(CreateOperatorStmt)),
        CreateCast(boxed!(CreateCastStmt)),
        CreateTransform(boxed!(CreateTransformStmt)),
        CreateCollation(boxed!(CreateCollationStmt)),
        CreateExtension(CreateExtensionStmt),
        CreatePolicy(boxed!(CreatePolicyStmt)),
        CreateStatistics(boxed!(CreateStatisticsStmt)),
        CreatePublication(boxed!(CreatePublicationStmt)),
        CreateSubscription(boxed!(CreateSubscriptionStmt)),
        CreateConversion(boxed!(CreateConversionStmt)),
        CreateServer(boxed!(CreateServerStmt)),
        CreateLanguage(boxed!(CreateLanguageStmt)),
        CreateDatabase(CreateDatabaseStmt),
        CreateTable(boxed!(CreateTableStmt)),
        // DROP variants
        DropFunction(boxed!(DropFunctionStmt)),
        DropProcedure(boxed!(DropProcedureStmt)),
        DropRoutine(boxed!(DropRoutineStmt)),
        DropTablespace(boxed!(DropTablespaceStmt)),
        DropTrigger(DropTriggerStmt),
        DropEventTrigger(DropEventTriggerStmt),
        DropAccessMethod(DropAccessMethodStmt),
        DropMaterializedView(DropMaterializedViewStmt),
        DropTextSearch(DropTextSearchStmt),
        DropForeign(DropForeignStmt),
        DropOwned(DropOwnedStmt),
        DropIndex(DropIndexStmt),
        DropView(DropViewStmt),
        DropRule(DropRuleStmt),
        DropGroup(DropGroupStmt),
        DropRole(DropRoleStmt),
        // `DROP USER MAPPING ...` is a three-keyword lead and must precede
        // the bare `DROP USER ...` variant so longest-match-wins picks the
        // specific path.
        DropUserMapping(DropUserMappingStmt),
        DropUser(DropUserStmt),
        DropSchema(DropSchemaStmt),
        DropSequence(DropSequenceStmt),
        DropType(DropTypeStmt),
        DropDomain(DropDomainStmt),
        DropAggregate(DropAggregateStmt),
        // `DROP OPERATOR CLASS ...` and `DROP OPERATOR FAMILY ...` are
        // three-keyword leads and must precede the bare `DROP OPERATOR ...`
        // variant so longest-match-wins picks the specific path.
        DropOperatorClass(DropOperatorClassStmt),
        DropOperatorFamily(DropOperatorFamilyStmt),
        DropOperator(boxed!(DropOperatorStmt)),
        DropCast(DropCastStmt),
        DropTransform(DropTransformStmt),
        DropCollation(DropCollationStmt),
        DropExtension(DropExtensionStmt),
        DropPolicy(DropPolicyStmt),
        DropStatistics(DropStatisticsStmt),
        DropPublication(DropPublicationStmt),
        DropSubscription(DropSubscriptionStmt),
        DropConversion(DropConversionStmt),
        DropServer(DropServerStmt),
        DropLanguage(DropLanguageStmt),
        DropDatabase(DropDatabaseStmt),
        DropTable(boxed!(DropTableStmt)),
        // ALTER variants: multi-keyword before single-keyword
        AlterDefaultPrivileges(boxed!(AlterDefaultPrivilegesStmt)),
        AlterForeign(boxed!(AlterForeignStmt)),
        AlterEventTrigger(AlterEventTriggerStmt),
        AlterTrigger(boxed!(AlterTriggerStmt)),
        AlterMaterializedView(boxed!(AlterMaterializedViewStmt)),
        AlterTextSearch(boxed!(AlterTextSearchStmt)),
        AlterLargeObject(AlterLargeObjectStmt),
        AlterTablespace(AlterTablespaceStmt),
        AlterTable(boxed!(AlterTableStmt)),
        AlterRule(AlterRuleStmt),
        AlterGroup(AlterGroupStmt),
        AlterRole(boxed!(AlterRoleStmt)),
        // `ALTER USER MAPPING ...` is a three-keyword lead and must precede
        // the bare `ALTER USER ...` variant so longest-match-wins picks the
        // specific path.
        AlterUserMapping(boxed!(AlterUserMappingStmt)),
        AlterUser(boxed!(AlterUserStmt)),
        AlterSchema(AlterSchemaStmt),
        AlterSequence(boxed!(AlterSequenceStmt)),
        AlterType(boxed!(AlterTypeStmt)),
        AlterDomain(boxed!(AlterDomainStmt)),
        AlterAggregate(boxed!(AlterAggregateStmt)),
        // `ALTER OPERATOR CLASS ...` and `ALTER OPERATOR FAMILY ...` are
        // three-keyword leads and must precede the bare `ALTER OPERATOR ...`
        // variant so longest-match-wins picks the specific path.
        AlterOperatorClass(boxed!(AlterOperatorClassStmt)),
        AlterOperatorFamily(boxed!(AlterOperatorFamilyStmt)),
        AlterOperator(boxed!(AlterOperatorStmt)),
        AlterCollation(AlterCollationStmt),
        AlterExtension(AlterExtensionStmt),
        AlterPolicy(boxed!(AlterPolicyStmt)),
        AlterStatistics(AlterStatisticsStmt),
        AlterPublication(AlterPublicationStmt),
        AlterSubscription(AlterSubscriptionStmt),
        AlterConversion(AlterConversionStmt),
        AlterServer(AlterServerStmt),
        AlterLanguage(AlterLanguageStmt),
        AlterDatabase(boxed!(AlterDatabaseStmt)),
        AlterIndex(boxed!(AlterIndexStmt)),
        AlterView(boxed!(AlterViewStmt)),
        AlterFunction(boxed!(AlterFunctionStmt)),
        AlterProcedure(boxed!(AlterProcedureStmt)),
        AlterRoutine(boxed!(AlterRoutineStmt)),
        // CALL stored procedure
        Call(boxed!(CallStmt)),
        // DML
        Insert(boxed!(InsertStmt)),
        Update(boxed!(UpdateStmt)),
        /// Added in 15: research, PostgreSQL 15, "New statements".
        #[cfg(feature = "since-pg15")]
        Merge(boxed!(crate::ast::dml::merge::MergeStmt)),
        Delete(boxed!(DeleteStmt)),
        // Transaction control
        Rollback(RollbackStmt),
        Savepoint(SavepointStmt),
        Release(ReleaseStmt),
        StartTransaction(StartTransactionStmt),
        Begin(BeginStmt),
        Commit(CommitStmt),
        End(EndStmt),
        Abort(AbortStmt),
        // PREPARE / EXECUTE / DEALLOCATE
        Deallocate(DeallocateStmt),
        Prepare(PrepareStmt),
        Execute(ExecuteStmt),
        // Permissions
        Grant(boxed!(GrantStmt)),
        Revoke(boxed!(RevokeStmt)),
        // Utility
        SecurityLabel(boxed!(SecurityLabelStmt)),
        Comment(boxed!(CommentStmt)),
        Copy(boxed!(CopyStmt)),
        Truncate(TruncateStmt),
        Reindex(boxed!(ReindexStmt)),
        Refresh(RefreshStmt),
        Cluster(boxed!(ClusterStmt)),
        Checkpoint(CheckpointStmt),
        /// Added in 19: gram.y `RepackStmt` (b73d13c:12080; research
        /// PostgreSQL 19, "New statements").
        #[cfg(feature = "since-pg19")]
        Repack(boxed!(crate::ast::utility::repack::RepackStmt)),
        /// Added in 19: gram.y `WaitStmt` (b73d13c:16635; research
        /// PostgreSQL 19, "New statements").
        #[cfg(feature = "since-pg19")]
        Wait(crate::ast::utility::wait::WaitStmt),
        Vacuum(boxed!(VacuumStmt)),
        Lock(LockStmt),
        Notify(NotifyStmt),
        Listen(ListenStmt),
        Unlisten(UnlistenStmt),
        Discard(DiscardStmt),
        Reassign(ReassignStmt),
        Do(boxed!(DoStmt)),
        // Cursor
        Declare(DeclareStmt),
        Fetch(FetchStmt),
        Close(CloseStmt),
        Move(MoveStmt),
        // Configuration
        // VariableSetStmt owns PostgreSQL's complete `SET set_rest` family,
        // including literal `LOCAL` / `SESSION` prefixes.
        SetConstraints(SetConstraintsStmt),
        Set(VariableSetStmt),
        Reset(ResetStmt),
        Show(ShowStmt),
        Load(LoadStmt),
        Analyze(AnalyzeStmt),
        /// A query without a `WITH` prefix: gram.y `SelectStmt` less its
        /// `with_clause` forms, which `With` owns.
        Query(boxed!(QueryBody)),
        /// `WITH ... { query | INSERT | UPDATE | DELETE | MERGE }`: the CTE list
        /// is the shared prefix of five statements, so it is factored once.
        With(boxed!(crate::ast::shared::with_clause::WithStatement)),
    }
}
