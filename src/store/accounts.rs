// SPDX-License-Identifier: MPL-2.0
impl AccountStore {
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


}
