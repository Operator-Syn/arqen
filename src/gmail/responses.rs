fn parse_json_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::blocking::Response,
) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::Error::new(GmailApiError { status }));
    }
    response.json().context("parse Gmail API response")
}

fn parse_empty_json_response(response: reqwest::blocking::Response) -> Result<()> {
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::Error::new(GmailApiError { status }));
    }
    let body: serde_json::Value = response.json().context("parse Gmail API response")?;
    anyhow::ensure!(body.is_object(), "Gmail returned an invalid delete response");
    Ok(())
}

fn parse_read_email_response(
    response: reqwest::blocking::Response,
) -> Result<MessageResource> {
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::Error::new(GmailApiError { status }));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_READ_EMAIL_GMAIL_RESPONSE_BYTES as u64)
    {
        return Err(anyhow::Error::new(ReadEmailTooLarge));
    }
    let mut body = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(MAX_READ_EMAIL_GMAIL_RESPONSE_BYTES as u64) as usize,
    );
    response
        .take(MAX_READ_EMAIL_GMAIL_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut body)
        .context("read bounded Gmail message response")?;
    if body.len() > MAX_READ_EMAIL_GMAIL_RESPONSE_BYTES {
        return Err(anyhow::Error::new(ReadEmailTooLarge));
    }
    serde_json::from_slice(&body).context("parse Gmail message response")
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

fn email_read_response(message: MessageResource) -> Result<EmailReadResponse> {
    let (headers, body_text, body_status) = match message.payload {
        Some(payload) => {
            let headers = payload.headers.clone();
            let (body_text, body_status) = read_body_text(&payload)?;
            (headers, body_text, body_status)
        }
        None => (
            Vec::new(),
            None,
            EmailBodyStatus::NoReadableBody,
        ),
    };
    let header = |name: &str| {
        headers
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.clone())
    };
    let header_values = |name: &str| {
        headers
            .iter()
            .filter(|header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.clone())
            .collect()
    };
    anyhow::ensure!(!message.id.is_empty(), "Gmail message response has no id");
    anyhow::ensure!(
        !message.thread_id.is_empty(),
        "Gmail message response has no thread id"
    );
    Ok(EmailReadResponse {
        message_id: message.id,
        thread_id: message.thread_id,
        from: header("From"),
        recipients: EmailRecipients {
            to: header_values("To"),
            cc: header_values("Cc"),
            bcc: header_values("Bcc"),
        },
        date: header("Date"),
        subject: header("Subject"),
        labels: message.label_ids,
        body_text,
        body_status,
    })
}

#[derive(Default)]
struct TextBodyCandidates {
    plain: Option<DecodedText>,
    html: Option<DecodedText>,
    plain_malformed: bool,
    html_malformed: bool,
    malformed_tree: bool,
    oversized_plain: bool,
    oversized_html: bool,
    visited_parts: usize,
}

struct DecodedText {
    text: String,
    had_errors: bool,
}

fn read_body_text(payload: &MessagePayload) -> Result<(Option<String>, EmailBodyStatus)> {
    const MAX_MIME_DEPTH: usize = 32;
    const MAX_MIME_PARTS: usize = 1_024;
    let mut candidates = TextBodyCandidates::default();
    collect_text_parts(payload, 0, MAX_MIME_DEPTH, MAX_MIME_PARTS, &mut candidates);

    if candidates.oversized_plain || (candidates.plain.is_none() && candidates.oversized_html) {
        return Err(anyhow::Error::new(ReadEmailTooLarge));
    }
    if let Some(plain) = candidates.plain {
        return Ok((
            Some(plain.text),
            if plain.had_errors || candidates.plain_malformed || candidates.malformed_tree {
                EmailBodyStatus::Incomplete
            } else {
                EmailBodyStatus::Complete
            },
        ));
    }
    if let Some(html) = candidates.html {
        if let Ok(converted) = html2text::from_read(html.text.as_bytes(), 100) {
            if converted.len() > MAX_READ_EMAIL_BODY_BYTES {
                return Err(anyhow::Error::new(ReadEmailTooLarge));
            }
            if !converted.trim().is_empty() {
                let incomplete = html.had_errors
                    || candidates.plain_malformed
                    || candidates.html_malformed
                    || candidates.malformed_tree;
                return Ok((
                    Some(converted),
                    if incomplete {
                        EmailBodyStatus::Incomplete
                    } else {
                        EmailBodyStatus::Complete
                    },
                ));
            }
        }
        candidates.html_malformed = true;
    }
    if candidates.plain_malformed || candidates.html_malformed || candidates.malformed_tree {
        Ok((None, EmailBodyStatus::Incomplete))
    } else {
        Ok((None, EmailBodyStatus::NoReadableBody))
    }
}

