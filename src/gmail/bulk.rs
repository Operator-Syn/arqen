// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::collections::HashSet;
mod contracts;
pub use contracts::*;
mod budget;
pub use budget::MAX_BULK_RESULT_BYTES;
pub(crate) use budget::ReadAssembly;
#[cfg(test)]
#[path = "../../tests/unit/gmail_bulk_contracts.rs"]
mod contract_tests;

pub const MAX_BULK_MESSAGES: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BulkMessageIdsRequest {
    /// Explicit message IDs from list_emails, in caller order; duplicates are rejected.
    #[schemars(
        length(min = 1, max = 100),
        inner(length(min = 1, max = 256), regex(pattern = r"^[A-Za-z0-9_-]+$"))
    )]
    pub message_ids: Vec<String>,
}

impl BulkMessageIdsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_message_ids(&self.message_ids, MAX_BULK_MESSAGES)?;
        Ok(self)
    }
}

fn validate_message_ids(ids: &[String], limit: usize) -> Result<()> {
    anyhow::ensure!(
        (1..=limit).contains(&ids.len()),
        "message_ids must contain between 1 and {limit} IDs"
    );
    let mut seen = HashSet::with_capacity(ids.len());
    for id in ids {
        ReadEmailRequest {
            message_id: id.clone(),
        }
        .validate()?;
        anyhow::ensure!(seen.insert(id), "message_ids must not contain duplicates");
    }
    Ok(())
}
