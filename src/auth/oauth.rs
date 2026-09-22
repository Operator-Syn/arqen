impl GoogleOAuth {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let credentials = read_credentials(path)?;
        Ok(Self {
            credentials,
            client: Client::new(),
            verifier: None,
            state: None,
            redirect_uri: REDIRECT_URI.into(),
        })
    }

    pub fn set_redirect_uri(&mut self, redirect_uri: impl Into<String>) {
        self.redirect_uri = redirect_uri.into();
    }

    pub fn authorization_url(&mut self) -> Result<String> {
        let state = Uuid::new_v4().to_string();
        let verifier = Uuid::new_v4().to_string() + &Uuid::new_v4().to_string();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url = Url::parse(&self.credentials.auth_uri)?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.credentials.client_id)
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("response_type", "code")
            .append_pair("scope", REQUESTED_SCOPES)
            .append_pair("access_type", "offline")
            .append_pair("prompt", "consent")
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        self.verifier = Some(verifier);
        self.state = Some(state);
        Ok(url.to_string())
    }

    pub fn finish(
        &mut self,
        callback: Callback,
        expected_subject: Option<&str>,
    ) -> Result<GoogleLogin> {
        let expected_state = self.state.take().context("no login is in progress")?;
        anyhow::ensure!(callback.state == expected_state, "OAuth state mismatch");
        let verifier = self
            .verifier
            .take()
            .context("no PKCE verifier is available")?;
        let token: TokenResponse = self
            .client
            .post(&self.credentials.token_uri)
            .form(&[
                ("code", callback.code.as_str()),
                ("client_id", self.credentials.client_id.as_str()),
                ("client_secret", self.credentials.client_secret.as_str()),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", verifier.as_str()),
            ])
            .send()
            .context("send authorization-code exchange to Google")?
            .error_for_status()
            .context("Google rejected the authorization-code exchange")?
            .json()
            .context("parse Google's token response")?;
        let granted_scopes = granted_scopes(token.scope.as_deref())?;
        let profile: GoogleProfile = self
            .client
            .get("https://openidconnect.googleapis.com/v1/userinfo")
            .bearer_auth(&token.access_token)
            .send()
            .context("request Google account profile")?
            .error_for_status()
            .context("Google rejected the profile request")?
            .json()
            .context("parse Google's account profile")?;
        ensure_expected_subject(&profile, expected_subject)?;
        let refresh_token = token
            .refresh_token
            .context("Google did not return a refresh token; retry login with consent")?;
        let token_key = token_reference(&profile.sub);
        store_refresh_token(Some(&token_key), &profile.sub, &refresh_token)?;
        Ok(GoogleLogin {
            profile,
            granted_scopes,
        })
    }
}
