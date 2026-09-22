fn canonical_scopes(raw: &str) -> Result<Vec<String>> {
    let mut scopes: Vec<String> = raw.split_whitespace().map(str::to_owned).collect();
    scopes.sort();
    scopes.dedup();
    anyhow::ensure!(
        !scopes.is_empty(),
        "Google returned an empty granted-scope set"
    );
    Ok(scopes)
}

fn granted_scopes(raw: Option<&str>) -> Result<Vec<String>> {
    raw.context("Google did not return granted scopes")
        .and_then(canonical_scopes)
}

fn ensure_expected_subject(profile: &GoogleProfile, expected_subject: Option<&str>) -> Result<()> {
    if let Some(expected_subject) = expected_subject {
        anyhow::ensure!(profile.sub == expected_subject, SUBJECT_MISMATCH_MESSAGE);
    }
    Ok(())
}

pub fn token_key(subject: &str) -> String {
    token_reference(subject)
}
