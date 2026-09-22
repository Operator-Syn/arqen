impl ListEmailsRequest {
    pub fn validate(mut self) -> Result<Self> {
        if let Some(query) = self.query.take() {
            validate_text("query", &query, MAX_QUERY_LENGTH)?;
            let query = query.trim().to_owned();
            self.query = (!query.is_empty()).then_some(query);
        }
        anyhow::ensure!(
            (1..=MAX_MAX_RESULTS).contains(&self.max_results),
            "max_results must be between 1 and {MAX_MAX_RESULTS}"
        );
        anyhow::ensure!(
            self.label_ids.len() <= MAX_LABEL_IDS,
            "at most {MAX_LABEL_IDS} label IDs may be requested"
        );
        for label_id in &self.label_ids {
            anyhow::ensure!(!label_id.is_empty(), "label IDs cannot be empty");
            validate_text("label ID", label_id, MAX_LABEL_ID_LENGTH)?;
        }
        if let Some(page_token) = &self.page_token {
            anyhow::ensure!(!page_token.is_empty(), "page_token cannot be empty");
            validate_text("page token", page_token, MAX_PAGE_TOKEN_LENGTH)?;
        }
        Ok(self)
    }

    pub fn effective_query(&self) -> &str {
        self.query.as_deref().unwrap_or("in:inbox")
    }
}

fn validate_text(name: &str, value: &str, max_length: usize) -> Result<()> {
    anyhow::ensure!(
        value.chars().count() <= max_length,
        "{name} exceeds the maximum length of {max_length} characters"
    );
    anyhow::ensure!(
        !value.chars().any(char::is_control),
        "{name} contains unsupported control characters"
    );
    Ok(())
}
