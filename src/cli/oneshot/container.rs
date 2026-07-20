// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

use clap::ValueEnum;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use uuid::Uuid;

pub(super) const DEFAULT_SURICATA_IMAGE: &str = "docker.io/jasonish/suricata:8.0";
const MINIMUM_SURICATA_VERSION: &str = "8.0.6";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(super) enum ContainerRuntimeChoice {
    #[default]
    Auto,
    Podman,
    Docker,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContainerRuntime {
    Podman,
    Docker,
}

impl ContainerRuntime {
    pub(super) fn program(self) -> &'static str {
        match self {
            Self::Podman => "podman",
            Self::Docker => "docker",
        }
    }

    /// The runtime name for display in messages.
    fn name(self) -> &'static str {
        match self {
            Self::Podman => "Podman",
            Self::Docker => "Docker",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CommandSpec {
    program: OsString,
    args: Vec<OsString>,
}

impl CommandSpec {
    fn new(
        program: impl Into<OsString>,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
        }
    }

    fn runtime(
        runtime: ContainerRuntime,
        args: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Self {
        Self::new(runtime.program(), args)
    }

    fn display(&self) -> String {
        std::iter::once(&self.program)
            .chain(self.args.iter())
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Copy, Debug)]
enum ExecutionMode {
    /// Capture output, keeping podman/docker chatter such as created and
    /// removed resource names out of the user's terminal.
    Capture,
    /// Show live output, for the containers doing real, visible work.
    Interruptible,
}

#[derive(Debug)]
struct CommandResult {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Debug, thiserror::Error)]
enum ExecuteError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("interrupted")]
    Interrupted,
}

#[async_trait::async_trait]
trait ProcessExecutor: Send + Sync {
    async fn execute(
        &self,
        command: &CommandSpec,
        mode: ExecutionMode,
    ) -> Result<CommandResult, ExecuteError>;
}

struct TokioProcessExecutor;

