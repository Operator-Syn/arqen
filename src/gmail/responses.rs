fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::blocking::Response,
) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        let message = response
            .json::<GoogleErrorResponse>()
            .ok()
            .and_then(|error| error.error)
            .and_then(|error| error.message)
            .unwrap_or_else(|| "the upstream request failed".into());
        return Err(anyhow::Error::new(GmailApiError { status, message }));
    }
    response.json().context("parse Gmail API response")
}

#[derive(Debug, Deserialize)]
struct GoogleErrorResponse {
    error: Option<GoogleErrorBody>,
}

#[derive(Debug, Deserialize)]
struct GoogleErrorBody {
    message: Option<String>,
}

fn email_summary(detail: MessageResource, reference: &MessageReference) -> EmailSummary {
    let headers = detail
        .payload
        .map(|payload| payload.headers)
        .unwrap_or_default();
    let header = |name: &str| {
        headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.clone())
    };
    let (snippet, snippet_truncated) = truncate_snippet(&detail.snippet);
    EmailSummary {
        id: if detail.id.is_empty() {
            reference.id.clone()
        } else {
            detail.id
        },
        thread_id: if detail.thread_id.is_empty() {
            reference.thread_id.clone()
        } else {
            detail.thread_id
        },
        from: header("From"),
        subject: header("Subject"),
        date: header("Date"),
        labels: detail.label_ids,
        snippet,
        snippet_truncated,
    }
}

fn truncate_snippet(snippet: &str) -> (String, bool) {
    let mut chars = snippet.chars();
    let truncated: String = chars.by_ref().take(MAX_SNIPPET_CHARS).collect();
    (truncated, chars.next().is_some())
}
