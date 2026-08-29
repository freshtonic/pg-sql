//! NOTIFY / LISTEN / UNLISTEN.

use recursa_diagram::railroad;

use crate::tokens::keyword::*;
use crate::tokens::{literal, punct};

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
    #[tok(STAR)] /// `*` — unlisten from every channel.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::test_support::*;

    #[test]
    fn listen_is_modelled() {
        let stmt: ListenStmt = parse_stmt("LISTEN foo_event");
        assert_eq!(stmt.channel.text(), "foo_event");
        assert_eq!(
            roundtrip::<ListenStmt>("LISTEN foo_event"),
            "LISTEN foo_event"
        );
    }

    #[test]
    fn notify_without_payload_is_modelled() {
        let stmt: NotifyStmt = parse_stmt("NOTIFY notify_async2");
        assert_eq!(stmt.channel.text(), "notify_async2");
        assert!(stmt.payload.is_none());
        assert_eq!(
            roundtrip::<NotifyStmt>("NOTIFY notify_async2"),
            "NOTIFY notify_async2"
        );
    }

    #[test]
    fn notify_with_payload_keeps_payload() {
        let stmt: NotifyStmt = parse_stmt("NOTIFY chan, 'a message'");
        assert!(stmt.payload.is_some());
        assert_eq!(
            roundtrip::<NotifyStmt>("NOTIFY chan, 'a message'"),
            "NOTIFY chan, 'a message'"
        );
    }
}