#[async_trait::async_trait]
impl ProcessExecutor for TokioProcessExecutor {
    async fn execute(
        &self,
        command: &CommandSpec,
        mode: ExecutionMode,
    ) -> Result<CommandResult, ExecuteError> {
        let mut process = tokio::process::Command::new(&command.program);
        process.args(&command.args).stdin(Stdio::null());

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

/// Resolve the container runtime and make sure the Suricata image is
/// present and current, before any real processing starts.
pub(super) async fn prepare_runtime(
    choice: ContainerRuntimeChoice,
    image: &str,
) -> Result<ContainerRuntime> {
    let executor = TokioProcessExecutor;
    let runtime = resolve_runtime(choice, &executor).await?;
    ensure_image_is_current(&executor, runtime, image, &TerminalImageUpdatePrompt).await?;
    Ok(runtime)
}

pub(super) struct EveGenerator {
    runtime: ContainerRuntime,
    image: String,
    data_dir: PathBuf,
    tty: bool,
}

impl EveGenerator {
    pub(super) async fn prepare(
        runtime: ContainerRuntime,
        image: &str,
        data_dir: &Path,
    ) -> Result<Self> {
        let executor = TokioProcessExecutor;
        // Give the containers a TTY when we have one so tools like
        // suricata-update keep their colored output.
        let tty = std::io::stdout().is_terminal();
        Self::prepare_with(&executor, runtime, image, data_dir, tty).await
    }

    async fn prepare_with(
        executor: &dyn ProcessExecutor,
        runtime: ContainerRuntime,
        image: &str,
        data_dir: &Path,
        tty: bool,
    ) -> Result<Self> {
        let generator = Self {
            runtime,
            image: image.to_string(),
            data_dir: data_dir.to_path_buf(),
            tty,
        };
        let mut job = ContainerJob::new(
            executor,
            runtime,
            &generator.image,
            &generator.data_dir,
            JobNames::random(),
            tty,
        );
        let result = job.update_rules().await;
        job.cleanup().await;
        result?;
        Ok(generator)
    }

    pub(super) async fn generate(&self, pcap: &Path, output: &Path) -> Result<()> {
        self.generate_with(&TokioProcessExecutor, pcap, output)
            .await
    }

    async fn generate_with(
        &self,
        executor: &dyn ProcessExecutor,
        pcap: &Path,
        output: &Path,
    ) -> Result<()> {
        let mut job = ContainerJob::new(
            executor,
            self.runtime,
            &self.image,
            &self.data_dir,
            JobNames::random(),
            self.tty,
        );
        let result = job.replay(pcap, output).await;
        job.cleanup().await;
        result?;
        validate_eve_output(output)
    }
}

trait ImageUpdatePrompt: Send + Sync {
    fn should_pull(
        &self,
        runtime: ContainerRuntime,
        image: &str,
        version: &semver::Version,
    ) -> Result<bool>;
}

struct TerminalImageUpdatePrompt;

impl ImageUpdatePrompt for TerminalImageUpdatePrompt {
    fn should_pull(
        &self,
        runtime: ContainerRuntime,
        image: &str,
        version: &semver::Version,
    ) -> Result<bool> {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            warn!(
                "Suricata image {image} provides version {version}, older than the recommended \
                 {MINIMUM_SURICATA_VERSION}; non-interactive input prevents an update prompt. \
                 Run `{} pull {image}` to update it",
                runtime.program()
            );
            return Ok(false);
        }

        let question = format!(
            "Suricata image {image} provides version {version}, older than the recommended \
             {MINIMUM_SURICATA_VERSION}. Pull an updated image now?"
        );
        match inquire::Confirm::new(&question).with_default(true).prompt() {
            Ok(answer) => Ok(answer),
            Err(inquire::InquireError::OperationInterrupted) => {
                anyhow::bail!("interrupted at the image update prompt")
            }
            Err(_) => Ok(false),
        }
    }
}

async fn ensure_image_is_current(
    executor: &dyn ProcessExecutor,
    runtime: ContainerRuntime,
    image: &str,
    prompt: &dyn ImageUpdatePrompt,
) -> Result<semver::Version> {
    let mut just_pulled = false;
    if !image_exists(executor, runtime, image).await? {
        info!("Downloading Suricata image {image}, this may take a few minutes");
        execute_checked(
            executor,
            "downloading the Suricata image",
            CommandSpec::runtime(runtime, ["pull", image]),
            ExecutionMode::Interruptible,
        )
        .await?;
        just_pulled = true;
    }

    let minimum = semver::Version::parse(MINIMUM_SURICATA_VERSION).unwrap();
    let mut version = inspect_suricata_version(executor, runtime, image).await?;
    if version >= minimum {
        return Ok(version);
    }

    // A freshly pulled image can't get any newer by pulling again, so
    // don't offer to.
    if just_pulled {
        warn!(
            "Freshly pulled image {image} only provides Suricata {version}; version \
             {MINIMUM_SURICATA_VERSION} or newer is recommended"
        );
        return Ok(version);
    }

    if !prompt.should_pull(runtime, image, &version)? {
        warn!(
            "Continuing with Suricata {version}; version {MINIMUM_SURICATA_VERSION} or newer is \
             recommended"
        );
        return Ok(version);
    }

    info!("Pulling updated Suricata image {image}");
    execute_checked(
        executor,
        "pulling the updated Suricata image",
        CommandSpec::runtime(runtime, ["pull", image]),
        ExecutionMode::Interruptible,
    )
    .await?;
    version = inspect_suricata_version(executor, runtime, image).await?;
    if version < minimum {
        warn!(
            "Updated image {image} still provides Suricata {version}; version \
             {MINIMUM_SURICATA_VERSION} or newer is recommended"
        );
    } else {
        info!("Using updated Suricata {version}");
    }
    Ok(version)
}

async fn image_exists(
    executor: &dyn ProcessExecutor,
    runtime: ContainerRuntime,
    image: &str,
) -> Result<bool> {
    let command = CommandSpec::runtime(runtime, ["image", "inspect", "--format", "{{.Id}}", image]);
    let result = executor
        .execute(&command, ExecutionMode::Capture)
        .await
        .with_context(|| format!("failed to check for a local copy of image {image}"))?;
    Ok(result.success)
}

async fn inspect_suricata_version(
    executor: &dyn ProcessExecutor,
    runtime: ContainerRuntime,
    image: &str,
) -> Result<semver::Version> {
    let command = CommandSpec::runtime(
        runtime,
        [
            "run",
            "--rm",
            "--pull=missing",
            "--entrypoint",
            "/usr/bin/suricata",
            image,
            "-V",
        ],
    );
    let result = executor
        .execute(&command, ExecutionMode::Capture)
        .await
        .with_context(|| format!("failed to inspect Suricata image {image}"))?;
    if !result.success {
        let detail = result.stderr.trim();
        if detail.is_empty() {
            anyhow::bail!("failed to inspect Suricata image {image}");
        }
        anyhow::bail!("failed to inspect Suricata image {image}: {detail}");
    }
    parse_suricata_version(&result.stdout).with_context(|| {
        format!("failed to determine the Suricata version provided by image {image}")
    })
}

fn parse_suricata_version(output: &str) -> Result<semver::Version> {
    output
        .split_whitespace()
        .find_map(|word| semver::Version::parse(word).ok())
        .context("Suricata version output did not contain a semantic version")
}

async fn resolve_runtime(
    choice: ContainerRuntimeChoice,
    executor: &dyn ProcessExecutor,
) -> Result<ContainerRuntime> {
    let environment = RuntimeEnvironment::current();
    resolve_runtime_with_environment(choice, executor, &environment).await
}

#[derive(Default)]
struct RuntimeEnvironment {
    docker_host: Option<OsString>,
}

impl RuntimeEnvironment {
    fn current() -> Self {
        Self {
            docker_host: std::env::var_os("DOCKER_HOST"),
        }
    }
}

async fn resolve_runtime_with_environment(
    choice: ContainerRuntimeChoice,
    executor: &dyn ProcessExecutor,
    environment: &RuntimeEnvironment,
) -> Result<ContainerRuntime> {
    match choice {
        ContainerRuntimeChoice::Podman => {
            probe_runtime(ContainerRuntime::Podman, executor, environment)
                .await
                .map_err(|err| anyhow::anyhow!("selected Podman runtime is not usable: {err}"))?;
            Ok(ContainerRuntime::Podman)
        }
        ContainerRuntimeChoice::Docker => {
            probe_runtime(ContainerRuntime::Docker, executor, environment)
                .await
                .map_err(|err| anyhow::anyhow!("selected Docker runtime is not usable: {err}"))?;
            Ok(ContainerRuntime::Docker)
        }
        ContainerRuntimeChoice::Auto => {
            let podman_error =
                match probe_runtime(ContainerRuntime::Podman, executor, environment).await {
                    Ok(()) => return Ok(ContainerRuntime::Podman),
                    Err(err) => err,
                };
            debug!("Podman is not usable for oneshot PCAP processing: {podman_error:#}");

            let docker_error =
                match probe_runtime(ContainerRuntime::Docker, executor, environment).await {
                    Ok(()) => return Ok(ContainerRuntime::Docker),
                    Err(err) => err,
                };
            debug!("Docker is not usable for oneshot PCAP processing: {docker_error:#}");

            anyhow::bail!("no usable local container runtime found (tried Podman, then Docker)");
        }
    }
}

async fn probe_runtime(
    runtime: ContainerRuntime,
    executor: &dyn ProcessExecutor,
    environment: &RuntimeEnvironment,
) -> Result<()> {
    match runtime {
        ContainerRuntime::Podman => {
            let output = run_probe(
                executor,
                CommandSpec::runtime(runtime, ["info", "--format", "{{.Host.ServiceIsRemote}}"]),
            )
            .await?;
            if output.trim() != "false" {
                anyhow::bail!("Podman is not connected to a local engine");
            }
        }
        ContainerRuntime::Docker => {
            // An empty DOCKER_HOST is treated as unset, like the Docker
            // CLI does.
            let docker_host = environment.docker_host.as_ref().filter(|v| !v.is_empty());
            let endpoint = if let Some(host) = docker_host {
                host.to_string_lossy().into_owned()
            } else {
                run_probe(
                    executor,
                    CommandSpec::runtime(
                        runtime,
                        [
                            "context",
                            "inspect",
                            "--format",
                            "{{.Endpoints.docker.Host}}",
                        ],
                    ),
                )
                .await?
            };
            if !endpoint.trim().starts_with("unix://") {
                anyhow::bail!("Docker is not connected to a local Unix socket");
            }
            run_probe(executor, CommandSpec::runtime(runtime, ["info"])).await?;
        }
    }
    Ok(())
}

async fn run_probe(executor: &dyn ProcessExecutor, command: CommandSpec) -> Result<String> {
    let result = executor
        .execute(&command, ExecutionMode::Capture)
        .await
        .with_context(|| format!("failed to run `{}`", command.display()))?;
    if !result.success {
        let detail = result.stderr.trim();
        if detail.is_empty() {
            anyhow::bail!("`{}` exited unsuccessfully", command.display());
        }
        anyhow::bail!("`{}` failed: {}", command.display(), detail);
    }
    Ok(result.stdout)
}

async fn execute_checked(
    executor: &dyn ProcessExecutor,
    phase: &str,
    command: CommandSpec,
    mode: ExecutionMode,
) -> Result<()> {
    debug!("Running container command: {}", command.display());
    let result = executor.execute(&command, mode).await;
    match result {
        Err(ExecuteError::Interrupted) => anyhow::bail!("{phase} was interrupted"),
        Err(err) => anyhow::bail!("failed while {phase}: {err}"),
        Ok(result) if !result.success => {
            let detail = result.stderr.trim();
            if !detail.is_empty() {
                anyhow::bail!("{phase} failed: {detail}");
            }
            if let Some(code) = result.code {
                anyhow::bail!("{phase} failed: container runtime exited with status {code}");
            }
            anyhow::bail!("{phase} failed: container runtime exited unsuccessfully");
        }
        Ok(_) => Ok(()),
    }
}

#[derive(Clone, Debug)]
struct JobNames {
    update_container: String,
    replay_container: String,
}

impl JobNames {
    fn random() -> Self {
        Self::with_suffix(&Uuid::new_v4().simple().to_string())
    }

