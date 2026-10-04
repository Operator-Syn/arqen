// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::io::Cursor;

#[test]
fn unicode_draft_boundaries_fit_the_actual_broker_reader() {
    let requests = [
        BrokerRequest::CreateDraft {
            request: crate::gmail::CreateDraftRequest {
                to: format!("{}@x", "é".repeat(318)),
                subject: "😀".repeat(998),
                body: "😀".repeat(24_576),
            },
        },
        BrokerRequest::CreateReplyDraft {
            request: crate::gmail::CreateReplyDraftRequest {
                message_id: "a".repeat(256),
                body: "😀".repeat(24_576),
            },
        },
    ];
    for request in requests {
        assert_eq!(request.clone().validate().unwrap(), request);
        let mut wire = serde_json::to_vec(&request).unwrap();
        wire.push(b'\n');
        let read = read_request(&mut Cursor::new(wire));
        assert_eq!(read.unwrap(), request);
    }
}

#[test]
fn broker_reader_still_rejects_frames_above_the_transport_cap() {
    let wire = vec![b'x'; MAX_REQUEST_BYTES + 1];
    assert!(read_request(&mut Cursor::new(wire)).is_err());
}
