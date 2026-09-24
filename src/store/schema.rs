// SPDX-License-Identifier: MPL-2.0
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


}
