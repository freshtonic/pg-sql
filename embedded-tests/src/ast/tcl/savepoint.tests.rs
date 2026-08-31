#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn savepoint_roundtrips() {
        assert_eq!(roundtrip::<SavepointStmt>("SAVEPOINT one"), "SAVEPOINT one");
    }

    #[test]
    fn release_savepoint_roundtrips() {
        assert_eq!(
            roundtrip::<ReleaseStmt>("RELEASE SAVEPOINT one"),
            "RELEASE SAVEPOINT one"
        );
    }

    #[test]
    fn release_name_roundtrips() {
        assert_eq!(roundtrip::<ReleaseStmt>("RELEASE two"), "RELEASE two");
    }
}
