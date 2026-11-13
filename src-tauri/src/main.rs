// Do not open a console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Started from a terminal, a release build hands itself to a new session and
/// returns, so the shell prompt comes back. Launching from a menu (no terminal),
/// `tauri dev`, or `--foreground` / `STARFLEET_FOREGROUND` keeps it attached.
#[cfg(all(unix, not(debug_assertions)))]
fn detach_from_terminal() {
    use std::io::IsTerminal;
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let stay = std::env::args().any(|a| a == "--foreground")
        || std::env::var_os("STARFLEET_FOREGROUND").is_some();
    if stay || !std::io::stdin().is_terminal() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut child = Command::new(exe);
    child
        .args(std::env::args_os().skip(1))
        .env("STARFLEET_FOREGROUND", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: `setsid` is async-signal-safe and touches no Rust state.
    unsafe {
        child.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    // If the copy cannot start, carry on in this process instead.
    if child.spawn().is_ok() {
        std::process::exit(0);
    }
}

fn main() {
    #[cfg(all(unix, not(debug_assertions)))]
    detach_from_terminal();
    starfleet_lib::run()
}
