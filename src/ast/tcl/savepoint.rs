//! Savepoint statements: SAVEPOINT, RELEASE.

use recursa_diagram::railroad;

use crate::tokens::keyword::*;

/// SAVEPOINT name
#[derive(recursa::Node, Debug, Clone)]
pub struct SavepointStmt<'input> {
    #[tok(SAVEPOINT, this)]
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(ColId))]
    pub name: crate::tokens::ColId<'input>,
}

/// ```sql
/// RELEASE [SAVEPOINT] name
/// ```
#[derive(recursa::Node, Debug, Clone)]
pub struct ReleaseStmt<'input> {
    #[tok(RELEASE, optional(SAVEPOINT), this)]
    #[lex(pattern = r#"(?i:U)&"[^"]*(?:""[^"]*)*"|"[^"]*(?:""[^"]*)*"|[A-Za-z_][A-Za-z0-9_]*"#, admits(ColId))]
    pub name: crate::tokens::ColId<'input>,
}

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
