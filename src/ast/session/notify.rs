//! NOTIFY / LISTEN / UNLISTEN.

use crate::tokens::literal;

// --- NOTIFY / LISTEN / UNLISTEN ---

recursa::ast_node! {
    /// The `, payload` clause on a `NOTIFY` statement (Postgres `notify_payload`).
    #[derive(Debug)]
    pub struct NotifyPayload {
        #[tok(COMMA, this)]
        pub payload: literal::StringLit,
    }
}

recursa::ast_node! {
    /// NOTIFY channel [, payload]
    #[derive(Debug)]
    pub struct NotifyStmt {
        #[tok(NOTIFY, this)]
        pub channel: crate::tokens::ColId,
        pub payload: Option<NotifyPayload>,
    }
}

recursa::ast_node! {
    /// LISTEN channel
    #[derive(Debug)]
    pub struct ListenStmt {
        #[tok(LISTEN, this)]
        pub channel: crate::tokens::ColId,
    }
}

recursa::ast_node! {
    /// Target of an UNLISTEN statement: a channel name or `*` (all channels).
    #[derive(Debug)]
    pub enum UnlistenTarget {
        #[tok(STAR)]
        /// `*` — unlisten from every channel.
        All,
        /// A specific channel name.
        Channel(crate::tokens::ColId),
    }
}

recursa::ast_node! {
    /// UNLISTEN channel | *
    #[derive(Debug)]
    pub struct UnlistenStmt {
        #[tok(UNLISTEN, this)]
        pub target: UnlistenTarget,
    }
}
