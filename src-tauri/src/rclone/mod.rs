pub mod errors;
pub mod providers;

use std::{
    ffi::OsStr, marker::PhantomData, os::unix::fs::PermissionsExt, path::Path, process::Output,
};

use tokio::{fs, process::Command};

use crate::{rclone::errors::RCloneError, utils::constants};

/// Whether an executable `rclone` is in the `PATH`. rclone is an optional dependency, only needed
/// to upload to the cloud. Checked on every call, so installing it doesn't need a restart.
pub fn is_installed() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| {
            std::fs::metadata(dir.join("rclone"))
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
    })
}

pub struct RCloneClient<T> {
    _marker: PhantomData<T>,
}

impl<T> Default for RCloneClient<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

trait RCloneTrait {
    async fn configure(&self) -> errors::Result<()>;

    fn get_provider_suffix() -> String;

    fn get_provider_key() -> String {
        format!(
            "{}-{}",
            *constants::RCLONE_CONFIG_NAME,
            Self::get_provider_suffix()
        )
    }

    async fn is_configured(&self) -> errors::Result<bool> {
        if let Ok(exist) = fs::try_exists(constants::RCLONE_CONFIG_FILE.as_path()).await
            && exist
        {
            let content = fs::read_to_string(constants::RCLONE_CONFIG_FILE.as_path())
                .await
                .map_err(RCloneError::ReadConfig)?;

            let has_remote = |key: &str| {
                let marker = format!("[{key}]");
                content.lines().any(|l| l.trim() == marker)
            };
            if has_remote(&Self::get_provider_key()) {
                return Ok(true);
            }

            // Remotes configured before a rename are renamed instead of authorizing again
            for legacy in constants::LEGACY_APP_NAMES {
                let legacy_key = format!("{legacy}-{}", Self::get_provider_suffix());
                if has_remote(&legacy_key) {
                    Self::run_rclone(
                        &[
                            OsStr::new("config"),
                            OsStr::new("rename"),
                            OsStr::new(&legacy_key),
                            OsStr::new(&Self::get_provider_key()),
                        ],
                        RCloneError::CommandError,
                    )
                    .await?;
                    return Ok(true);
                }
            }
            Ok(false)
        } else {
            Ok(false)
        }
    }

    async fn upload<P>(&self, path: P) -> errors::Result<()>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let remote = format!("{}:{}", Self::get_provider_key(), *constants::APP_NAME);

        Self::run_rclone(
            &[OsStr::new("copy"), path.as_os_str(), OsStr::new(&remote)],
            RCloneError::CommandError,
        )
        .await?;

        Ok(())
    }

    async fn run_rclone(
        args: &[&OsStr],
        io_err: fn(std::io::Error) -> RCloneError,
    ) -> errors::Result<Output> {
        let output = Command::new("rclone")
            .args(args)
            .output()
            .await
            .map_err(io_err)?;

        if output.status.success() {
            Ok(output)
        } else {
            Err(RCloneError::BadExitStatus(
                output.status,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}
