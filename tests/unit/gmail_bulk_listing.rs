// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::bulk_fixture as fixture;
#[test]
fn bulk_listing_metadata_is_concurrent_bounded_and_ordered() {
    use std::sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    for drafts in [false, true] {
        let rendezvous = Arc::new((Mutex::new(0), Condvar::new()));
        let r = rendezvous.clone();
        let peak = Arc::new(AtomicUsize::new(0));
        let p = peak.clone();
        let provider = fixture::Fixture::new(move |call| {
            assert_eq!(call.method, "GET");
            assert!(call.body.is_null());
            let path = call.path.split('?').next().unwrap();
            if path == "/users/me/messages" {
                return Some((
                    200,
                    serde_json::json!({"messages":(0..8).map(|i|serde_json::json!({"id":format!("m{i}"),"threadId":format!("t{i}")})).collect::<Vec<_>>(),"nextPageToken":"next"}),
                ));
            }
            if path == "/users/me/drafts" {
                return Some((
                    200,
                    serde_json::json!({"drafts":(0..8).map(|i|serde_json::json!({"id":format!("d{i}")})).collect::<Vec<_>>(),"nextPageToken":"next"}),
                ));
            }
            let id = path.split('/').next_back().unwrap();
            let (lock, cv) = &*r;
            let mut active = lock.lock().unwrap();
            *active += 1;
            p.fetch_max(*active, Ordering::Relaxed);
            cv.notify_all();
            if *active < 4 {
                let (guard, _) = cv.wait_timeout(active, Duration::from_millis(200)).unwrap();
                active = guard;
            }
            *active -= 1;
            drop(active);
            let detail = if drafts {
                serde_json::json!({"id":id,"message":{"id":format!("m{}",&id[1..]),"threadId":"thread","labelIds":["DRAFT","IMPORTANT"]}})
            } else {
                serde_json::json!({"id":id,"threadId":format!("t{}",&id[1..]),"labelIds":["INBOX"]})
            };
            Some((200, detail))
        });
        let api = GmailApi::with_base_url(&provider.url).unwrap();
        let ids = if drafts {
            let result = api
                .list_drafts(
                    "fixture",
                    "fixture@example.com",
                    ListDraftsRequest::default(),
                )
                .unwrap();
            assert_eq!(result.next_page_token.as_deref(), Some("next"));
            result
                .drafts
                .into_iter()
                .map(|d| d.draft_id)
                .collect::<Vec<_>>()
        } else {
            let result = api
                .list_emails(
                    "fixture",
                    "fixture@example.com",
                    ListEmailsRequest::default(),
                )
                .unwrap();
            assert_eq!(result.next_page_token.as_deref(), Some("next"));
            result
                .messages
                .into_iter()
                .map(|m| m.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids,
            (0..8)
                .map(|i| format!("{}{i}", if drafts { "d" } else { "m" }))
                .collect::<Vec<_>>()
        );
        assert!(
            peak.load(Ordering::Relaxed) > 1,
            "listing detail requests did not overlap"
        );
        assert!(peak.load(Ordering::Relaxed) <= 4);
        assert_eq!(provider.snapshot().len(), 9);
    }
}
