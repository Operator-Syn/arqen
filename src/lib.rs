pub mod auth;

use anyhow::Result;
use rusqlite::{Connection, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub subject: String,
    pub email: String,
    pub display_name: Option<String>,
    /// Identifier for a future OS-keyring entry; never the token itself.
    pub token_key: Option<String>,
}

pub struct AccountStore {
    connection: Connection,
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
                 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                 last_used_at TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_google_accounts_email
                 ON google_accounts(email);",
        )?;
        Ok(())
    }

    pub fn upsert_google_account(&self, account: &Account) -> Result<()> {
        self.connection.execute(
            "INSERT INTO google_accounts
                (id, google_subject, email, display_name, token_key, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
             ON CONFLICT(google_subject) DO UPDATE SET
                id = excluded.id,
                email = excluded.email,
                display_name = excluded.display_name,
                token_key = excluded.token_key,
                last_used_at = excluded.last_used_at",
            params![
                account.id,
                account.subject,
                account.email,
                account.display_name,
                account.token_key,
            ],
        )?;
        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<Account>> {
        let mut statement = self.connection.prepare(
            "SELECT id, google_subject, email, display_name, token_key
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
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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
    use super::{Account, AccountStore};

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
            })
            .unwrap();
        store
            .upsert_google_account(&Account {
                id: "account-two".into(),
                subject: "google-subject-two".into(),
                email: "two@example.com".into(),
                display_name: None,
                token_key: None,
            })
            .unwrap();

        let accounts = store.list_accounts().unwrap();

        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].email, "one@example.com");
        assert_eq!(accounts[1].email, "two@example.com");
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
        };
        store.upsert_google_account(&account).unwrap();

        store
            .upsert_google_account(&Account {
                email: "new@example.com".into(),
                display_name: Some("New".into()),
                ..account
            })
            .unwrap();

        let accounts = store.list_accounts().unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email, "new@example.com");
        assert_eq!(accounts[0].display_name.as_deref(), Some("New"));
    }
}
