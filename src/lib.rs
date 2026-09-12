pub mod auth;
pub mod broker;
pub mod gmail;
pub mod mcp;

use anyhow::Result;
use rusqlite::{Connection, params};

pub const GMAIL_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub subject: String,
    pub email: String,
    pub display_name: Option<String>,
    /// Identifier for a future OS-keyring entry; never the token itself.
    pub token_key: Option<String>,
    /// The exact scope set returned by Google for the last successful login.
    /// `None` means this account predates scope tracking or has no verified grant.
    pub granted_scopes: Option<Vec<String>>,
    pub connection_state: ConnectionState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Indeterminate,
}

impl ConnectionState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::Indeterminate => "indeterminate",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "connected" => Ok(Self::Connected),
            "disconnected" => Ok(Self::Disconnected),
            "indeterminate" => Ok(Self::Indeterminate),
            _ => anyhow::bail!("unknown account connection state: {value}"),
        }
    }
}

pub struct AccountStore {
    connection: Connection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConfiguration {
    pub target_google_subject: Option<String>,
}

impl AccountStore {
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self> {
        let connection = Connection::open(path)?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        let store = Self { connection };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS google_accounts (
                 id TEXT PRIMARY KEY NOT NULL,
                 google_subject TEXT NOT NULL UNIQUE,
                 email TEXT NOT NULL,
                 display_name TEXT,
                 token_key TEXT,
                 connection_state TEXT NOT NULL DEFAULT 'connected',
                 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                 last_used_at TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_google_accounts_email
                 ON google_accounts(email);
             CREATE TABLE IF NOT EXISTS google_account_scopes (
                 google_subject TEXT NOT NULL,
                 scope TEXT NOT NULL,
                 PRIMARY KEY (google_subject, scope),
                 FOREIGN KEY (google_subject)
                     REFERENCES google_accounts(google_subject)
                     ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS mcp_configuration (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 target_google_subject TEXT,
                 updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                 FOREIGN KEY (target_google_subject)
                     REFERENCES google_accounts(google_subject)
                     ON DELETE SET NULL
             );
             INSERT OR IGNORE INTO mcp_configuration (id) VALUES (1);",
        )?;
        let has_connection_state = {
            let mut statement = self
                .connection
                .prepare("PRAGMA table_info(google_accounts)")?;
            statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|column| column == "connection_state")
        };
        if !has_connection_state {
            self.connection.execute(
                "ALTER TABLE google_accounts
                 ADD COLUMN connection_state TEXT NOT NULL DEFAULT 'connected'",
                [],
            )?;
        }
        Ok(())
    }

    pub fn upsert_google_account(&self, account: &Account) -> Result<()> {
        if let Some(scopes) = &account.granted_scopes {
            anyhow::ensure!(
                !scopes.is_empty(),
                "cannot persist an empty verified scope set"
            );
        }
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO google_accounts
                (id, google_subject, email, display_name, token_key, connection_state, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(google_subject) DO UPDATE SET
                id = excluded.id,
                email = excluded.email,
                display_name = excluded.display_name,
                token_key = excluded.token_key,
                connection_state = excluded.connection_state,
                last_used_at = excluded.last_used_at",
            params![
                account.id,
                account.subject,
                account.email,
                account.display_name,
                account.token_key,
                account.connection_state.as_str(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM google_account_scopes WHERE google_subject = ?1",
            params![account.subject],
        )?;
        if let Some(scopes) = &account.granted_scopes {
            for scope in scopes {
                transaction.execute(
                    "INSERT INTO google_account_scopes (google_subject, scope)
                     VALUES (?1, ?2)",
                    params![account.subject, scope],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn set_connection_state(&self, subject: &str, state: ConnectionState) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE google_accounts
             SET connection_state = ?1
             WHERE google_subject = ?2",
            params![state.as_str(), subject],
        )?;
        anyhow::ensure!(
            changed == 1,
            "Google account subject was not found while updating connection state"
        );
        transaction.commit()?;
        Ok(())
    }

    pub fn mcp_configuration(&self) -> Result<McpConfiguration> {
        let target_google_subject = self.connection.query_row(
            "SELECT target_google_subject
             FROM mcp_configuration
             WHERE id = 1",
            [],
            |row| row.get::<_, Option<String>>(0),
        )?;
        Ok(McpConfiguration {
            target_google_subject,
        })
    }

    pub fn set_mcp_target_subject(&self, subject: Option<&str>) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        if let Some(subject) = subject {
            anyhow::ensure!(!subject.is_empty(), "MCP target subject cannot be empty");
            let eligible: i64 = transaction.query_row(
                "SELECT COUNT(*)
                 FROM google_accounts AS account
                 WHERE account.google_subject = ?1
                   AND account.connection_state = 'connected'
                   AND account.token_key IS NOT NULL
                   AND EXISTS (
                       SELECT 1
                       FROM google_account_scopes AS scope
                       WHERE scope.google_subject = account.google_subject
                         AND scope.scope = ?2
                   )",
                params![subject, GMAIL_READONLY_SCOPE],
                |row| row.get(0),
            )?;
            anyhow::ensure!(
                eligible == 1,
                "MCP target must be a connected Google account with a recorded Gmail read-only grant and a keyring reference"
            );
        }
        transaction.execute(
            "UPDATE mcp_configuration
             SET target_google_subject = ?1,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = 1",
            params![subject],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut statement = self.connection.prepare(
            "SELECT id, google_subject, email, display_name, token_key, connection_state
             FROM google_accounts
             ORDER BY created_at ASC, id ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                subject: row.get(1)?,
                email: row.get(2)?,
                display_name: row.get(3)?,
                token_key: row.get(4)?,
                connection_state: ConnectionState::parse(&row.get::<_, String>(5)?).map_err(
                    |error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            5,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                error.to_string(),
                            )),
                        )
                    },
                )?,
                granted_scopes: None,
            })
        })?;
        let mut accounts = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let mut scope_statement = self.connection.prepare(
            "SELECT scope
             FROM google_account_scopes
             WHERE google_subject = ?1
             ORDER BY scope ASC",
        )?;
        for account in &mut accounts {
            let scopes = scope_statement
                .query_map(params![account.subject], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            if !scopes.is_empty() {
                account.granted_scopes = Some(scopes);
            }
        }
        Ok(accounts)
    }

    pub fn remove_account(&self, id: &str) -> Result<bool> {
        Ok(self
            .connection
            .execute("DELETE FROM google_accounts WHERE id = ?1", params![id])?
            > 0)
    }
}

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