    fn with_suffix(suffix: &str) -> Self {
        Self {
            update_container: format!("evebox-oneshot-update-{suffix}"),
            replay_container: format!("evebox-oneshot-replay-{suffix}"),
        }
    }
}

/// Build a bind mount option, refusing paths a mount option can't express.
fn bind_mount(src: &Path, dst: &str, read_only: bool) -> Result<OsString> {
    if src.to_string_lossy().contains(',') {
        anyhow::bail!(
            "path {} contains a comma, which container mount options cannot express",
            src.display()
        );
    }
    let mut spec = OsString::from("type=bind,src=");
    spec.push(src.as_os_str());
    spec.push(format!(",dst={dst}"));
    if read_only {
        spec.push(",ro");
    }
    Ok(spec)
}

struct ContainerJob<'a> {
    executor: &'a dyn ProcessExecutor,
    runtime: ContainerRuntime,
    image: &'a str,
    data_dir: &'a Path,
    names: JobNames,
    tty: bool,
    update_active: bool,
    replay_active: bool,
}

impl<'a> ContainerJob<'a> {
    fn new(
        executor: &'a dyn ProcessExecutor,
        runtime: ContainerRuntime,
        image: &'a str,
        data_dir: &'a Path,
        names: JobNames,
        tty: bool,
    ) -> Self {
        Self {
            executor,
            runtime,
            image,
            data_dir,
            names,
            tty,
            update_active: false,
            replay_active: false,
        }
    }

