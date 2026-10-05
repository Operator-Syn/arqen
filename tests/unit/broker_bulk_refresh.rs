// SPDX-License-Identifier: MPL-2.0
use super::*;
fn fixture() -> (BrokerState, Account) {
    let state = BrokerState {
        database_path: PathBuf::from("never-opened"),
        credentials_path: PathBuf::from("never-opened"),
        api: Arc::new(GmailApi::new().unwrap()),
        refresh_locks: Default::default(),
        access_tokens: Default::default(),
        pending_actions: Default::default(),
    };
    let account = Account {
        id: "fixture".into(),
        subject: "subject".into(),
        email: "fixture@example.com".into(),
        display_name: None,
        token_key: None,
        granted_scopes: None,
        connection_state: ConnectionState::Connected,
    };
    (state, account)
}
#[test]
fn bulk_refresh_serializes_expired_cache_once_per_subject() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (state, account) = fixture();
    let count = AtomicUsize::new(0);
    let start = std::sync::Barrier::new(8);
    thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                start.wait();
                let value = refresh_token_using(&account, &state, None, || {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok(crate::auth::RefreshedAccessToken {
                        value: "synthetic-new".into(),
                        expires_in: Some(3600),
                    })
                })
                .unwrap();
                assert_eq!(value, "synthetic-new");
            });
        }
    });
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
#[test]
fn bulk_refresh_stale_401_reuses_newer_token_without_invalidating_it() {
    let (state, account) = fixture();
    state.access_tokens.lock().unwrap().insert(
        account.subject.clone(),
        CachedAccessToken {
            value: "synthetic-new".into(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        },
    );
    let value = refresh_token_using(&account, &state, Some("synthetic-old"), || {
        panic!("stale rejection must not refresh newer token")
    })
    .unwrap();
    assert_eq!(value, "synthetic-new");
}
