// SPDX-License-Identifier: MPL-2.0
use super::*;
fn state() -> BrokerState {
    BrokerState {
        api: Arc::new(GmailApi::new().unwrap()),
        refresh_locks: Default::default(),
        database_path: "unused".into(),
        credentials_path: "unused".into(),
        access_tokens: Default::default(),
        pending_actions: Default::default(),
    }
}
#[test]
fn bulk_marks_register_all_or_none_at_capacity() {
    let s = state();
    for i in 0..255 {
        register_action_mark(
            &s,
            "subject",
            &format!("m{i}"),
            None,
            PendingActionKind::TrashMessage,
        )
        .unwrap();
    }
    let resources = vec![
        ActionResource {
            message_id: "new1".into(),
            draft_id: None,
        },
        ActionResource {
            message_id: "new2".into(),
            draft_id: None,
        },
    ];
    assert!(
        register_action_marks(&s, "subject", &resources, PendingActionKind::TrashMessage).is_err()
    );
    assert_eq!(s.pending_actions.lock().unwrap().len(), 255);
}
#[test]
fn bulk_consume_invalid_member_consumes_none() {
    let s = state();
    let id =
        register_action_mark(&s, "subject", "m1", None, PendingActionKind::TrashMessage).unwrap();
    assert!(
        consume_action_marks(
            &s,
            &[id.clone(), "bad".into()],
            "subject",
            PendingActionKind::TrashMessage
        )
        .is_err()
    );
    assert!(consume_action_mark(&s, &id, "subject", PendingActionKind::TrashMessage).is_ok());
}
