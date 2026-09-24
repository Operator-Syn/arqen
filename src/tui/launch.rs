// SPDX-License-Identifier: MPL-2.0
fn open_in_browser(url: &str) -> Result<()> {
    let candidates = browser_candidates(url, env::var_os("BROWSER"));
    let mut failures = Vec::new();
    for (program, args) in candidates {
        let result = std::process::Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match result {
            Ok(_) => return Ok(()),
            Err(error) => failures.push(format!("{}: {error}", program.to_string_lossy())),
        }
    }
    if !failures.is_empty() {
        anyhow::bail!(
            "no browser launcher was available ({})",
            failures.join("; ")
        )
    } else {
        anyhow::bail!("no browser launcher was configured")
    }
}

fn launch_browser(url: &str) -> Result<Option<OwnedBrowser>> {
    // A unique profile is what makes terminating the child safe: Chromium and
    // Firefox cannot route this login into the user's normal browser process.
    let profile_dir = match create_browser_profile() {
        Ok(path) => path,
        Err(profile_error) => {
            open_in_browser(url).with_context(|| {
                format!(
                    "create an isolated browser profile ({profile_error:#}); browser fallback also failed"
                )
            })?;
            return Ok(None);
        }
    };
    let mut failures = Vec::new();
    for (program, _) in browser_candidates(url, env::var_os("BROWSER")) {
        let Some(args) = owned_browser_arguments(&program, url, &profile_dir) else {
            continue;
        };
        let mut command = std::process::Command::new(&program);
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let result = command.spawn();
        match result {
            Ok(child) => {
                #[cfg(unix)]
                let process_group = child.id() as libc::pid_t;
                return Ok(Some(OwnedBrowser {
                    child,
                    profile_dir,
                    #[cfg(unix)]
                    process_group,
                }));
            }
            Err(error) => failures.push(format!("{}: {error}", program.to_string_lossy())),
        }
    }

    let _ = fs::remove_dir_all(&profile_dir);
    open_in_browser(url).with_context(|| {
        if failures.is_empty() {
            "no isolated browser launcher was available; browser fallback also failed".to_owned()
        } else {
            format!(
                "isolated browser launchers were unavailable ({}); browser fallback also failed",
                failures.join("; ")
            )
        }
    })?;
    Ok(None)
}
