//! DROP/CREATE option flags shared across statement families.

/// `CASCADE | RESTRICT` drop behavior.
#[derive(recursa::Node, Debug, Clone)]
pub enum DropBehavior {
    #[tok(CASCADE)]
    Cascade,
    #[tok(RESTRICT)]
    Restrict,
}

/// `IF EXISTS` modifier, shared by every DROP statement that allows it.
#[derive(recursa::Node, Debug, Clone)]
pub enum IfExists {
    #[tok(IF, EXISTS)]
    Value,
}

/// `IF NOT EXISTS` modifier, shared by CREATE statements that allow it.
#[derive(recursa::Node, Debug, Clone)]
pub enum IfNotExists {
    #[tok(IF, NOT, EXISTS)]
    Value,
}
