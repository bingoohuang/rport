use anyhow::Result;
use std::fs::File;
use std::path::Path;

#[cfg(unix)]
use daemonize::Daemonize;

/// Daemonize with log file support
#[cfg(unix)]
pub fn daemonize_with_log<P: AsRef<Path>>(log_file: P) -> Result<()> {
    let stdout = File::create(&log_file)?;
    let stderr = File::create(&log_file)?;

    // Get current working directory before daemonizing
    let current_dir = std::env::current_dir().unwrap_or_else(|_| "/".into());

    let daemonize = Daemonize::new()
        .pid_file("/tmp/rport.pid") // Every method except `new` and `start`
        .chown_pid_file(true) // is optional, see `Daemonize` documentation
        .working_directory(current_dir) // Keep current working directory
        .stdout(stdout) // Redirect stdout to log file.
        .stderr(stderr) // Redirect stderr to log file.
        .privileged_action(|| "Executed before drop privileges");

    match daemonize.start() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Error daemonizing: {}", e)),
    }
}

#[cfg(not(unix))]
pub fn daemonize_with_log<P: AsRef<Path>>(_log_file: P) -> Result<()> {
    Err(anyhow::anyhow!(
        "Daemon mode is only supported on Unix systems"
    ))
}
