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
