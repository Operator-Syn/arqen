// SPDX-License-Identifier: MPL-2.0
impl AccountStore {
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
                "MCP target must be a connected Google account with a recorded Gmail read-only grant and a protected credential reference"
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


}
