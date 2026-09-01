# Recursa analysis warning classification

Reviewed classification of every advisory analysis finding the pg-sql build
emits, recorded for the warning-free CI gate (#22) per issue #26. Baseline
history: 318 raw directives, then 225 distinct findings after the transport
deduplication, now 195 after suffix-proving viability decisions retire 30
`RCA0300` findings by consuming the frozen decision proof (an overlap kind is
retired only when the frozen trie defers past it, so commitment requires the
element's own deeper suffix).

## Verdict key

- **`RCA0300` retained (166)** — the optional element's viability cannot be
  proven by bounded suffix within `max_lookahead = 5`: the element language is
  open (expression-shaped or depth-cut), the element can fully end on the
  shared token (inherent ambiguity), the overlap is static and handled by the
  differential trie, or the site is a repetition rather than an optional. The
  generated parser keeps the greedy commitment, matching PostgreSQL bison's
  shift preference. Every finding carries `lookahead=Some(5)->None`: no finite
  depth separates the overlap on the available FOLLOW facts.
- **`RCA0301` retained (29)** — a Pratt extender shares a kind with caller
  FOLLOW. Strict Pratt deliberately preserves this ambiguity rather than
  resolving it by convention (the recursa#97 principle); these findings are
  expected and remain visible.

## Proposed accepted-ambiguity suppression (maintainer decision required)

A reviewed suppression mechanism could retire the 29 `RCA0301` findings and
the inherent-ambiguity subset of `RCA0300`: a checked-in ledger mapping each
finding's stable identity (site path, code, overlap kinds) to a recorded
acceptance rationale, validated at build time so a new or changed finding
fails the gate instead of hiding. This is NOT implemented; adopting it, and
its shape, is a maintainer decision.

## Findings

| Code | Site | Overlap | Verdict |
| --- | --- | --- | --- |
| RCA0300 | `ast::cursor::declare::DeclareStmt` | NO, SCROLL, BINARY, ASENSITIVE, INSENSITIVE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::cursor::fetch::FetchBackward` | ALL | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::cursor::fetch::FetchForward` | ALL | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::aggregate::DropAggregateStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::database::CreateDatabaseStmt` | 485 kinds (SELECT, FROM, WHERE, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::database::DropDatabaseOptions` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::database::DropDatabaseStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::domain::AlterDomainCheckConstraint` | NOT, DEFERRABLE, INITIALLY, NO | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::domain::AlterDomainNotNullConstraint` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::domain::CreateDomainStmt` | NOT, NULL, DEFAULT, CONSTRAINT, CHECK | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::extension::CreateExtensionStmt` | WITH, SCHEMA, CASCADE, VERSION | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::foreign::AlterFdwOptsAction` | NO, HANDLER, VALIDATOR | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::foreign::CreateFdwBody` | NO, HANDLER, VALIDATOR | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::CreateFunctionStmt` | 21 kinds (AS, NOT, SET, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::DropFunctionStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::DropRoutineStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionBuiltinType` | 9 kinds (WITH, WITHOUT, YEAR, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionCastTypeTail` | 6 kinds (YEAR, MONTH, DAY, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionCastTypeTail` | VARYING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionCastTypeTail` | [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionIdentifierType` | 9 kinds (WITH, WITHOUT, YEAR, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionIdentifierTypeSuffix` | 9 kinds (WITH, WITHOUT, YEAR, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::function::FunctionTypeName` | . | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::index::CreateIndexStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::index::DropIndexStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::index::DropIndexStmt::__recursa_presence_envelope` | CONCURRENTLY | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::policy::AlterPolicyAction` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::policy::AlterPolicyModify` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::policy::AlterPolicyStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::policy::CreatePolicyStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::procedure::CreateProcedureStmt` | 21 kinds (AS, NOT, SET, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::procedure::DropProcedureStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::publication::CreatePublicationStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::role::AlterRoleWithOptions` | 20 kinds (USER, INHERIT, VALID, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::role::CreateGroupStmt` | 25 kinds (IN, WITH, USER, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::role::CreateRoleStmt` | 25 kinds (IN, WITH, USER, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::role::CreateUserStmt` | 25 kinds (IN, WITH, USER, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::role::DefArgNamedType` | [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::schema::CreateSchemaStmt` | CREATE, GRANT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::sequence::CreateSequenceStmt` | 12 kinds (AS, CYCLE, UNLOGGED, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::sequence::SeqOptList` | 12 kinds (AS, CYCLE, UNLOGGED, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::statistics::CreateStatisticsStmt` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::statistics::CreateStatisticsStmt` | ON | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::subscription::AlterSubscriptionAddPublication` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::subscription::AlterSubscriptionDropPublication` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::subscription::AlterSubscriptionRefresh` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::subscription::AlterSubscriptionSetPublication` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::subscription::CreateSubscriptionStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::ColumnDef` | 11 kinds (NOT, NULL, PRIMARY, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::ColumnsBody` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::DropTableStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::GeneratedIdentityConstraint` | ( | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::IndexedConstraintColumns` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::LikeClause` | INCLUDING, EXCLUDING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::PartitionColumnOptionDef` | 11 kinds (NOT, NULL, PRIMARY, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::PartitionOfBody` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::PrimaryKeyConstraint` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::ReferencesConstraint` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::ReferencesConstraint` | ON | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::TableExclude` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::TableExclude` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::TablePrimaryKey` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::TableUnique` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::table::UniqueConstraint` | NOT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::tablespace::CreateTablespaceStmt` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::trigger::CreateConstraintTriggerStmt` | NOT, DEFERRABLE, INITIALLY, NO | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::trigger::TriggerReferencing` | NEW, OLD | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::ddl::view::DropViewStmt` | ABSENT, RESTRICT, CASCADE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::delete::DeleteStmt` | NULL, ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::delete::DeleteStmt` | RETURNING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::insert::InsertColumnItem` | ., [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::insert::InsertStmt` | ON | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::insert::InsertStmt` | RETURNING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::insert::OnConflictClause` | ON | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::insert::OnConflictClause` | WHERE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::merge::MergeStmt` | RETURNING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::ColNameTableRef` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::FromClause` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::FuncTableRef` | 9 kinds (JOIN, LEFT, RIGHT, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::GroupByClause` | 17 kinds (NULL, CREATE, ORDER, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::JoinSuffix` | USING, ON | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::JoinUsing` | 8 kinds (JOIN, LEFT, RIGHT, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::JsonTableRef` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::JsonTableTypedColumn` | 7 kinds (TRUE, FALSE, NULL, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::LateralSubquery` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::NamedFunctionTableTail` | 9 kinds (JOIN, LEFT, RIGHT, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::NamedInheritedTail` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::NamedTableRef` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::OnlyTableRef` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::OrderByClause` | 18 kinds (NULL, CREATE, ORDER, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::ParenTableRef` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::RowsFromRef` | 9 kinds (JOIN, LEFT, RIGHT, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::SelectExprItem` | 405 kinds (NULL, TABLE, VALUES, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::SelectStmt` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::SelectStmt` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::SelectStmt` | ORDER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::SelectTargetList` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::SpecialFuncTableRef` | 9 kinds (JOIN, LEFT, RIGHT, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::TableRef` | 7 kinds (JOIN, LEFT, RIGHT, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::select::XmlTableRef` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::update::ReturningClause` | 14 kinds (NULL, CREATE, ORDER, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::update::SetClause` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::update::SetTarget` | ., [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::update::SingleAssignment` | ., [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::update::UpdateStmt` | RETURNING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::CompoundBody` | UNION, EXCEPT, INTERSECT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::CompoundParen` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::CompoundParen` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::CompoundParen` | ORDER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::CompoundParen` | UNION, EXCEPT, INTERSECT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::TableStmt` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::TableStmt` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::dml::values::TableStmt` | ORDER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::CaseSearched` | WHEN | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::CaseSimple` | WHEN | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::CastType` | ARRAY | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::CastType` | [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::DirectParenthesizedSet` | OFFSET, LIMIT, FETCH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::Expr` | ESCAPE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::FunctionPlainTail` | FILTER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::FunctionPlainTail` | OVER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::FunctionWithinGroupTail` | FILTER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::FunctionWithinGroupTail` | OVER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::GeneralCastType` | VARYING | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::IntervalCastType` | 6 kinds (YEAR, MONTH, DAY, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::IntervalLit` | 6 kinds (YEAR, MONTH, DAY, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::IsJsonTail` | ARRAY, SCALAR, OBJECT, VALUE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonArrayAgg` | FILTER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonArrayAgg` | OVER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonObjectAgg` | FILTER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonObjectAgg` | OVER | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonQueryInner` | 7 kinds (TRUE, FALSE, NULL, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonUniqueKeys` | KEYS | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::JsonValueInner` | 7 kinds (TRUE, FALSE, NULL, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::ParenthesizedExpr` | ., [ | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::PsqlVariableExpr` | 485 kinds (SELECT, FROM, WHERE, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::SubscriptSlice` | : | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::TrimValues` | ,  | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::expr::WindowPartitionBy` | ORDER, ROWS, RANGE, GROUPS | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::names::QualifiedOperatorPath` | 420 kinds (IS, VALUES, NULLS, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::with_clause::CycleClause` | SET | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::shared::with_clause::SearchColumnList` | SET | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::tcl::prepared::DeallocateStmt::__recursa_presence_envelope` | PREPARE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::tcl::savepoint::ReleaseStmt::__recursa_presence_envelope` | SAVEPOINT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::tcl::transaction::RollbackToClause::__recursa_presence_envelope` | SAVEPOINT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::analyze::AnalyzeStmt` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::analyze::AnalyzeStmt` | VERBOSE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::cluster::ClusterStmt` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::cluster::ClusterStmt` | VERBOSE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::comment::CommentConstraintObject::__recursa_presence_envelope` | DOMAIN | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyLegacyOptions` | 11 kinds (NULL, ENCODING, ESCAPE, …) | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyOptions` | NULL | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyQueryBody` | NULL, WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyQueryBody::__recursa_attachment` | NULL | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyQueryBody::__recursa_attachment` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyTableBody` | NULL, WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyTableBody::__recursa_attachment` | NULL | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::copy::CopyTableBody::__recursa_attachment` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::grant::AlterDefaultPrivilegesStmt` | FOR, IN | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::grant::GrantRoleBody` | WITH | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::refresh::RefreshStmt::__recursa_presence_envelope` | CONCURRENTLY | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::reindex::ReindexAllTarget` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` | ABSENT | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` | FREEZE | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` | FULL | unproven within lookahead 5 — greedy commitment retained |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` | VERBOSE | unproven within lookahead 5 — greedy commitment retained |
| RCA0301 | `ast::ddl::domain::DomainDefault::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::ddl::function::CostOption::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::ddl::function::ReturnOption::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::ddl::function::RowsOption::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::ddl::table::DefaultConstraint::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::ddl::table::PartitionKeyItem` | 15 kinds (AND, OR, NOT, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::dml::insert::ConflictTargetItem` | COLLATE, BETWEEN, OPERATOR, AT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::dml::select::SelectExprItem` | 15 kinds (AND, OR, NOT, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::dml::select::XmlTableColumnDefault::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::dml::select::XmlTableColumnPath::__recursa_attachment` | NOT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::EscapeClause::__recursa_attachment` | 15 kinds (AND, OR, NOT, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr` | 15 kinds (AND, OR, NOT, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr` | COLLATE, AT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 11 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 11 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 11 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 13 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 13 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 13 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 13 kinds (NOT, IS, IN, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | 14 kinds (AND, NOT, IS, …) | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | COLLATE | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | COLLATE | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | COLLATE | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | COLLATE | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | COLLATE | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::Expr::__recursa_attachment` | COLLATE, AT | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::PositionInner` | IN | strict-Pratt preserved ambiguity — by design |
| RCA0301 | `ast::shared::expr::SubstringInner` | SIMILAR | strict-Pratt preserved ambiguity — by design |
