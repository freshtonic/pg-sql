//! Savepoint statements: SAVEPOINT, RELEASE.

use recursa::{FormatTokens, Transform, Visit};
use recursa_diagram::railroad;

use crate::tokens::keyword::*;

/// SAVEPOINT name
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct SavepointStmt<'input> {
    pub savepoint: SAVEPOINT,
    pub name: crate::tokens::ColId<'input>,
}

/// ```sql
/// RELEASE [SAVEPOINT] name
/// ```
#[railroad]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, FormatTokens, Visit, Transform)]
#[recursa::parser(rules = SqlRules, meta_tags = ["tcl"])]
pub struct ReleaseStmt<'input> {
    pub release: RELEASE,
    pub savepoint: Option<SAVEPOINT>,
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
