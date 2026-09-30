// SPDX-License-Identifier: MPL-2.0
use super::*;

#[derive(Debug)]
pub(super) struct FailedLoginState {
    window_started: Instant,
    failures: u8,
}

impl Default for FailedLoginState {
    fn default() -> Self {
        Self {
            window_started: Instant::now(),
            failures: 0,
        }
    }
}

impl FailedLoginState {
    pub(super) fn retry_after(&mut self) -> Option<Duration> {
        let now = Instant::now();
        if now.duration_since(self.window_started) >= FAILED_LOGIN_WINDOW {
            self.window_started = now;
            self.failures = 0;
        }
        if self.failures >= MAX_FAILED_LOGINS {
            Some(FAILED_LOGIN_WINDOW.saturating_sub(now.duration_since(self.window_started)))
        } else {
            None
        }
    }

    pub(super) fn record_failure(&mut self) {
        let _ = self.retry_after();
        self.failures = self.failures.saturating_add(1);
    }

    pub(super) fn reset(&mut self) {
        self.window_started = Instant::now();
        self.failures = 0;
    }
}
