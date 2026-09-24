// SPDX-License-Identifier: MPL-2.0
impl AccountStore {
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