fn collect_text_parts(
    part: &MessagePayload,
    depth: usize,
    max_depth: usize,
    max_parts: usize,
    candidates: &mut TextBodyCandidates,
) {
    candidates.visited_parts += 1;
    if depth > max_depth || candidates.visited_parts > max_parts {
        candidates.malformed_tree = true;
        return;
    }
    if is_attachment(part) {
        return;
    }
    let mime_type = part.mime_type.to_ascii_lowercase();
    if mime_type == "text/plain" || mime_type == "text/html" {
        let is_plain = mime_type == "text/plain";
        match part.body.as_ref().and_then(|body| body.data.as_deref()) {
            Some(data) if !data.is_empty() => match decode_mime_body(data, part) {
                Ok(decoded) if decoded.text.len() > MAX_READ_EMAIL_BODY_BYTES => {
                    if is_plain {
                        candidates.oversized_plain = true;
                    } else {
                        candidates.oversized_html = true;
                    }
                }
                Ok(decoded) if !decoded.text.trim().is_empty() => {
                    let destination = if is_plain {
                        &mut candidates.plain
                    } else {
                        &mut candidates.html
                    };
                    if destination.is_none() {
                        *destination = Some(decoded);
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    if is_plain {
                        candidates.plain_malformed = true;
                    } else {
                        candidates.html_malformed = true;
                    }
                }
            },
            _ if part.body.as_ref().is_some_and(|body| body.size > 0) => {
                if is_plain {
                    candidates.plain_malformed = true;
                } else {
                    candidates.html_malformed = true;
                }
            }
            _ => {}
        }
    }
    for child in &part.parts {
        collect_text_parts(child, depth + 1, max_depth, max_parts, candidates);
    }
}

fn is_attachment(part: &MessagePayload) -> bool {
    !part.filename.is_empty()
        || part.headers.iter().any(|header| {
            header.name.eq_ignore_ascii_case("Content-Disposition")
                && header
                    .value
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("attachment")
        })
}

fn decode_mime_body(data: &str, part: &MessagePayload) -> Result<DecodedText> {
    use base64::engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD};

    let bytes = URL_SAFE_NO_PAD
        .decode(data)
        .or_else(|_| URL_SAFE.decode(data))
        .context("decode Gmail base64url message part")?;
    let charset = part.headers.iter().find_map(|header| {
        if !header.name.eq_ignore_ascii_case("Content-Type") {
            return None;
        }
        header.value.split(';').skip(1).find_map(|parameter| {
            let (name, value) = parameter.trim().split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case("charset")
                .then(|| value.trim().trim_matches(['\"', '\'']).to_owned())
        })
    });
    let encoding = charset
        .as_deref()
        .and_then(|label| Encoding::for_label(label.as_bytes()));
    let unknown_charset = charset.is_some() && encoding.is_none();
    let (text, _, had_decode_errors) = encoding.unwrap_or(UTF_8).decode(&bytes);
    Ok(DecodedText {
        text: text.into_owned(),
        had_errors: unknown_charset || had_decode_errors,
    })
}

fn truncate_snippet(snippet: &str) -> (String, bool) {
    let mut chars = snippet.chars();
    let truncated: String = chars.by_ref().take(MAX_SNIPPET_CHARS).collect();
    (truncated, chars.next().is_some())
}
