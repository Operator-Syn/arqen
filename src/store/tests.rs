// SPDX-License-Identifier: MPL-2.0
#[cfg(test)]
mod tests {
    use super::{Account, AccountStore, ConnectionState, GMAIL_READONLY_SCOPE};

    #[test]
    fn stores_and_lists_multiple_google_accounts() {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&Account {
                id: "account-one".into(),
                subject: "google-subject-one".into(),
                email: "one@example.com".into(),
                display_name: Some("One".into()),
                token_key: Some("google/account-one".into()),
                granted_scopes: Some(vec!["email".into(), "openid".into()]),
                connection_state: ConnectionState::Connected,
            })
            .unwrap();
        store
            .upsert_google_account(&Account {
                id: "account-two".into(),
                subject: "google-subject-two".into(),
                email: "two@example.com".into(),
                display_name: None,
                token_key: None,
                granted_scopes: None,
                connection_state: ConnectionState::Connected,
            })
            .unwrap();

        let accounts = store.list_accounts().unwrap();

        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].email, "one@example.com");
        assert_eq!(accounts[1].email, "two@example.com");
        assert_eq!(
            accounts[0].granted_scopes.as_deref(),
            Some(["email".to_string(), "openid".to_string()].as_slice())
        );
        assert_eq!(accounts[1].granted_scopes, None);
    }

    #[test]
    fn reauth_updates_existing_google_account_without_duplicates() {
        let store = AccountStore::in_memory().unwrap();
        let account = Account {
            id: "account-one".into(),
            subject: "google-subject-one".into(),
            email: "old@example.com".into(),
            display_name: Some("Old".into()),
            token_key: Some("google/account-one".into()),
            granted_scopes: Some(vec!["openid".into(), "profile".into()]),
            connection_state: ConnectionState::Connected,
        };
        store.upsert_google_account(&account).unwrap();

        store
            .upsert_google_account(&Account {
                id: "account-two".into(),
                email: "new@example.com".into(),
                display_name: Some("New".into()),
                granted_scopes: Some(vec!["email".into(), "https://example.test/scope".into()]),
                ..account
            })
            .unwrap();

        let accounts = store.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, "account-two");
        assert_eq!(accounts[0].email, "new@example.com");
        assert_eq!(accounts[0].display_name.as_deref(), Some("New"));
        assert_eq!(
            accounts[0].granted_scopes.as_deref(),
            Some(
                [
                    "email".to_string(),
                    "https://example.test/scope".to_string()
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn removes_scope_rows_with_the_account() {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&Account {
                id: "account-one".into(),
                subject: "google-subject-one".into(),
                email: "one@example.com".into(),
                display_name: None,
                token_key: None,
                granted_scopes: Some(vec!["openid".into()]),
                connection_state: ConnectionState::Connected,
            })
            .unwrap();

        assert!(store.remove_account("account-one").unwrap());
        assert!(store.list_accounts().unwrap().is_empty());
        let scope_count: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM google_account_scopes", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(scope_count, 0);
    }

    #[test]
    fn rejects_empty_verified_scope_sets() {
        let store = AccountStore::in_memory().unwrap();
        let error = store
            .upsert_google_account(&Account {
                id: "account-one".into(),
                subject: "google-subject-one".into(),
                email: "one@example.com".into(),
                display_name: None,
                token_key: None,
                granted_scopes: Some(Vec::new()),
                connection_state: ConnectionState::Connected,
            })
            .unwrap_err();
        assert!(error.to_string().contains("empty verified scope set"));
    }

    #[test]
    fn state_transitions_round_trip_without_touching_scope_history() {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&Account {
                id: "account-one".into(),
                subject: "google-subject-one".into(),
                email: "one@example.com".into(),
                display_name: None,
                token_key: Some("keyring:arqen:google-subject-one".into()),
                granted_scopes: Some(vec!["openid".into()]),
                connection_state: ConnectionState::Connected,
            })
            .unwrap();

        store
            .set_connection_state("google-subject-one", ConnectionState::Indeterminate)
            .unwrap();
        assert_eq!(
            store.list_accounts().unwrap()[0].connection_state,
            ConnectionState::Indeterminate
        );
        store
            .set_connection_state("google-subject-one", ConnectionState::Disconnected)
            .unwrap();
        let account = &store.list_accounts().unwrap()[0];
        assert_eq!(account.connection_state, ConnectionState::Disconnected);
        assert_eq!(
            account.granted_scopes.as_deref(),
            Some(["openid".to_string()].as_slice())
        );
    }

    fn eligible_account(id: &str, subject: &str, email: &str) -> Account {
        Account {
            id: id.into(),
            subject: subject.into(),
            email: email.into(),
            display_name: Some("Eligible account".into()),
            token_key: Some(format!("keyring:arqen:{subject}")),
            granted_scopes: Some(vec![GMAIL_READONLY_SCOPE.into(), "openid".into()]),
            connection_state: ConnectionState::Connected,
        }
    }

    #[test]
    fn mcp_target_configuration_defaults_to_empty_and_round_trips() {
        let store = AccountStore::in_memory().unwrap();
        assert_eq!(
            store.mcp_configuration().unwrap().target_google_subject,
            None
        );
        store
            .upsert_google_account(&eligible_account(
                "account-one",
                "google-subject-one",
                "one@example.com",
            ))
            .unwrap();

        store
            .set_mcp_target_subject(Some("google-subject-one"))
            .unwrap();
        assert_eq!(
            store
                .mcp_configuration()
                .unwrap()
                .target_google_subject
                .as_deref(),
            Some("google-subject-one")
        );

        store.set_mcp_target_subject(None).unwrap();
        assert_eq!(
            store.mcp_configuration().unwrap().target_google_subject,
            None
        );
    }

    #[test]
    fn mcp_target_replaces_previous_target_atomically() {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&eligible_account(
                "account-one",
                "google-subject-one",
                "one@example.com",
            ))
            .unwrap();
        store
            .upsert_google_account(&eligible_account(
                "account-two",
                "google-subject-two",
                "two@example.com",
            ))
            .unwrap();

        store
            .set_mcp_target_subject(Some("google-subject-one"))
            .unwrap();
        store
            .set_mcp_target_subject(Some("google-subject-two"))
            .unwrap();

        assert_eq!(
            store
                .mcp_configuration()
                .unwrap()
                .target_google_subject
                .as_deref(),
            Some("google-subject-two")
        );
    }

    #[test]
    fn mcp_target_requires_connected_verified_gmail_account_with_keyring_reference() {
        let store = AccountStore::in_memory().unwrap();
        let mut no_gmail = eligible_account("one", "subject-one", "one@example.com");
        no_gmail.granted_scopes = Some(vec!["openid".into()]);
        store.upsert_google_account(&no_gmail).unwrap();
        let mut disconnected = eligible_account("two", "subject-two", "two@example.com");
        disconnected.connection_state = ConnectionState::Disconnected;
        store.upsert_google_account(&disconnected).unwrap();
        let mut no_keyring = eligible_account("three", "subject-three", "three@example.com");
        no_keyring.token_key = None;
        store.upsert_google_account(&no_keyring).unwrap();

        for subject in ["missing", "subject-one", "subject-two", "subject-three"] {
            let error = store.set_mcp_target_subject(Some(subject)).unwrap_err();
            assert!(error.to_string().contains("connected Google account"));
        }
    }

    #[test]
    fn deleting_an_account_clears_its_mcp_target_without_affecting_other_accounts() {
        let store = AccountStore::in_memory().unwrap();
        store
            .upsert_google_account(&eligible_account(
                "account-one",
                "google-subject-one",
                "one@example.com",
            ))
            .unwrap();
        store
            .set_mcp_target_subject(Some("google-subject-one"))
            .unwrap();

        assert!(store.remove_account("account-one").unwrap());
        assert_eq!(
            store.mcp_configuration().unwrap().target_google_subject,
            None
        );
    }

    #[test]
    fn migrates_old_account_rows_as_connected() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE google_accounts (
                     id TEXT PRIMARY KEY NOT NULL,
                     google_subject TEXT NOT NULL UNIQUE,
                     email TEXT NOT NULL,
                     display_name TEXT,
                     token_key TEXT,
                     created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                     last_used_at TEXT
                 );
                 INSERT INTO google_accounts
                     (id, google_subject, email, display_name, token_key)
                 VALUES ('old', 'old-subject', 'old@example.com', NULL, NULL);",
            )
            .unwrap();
        let store = AccountStore { connection };
        store.migrate().unwrap();

        let accounts = store.list_accounts().unwrap();
        assert_eq!(accounts[0].connection_state, ConnectionState::Connected);
        assert_eq!(accounts[0].granted_scopes, None);
    }
}
