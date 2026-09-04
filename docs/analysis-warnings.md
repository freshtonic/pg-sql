# Recursa analysis warning classification

Reviewed classification of every advisory analysis finding the pg-sql build
emits, recorded for the warning-free CI gate (#22) per issue #26. Baseline
history: Reviewed classification of every advisory analysis finding the pg-sql build
emits, recorded for the warning-free CI gate (#22) per issue #26. Baseline
history: 318 raw directives, then 225 distinct findings after the transport
deduplication, then 195 after suffix-proving viability decisions retire 30
`RCA0300` findings by consuming the frozen decision proof (an overlap kind is
retired only when the frozen trie defers past it, so commitment requires the
element's own deeper suffix), now 192 after the structural limit/offset tail
(#27) replaces each optional clause pair with one ordered clause: the four
pair findings retire (one duplicate row each at `SelectStmt`, `TableStmt`,
and `CompoundParen`, plus the sole `DirectParenthesizedSet` row), the three
statement sites keep one finding for the whole optional tail against nested
caller FOLLOW, and the new `LimitThenOffset` / `OffsetThenLimit` shapes add
one finding each for their optional second clause (net −4 +2). Now 160 after the CTE, `WITH`, and CTAS bodies embed
query-shaped types instead of the whole `Statement` enum: the `Statement`
FOLLOW set no longer inherits `WITH [NO] DATA`, set operators, and the other
continuations of a subquery position, so 31 `RCA0300` findings retire (every
`WITH`-overlap tail on DDL statements, the `ABSENT`/`NULL`/`ON` leaks into
`DROP ...`, `COPY`, `ANALYZE`, `VACUUM`, `CLUSTER`, and `REINDEX`), and ten
retained findings shrink to their inherent overlap (`RESTRICT, CASCADE` on the
`DROP` statements, eight interval kinds on the function type tails). Now 0: every retained finding is accepted where it lives with
`#[greedy(...)]` (recursa protocol 18), so the build prints no advisory
analysis warning and the gate is the attribute's own exactness check.

## Verdict key

- **`RCA0300` accepted (130 sites)** — the optional element's viability cannot
  be proven by bounded suffix within `max_lookahead = 5`: the element language
  is open (expression-shaped or depth-cut), the element can fully end on the
  shared token (inherent ambiguity), the overlap is static and handled by the
  differential trie, or the site is a repetition rather than an optional. The
  generated parser keeps the greedy commitment, matching PostgreSQL bison's
  shift preference.
- **`RCA0301` accepted (14 sites)** — a Pratt extender shares a kind with
  caller FOLLOW. Strict Pratt deliberately preserves this ambiguity rather
  than resolving it by convention (the recursa#97 principle); the operand keeps
  extending, which is PostgreSQL's precedence resolution.

## Acceptance mechanism

Each accepted overlap is declared on the field, variant, or Pratt enum that
carries it with `#[greedy(KIND, ...)]`, or `#[greedy(all)]` when every kind
that can start the element is shared. A one-line `/// Greedy:` doc comment
above the attribute states the rationale. The analysis checks every
acceptance on every build against the overlap it finds at that site:

- a declared kind that no overlap contains is `RCA0302` (stale acceptance),
- an overlap kind the attribute omits is `RCA0303` (undeclared overlap),
- malformed syntax or an unknown name is `RCA3131`.

All three fail the build, so an acceptance never hides a new or changed
ambiguity, and a fixed grammar shape surfaces as a stale acceptance to remove.
Railroad diagrams label an accepted element `greedy(...)`. The enum-level
acceptance on `ast::shared::expr::Expr` covers every left-denotation operand
and the `ESCAPE` tails of the LIKE family in one declaration.

Counts: 143 attributes (16 `all`, 1 enum-level, the rest
exact kind lists).

## Accepted sites

| Code | Site | Kinds | Rationale |
| --- | --- | --- | --- |
| RCA0300 | `ast::cursor::declare::DeclareStmt` `options` | ASENSITIVE, BINARY, INSENSITIVE, NO, SCROLL | A leading token from any of 5 kinds starts this element instead of ending `DeclareStmt` (bison shift preference). |
| RCA0300 | `ast::cursor::fetch::FetchBackward` `count` | ALL | A leading ALL starts this element instead of ending `FetchBackward` (bison shift preference). |
| RCA0300 | `ast::cursor::fetch::FetchForward` `count` | ALL | A leading ALL starts this element instead of ending `FetchForward` (bison shift preference). |
| RCA0300 | `ast::ddl::aggregate::DropAggregateStmt` `targets` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropAggregateStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::database::CreateDatabaseStmt` `options` | `all` | Any kind that can start this element continues it instead of ending `CreateDatabaseStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::domain::AlterDomainCheckConstraint` `attrs` | DEFERRABLE, INITIALLY, NO, NOT | A leading DEFERRABLE, INITIALLY, NO, NOT starts this element instead of ending `AlterDomainCheckConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::domain::AlterDomainNotNullConstraint` `attrs` | NOT | A leading NOT starts this element instead of ending `AlterDomainNotNullConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::domain::CreateDomainStmt` `constraints` | CHECK, CONSTRAINT, DEFAULT, NOT, NULL | A leading token from any of 5 kinds starts this element instead of ending `CreateDomainStmt` (bison shift preference). |
| RCA0301 | `ast::ddl::domain::DomainDefault` `expr` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `DomainDefault`. |
| RCA0300 | `ast::ddl::extension::CreateExtensionStmt` `options` | CASCADE, SCHEMA, VERSION, WITH | A leading CASCADE, SCHEMA, VERSION, WITH starts this element instead of ending `CreateExtensionStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::foreign::AlterFdwOptsAction` `rest` | HANDLER, NO, VALIDATOR | A leading HANDLER, NO, VALIDATOR starts this element instead of ending `AlterFdwOptsAction` (bison shift preference). |
| RCA0300 | `ast::ddl::foreign::CreateFdwBody` `fdw_options` | HANDLER, NO, VALIDATOR | A leading HANDLER, NO, VALIDATOR starts this element instead of ending `CreateFdwBody` (bison shift preference). |
| RCA0301 | `ast::ddl::function::CostOption` `value` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `CostOption`. |
| RCA0300 | `ast::ddl::function::CreateFunctionStmt` `options` | `all` | Any kind that can start this element continues it instead of ending `CreateFunctionStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::function::DropFunctionStmt` `targets` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropFunctionStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::function::DropRoutineStmt` `targets` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropRoutineStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::function::FunctionBuiltinType` `tail` | DAY, HOUR, MINUTE, MONTH, SECOND, VARYING, WITHOUT, YEAR | A leading token from any of 8 kinds starts this element instead of ending `FunctionBuiltinType` (bison shift preference). |
| RCA0300 | `ast::ddl::function::FunctionCastTypeTail` `array_suffixes` | LBRACKET | A leading LBRACKET starts this element instead of ending `FunctionCastTypeTail` (bison shift preference). |
| RCA0300 | `ast::ddl::function::FunctionCastTypeTail` `interval_qualifier` | DAY, HOUR, MINUTE, MONTH, SECOND, YEAR | A leading token from any of 6 kinds starts this element instead of ending `FunctionCastTypeTail` (bison shift preference). |
| RCA0300 | `ast::ddl::function::FunctionCastTypeTail` `varying` | VARYING | A leading VARYING starts this element instead of ending `FunctionCastTypeTail` (bison shift preference). |
| RCA0300 | `ast::ddl::function::FunctionIdentifierType` `suffix` | DAY, HOUR, MINUTE, MONTH, SECOND, VARYING, WITHOUT, YEAR | A leading token from any of 8 kinds starts this element instead of ending `FunctionIdentifierType` (bison shift preference). |
| RCA0300 | `ast::ddl::function::FunctionTypeName` `rest` | DOT | A leading DOT starts this element instead of ending `FunctionTypeName` (bison shift preference). |
| RCA0301 | `ast::ddl::function::ReturnOption` `expr` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `ReturnOption`. |
| RCA0301 | `ast::ddl::function::RowsOption` `value` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `RowsOption`. |
| RCA0300 | `ast::ddl::index::DropIndexStmt` `concurrently` | CONCURRENTLY | A leading CONCURRENTLY starts this element instead of ending `DropIndexStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::index::DropIndexStmt` `names` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropIndexStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::procedure::CreateProcedureStmt` `options` | `all` | Any kind that can start this element continues it instead of ending `CreateProcedureStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::procedure::DropProcedureStmt` `targets` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropProcedureStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::role::AlterRoleWithOptions` `options` | `all` | Any kind that can start this element continues it instead of ending `AlterRoleWithOptions` (bison shift preference). |
| RCA0300 | `ast::ddl::role::CreateGroupStmt` `options` | `all` | Any kind that can start this element continues it instead of ending `CreateGroupStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::role::CreateRoleStmt` `options` | `all` | Any kind that can start this element continues it instead of ending `CreateRoleStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::role::CreateUserStmt` `options` | `all` | Any kind that can start this element continues it instead of ending `CreateUserStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::role::DefArgNamedType` `array_suffixes` | LBRACKET | A leading LBRACKET starts this element instead of ending `DefArgNamedType` (bison shift preference). |
| RCA0300 | `ast::ddl::schema::CreateSchemaStmt` `elements` | CREATE, GRANT | A leading CREATE, GRANT starts this element instead of ending `CreateSchemaStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::sequence::CreateSequenceStmt` `options` | 12 kinds (AS, CACHE, CYCLE, …) | A leading token from any of 12 kinds starts this element instead of ending `CreateSequenceStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::sequence::SeqOptList` `rest` | 12 kinds (AS, CACHE, CYCLE, …) | A leading token from any of 12 kinds starts this element instead of ending `SeqOptList` (bison shift preference). |
| RCA0300 | `ast::ddl::table::ColumnDef` `constraints` | 11 kinds (CHECK, COMPRESSION, CONSTRAINT, …) | A leading token from any of 11 kinds starts this element instead of ending `ColumnDef` (bison shift preference). |
| RCA0301 | `ast::ddl::table::DefaultConstraint` `expr` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `DefaultConstraint`. |
| RCA0300 | `ast::ddl::table::DropTableStmt` `names` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropTableStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::table::GeneratedIdentityConstraint` `seq_options` | LPAREN | A leading LPAREN starts this element instead of ending `GeneratedIdentityConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::table::LikeClause` `options` | EXCLUDING, INCLUDING | A leading EXCLUDING, INCLUDING starts this element instead of ending `LikeClause` (bison shift preference). |
| RCA0300 | `ast::ddl::table::PartitionColumnOptionDef` `constraints` | 11 kinds (CHECK, COMPRESSION, CONSTRAINT, …) | A leading token from any of 11 kinds starts this element instead of ending `PartitionColumnOptionDef` (bison shift preference). |
| RCA0301 | `ast::ddl::table::PartitionKeyItem` `expr` | `all` | The expression keeps extending on every shared extender instead of yielding to what may follow `PartitionKeyItem`. |
| RCA0300 | `ast::ddl::table::PrimaryKeyConstraint` `attrs` | NOT | A leading NOT starts this element instead of ending `PrimaryKeyConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::table::ReferencesConstraint` `actions` | ON | A leading ON starts this element instead of ending `ReferencesConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::table::ReferencesConstraint` `deferrable` | NOT | A leading NOT starts this element instead of ending `ReferencesConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::table::TableExclude` `attrs` | NOT | A leading NOT starts this element instead of ending `TableExclude` (bison shift preference). |
| RCA0300 | `ast::ddl::table::TablePrimaryKey` `attrs` | NOT | A leading NOT starts this element instead of ending `TablePrimaryKey` (bison shift preference). |
| RCA0300 | `ast::ddl::table::TableUnique` `attrs` | NOT | A leading NOT starts this element instead of ending `TableUnique` (bison shift preference). |
| RCA0300 | `ast::ddl::table::UniqueConstraint` `attrs` | NOT | A leading NOT starts this element instead of ending `UniqueConstraint` (bison shift preference). |
| RCA0300 | `ast::ddl::trigger::CreateConstraintTriggerStmt` `constraint_attrs` | DEFERRABLE, INITIALLY, NO, NOT | A leading DEFERRABLE, INITIALLY, NO, NOT starts this element instead of ending `CreateConstraintTriggerStmt` (bison shift preference). |
| RCA0300 | `ast::ddl::trigger::TriggerReferencing` `transitions` | NEW, OLD | A leading NEW, OLD starts this element instead of ending `TriggerReferencing` (bison shift preference). |
| RCA0300 | `ast::ddl::view::DropViewStmt` `names` | CASCADE, RESTRICT | A leading CASCADE, RESTRICT starts this element instead of ending `DropViewStmt` (bison shift preference). |
| RCA0300 | `ast::dml::delete::DeleteStmt` `alias` | ABSENT, NULL | A leading ABSENT, NULL starts this element instead of ending `DeleteStmt` (bison shift preference). |
| RCA0300 | `ast::dml::delete::DeleteStmt` `returning` | RETURNING | A leading RETURNING starts this element instead of ending `DeleteStmt` (bison shift preference). |
| RCA0301 | `ast::dml::insert::ConflictTargetItem` `expr` | AT, BETWEEN, COLLATE, OPERATOR | The expression keeps extending on AT, BETWEEN, COLLATE, OPERATOR instead of yielding to what may follow `ConflictTargetItem`. |
| RCA0300 | `ast::dml::insert::InsertColumnItem` `indirection` | DOT, LBRACKET | A leading DOT, LBRACKET starts this element instead of ending `InsertColumnItem` (bison shift preference). |
| RCA0300 | `ast::dml::insert::InsertStmt` `on_conflict` | ON | A leading ON starts this element instead of ending `InsertStmt` (bison shift preference). |
| RCA0300 | `ast::dml::insert::InsertStmt` `returning` | RETURNING | A leading RETURNING starts this element instead of ending `InsertStmt` (bison shift preference). |
| RCA0300 | `ast::dml::insert::OnConflictClause` `target` | ON | A leading ON starts this element instead of ending `OnConflictClause` (bison shift preference). |
| RCA0300 | `ast::dml::insert::OnConflictClause` `where_clause` | WHERE | A leading WHERE starts this element instead of ending `OnConflictClause` (bison shift preference). |
| RCA0300 | `ast::dml::merge::MergeStmt` `returning` | RETURNING | A leading RETURNING starts this element instead of ending `MergeStmt` (bison shift preference). |
| RCA0300 | `ast::dml::select::ColNameTableRef` `tail` | ABSENT | A leading ABSENT starts this element instead of ending `ColNameTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::FuncTableRef` `alias` | 9 kinds (ABSENT, CROSS, FULL, …) | A leading token from any of 9 kinds starts this element instead of ending `FuncTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::GroupByClause` `items` | `all` | Any kind that can start this element continues it instead of ending `GroupByClause` (bison shift preference). |
| RCA0300 | `ast::dml::select::JoinSuffix` `condition` | ON, USING | A leading ON, USING starts this element instead of ending `JoinSuffix` (bison shift preference). |
| RCA0300 | `ast::dml::select::JoinUsing` `alias` | ABSENT, CROSS, FULL, INNER, JOIN, LEFT, NATURAL, RIGHT | A leading token from any of 8 kinds starts this element instead of ending `JoinUsing` (bison shift preference). |
| RCA0300 | `ast::dml::select::JsonTableRef` `alias` | ABSENT | A leading ABSENT starts this element instead of ending `JsonTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::JsonTableTypedColumn` `on_empty_behaviour` | DEFAULT, EMPTY, ERROR, FALSE, NULL, TRUE, UNKNOWN | A leading token from any of 7 kinds starts this element instead of ending `JsonTableTypedColumn` (bison shift preference). |
| RCA0300 | `ast::dml::select::LateralSubquery` `alias` | ABSENT | A leading ABSENT starts this element instead of ending `LateralSubquery` (bison shift preference). |
| RCA0300 | `ast::dml::select::LimitThenOffset` `offset` | OFFSET | A leading OFFSET starts this element instead of ending `LimitThenOffset` (bison shift preference). |
| RCA0300 | `ast::dml::select::NamedFunctionTableTail` `alias` | 9 kinds (ABSENT, CROSS, FULL, …) | A leading token from any of 9 kinds starts this element instead of ending `NamedFunctionTableTail` (bison shift preference). |
| RCA0300 | `ast::dml::select::NamedInheritedTail` `alias` | ABSENT | A leading ABSENT starts this element instead of ending `NamedInheritedTail` (bison shift preference). |
| RCA0300 | `ast::dml::select::NamedTableRef` `tail` | ABSENT | A leading ABSENT starts this element instead of ending `NamedTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::OffsetThenLimit` `limit` | FETCH, LIMIT | A leading FETCH, LIMIT starts this element instead of ending `OffsetThenLimit` (bison shift preference). |
| RCA0300 | `ast::dml::select::OnlyTableRef` `alias` | ABSENT | A leading ABSENT starts this element instead of ending `OnlyTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::OrderByClause` `items` | `all` | Any kind that can start this element continues it instead of ending `OrderByClause` (bison shift preference). |
| RCA0300 | `ast::dml::select::ParenTableRef` `alias` | ABSENT | A leading ABSENT starts this element instead of ending `ParenTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::RowsFromRef` `alias` | 9 kinds (ABSENT, CROSS, FULL, …) | A leading token from any of 9 kinds starts this element instead of ending `RowsFromRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::SelectExprItem` `alias` | `all` | Any kind that can start this element continues it instead of ending `SelectExprItem` (bison shift preference). |
| RCA0301 | `ast::dml::select::SelectExprItem` `expr` | `all` | The expression keeps extending on every shared extender instead of yielding to what may follow `SelectExprItem`. |
| RCA0300 | `ast::dml::select::SelectStmt` `limit_offset` | FETCH, LIMIT, OFFSET | A leading FETCH, LIMIT, OFFSET starts this element instead of ending `SelectStmt` (bison shift preference). |
| RCA0300 | `ast::dml::select::SelectStmt` `order_by` | ORDER | A leading ORDER starts this element instead of ending `SelectStmt` (bison shift preference). |
| RCA0300 | `ast::dml::select::SelectTargetList` `into` | ABSENT | A leading ABSENT starts this element instead of ending `SelectTargetList` (bison shift preference). |
| RCA0300 | `ast::dml::select::SpecialFuncTableRef` `alias` | 9 kinds (ABSENT, CROSS, FULL, …) | A leading token from any of 9 kinds starts this element instead of ending `SpecialFuncTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::select::TableRef` `joins` | CROSS, FULL, INNER, JOIN, LEFT, NATURAL, RIGHT | A leading token from any of 7 kinds starts this element instead of ending `TableRef` (bison shift preference). |
| RCA0301 | `ast::dml::select::XmlTableColumnDefault` `value` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `XmlTableColumnDefault`. |
| RCA0301 | `ast::dml::select::XmlTableColumnPath` `xpath` | NOT | The expression keeps extending on NOT instead of yielding to what may follow `XmlTableColumnPath`. |
| RCA0300 | `ast::dml::select::XmlTableRef` `alias` | ABSENT | A leading ABSENT starts this element instead of ending `XmlTableRef` (bison shift preference). |
| RCA0300 | `ast::dml::update::ReturningClause` `items` | `all` | Any kind that can start this element continues it instead of ending `ReturningClause` (bison shift preference). |
| RCA0300 | `ast::dml::update::SetClause` tuple field 0 | ABSENT | A leading ABSENT starts this element instead of ending `SetClause` (bison shift preference). |
| RCA0300 | `ast::dml::update::SetTarget` `indirection` | DOT, LBRACKET | A leading DOT, LBRACKET starts this element instead of ending `SetTarget` (bison shift preference). |
| RCA0300 | `ast::dml::update::SingleAssignment` `indirection` | DOT, LBRACKET | A leading DOT, LBRACKET starts this element instead of ending `SingleAssignment` (bison shift preference). |
| RCA0300 | `ast::dml::update::UpdateStmt` `returning` | RETURNING | A leading RETURNING starts this element instead of ending `UpdateStmt` (bison shift preference). |
| RCA0300 | `ast::dml::values::CompoundBody` `set_op` | EXCEPT, INTERSECT, UNION | A leading EXCEPT, INTERSECT, UNION starts this element instead of ending `CompoundBody` (bison shift preference). |
| RCA0300 | `ast::dml::values::CompoundParen` `limit_offset` | FETCH, LIMIT, OFFSET | A leading FETCH, LIMIT, OFFSET starts this element instead of ending `CompoundParen` (bison shift preference). |
| RCA0300 | `ast::dml::values::CompoundParen` `order_by` | ORDER | A leading ORDER starts this element instead of ending `CompoundParen` (bison shift preference). |
| RCA0300 | `ast::dml::values::CompoundParen` `set_op` | EXCEPT, INTERSECT, UNION | A leading EXCEPT, INTERSECT, UNION starts this element instead of ending `CompoundParen` (bison shift preference). |
| RCA0300 | `ast::dml::values::TableStmt` `limit_offset` | FETCH, LIMIT, OFFSET | A leading FETCH, LIMIT, OFFSET starts this element instead of ending `TableStmt` (bison shift preference). |
| RCA0300 | `ast::dml::values::TableStmt` `order_by` | ORDER | A leading ORDER starts this element instead of ending `TableStmt` (bison shift preference). |
| RCA0300 | `ast::shared::expr::CaseSearched` `rest_arms` | WHEN | A leading WHEN starts this element instead of ending `CaseSearched` (bison shift preference). |
| RCA0300 | `ast::shared::expr::CaseSimple` `rest_arms` | WHEN | A leading WHEN starts this element instead of ending `CaseSimple` (bison shift preference). |
| RCA0300 | `ast::shared::expr::CastType` `array_kw_suffix` | ARRAY | A leading ARRAY starts this element instead of ending `CastType` (bison shift preference). |
| RCA0300 | `ast::shared::expr::CastType` `array_suffixes` | LBRACKET | A leading LBRACKET starts this element instead of ending `CastType` (bison shift preference). |
| RCA0301 | `ast::shared::expr::EscapeClause` `char` | `all` | The expression keeps extending on every shared extender instead of yielding to what may follow `EscapeClause`. |
| RCA0300/RCA0301 | `ast::shared::expr::Expr` (enum) | 16 kinds (AND, AT, BETWEEN, …) | This enum-level acceptance covers every left-denotation operand inside `Expr` (the right operands of infix and postfix variants and the operands of prefix forms) plus the optional `ESCAPE` tails of the LIKE family. An operand keeps extending on a shared extender instead of yielding to whatever may follow the enclosing expression, which is PostgreSQL's precedence resolution; `ESCAPE` starts the tail instead of ending the pattern operand. |
| RCA0300 | `ast::shared::expr::FunctionPlainTail` `filter` | FILTER | A leading FILTER starts this element instead of ending `FunctionPlainTail` (bison shift preference). |
| RCA0300 | `ast::shared::expr::FunctionPlainTail` `window` | OVER | A leading OVER starts this element instead of ending `FunctionPlainTail` (bison shift preference). |
| RCA0300 | `ast::shared::expr::FunctionWithinGroupTail` `filter` | FILTER | A leading FILTER starts this element instead of ending `FunctionWithinGroupTail` (bison shift preference). |
| RCA0300 | `ast::shared::expr::FunctionWithinGroupTail` `window` | OVER | A leading OVER starts this element instead of ending `FunctionWithinGroupTail` (bison shift preference). |
| RCA0300 | `ast::shared::expr::GeneralCastType` `varying` | VARYING | A leading VARYING starts this element instead of ending `GeneralCastType` (bison shift preference). |
| RCA0300 | `ast::shared::expr::IntervalCastType` `modifier` | DAY, HOUR, MINUTE, MONTH, SECOND, YEAR | A leading token from any of 6 kinds starts this element instead of ending `IntervalCastType` (bison shift preference). |
| RCA0300 | `ast::shared::expr::IntervalLit` `qualifier` | DAY, HOUR, MINUTE, MONTH, SECOND, YEAR | A leading token from any of 6 kinds starts this element instead of ending `IntervalLit` (bison shift preference). |
| RCA0300 | `ast::shared::expr::IsJsonTail` `type_kind` | ARRAY, OBJECT, SCALAR, VALUE | A leading ARRAY, OBJECT, SCALAR, VALUE starts this element instead of ending `IsJsonTail` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonArrayAgg` `filter` | FILTER | A leading FILTER starts this element instead of ending `JsonArrayAgg` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonArrayAgg` `window` | OVER | A leading OVER starts this element instead of ending `JsonArrayAgg` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonObjectAgg` `filter` | FILTER | A leading FILTER starts this element instead of ending `JsonObjectAgg` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonObjectAgg` `window` | OVER | A leading OVER starts this element instead of ending `JsonObjectAgg` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonQueryInner` `on_behavior_1` | DEFAULT, EMPTY, ERROR, FALSE, NULL, TRUE, UNKNOWN | A leading token from any of 7 kinds starts this element instead of ending `JsonQueryInner` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonUniqueKeys` `keys` | KEYS | A leading KEYS starts this element instead of ending `JsonUniqueKeys` (bison shift preference). |
| RCA0300 | `ast::shared::expr::JsonValueInner` `on_behavior_1` | DEFAULT, EMPTY, ERROR, FALSE, NULL, TRUE, UNKNOWN | A leading token from any of 7 kinds starts this element instead of ending `JsonValueInner` (bison shift preference). |
| RCA0300 | `ast::shared::expr::ParenthesizedExpr` `indirection` | DOT, LBRACKET | A leading DOT, LBRACKET starts this element instead of ending `ParenthesizedExpr` (bison shift preference). |
| RCA0301 | `ast::shared::expr::PositionInner` `needle` | IN | The expression keeps extending on IN instead of yielding to what may follow `PositionInner`. |
| RCA0300 | `ast::shared::expr::PsqlVariableExpr` `value` | `all` | Any kind that can start this element continues it instead of ending `PsqlVariableExpr` (bison shift preference). |
| RCA0300 | `ast::shared::expr::SubscriptSlice` `lower` | COLON | A leading COLON starts this element instead of ending `SubscriptSlice` (bison shift preference). |
| RCA0301 | `ast::shared::expr::SubstringInner` `source` | SIMILAR | The expression keeps extending on SIMILAR instead of yielding to what may follow `SubstringInner`. |
| RCA0300 | `ast::shared::expr::TrimValues` `more` | COMMA | A leading COMMA starts this element instead of ending `TrimValues` (bison shift preference). |
| RCA0300 | `ast::shared::expr::WindowPartitionBy` `exprs` | GROUPS, ORDER, RANGE, ROWS | A leading GROUPS, ORDER, RANGE, ROWS starts this element instead of ending `WindowPartitionBy` (bison shift preference). |
| RCA0300 | `ast::shared::names::QualifiedOperatorPath` `rest` | `all` | Any kind that can start this element continues it instead of ending `QualifiedOperatorPath` (bison shift preference). |
| RCA0300 | `ast::shared::with_clause::CycleClause` `columns` | SET | A leading SET starts this element instead of ending `CycleClause` (bison shift preference). |
| RCA0300 | `ast::shared::with_clause::SearchColumnList` tuple field 0 | SET | A leading SET starts this element instead of ending `SearchColumnList` (bison shift preference). |
| RCA0300 | `ast::tcl::prepared::DeallocateStmt` `prepare` | PREPARE | A leading PREPARE starts this element instead of ending `DeallocateStmt` (bison shift preference). |
| RCA0300 | `ast::tcl::savepoint::ReleaseStmt` `savepoint` | SAVEPOINT | A leading SAVEPOINT starts this element instead of ending `ReleaseStmt` (bison shift preference). |
| RCA0300 | `ast::tcl::transaction::RollbackToClause` `savepoint` | SAVEPOINT | A leading SAVEPOINT starts this element instead of ending `RollbackToClause` (bison shift preference). |
| RCA0300 | `ast::utility::analyze::AnalyzeStmt` `verbose` | VERBOSE | A leading VERBOSE starts this element instead of ending `AnalyzeStmt` (bison shift preference). |
| RCA0300 | `ast::utility::cluster::ClusterStmt` `verbose` | VERBOSE | A leading VERBOSE starts this element instead of ending `ClusterStmt` (bison shift preference). |
| RCA0300 | `ast::utility::comment::CommentConstraintObject` `domain` | DOMAIN | A leading DOMAIN starts this element instead of ending `CommentConstraintObject` (bison shift preference). |
| RCA0300 | `ast::utility::copy::CopyLegacyOptions` `items` | 11 kinds (BINARY, CSV, DELIMITER, …) | A leading token from any of 11 kinds starts this element instead of ending `CopyLegacyOptions` (bison shift preference). |
| RCA0300 | `ast::utility::grant::AlterDefaultPrivilegesStmt` `options` | FOR, IN | A leading FOR, IN starts this element instead of ending `AlterDefaultPrivilegesStmt` (bison shift preference). |
| RCA0300 | `ast::utility::refresh::RefreshStmt` `concurrently` | CONCURRENTLY | A leading CONCURRENTLY starts this element instead of ending `RefreshStmt` (bison shift preference). |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` `freeze` | FREEZE | A leading FREEZE starts this element instead of ending `VacuumStmt` (bison shift preference). |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` `full` | FULL | A leading FULL starts this element instead of ending `VacuumStmt` (bison shift preference). |
| RCA0300 | `ast::utility::vacuum::VacuumStmt` `verbose` | VERBOSE | A leading VERBOSE starts this element instead of ending `VacuumStmt` (bison shift preference). |

## Still warning

None. A forced analysis prints no `RCA0300`, `RCA0301`, `RCA0302`, or
`RCA0303` line.
