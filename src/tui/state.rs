// SPDX-License-Identifier: MPL-2.0
use super::*;

pub(crate) const LOGIN_HELPER_ENABLED: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoginIntent {
    Add,
    Reauthenticate { subject: String },
    Reconnect { subject: String },
}

pub(in crate::tui) struct OwnedBrowser {
    pub(in crate::tui) child: Child,
    pub(in crate::tui) profile_dir: PathBuf,
    #[cfg(unix)]
    pub(in crate::tui) process_group: libc::pid_t,
}

impl Drop for OwnedBrowser {
    fn drop(&mut self) {
        self.terminate();
        let _ = fs::remove_dir_all(&self.profile_dir);
    }
}

impl OwnedBrowser {
    #[cfg(unix)]
    fn terminate(&mut self) {
        let process_group = -self.process_group;
        // The browser may daemonize its visible window away from the direct
        // child, so terminate the isolated process group rather than only the
        // launcher PID. The group was created exclusively for this session.
        unsafe {
            libc::kill(process_group, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        unsafe {
            libc::kill(process_group, libc::SIGKILL);
        }
        let _ = self.child.wait();
    }

    #[cfg(not(unix))]
    fn terminate(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneFocus {
    Accounts,
    Details,
}

pub(crate) enum Screen {
    Accounts,
    Authorization {
        oauth: Option<GoogleOAuth>,
        url: String,
        callback: Option<crate::callback::CallbackServer>,
        remote: bool,
        intent: LoginIntent,
    },
    Redirect {
        oauth: Option<GoogleOAuth>,
        input: String,
        intent: LoginIntent,
    },
    Error(String),
    ConfirmQuit,
    ConfirmDisconnect {
        subject: String,
        email: String,
        retry: bool,
    },
}

pub(in crate::tui) struct App {
    pub(in crate::tui) store: AccountStore,
    pub(in crate::tui) accounts: Vec<Account>,
    pub(in crate::tui) selected: usize,
    pub(in crate::tui) mcp_target_subject: Option<String>,
    pub(in crate::tui) pane_focus: PaneFocus,
    pub(in crate::tui) accounts_scroll: usize,
    pub(in crate::tui) details_scroll: usize,
    pub(in crate::tui) screen: Screen,
    pub(in crate::tui) browser: Option<OwnedBrowser>,
    pub(in crate::tui) notice: Option<String>,
    pub(in crate::tui) clipboard: Option<arboard::Clipboard>,
}
