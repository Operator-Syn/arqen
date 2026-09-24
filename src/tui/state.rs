// SPDX-License-Identifier: MPL-2.0
pub(crate) const LOGIN_HELPER_ENABLED: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoginIntent {
    Add,
    Reauthenticate { subject: String },
    Reconnect { subject: String },
}

struct OwnedBrowser {
    child: Child,
    profile_dir: PathBuf,
    #[cfg(unix)]
    process_group: libc::pid_t,
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

struct App {
    store: AccountStore,
    accounts: Vec<Account>,
    selected: usize,
    mcp_target_subject: Option<String>,
    pane_focus: PaneFocus,
    accounts_scroll: usize,
    details_scroll: usize,
    screen: Screen,
    browser: Option<OwnedBrowser>,
    notice: Option<String>,
    clipboard: Option<arboard::Clipboard>,
}
