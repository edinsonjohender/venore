//! Child-process helpers
//!
//! On Windows, a GUI app (compiled with `windows_subsystem = "windows"`) that
//! spawns a console program — `git`, `docker`, an LSP server — makes Windows
//! create a console window for that child, which flashes on screen for a
//! fraction of a second. Passing the `CREATE_NO_WINDOW` creation flag tells
//! Windows not to allocate that console, so the spawn stays invisible.
//!
//! Use these constructors instead of `Command::new(...)` everywhere a child
//! process is launched. On non-Windows platforms they are plain `Command::new`.

/// `CREATE_NO_WINDOW` — suppresses the console window for a child process.
/// See <https://learn.microsoft.com/windows/win32/procthread/process-creation-flags>.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `std::process::Command` that won't flash a console window on Windows.
pub fn quiet_command(program: &str) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Build a `tokio::process::Command` that won't flash a console window on Windows.
///
/// `tokio::process::Command::creation_flags` is an inherent method on Windows,
/// so no extension-trait import is needed here.
pub fn quiet_tokio_command(program: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}
