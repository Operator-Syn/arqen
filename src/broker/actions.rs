// SPDX-License-Identifier: MPL-2.0
use super::*;

pub(super) const DELETION_MARK_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_PENDING_ACTION_MARKS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingActionKind {
    TrashMessage,
    DeleteDraft,
    SendDraft,
}

#[derive(Debug, Clone)]
pub(super) struct PendingActionMark {
    pub(super) marker_id: String,
    pub(super) google_subject: String,
    pub(super) message_id: String,
    pub(super) draft_id: Option<String>,
    pub(super) kind: PendingActionKind,
    pub(super) expires_at: Instant,
    executing: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ActionMarkFailure {
    StoreUnavailable,
    Limit,
    InProgress,
    Required,
}

pub(super) fn register_action_mark(
    state: &BrokerState,
    google_subject: &str,
    message_id: &str,
    draft_id: Option<&str>,
    kind: PendingActionKind,
) -> std::result::Result<String, ActionMarkFailure> {
    let mut marks = state
        .pending_actions
        .lock()
        .map_err(|_| ActionMarkFailure::StoreUnavailable)?;
    let now = Instant::now();
    marks.retain(|_, mark| mark.executing || mark.expires_at > now);
    let key = (google_subject.to_owned(), message_id.to_owned());
    if marks.get(&key).is_some_and(|mark| mark.executing) {
        return Err(ActionMarkFailure::InProgress);
    }
    if !marks.contains_key(&key) && marks.len() >= MAX_PENDING_ACTION_MARKS {
        return Err(ActionMarkFailure::Limit);
    }
    let marker_id = uuid::Uuid::new_v4().simple().to_string();
    marks.insert(
        key,
        PendingActionMark {
            marker_id: marker_id.clone(),
            google_subject: google_subject.to_owned(),
            message_id: message_id.to_owned(),
            draft_id: draft_id.map(str::to_owned),
            kind,
            expires_at: now + DELETION_MARK_TTL,
            executing: false,
        },
    );
    Ok(marker_id)
}

pub(super) fn consume_action_mark(
    state: &BrokerState,
    marker_id: &str,
    google_subject: &str,
    expected: PendingActionKind,
) -> std::result::Result<PendingActionMark, ActionMarkFailure> {
    let mut marks = state
        .pending_actions
        .lock()
        .map_err(|_| ActionMarkFailure::StoreUnavailable)?;
    let Some((_, mark)) = marks
        .iter_mut()
        .find(|(_, mark)| mark.marker_id == marker_id)
    else {
        return Err(ActionMarkFailure::Required);
    };
    if mark.google_subject != google_subject
        || mark.kind != expected
        || mark.expires_at <= Instant::now()
        || mark.executing
    {
        return Err(ActionMarkFailure::Required);
    }
    mark.executing = true;
    Ok(mark.clone())
}

pub(super) fn finish_action_mark(state: &BrokerState, mark: &PendingActionMark) {
    if let Ok(mut marks) = state.pending_actions.lock() {
        let key = (mark.google_subject.clone(), mark.message_id.clone());
        if marks
            .get(&key)
            .is_some_and(|current| current.marker_id == mark.marker_id)
        {
            marks.remove(&key);
        }
    }
}