    /// Common `run` arguments for the containers whose output is shown to
    /// the user, allocating a TTY when we are attached to one so container
    /// tools keep their colored output.
    fn run_args(&self, name: &str) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("run"),
            OsString::from("--name"),
            OsString::from(name),
            OsString::from("--pull=missing"),
        ];
        if self.tty {
            args.push(OsString::from("--tty"));
        }
        args
    }

    #[cfg(test)]
    async fn run(&mut self, pcap: &Path, output: &Path) -> Result<()> {
        self.update_rules().await?;
        self.replay(pcap, output).await
    }

    async fn replay(&mut self, pcap: &Path, output: &Path) -> Result<()> {
        info!(
            "Processing {} with Suricata in {}",
            pcap.display(),
            self.runtime.name()
        );
        self.replay_active = true;
        let pcap_mount = bind_mount(pcap, "/input/capture.pcap", true)?;
        let rules_mount = bind_mount(
            &self.data_dir.join("rules"),
            "/var/lib/suricata/rules",
            true,
        )?;
        let mut replay_args = self.run_args(&self.names.replay_container);
        replay_args.extend([
            OsString::from("--user=suricata"),
            OsString::from("--network=none"),
            OsString::from("--cap-drop=ALL"),
            OsString::from("--security-opt=no-new-privileges"),
            OsString::from("--security-opt=label=disable"),
            OsString::from("--mount"),
            pcap_mount,
            OsString::from("--mount"),
            rules_mount,
            OsString::from(self.image),
            OsString::from("/usr/bin/suricata"),
            OsString::from("--runmode"),
            OsString::from("single"),
            OsString::from("-r"),
            OsString::from("/input/capture.pcap"),
            OsString::from("-S"),
            OsString::from("/var/lib/suricata/rules/suricata.rules"),
            OsString::from("-k"),
            OsString::from("none"),
            OsString::from("-l"),
            OsString::from("/var/log/suricata"),
            OsString::from("--init-errors-fatal"),
        ]);
        self.run_checked(
            "running Suricata against the PCAP",
            CommandSpec::runtime(self.runtime, replay_args),
        )
        .await?;

        info!("Copying generated EVE output");
        let source = format!("{}:/var/log/suricata/eve.json", self.names.replay_container);
        self.run_checked(
            "copying generated eve.json",
            CommandSpec::runtime(
                self.runtime,
                [
                    OsString::from("cp"),
                    OsString::from(source),
                    output.as_os_str().to_owned(),
                ],
            ),
        )
        .await?;

        Ok(())
    }

    /// Run suricata-update with a persistent data directory so its own
    /// caching avoids re-downloading recently fetched rules.
    ///
    /// suricata-update runs as container root and creates directories the
    /// unprivileged replay user can't read, so read permissions are opened
    /// up after the update. The shell is invoked directly as the image
    /// entrypoint word-splits a `sh -c` command.
    async fn update_rules(&mut self) -> Result<()> {
        const UPDATE_COMMAND: &str = concat!(
            "/usr/bin/suricata-update update-sources && ",
            "/usr/bin/suricata-update enable-source et/open && ",
            "/usr/bin/suricata-update enable-source pawpatrules && ",
            "/usr/bin/suricata-update enable-source the-hunters-ledger/open && ",
            "/usr/bin/suricata-update --no-test --no-reload --fail && ",
            "chmod -R a+rX /var/lib/suricata",
        );
        info!(
            "Updating ET/Open, PawPatRules, and The Hunters Ledger rules with {}",
            self.runtime.name()
        );
        self.update_active = true;
        let update_mount = bind_mount(self.data_dir, "/var/lib/suricata", false)?;
        let mut update_args = self.run_args(&self.names.update_container);
        update_args.extend([
            OsString::from("--security-opt=no-new-privileges"),
            OsString::from("--security-opt=label=disable"),
            OsString::from("--entrypoint"),
            OsString::from("/bin/sh"),
            OsString::from("--mount"),
            update_mount,
            OsString::from(self.image),
            OsString::from("-c"),
            OsString::from(UPDATE_COMMAND),
        ]);
        self.run_checked(
            "downloading Suricata rules",
            CommandSpec::runtime(self.runtime, update_args),
        )
        .await?;
        self.remove_update_container().await;
        Ok(())
    }

    async fn run_checked(&self, phase: &str, command: CommandSpec) -> Result<()> {
        execute_checked(self.executor, phase, command, ExecutionMode::Interruptible).await
    }

    async fn remove_update_container(&mut self) {
        if self.update_active
            && self
                .cleanup_command(
                    "rule-update container",
                    CommandSpec::runtime(
                        self.runtime,
                        [
                            "rm",
                            "--force",
                            "--volumes",
                            self.names.update_container.as_str(),
                        ],
                    ),
                )
                .await
        {
            self.update_active = false;
        }
    }

    async fn cleanup(&mut self) {
        if self.replay_active
            && self
                .cleanup_command(
                    "replay container",
                    CommandSpec::runtime(
                        self.runtime,
                        [
                            "rm",
                            "--force",
                            "--volumes",
                            self.names.replay_container.as_str(),
                        ],
                    ),
                )
                .await
        {
            self.replay_active = false;
        }
        self.remove_update_container().await;
    }

    async fn cleanup_command(&self, resource: &str, command: CommandSpec) -> bool {
        debug!("Running cleanup command: {}", command.display());
        match self
            .executor
            .execute(&command, ExecutionMode::Capture)
            .await
        {
            Ok(result) if result.success => true,
            Ok(result) => {
                let detail = result.stderr.trim();
                if detail.is_empty() {
                    warn!("Failed to remove oneshot PCAP {resource}");
                } else {
                    warn!("Failed to remove oneshot PCAP {resource}: {detail}");
                }
                false
            }
            Err(err) => {
                warn!("Failed to remove oneshot PCAP {resource}: {err}");
                false
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
async fn generate_eve_with(
    executor: &dyn ProcessExecutor,
    runtime: ContainerRuntime,
    image: &str,
    pcap: &Path,
    output: &Path,
    data_dir: &Path,
    names: JobNames,
    tty: bool,
) -> Result<()> {
    let mut job = ContainerJob::new(executor, runtime, image, data_dir, names, tty);
    let result = job.run(pcap, output).await;
    job.cleanup().await;
    result?;
    validate_eve_output(output)
}

fn validate_eve_output(output: &Path) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::Mutex;

    enum FakeResponse {
        Result(CommandResult),
        Error(ExecuteError),
    }

    struct FakeExecutor {
        responses: Mutex<VecDeque<FakeResponse>>,
        calls: Mutex<Vec<CommandSpec>>,
        write_copy_output: bool,
    }

    impl FakeExecutor {
        fn new(responses: Vec<FakeResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
                calls: Mutex::new(Vec::new()),
                write_copy_output: false,
            }
        }

        fn with_copy_output(mut self) -> Self {
            self.write_copy_output = true;
            self
        }

        fn calls(&self) -> Vec<CommandSpec> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl ProcessExecutor for FakeExecutor {
        async fn execute(
            &self,
            command: &CommandSpec,
            _mode: ExecutionMode,
        ) -> Result<CommandResult, ExecuteError> {
            self.calls.lock().unwrap().push(command.clone());
            let response = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .expect("fake response missing");
            match response {
                FakeResponse::Result(result) => {
                    if result.success
                        && self.write_copy_output
                        && command.args.first() == Some(&OsString::from("cp"))
                    {
                        let output = PathBuf::from(command.args.last().unwrap());
                        std::fs::write(output, b"\n").unwrap();
                    }
                    Ok(result)
                }
                FakeResponse::Error(err) => Err(err),
            }
        }
    }

    fn ok(stdout: &str) -> FakeResponse {
        FakeResponse::Result(CommandResult {
            success: true,
            code: Some(0),
            stdout: stdout.to_string(),
            stderr: String::new(),
        })
    }

    fn failed() -> FakeResponse {
        FakeResponse::Result(CommandResult {
            success: false,
            code: Some(125),
            stdout: String::new(),
            stderr: "failed".to_string(),
        })
    }

    // One response per command of a full run with no cached rules:
    // update run, update rm, replay run, cp, replay rm.
    fn happy_responses() -> Vec<FakeResponse> {
        vec![ok(""), ok(""), ok(""), ok(""), ok("")]
    }

    fn arg_strings(command: &CommandSpec) -> Vec<String> {
        command
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    struct FakePrompt {
        answer: bool,
        calls: Mutex<usize>,
    }

    impl FakePrompt {
        fn new(answer: bool) -> Self {
            Self {
                answer,
                calls: Mutex::new(0),
            }
        }

        fn call_count(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl ImageUpdatePrompt for FakePrompt {
        fn should_pull(
            &self,
            _runtime: ContainerRuntime,
            _image: &str,
            _version: &semver::Version,
        ) -> Result<bool> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.answer)
        }
    }

    #[test]
    fn parses_suricata_version_output() {
        assert_eq!(
            parse_suricata_version("This is Suricata version 8.0.6 RELEASE")
                .unwrap()
                .to_string(),
            "8.0.6"
        );
        assert!(parse_suricata_version("unexpected output").is_err());
    }

    #[tokio::test]
    async fn current_image_does_not_prompt_or_pull() {
        let executor = FakeExecutor::new(vec![
            ok("sha256:test\n"),
            ok("This is Suricata version 8.0.6 RELEASE\n"),
        ]);
        let prompt = FakePrompt::new(true);
        let version = ensure_image_is_current(
            &executor,
            ContainerRuntime::Podman,
            DEFAULT_SURICATA_IMAGE,
            &prompt,
        )
        .await
        .unwrap();
        assert_eq!(version, semver::Version::new(8, 0, 6));
        assert_eq!(prompt.call_count(), 0);
        let calls = executor.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            arg_strings(&calls[0]),
            [
                "image",
                "inspect",
                "--format",
                "{{.Id}}",
                DEFAULT_SURICATA_IMAGE,
            ]
        );
        assert_eq!(
            arg_strings(&calls[1]),
            [
                "run",
                "--rm",
                "--pull=missing",
                "--entrypoint",
                "/usr/bin/suricata",
                DEFAULT_SURICATA_IMAGE,
                "-V",
            ]
        );
    }

    #[tokio::test]
    async fn missing_image_is_pulled_before_inspection() {
        let executor = FakeExecutor::new(vec![
            failed(),
            ok(""),
            ok("This is Suricata version 8.0.6 RELEASE\n"),
        ]);
        let prompt = FakePrompt::new(false);
        let version = ensure_image_is_current(
            &executor,
            ContainerRuntime::Podman,
            DEFAULT_SURICATA_IMAGE,
            &prompt,
        )
        .await
        .unwrap();
        assert_eq!(version, semver::Version::new(8, 0, 6));
        assert_eq!(prompt.call_count(), 0);
        let calls = executor.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(arg_strings(&calls[1]), ["pull", DEFAULT_SURICATA_IMAGE]);
    }

    #[tokio::test]
    async fn freshly_pulled_old_image_does_not_prompt() {
        let executor = FakeExecutor::new(vec![
            failed(),
            ok(""),
            ok("This is Suricata version 8.0.2 RELEASE\n"),
        ]);
        let prompt = FakePrompt::new(true);
        let version = ensure_image_is_current(
            &executor,
            ContainerRuntime::Podman,
            DEFAULT_SURICATA_IMAGE,
            &prompt,
        )
        .await
        .unwrap();
        assert_eq!(version, semver::Version::new(8, 0, 2));
        assert_eq!(prompt.call_count(), 0);
        assert_eq!(executor.calls().len(), 3);
    }

    #[tokio::test]
    async fn outdated_image_is_pulled_when_confirmed() {
        let executor = FakeExecutor::new(vec![
            ok("sha256:test\n"),
            ok("This is Suricata version 8.0.2 RELEASE\n"),
            ok(""),
            ok("This is Suricata version 8.0.6 RELEASE\n"),
        ]);
        let prompt = FakePrompt::new(true);
        let version = ensure_image_is_current(
            &executor,
            ContainerRuntime::Docker,
            DEFAULT_SURICATA_IMAGE,
            &prompt,
        )
        .await
        .unwrap();
        assert_eq!(version, semver::Version::new(8, 0, 6));
        assert_eq!(prompt.call_count(), 1);
        let calls = executor.calls();
        assert_eq!(arg_strings(&calls[2]), ["pull", DEFAULT_SURICATA_IMAGE]);
        assert_eq!(calls.len(), 4);
    }

    #[tokio::test]
    async fn outdated_image_continues_when_update_is_declined() {
        let executor = FakeExecutor::new(vec![
            ok("sha256:test\n"),
            ok("This is Suricata version 8.0.2 RELEASE\n"),
        ]);
        let prompt = FakePrompt::new(false);
        let version = ensure_image_is_current(
            &executor,
            ContainerRuntime::Podman,
            DEFAULT_SURICATA_IMAGE,
            &prompt,
        )
        .await
        .unwrap();
        assert_eq!(version, semver::Version::new(8, 0, 2));
        assert_eq!(prompt.call_count(), 1);
        assert_eq!(executor.calls().len(), 2);
    }

    #[tokio::test]
    async fn confirmed_pull_failure_is_reported() {
        let executor = FakeExecutor::new(vec![
            ok("sha256:test\n"),
            ok("This is Suricata version 8.0.2 RELEASE\n"),
            failed(),
        ]);
        let prompt = FakePrompt::new(true);
        let err = ensure_image_is_current(
            &executor,
            ContainerRuntime::Docker,
            DEFAULT_SURICATA_IMAGE,
            &prompt,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("pulling the updated Suricata image")
        );
    }

    #[tokio::test]
    async fn auto_prefers_usable_podman() {
        let executor = FakeExecutor::new(vec![ok("false\n")]);
        let runtime = resolve_runtime_with_environment(
            ContainerRuntimeChoice::Auto,
            &executor,
            &RuntimeEnvironment::default(),
        )
        .await
        .unwrap();
        assert_eq!(runtime, ContainerRuntime::Podman);
        assert_eq!(executor.calls()[0].program, "podman");
    }

    #[tokio::test]
    async fn auto_falls_back_to_usable_docker() {
        let executor =
            FakeExecutor::new(vec![failed(), ok("unix:///var/run/docker.sock\n"), ok("")]);
        let runtime = resolve_runtime_with_environment(
            ContainerRuntimeChoice::Auto,
            &executor,
            &RuntimeEnvironment::default(),
        )
        .await
        .unwrap();
        assert_eq!(runtime, ContainerRuntime::Docker);
        let programs: Vec<_> = executor
            .calls()
            .iter()
            .map(|call| call.program.to_string_lossy().into_owned())
            .collect();
        assert_eq!(programs, ["podman", "docker", "docker"]);
    }

    #[tokio::test]
    async fn explicit_runtime_does_not_fall_back() {
        let executor = FakeExecutor::new(vec![failed()]);
        let err = resolve_runtime_with_environment(
            ContainerRuntimeChoice::Docker,
            &executor,
            &RuntimeEnvironment::default(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("selected Docker"));
        assert_eq!(executor.calls().len(), 1);
        assert_eq!(executor.calls()[0].program, "docker");
    }

    #[tokio::test]
    async fn missing_runtimes_report_clear_error() {
        let executor = FakeExecutor::new(vec![failed(), failed()]);
        let err = resolve_runtime_with_environment(
            ContainerRuntimeChoice::Auto,
            &executor,
            &RuntimeEnvironment::default(),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("no usable local container runtime")
        );
    }

    #[tokio::test]
    async fn rejects_remote_container_engines() {
        let podman = FakeExecutor::new(vec![ok("true\n")]);
        let err = resolve_runtime_with_environment(
            ContainerRuntimeChoice::Podman,
            &podman,
            &RuntimeEnvironment::default(),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("selected Podman"));

        let docker = FakeExecutor::new(vec![]);
        let environment = RuntimeEnvironment {
            docker_host: Some(OsString::from("tcp://remote.example:2376")),
        };
        let err =
            resolve_runtime_with_environment(ContainerRuntimeChoice::Docker, &docker, &environment)
                .await
                .unwrap_err();
        assert!(err.to_string().contains("selected Docker"));
        assert!(docker.calls().is_empty());
    }

    #[tokio::test]
    async fn empty_docker_host_is_treated_as_unset() {
        let executor = FakeExecutor::new(vec![ok("unix:///var/run/docker.sock\n"), ok("")]);
        let environment = RuntimeEnvironment {
            docker_host: Some(OsString::new()),
        };
        let runtime = resolve_runtime_with_environment(
            ContainerRuntimeChoice::Docker,
            &executor,
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(runtime, ContainerRuntime::Docker);
        // The endpoint must come from `docker context inspect`, not the
        // empty environment variable.
        assert_eq!(
            arg_strings(&executor.calls()[0])[..2],
            ["context", "inspect"]
        );
    }

    fn data_dir(tempdir: &tempfile::TempDir) -> PathBuf {
        let dir = tempdir.path().join("suricata-data");
        std::fs::create_dir(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn generator_updates_rules_once_for_multiple_pcaps() {
        let tempdir = tempfile::tempdir().unwrap();
        let data = data_dir(&tempdir);
        let first_pcap = tempdir.path().join("first.pcap");
        let second_pcap = tempdir.path().join("second.pcap");
        std::fs::write(&first_pcap, b"pcap").unwrap();
        std::fs::write(&second_pcap, b"pcap").unwrap();
        let executor = FakeExecutor::new(vec![
            ok(""),
            ok(""),
            ok(""),
            ok(""),
            ok(""),
            ok(""),
            ok(""),
            ok(""),
        ])
        .with_copy_output();

        let generator = EveGenerator::prepare_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &data,
            false,
        )
        .await
        .unwrap();
        generator
            .generate_with(&executor, &first_pcap, &tempdir.path().join("first.json"))
            .await
            .unwrap();
        generator
            .generate_with(&executor, &second_pcap, &tempdir.path().join("second.json"))
            .await
            .unwrap();

        let calls = executor.calls();
        assert_eq!(calls.len(), 8);
        assert!(
            arg_strings(&calls[0])
                .last()
                .unwrap()
                .starts_with("/usr/bin/suricata-update")
        );
        assert_eq!(arg_strings(&calls[1])[0], "rm");
        assert_eq!(arg_strings(&calls[2])[0], "run");
        assert_eq!(arg_strings(&calls[5])[0], "run");
    }

    #[tokio::test]
    async fn builds_secure_replay_command_and_cleans_up_in_order() {
        let tempdir = tempfile::tempdir().unwrap();
        let pcap = tempdir.path().join("capture with spaces.pcap");
        let output_directory = tempdir.path().join("output with spaces");
        std::fs::create_dir(&output_directory).unwrap();
        let output = output_directory.join("eve.json");
        std::fs::write(&pcap, b"pcap").unwrap();
        let data = data_dir(&tempdir);
        let executor = FakeExecutor::new(happy_responses()).with_copy_output();

        generate_eve_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &pcap,
            &output,
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap();

        let calls = executor.calls();

        let update = arg_strings(&calls[0]);
        assert_eq!(update[0], "run");
        assert!(update.contains(&"--security-opt=no-new-privileges".to_string()));
        assert!(update.contains(&"--security-opt=label=disable".to_string()));
        assert!(update.contains(&"--entrypoint".to_string()));
        assert!(update.contains(&format!(
            "type=bind,src={},dst=/var/lib/suricata",
            data.display()
        )));
        let update_command = update.last().unwrap();
        assert!(update_command.starts_with("/usr/bin/suricata-update"));
        for command in [
            "/usr/bin/suricata-update update-sources",
            "/usr/bin/suricata-update enable-source et/open",
            "/usr/bin/suricata-update enable-source pawpatrules",
            "/usr/bin/suricata-update enable-source the-hunters-ledger/open",
        ] {
            assert!(update_command.contains(command));
        }
        assert!(!update_command.contains("--etopen"));
        for option in ["--no-test", "--no-reload", "--fail"] {
            assert!(update_command.contains(option));
        }
        assert!(update_command.contains("chmod -R a+rX /var/lib/suricata"));
        assert_eq!(arg_strings(&calls[1])[0], "rm");

        let replay = arg_strings(&calls[2]);
        assert_eq!(replay[0], "run");
        assert!(!replay.contains(&"--tty".to_string()));
        assert!(replay.contains(&"--pull=missing".to_string()));
        assert!(replay.contains(&"--user=suricata".to_string()));
        assert!(replay.contains(&"--network=none".to_string()));
        assert!(replay.contains(&"--cap-drop=ALL".to_string()));
        assert!(replay.contains(&"--security-opt=no-new-privileges".to_string()));
        assert!(replay.contains(&"--security-opt=label=disable".to_string()));
        assert!(replay.contains(&format!(
            "type=bind,src={},dst=/input/capture.pcap,ro",
            pcap.display()
        )));
        assert!(replay.contains(&format!(
            "type=bind,src={},dst=/var/lib/suricata/rules,ro",
            data.join("rules").display()
        )));
        assert!(replay.ends_with(&[
            "-S".to_string(),
            "/var/lib/suricata/rules/suricata.rules".to_string(),
            "-k".to_string(),
            "none".to_string(),
            "-l".to_string(),
            "/var/log/suricata".to_string(),
            "--init-errors-fatal".to_string(),
        ]));

        assert_eq!(
            arg_strings(&calls[3]),
            [
                "cp",
                "evebox-oneshot-replay-test:/var/log/suricata/eve.json",
                output.to_str().unwrap(),
            ]
        );
        assert_eq!(arg_strings(&calls[4])[0], "rm");
        assert_eq!(calls.len(), 5);
    }

    #[tokio::test]
    async fn tty_runs_allocate_a_container_tty() {
        let tempdir = tempfile::tempdir().unwrap();
        let output = tempdir.path().join("eve.json");
        let data = data_dir(&tempdir);
        let executor = FakeExecutor::new(happy_responses()).with_copy_output();
        generate_eve_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &output,
            &data,
            JobNames::with_suffix("test"),
            true,
        )
        .await
        .unwrap();
        let calls = executor.calls();
        let update = arg_strings(&calls[0]);
        let replay = arg_strings(&calls[2]);
        assert!(update.contains(&"--tty".to_string()));
        assert!(replay.contains(&"--tty".to_string()));
        // The copy command must not get the TTY option.
        assert_eq!(arg_strings(&calls[3])[0], "cp");
    }

    #[tokio::test]
    async fn failed_rule_download_cleans_up_without_replay() {
        let tempdir = tempfile::tempdir().unwrap();
        let data = data_dir(&tempdir);
        let executor = FakeExecutor::new(vec![failed(), ok("")]);
        let err = generate_eve_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &tempdir.path().join("eve.json"),
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("downloading Suricata rules failed")
        );
        let calls = executor.calls();
        assert_eq!(arg_strings(&calls[1])[0], "rm");
        assert_eq!(calls.len(), 2);
    }

    #[tokio::test]
    async fn interruption_cleans_up_active_resources() {
        let tempdir = tempfile::tempdir().unwrap();
        let data = data_dir(&tempdir);
        let executor =
            FakeExecutor::new(vec![FakeResponse::Error(ExecuteError::Interrupted), ok("")]);
        let err = generate_eve_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &tempdir.path().join("eve.json"),
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("interrupted"));
        assert_eq!(executor.calls().len(), 2);
    }

    #[tokio::test]
    async fn failed_replay_is_reported_and_cleaned_up() {
        let tempdir = tempfile::tempdir().unwrap();
        let data = data_dir(&tempdir);
        let executor = FakeExecutor::new(vec![ok(""), ok(""), failed(), ok("")]);
        let err = generate_eve_with(
            &executor,
            ContainerRuntime::Docker,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &tempdir.path().join("eve.json"),
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("running Suricata"));
        assert_eq!(executor.calls().len(), 4);
    }

    #[tokio::test]
    async fn failed_copy_is_reported_and_cleaned_up() {
        let tempdir = tempfile::tempdir().unwrap();
        let data = data_dir(&tempdir);
        let executor = FakeExecutor::new(vec![ok(""), ok(""), ok(""), failed(), ok("")]);
        let err = generate_eve_with(
            &executor,
            ContainerRuntime::Docker,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &tempdir.path().join("eve.json"),
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("copying generated eve.json"));
        assert_eq!(executor.calls().len(), 5);
    }

    #[tokio::test]
    async fn missing_eve_after_successful_copy_is_rejected() {
        let tempdir = tempfile::tempdir().unwrap();
        let data = data_dir(&tempdir);
        let executor = FakeExecutor::new(happy_responses());
        let err = generate_eve_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &tempdir.path().join("eve.json"),
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("did not produce a readable eve.json")
        );
    }

    #[tokio::test]
    async fn cleanup_failures_do_not_hide_successful_output() {
        let tempdir = tempfile::tempdir().unwrap();
        let output = tempdir.path().join("eve.json");
        let data = data_dir(&tempdir);
        let executor =
            FakeExecutor::new(vec![ok(""), ok(""), ok(""), ok(""), failed()]).with_copy_output();
        generate_eve_with(
            &executor,
            ContainerRuntime::Podman,
            "suricata:test",
            &tempdir.path().join("capture.pcap"),
            &output,
            &data,
            JobNames::with_suffix("test"),
            false,
        )
        .await
        .unwrap();
        assert!(output.is_file());
    }

    fn append_pcap_packet(pcap: &mut Vec<u8>, timestamp: u32, packet: &[u8]) {
        pcap.extend_from_slice(&timestamp.to_le_bytes());
        pcap.extend_from_slice(&0_u32.to_le_bytes());
        pcap.extend_from_slice(&(packet.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&(packet.len() as u32).to_le_bytes());
        pcap.extend_from_slice(packet);
    }

    fn ethernet_ipv4_tcp(
        client_to_server: bool,
        sequence: u32,
        acknowledgement: u32,
        flags: u8,
        payload: &[u8],
    ) -> Vec<u8> {
        let client_mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
        let server_mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x02];
        let client_ip = [198, 51, 100, 10];
        let server_ip = [192, 168, 1, 10];
        let (source_mac, destination_mac, source_ip, destination_ip, source_port, destination_port) =
            if client_to_server {
                (
                    client_mac, server_mac, client_ip, server_ip, 49152_u16, 80_u16,
                )
            } else {
                (
                    server_mac, client_mac, server_ip, client_ip, 80_u16, 49152_u16,
                )
            };

        let mut packet = Vec::with_capacity(14 + 20 + 20 + payload.len());
        packet.extend_from_slice(&destination_mac);
        packet.extend_from_slice(&source_mac);
        packet.extend_from_slice(&0x0800_u16.to_be_bytes());
        packet.extend_from_slice(&[0x45, 0]);
        packet.extend_from_slice(&(40_u16 + payload.len() as u16).to_be_bytes());
        packet.extend_from_slice(&1_u16.to_be_bytes());
        packet.extend_from_slice(&0x4000_u16.to_be_bytes());
        packet.extend_from_slice(&[64, 6]);
        packet.extend_from_slice(&0_u16.to_be_bytes());
        packet.extend_from_slice(&source_ip);
        packet.extend_from_slice(&destination_ip);
        packet.extend_from_slice(&source_port.to_be_bytes());
        packet.extend_from_slice(&destination_port.to_be_bytes());
        packet.extend_from_slice(&sequence.to_be_bytes());
        packet.extend_from_slice(&acknowledgement.to_be_bytes());
        packet.extend_from_slice(&[0x50, flags]);
        packet.extend_from_slice(&64240_u16.to_be_bytes());
        packet.extend_from_slice(&0_u16.to_be_bytes());
        packet.extend_from_slice(&0_u16.to_be_bytes());
        packet.extend_from_slice(payload);
        packet
    }

    fn write_et_open_alert_pcap(path: &Path) {
        let mut pcap = Vec::new();
        pcap.extend_from_slice(&0xa1b2c3d4_u32.to_le_bytes());
        pcap.extend_from_slice(&2_u16.to_le_bytes());
        pcap.extend_from_slice(&4_u16.to_le_bytes());
        pcap.extend_from_slice(&0_i32.to_le_bytes());
        pcap.extend_from_slice(&0_u32.to_le_bytes());
        pcap.extend_from_slice(&65535_u32.to_le_bytes());
        pcap.extend_from_slice(&1_u32.to_le_bytes());

        append_pcap_packet(&mut pcap, 1, &ethernet_ipv4_tcp(true, 100, 0, 0x02, b""));
        append_pcap_packet(&mut pcap, 2, &ethernet_ipv4_tcp(false, 200, 101, 0x12, b""));
        append_pcap_packet(&mut pcap, 3, &ethernet_ipv4_tcp(true, 101, 201, 0x10, b""));
        let request = b"GET / HTTP/1.1\r\nHost: example.test\r\nUser-Agent: Nmap NSE\r\nConnection: close\r\n\r\n";
        append_pcap_packet(
            &mut pcap,
            4,
            &ethernet_ipv4_tcp(true, 101, 201, 0x18, request),
        );
        append_pcap_packet(
            &mut pcap,
            5,
            &ethernet_ipv4_tcp(false, 201, 101 + request.len() as u32, 0x10, b""),
        );
        std::fs::write(path, pcap).unwrap();
    }

    #[tokio::test]
    #[ignore = "requires network access plus a local Podman or Docker engine"]
    async fn live_et_open_pcap_generates_an_importable_alert() {
        if std::env::var_os("EVEBOX_ONESHOT_PCAP_INTEGRATION").is_none() {
            eprintln!("set EVEBOX_ONESHOT_PCAP_INTEGRATION=1 to run this test");
            return;
        }

        let executor = TokioProcessExecutor;
        let mut tested = 0;
        for choice in [
            ContainerRuntimeChoice::Podman,
            ContainerRuntimeChoice::Docker,
        ] {
            let Ok(runtime) = resolve_runtime(choice, &executor).await else {
                continue;
            };
            let tempdir = tempfile::tempdir().unwrap();
            let pcap = tempdir.path().join("et-open-alert.pcap");
            let eve = tempdir.path().join("eve.json");
            let data = data_dir(&tempdir);
            write_et_open_alert_pcap(&pcap);
            ensure_image_is_current(
                &executor,
                runtime,
                DEFAULT_SURICATA_IMAGE,
                &FakePrompt::new(true),
            )
            .await
            .unwrap();
            generate_eve_with(
                &executor,
                runtime,
                DEFAULT_SURICATA_IMAGE,
                &pcap,
                &eve,
                &data,
                JobNames::random(),
                std::io::stdout().is_terminal(),
            )
            .await
            .unwrap();

            let contents = std::fs::read_to_string(&eve).unwrap();
            let has_expected_alert = contents.lines().any(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|event| event["alert"]["signature_id"].as_u64())
                    == Some(2_009_359)
            });
            assert!(
                has_expected_alert,
                "{} did not generate ET SCAN Nmap NSE alert 2009359",
                runtime.program()
            );

            let database = tempdir.path().join("events.sqlite");
            let mut connection = crate::sqlite::connection::open_connection(Some(&database), true)
                .await
                .unwrap();
            crate::sqlite::connection::init_event_db(&mut connection)
                .await
                .unwrap();
            super::super::run_import(
                Arc::new(tokio::sync::Mutex::new(connection)),
                0,
                std::slice::from_ref(&eve),
                Arc::new(crate::server::metrics::Metrics::default()),
            )
            .await
            .unwrap();
            tested += 1;
        }
        assert!(
            tested > 0,
            "no usable local container runtime was available"
        );
    }
}
