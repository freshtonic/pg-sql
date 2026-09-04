fn main() -> Result<(), recursa_explorer::RunError> {
    recursa_explorer::run(pg_sql::ast::Statement::explorer())
}
