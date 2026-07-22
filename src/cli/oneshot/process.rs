// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use anyhow::Context;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CommandSpec {
    pub(super) program: OsString,
    pub(super) args: Vec<OsString>,
    pub(super) current_dir: Option<PathBuf>,
}

impl CommandSpec {
    pub(super) fn new(
        program: impl Into<OsString>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            current_dir: None,
        }
    }

    pub(super) fn with_current_dir(mut self, directory: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(directory.into());
        self
    }

    pub(super) fn display(&self) -> String {
        std::iter::once(&self.program)
            .chain(self.args.iter())
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ExecutionMode {
    /// Capture output, keeping command chatter out of the user's terminal.
    Capture,
    /// Show live output and stop the child process on Ctrl-C.
    Interruptible,
}

#[derive(Debug)]
pub(super) struct CommandResult {
    pub(super) success: bool,
    pub(super) code: Option<i32>,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum ExecuteError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("interrupted")]
    Interrupted,
}

#[async_trait::async_trait]
pub(super) trait ProcessExecutor: Send + Sync {
    async fn execute(
        &self,
        command: &CommandSpec,
        mode: ExecutionMode,
    ) -> Result<CommandResult, ExecuteError>;
}

pub(super) struct TokioProcessExecutor;

#[async_trait::async_trait]
impl ProcessExecutor for TokioProcessExecutor {
    async fn execute(
        &self,
        command: &CommandSpec,
        mode: ExecutionMode,
    ) -> Result<CommandResult, ExecuteError> {
        let mut process = tokio::process::Command::new(&command.program);
        process.args(&command.args).stdin(Stdio::null());
        if let Some(directory) = &command.current_dir {
            process.current_dir(directory);
        }

        if matches!(mode, ExecutionMode::Capture) {
            process.kill_on_drop(true);
            let output = tokio::select! {
                output = process.output() => output?,
                signal = tokio::signal::ctrl_c() => {
                    signal?;
                    return Err(ExecuteError::Interrupted);
                }
            };
            return Ok(CommandResult {
                success: output.status.success(),
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        process.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let mut child = process.kill_on_drop(true).spawn()?;
        let status = tokio::select! {
            status = child.wait() => status?,
            signal = tokio::signal::ctrl_c() => {
                signal?;
                let _ = child.kill().await;
                return Err(ExecuteError::Interrupted);
            }
        };

        Ok(CommandResult {
            success: status.success(),
            code: status.code(),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

pub(super) fn path(value: impl AsRef<Path>) -> OsString {
    value.as_ref().as_os_str().to_os_string()
}

pub(super) fn validate_eve_output(output: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::metadata(output).with_context(|| {
        format!(
            "Suricata completed but did not produce a readable eve.json at {}",
            output.display()
        )
    })?;
    if !metadata.is_file() {
        anyhow::bail!(
            "Suricata output is not a regular eve.json file: {}",
            output.display()
        );
    }
    std::fs::File::open(output).with_context(|| {
        format!(
            "Suricata produced an unreadable eve.json at {}",
            output.display()
        )
    })?;
    Ok(())
}
