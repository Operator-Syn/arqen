// SPDX-License-Identifier: MPL-2.0
use super::*;
pub(super) fn read(
    request: ReadEmailsRequest,
    account: &Account,
    state: &BrokerState,
) -> BrokerResponse {
    let mut assembly = ReadAssembly::new(request.message_ids.len());
    for (window, ids) in request.message_ids.chunks(4).enumerate() {
        let results = run_items(ids, account, state, |api, token, id| {
            match api.read_email_exact(token, id) {
                Ok(result) => Ok(BulkOutcome::Succeeded { result }),
                Err(e) if is_unauthorized(&e) => Err(e),
                Err(e) => Ok(bulk_read_failure(&e)),
            }
        });
        for item in results.items {
            assembly.insert(window * 4 + item.index, item.outcome);
        }
        if assembly.exhausted {
            break;
        }
    }
    BrokerResponse::EmailsRead {
        result: assembly.response,
    }
}
