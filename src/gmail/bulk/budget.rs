// SPDX-License-Identifier: MPL-2.0
use super::*;
pub const MAX_BULK_RESULT_BYTES: usize = 1024 * 1024;
/// Complete accounting is present before fetching; accepted bodies replace placeholders.
pub(crate) struct ReadAssembly {
    pub response: BulkResponse<EmailReadResponse>,
    pub exhausted: bool,
}
impl ReadAssembly {
    pub fn new(count: usize) -> Self {
        Self {
            response: BulkResponse {
                items: (0..count)
                    .map(|index| BulkItem {
                        index,
                        outcome: budget_outcome(false),
                    })
                    .collect(),
            },
            exhausted: false,
        }
    }
    pub fn insert(&mut self, index: usize, outcome: BulkOutcome<EmailReadResponse>) {
        self.response.items[index].outcome = outcome;
        // Include the actual broker envelope and delimiter, not only its result.
        #[derive(Serialize)]
        struct Envelope<'a> {
            status: &'static str,
            result: &'a BulkResponse<EmailReadResponse>,
        }
        let envelope = Envelope {
            status: "emails_read",
            result: &self.response,
        };
        if serde_json::to_vec(&envelope).map_or(true, |v| v.len() + 1 > MAX_BULK_RESULT_BYTES) {
            self.response.items[index].outcome = budget_outcome(true);
            self.exhausted = true;
        }
    }
}
fn budget_outcome<T>(fetched: bool) -> BulkOutcome<T> {
    let code = "response_budget_exceeded".into();
    let message = "retry this ID in a smaller read set or use read_email".into();
    if fetched {
        BulkOutcome::Failed { code, message }
    } else {
        BulkOutcome::NotAttempted { code, message }
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/gmail_bulk_budget.rs"]
mod tests;
