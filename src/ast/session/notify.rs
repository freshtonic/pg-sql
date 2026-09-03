//! NOTIFY / LISTEN / UNLISTEN.

use crate::tokens::literal;

// --- NOTIFY / LISTEN / UNLISTEN ---

/// The `, payload` clause on a `NOTIFY` statement (Postgres `notify_payload`).
#[derive(recursa::Node, Debug, Clone)]
pub struct NotifyPayload<'input> {
    #[tok(COMMA, this)]
    pub payload: literal::StringLit<'input>,
}

/// NOTIFY channel [, payload]
#[derive(recursa::Node, Debug, Clone)]
pub struct NotifyStmt<'input> {
    #[tok(NOTIFY, this)]
    pub channel: crate::tokens::ColId<'input>,
    pub payload: Option<NotifyPayload<'input>>,
}

/// LISTEN channel
#[derive(recursa::Node, Debug, Clone)]
pub struct ListenStmt<'input> {
    #[tok(LISTEN, this)]
    pub channel: crate::tokens::ColId<'input>,
}

/// Target of an UNLISTEN statement: a channel name or `*` (all channels).
#[derive(recursa::Node, Debug, Clone)]
pub enum UnlistenTarget<'input> {
    #[tok(STAR)]
    /// `*` — unlisten from every channel.
    All,
    /// A specific channel name.
    Channel(crate::tokens::ColId<'input>),
}

/// UNLISTEN channel | *
#[derive(recursa::Node, Debug, Clone)]
pub struct UnlistenStmt<'input> {
    #[tok(UNLISTEN, this)]
    pub target: UnlistenTarget<'input>,
}
