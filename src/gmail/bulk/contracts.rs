// SPDX-License-Identifier: MPL-2.0
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ReadEmailsRequest {
    #[schemars(length(min = 1, max = 20))]
    pub message_ids: Vec<String>,
}
impl ReadEmailsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_message_ids(&self.message_ids, 20)?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BulkEmailMarkersRequest {
    #[schemars(length(min = 1, max = 100))]
    pub marker_ids: Vec<String>,
}
impl BulkEmailMarkersRequest {
    pub fn validate(self) -> Result<Self> {
        validate_set(&self.marker_ids, 100, |id| {
            DeleteMarkedEmailRequest {
                marker_id: id.clone(),
            }
            .validate()
            .map(|_| ())
        })?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CreateLabelsRequest {
    #[schemars(length(min = 1, max = 20))]
    pub names: Vec<String>,
}
impl CreateLabelsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_set(&self.names, 20, |name| {
            CreateLabelRequest { name: name.clone() }
                .validate()
                .map(|_| ())
        })?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DeleteLabelsRequest {
    #[schemars(length(min = 1, max = 20))]
    pub label_ids: Vec<String>,
}
impl DeleteLabelsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_set(&self.label_ids, 20, |id| {
            DeleteLabelRequest {
                label_id: id.clone(),
            }
            .validate()
            .map(|_| ())
        })?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CreateDraftsRequest {
    #[schemars(length(min = 1, max = 10))]
    pub drafts: Vec<CreateDraftRequest>,
}
impl CreateDraftsRequest {
    pub fn validate(self) -> Result<Self> {
        anyhow::ensure!(
            (1..=10).contains(&self.drafts.len()),
            "drafts must contain 1–10 items"
        );
        for draft in &self.drafts {
            draft.clone().validate()?;
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct CreateReplyDraftsRequest {
    #[schemars(length(min = 1, max = 10))]
    pub replies: Vec<CreateReplyDraftRequest>,
}
impl CreateReplyDraftsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_message_ids(
            &self
                .replies
                .iter()
                .map(|r| r.message_id.clone())
                .collect::<Vec<_>>(),
            10,
        )?;
        for reply in &self.replies {
            reply.clone().validate()?;
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BulkDraftIdsRequest {
    #[schemars(length(min = 1, max = 20))]
    pub draft_ids: Vec<String>,
}
impl BulkDraftIdsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_set(&self.draft_ids, 20, |id| {
            DraftIdRequest {
                draft_id: id.clone(),
            }
            .validate()
            .map(|_| ())
        })?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct BulkDraftMarkersRequest {
    #[schemars(length(min = 1, max = 20))]
    pub marker_ids: Vec<String>,
}
impl BulkDraftMarkersRequest {
    pub fn validate(self) -> Result<Self> {
        validate_set(&self.marker_ids, 20, |id| {
            ActionMarkerRequest {
                marker_id: id.clone(),
            }
            .validate()
            .map(|_| ())
        })?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ApplyLabelToEmailsRequest {
    #[schemars(length(min = 1, max = 100))]
    pub message_ids: Vec<String>,
    #[schemars(length(min = 1), regex(pattern = r"^[^\u0000-\u001F\u007F-\u009F]+$"))]
    pub label_id: String,
}
impl ApplyLabelToEmailsRequest {
    pub fn validate(self) -> Result<Self> {
        validate_message_ids(&self.message_ids, 100)?;
        DeleteLabelRequest {
            label_id: self.label_id.clone(),
        }
        .validate()?;
        Ok(self)
    }
}
fn validate_set(
    ids: &[String],
    limit: usize,
    scalar: impl Fn(&String) -> Result<()>,
) -> Result<()> {
    anyhow::ensure!(
        (1..=limit).contains(&ids.len()),
        "collection must contain 1–{limit} items"
    );
    let mut seen = HashSet::new();
    for id in ids {
        scalar(id)?;
        anyhow::ensure!(seen.insert(id), "duplicate inputs are not allowed");
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BulkResponse<T> {
    pub items: Vec<BulkItem<T>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct BulkItem<T> {
    pub index: usize,
    #[serde(flatten)]
    pub outcome: BulkOutcome<T>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BulkOutcome<T> {
    Succeeded { result: T },
    Failed { code: String, message: String },
    Unknown { code: String, message: String },
    NotAttempted { code: String, message: String },
}
