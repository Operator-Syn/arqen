// SPDX-License-Identifier: MPL-2.0
use super::*;
#[derive(Clone)]
pub(in crate::broker) struct ActionResource {
    pub message_id: String,
    pub draft_id: Option<String>,
}
pub(in crate::broker) fn register_action_marks(
    state: &BrokerState,
    subject: &str,
    resources: &[ActionResource],
    kind: PendingActionKind,
) -> Result<Vec<String>, ActionMarkFailure> {
    let mut seen = std::collections::HashSet::new();
    let mut drafts = std::collections::HashSet::new();
    for r in resources {
        if !seen.insert(&r.message_id) || r.draft_id.as_ref().is_some_and(|id| !drafts.insert(id)) {
            return Err(ActionMarkFailure::Required);
        }
    }
    let mut marks = state
        .pending_actions
        .lock()
        .map_err(|_| ActionMarkFailure::StoreUnavailable)?;
    let now = Instant::now();
    marks.retain(|_, m| m.executing || m.expires_at > now);
    let overlaps = |m: &PendingActionMark| {
        m.google_subject == subject
            && resources.iter().any(|r| {
                m.message_id == r.message_id
                    || r.draft_id
                        .as_ref()
                        .is_some_and(|id| m.draft_id.as_ref() == Some(id))
            })
    };
    if marks.values().any(|m| overlaps(m) && m.executing) {
        return Err(ActionMarkFailure::InProgress);
    }
    if marks.values().filter(|m| !overlaps(m)).count() + resources.len() > MAX_PENDING_ACTION_MARKS
    {
        return Err(ActionMarkFailure::Limit);
    }
    marks.retain(|_, m| !overlaps(m));
    let mut ids = Vec::with_capacity(resources.len());
    for r in resources {
        let marker_id = uuid::Uuid::new_v4().simple().to_string();
        marks.insert(
            (subject.into(), r.message_id.clone()),
            PendingActionMark {
                marker_id: marker_id.clone(),
                google_subject: subject.into(),
                message_id: r.message_id.clone(),
                draft_id: r.draft_id.clone(),
                kind,
                expires_at: now + DELETION_MARK_TTL,
                executing: false,
            },
        );
        ids.push(marker_id);
    }
    Ok(ids)
}
pub(in crate::broker) fn consume_action_marks(
    state: &BrokerState,
    markers: &[String],
    subject: &str,
    kind: PendingActionKind,
) -> Result<Vec<PendingActionMark>, ActionMarkFailure> {
    let mut marks = state
        .pending_actions
        .lock()
        .map_err(|_| ActionMarkFailure::StoreUnavailable)?;
    let now = Instant::now();
    let mut keys = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for id in markers {
        let Some((key, m)) = marks.iter().find(|(_, m)| m.marker_id == *id) else {
            return Err(ActionMarkFailure::Required);
        };
        if m.google_subject != subject
            || m.kind != kind
            || m.executing
            || m.expires_at <= now
            || !seen.insert(key.clone())
        {
            return Err(ActionMarkFailure::Required);
        }
        keys.push(key.clone());
    }
    Ok(keys
        .iter()
        .map(|key| {
            let m = marks
                .get_mut(key)
                .expect("validated while holding the action lock");
            m.executing = true;
            m.clone()
        })
        .collect())
}
pub(in crate::broker) struct ActionFinishGuard<'a> {
    pub state: &'a BrokerState,
    pub marks: &'a [PendingActionMark],
}
impl Drop for ActionFinishGuard<'_> {
    fn drop(&mut self) {
        for mark in self.marks {
            finish_action_mark(self.state, mark);
        }
    }
}
#[cfg(test)]
#[path = "../../../tests/unit/broker_bulk_actions.rs"]
mod tests;
