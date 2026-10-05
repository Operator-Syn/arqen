// SPDX-License-Identifier: MPL-2.0
use super::*;
fn email(body: String) -> EmailReadResponse {
    serde_json::from_value(serde_json::json!({"message_id":"m1","thread_id":"thread","from":null,"recipients":{"to":[],"cc":[],"bcc":[]},"date":null,"subject":null,"labels":[],"body_text":body,"body_status":"complete"})).unwrap()
}
#[test]
fn bulk_reads_budget_counts_broker_envelope_and_newline() {
    let mut assembly = ReadAssembly::new(20);
    assembly.insert(
        0,
        BulkOutcome::Succeeded {
            result: email(String::new()),
        },
    );
    let overhead = serde_json::to_vec(&assembly.response).unwrap().len();
    assembly.insert(
        0,
        BulkOutcome::Succeeded {
            result: email("x".repeat(MAX_BULK_RESULT_BYTES - overhead)),
        },
    );
    let wire = crate::mcp::BrokerResponse::EmailsRead {
        result: assembly.response,
    };
    assert!(
        serde_json::to_vec(&wire).unwrap().len() < MAX_BULK_RESULT_BYTES,
        "envelope exceeded budget"
    );
}
#[test]
fn bulk_reads_budget_counts_escaping_and_distinguishes_fetched_items() {
    let mut assembly = ReadAssembly::new(20);
    assembly.insert(
        0,
        BulkOutcome::Succeeded {
            result: email("\0".repeat(180_000)),
        },
    );
    assert!(assembly.exhausted);
    assert!(
        matches!(assembly.response.items[0].outcome, BulkOutcome::Failed { ref code, .. } if code == "response_budget_exceeded")
    );
    assembly.insert(
        1,
        BulkOutcome::Succeeded {
            result: email("complete small neighbor".into()),
        },
    );
    assert!(matches!(
        assembly.response.items[1].outcome,
        BulkOutcome::Succeeded { .. }
    ));
    for (index, item) in assembly.response.items.iter().enumerate() {
        assert_eq!(item.index, index);
        if index >= 2 {
            assert!(matches!(item.outcome, BulkOutcome::NotAttempted { .. }));
        }
    }
    assert!(serde_json::to_vec(&assembly.response).unwrap().len() <= MAX_BULK_RESULT_BYTES);
}
