pub mod errors;

use std::{
    io::Write,
    process::{Command, ExitStatus, Stdio},
};

use errors::{Result, UdevError};

use crate::utils::constants;

pub struct UdevManager {}

impl UdevManager {
    pub fn is_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    /// Writes the rules file and reloads udev. Must run as root.
    pub fn write_rules_file(auto_run: bool) -> Result<()> {
        let mut content =
            include_str!("../../../resources/99-strength-tracker-rs.rules").to_string();
        if auto_run {
            content.push('\n');
            content.push_str(include_str!(
                "../../../resources/99-strength-tracker-rs-launch.rules"
            ));
        }
        std::fs::write(constants::RULE_FILE, content).map_err(UdevError::Write)?;
        if std::fs::exists(constants::LEGACY_RULE_FILE).unwrap_or(false) {
            std::fs::remove_file(constants::LEGACY_RULE_FILE).map_err(UdevError::Write)?;
        }

        UdevManager::reload()?;
        UdevManager::trigger()
    }

    /// Relaunches this executable through sudo to write the rules file, feeding `password` to
    /// sudo's stdin. Fails with `UdevError::WrongPassword` if sudo rejects it.
    pub fn write_rules_file_with_sudo(auto_run: bool, password: &str) -> Result<()> {
        let current_exe = std::env::current_exe().map_err(UdevError::Write)?;

        let mut child = Command::new("sudo")
            // Ignore cached credentials, read the password from stdin and print no prompt
            .args(["-k", "-S", "-p", ""])
            .arg(&current_exe)
            .arg("--rules")
            .arg(auto_run.to_string())
            // sudo's messages are matched below, so they must not be localized
            .env("LC_ALL", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::inherit())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(UdevError::Reload)?;

        // Dropping stdin sends EOF, so a rejected password is not retried
        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{password}").map_err(UdevError::Write)?;
        }

        let output = child.wait_with_output().map_err(UdevError::Reload)?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() && stderr.contains("incorrect password") {
            return Err(UdevError::WrongPassword);
        }
        eprint!("{stderr}");

        Self::handle_command_status(&output.status)
    }

    pub fn reload() -> Result<()> {
        eprintln!("Reloading udev rules...");
        let status = Command::new("udevadm")
            .arg("control")
            .arg("--reload-rules")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(UdevError::Reload)?;

        Self::handle_command_status(&status)
    }
    pub fn trigger() -> Result<()> {
        eprintln!("Triggering udev rules...");
        let status = Command::new("udevadm")
            .arg("trigger")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(UdevError::Trigger)?;

        Self::handle_command_status(&status)
    }

    fn handle_command_status(status: &ExitStatus) -> Result<()> {
        if status.success() {
            Ok(())
        } else {
            let code = status.code();
            Err(UdevError::Reload(std::io::Error::other(format!(
                "Bad exit status: {:?}",
                code
            ))))
        }
    }
}
