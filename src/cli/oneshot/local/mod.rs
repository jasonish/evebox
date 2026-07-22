// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

use crate::prelude::*;

mod rules;

use super::process::{
    CommandResult, CommandSpec, ExecuteError, ExecutionMode, ProcessExecutor, TokioProcessExecutor,
    path, validate_eve_output,
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const MINIMUM_SURICATA_VERSION: &str = "8.0.0";
const RULE_SOURCES: [&str; 3] = ["et/open", "pawpatrules", "the-hunters-ledger/open"];
const REFERENCE_CONFIG: &str = r#"# EveBox oneshot reference configuration.
config reference: cve https://cve.mitre.org/cgi-bin/cvename.cgi?name=
config reference: nessus https://www.tenable.com/plugins/nessus/
config reference: url https://
config reference: mcafee https://vil.nai.com/vil/content/v_
config reference: bid https://www.securityfocus.com/bid/
config reference: bugtraq https://www.securityfocus.com/bid/
config reference: md5 https://www.threatexpert.com/report.aspx?md5=
config reference: secunia https://www.secunia.com/advisories/
config reference: arachnids https://www.whitehats.com/info/IDS
config reference: exploitdb https://www.exploit-db.com/exploits/
config reference: msft https://technet.microsoft.com/security/bulletin/
config reference: et https://doc.emergingthreats.net/
config reference: etpro https://doc.emergingthreatspro.com/
config reference: telus https://
config reference: xforce http://xforce.iss.net/xforce/xfdb/
config reference: osvdb http://osvdb.org/show/osvdb/
config reference: threatexpert http://www.threatexpert.com/report.aspx?md5=
config reference: openpacket https://www.openpacket.org/capture/grab/
config reference: securitytracker http://securitytracker.com/id?
"#;

#[derive(Clone, Debug)]
pub(super) struct Programs {
    suricata: PathBuf,
    suricata_update: Option<PathBuf>,
    version: semver::Version,
}

impl Programs {
    pub(super) async fn discover(
        suricata: Option<&Path>,
        suricata_update: Option<&Path>,
    ) -> Result<Self> {
        Self::discover_with(
            suricata,
            suricata_update,
            &SearchEnvironment::current(),
            &TokioProcessExecutor,
        )
        .await
    }

    async fn discover_with(
        suricata: Option<&Path>,
        suricata_update: Option<&Path>,
        environment: &SearchEnvironment,
        executor: &dyn ProcessExecutor,
    ) -> Result<Self> {
        let suricata = resolve_executable(suricata, "suricata", environment, "--suricata")?;
        let suricata_update = match suricata_update {
            Some(requested) => Some(resolve_executable(
                Some(requested),
                "suricata-update",
                environment,
                "--suricata-update",
            )?),
            None => {
                resolve_executable(None, "suricata-update", environment, "--suricata-update").ok()
            }
        };

        let version = inspect_suricata_version(executor, &suricata).await?;
        let minimum = semver::Version::parse(MINIMUM_SURICATA_VERSION)?;
        if version < minimum {
            anyhow::bail!(
                "local Suricata {} at {} is older than the supported minimum {}; use \
                 --suricata to select a compatible executable",
                version,
                suricata.display(),
                minimum
            );
        }

        info!("Using local Suricata {} at {}", version, suricata.display());
        if let Some(updater) = &suricata_update {
            debug!("Using suricata-update at {}", updater.display());
        } else {
            info!("suricata-update was not found; rules will be downloaded directly");
        }
        Ok(Self {
            suricata,
            suricata_update,
            version,
        })
    }
}

struct SearchEnvironment {
    platform: &'static str,
    path: Option<OsString>,
    program_files: Option<PathBuf>,
}

impl SearchEnvironment {
    fn current() -> Self {
        Self {
            platform: std::env::consts::OS,
            path: std::env::var_os("PATH"),
            program_files: std::env::var_os("ProgramFiles").map(PathBuf::from),
        }
    }
}

fn resolve_executable(
    requested: Option<&Path>,
    program: &str,
    environment: &SearchEnvironment,
    override_flag: &str,
) -> Result<PathBuf> {
    if let Some(requested) = requested
        && (requested.is_absolute() || requested.components().count() > 1)
    {
        return checked_executable(requested).with_context(|| {
            format!(
                "the executable selected by {override_flag} is not usable: {}",
                requested.display()
            )
        });
    }

    let requested_name = requested
        .and_then(Path::file_name)
        .unwrap_or_else(|| OsStr::new(program));
    let names = executable_names(requested_name, environment.platform);
    if let Some(search_path) = &environment.path {
        for directory in std::env::split_paths(search_path) {
            for name in &names {
                let candidate = directory.join(name);
                if let Ok(path) = checked_executable(&candidate) {
                    return Ok(path);
                }
            }
        }
    }

    if environment.platform == "windows"
        && let Some(program_files) = &environment.program_files
    {
        for name in &names {
            let candidate = program_files.join("Suricata").join(name);
            if let Ok(path) = checked_executable(&candidate) {
                return Ok(path);
            }
        }
    }

    anyhow::bail!(
        "could not find {program} in the supported search locations; use {override_flag} PATH \
         to select it"
    )
}

fn executable_names(name: &OsStr, platform: &str) -> Vec<OsString> {
    if platform != "windows" || Path::new(name).extension().is_some() {
        return vec![name.to_os_string()];
    }
    let mut with_exe = name.to_os_string();
    with_exe.push(".exe");
    vec![with_exe, name.to_os_string()]
}

fn checked_executable(path: &Path) -> Result<PathBuf> {
    if !std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file()) {
        anyhow::bail!("{} is not a regular file", path.display());
    }
    path.canonicalize()
        .with_context(|| format!("failed to resolve executable {}", path.display()))
}

async fn inspect_suricata_version(
    executor: &dyn ProcessExecutor,
    suricata: &Path,
) -> Result<semver::Version> {
    let command = CommandSpec::new(path(suricata), [OsString::from("-V")]);
    let result = executor
        .execute(&command, ExecutionMode::Capture)
        .await
        .with_context(|| format!("failed to run local Suricata at {}", suricata.display()))?;
    if !result.success {
        return failed_command("checking the local Suricata version", &command, result);
    }
    parse_suricata_version(&format!("{} {}", result.stdout, result.stderr)).with_context(|| {
        format!(
            "failed to determine the version of local Suricata at {}",
            suricata.display()
        )
    })
}

fn parse_suricata_version(output: &str) -> Result<semver::Version> {
    output
        .split_whitespace()
        .find_map(|word| semver::Version::parse(word).ok())
        .context("Suricata version output did not contain a semantic version")
}

#[derive(Clone, Debug)]
struct WorkspaceLayout {
    suricata_config: PathBuf,
    reference_config: PathBuf,
    threshold_config: PathBuf,
    classification_config: PathBuf,
    updater_config: PathBuf,
    updater_data: PathBuf,
    updater_cache: PathBuf,
    disable_config: PathBuf,
    enable_config: PathBuf,
    drop_config: PathBuf,
    modify_config: PathBuf,
    rules: PathBuf,
    rule_file: PathBuf,
    default_log: PathBuf,
}

impl WorkspaceLayout {
    fn create(root: &Path, persistent_updater_cache: Option<PathBuf>) -> Result<Self> {
        let config = root.join("config");
        let updater = root.join("updater");
        let rules = root.join("rules");
        let updater_cache = persistent_updater_cache.unwrap_or_else(|| updater.join("cache"));
        let layout = Self {
            suricata_config: config.join("suricata.yaml"),
            reference_config: rules.join("reference.config"),
            threshold_config: config.join("threshold.config"),
            classification_config: rules.join("classification.config"),
            updater_config: updater.join("update.yaml"),
            updater_data: updater.join("data"),
            updater_cache,
            disable_config: updater.join("disable.conf"),
            enable_config: updater.join("enable.conf"),
            drop_config: updater.join("drop.conf"),
            modify_config: updater.join("modify.conf"),
            rule_file: rules.join("suricata.rules"),
            rules,
            default_log: root.join("log"),
        };

        for directory in [
            &config,
            &updater,
            &layout.rules,
            &layout.updater_data,
            &layout.updater_cache,
            &layout.default_log,
        ] {
            std::fs::create_dir_all(directory).with_context(|| {
                format!(
                    "failed to create local Suricata directory {}",
                    directory.display()
                )
            })?;
        }
        layout.write_files()?;
        Ok(layout)
    }

    fn write_files(&self) -> Result<()> {
        let suricata_config = serde_json::json!({
            "vars": {
                "address-groups": {
                    "HOME_NET": "[192.168.0.0/16,10.0.0.0/8,172.16.0.0/12]",
                    "EXTERNAL_NET": "!$HOME_NET",
                    "HTTP_SERVERS": "$HOME_NET",
                    "SMTP_SERVERS": "$HOME_NET",
                    "SQL_SERVERS": "$HOME_NET",
                    "DNS_SERVERS": "$HOME_NET",
                    "TELNET_SERVERS": "$HOME_NET",
                    "AIM_SERVERS": "$EXTERNAL_NET",
                    "DC_SERVERS": "$HOME_NET",
                    "DNP3_SERVER": "$HOME_NET",
                    "DNP3_CLIENT": "$HOME_NET",
                    "MODBUS_CLIENT": "$HOME_NET",
                    "MODBUS_SERVER": "$HOME_NET",
                    "ENIP_CLIENT": "$HOME_NET",
                    "ENIP_SERVER": "$HOME_NET"
                },
                "port-groups": {
                    "HTTP_PORTS": "80",
                    "SHELLCODE_PORTS": "!80",
                    "ORACLE_PORTS": 1521,
                    "SSH_PORTS": 22,
                    "DNP3_PORTS": 20000,
                    "MODBUS_PORTS": 502,
                    "FILE_DATA_PORTS": "[$HTTP_PORTS,110,143]",
                    "FTP_PORTS": 21,
                    "GENEVE_PORTS": 6081,
                    "VXLAN_PORTS": 4789,
                    "TEREDO_PORTS": 3544,
                    "SIP_PORTS": "[5060, 5061]"
                }
            },
            "default-log-dir": utf8_path(&self.default_log)?,
            "default-rule-path": utf8_path(&self.rules)?,
            "classification-file": utf8_path(&self.classification_config)?,
            "reference-config-file": utf8_path(&self.reference_config)?,
            "threshold-file": utf8_path(&self.threshold_config)?,
            "rule-files": [],
            // The DNP3 and Modbus parsers are disabled by default and must
            // be enabled for their loggers to produce events, while the
            // BitTorrent DHT and PostgreSQL loggers are only registered
            // when their app-layer nodes are present. Everything else is
            // left to the built-in defaults.
            "app-layer": {
                "protocols": {
                    "bittorrent-dht": {"enabled": true},
                    "dnp3": {"enabled": true},
                    "modbus": {"enabled": true},
                    "pgsql": {"enabled": true}
                }
            },
            "outputs": [{
                "eve-log": {
                    "enabled": true,
                    "filetype": "regular",
                    "filename": "eve.json",
                    // All protocol loggers, including those disabled in the
                    // default Suricata configuration, with extended logging
                    // where the logger supports it.
                    "types": [
                        "alert",
                        "anomaly",
                        {"http": {"extended": true}},
                        "dns",
                        "mdns",
                        {"tls": {"extended": true}},
                        {"files": {"force-hash": ["md5", "sha256"]}},
                        {"smtp": {"extended": true}},
                        "websocket",
                        "ftp",
                        "rdp",
                        "nfs",
                        "smb",
                        "tftp",
                        {"ike": {"extended": true}},
                        "dcerpc",
                        "krb5",
                        "bittorrent-dht",
                        "snmp",
                        "rfb",
                        "sip",
                        "dnp3",
                        "enip",
                        "modbus",
                        "quic",
                        "ldap",
                        "pop3",
                        "arp",
                        {"dhcp": {"extended": true}},
                        "ssh",
                        "mqtt",
                        "http2",
                        "doh2",
                        "pgsql",
                        "stats",
                        "flow"
                    ]
                }
            }],
            "logging": {
                "default-log-level": "notice",
                "outputs": [{"console": {"enabled": true}}]
            }
        });
        let yaml = serde_yaml::to_string(&suricata_config)
            .context("failed to generate the local Suricata configuration")?;
        std::fs::write(&self.suricata_config, format!("%YAML 1.1\n---\n{yaml}")).with_context(
            || {
                format!(
                    "failed to write local Suricata configuration {}",
                    self.suricata_config.display()
                )
            },
        )?;

        let updater_config = serde_json::json!({
            "cache-directory": utf8_path(&self.updater_cache)?,
            "sources": []
        });
        std::fs::write(
            &self.updater_config,
            serde_yaml::to_string(&updater_config)
                .context("failed to generate the isolated suricata-update configuration")?,
        )?;

        std::fs::write(&self.reference_config, REFERENCE_CONFIG).with_context(|| {
            format!(
                "failed to write local Suricata reference configuration {}",
                self.reference_config.display()
            )
        })?;
        for (path, description) in [
            (&self.threshold_config, "threshold"),
            (&self.classification_config, "classification"),
            (&self.enable_config, "enable filter"),
            (&self.drop_config, "drop filter"),
            (&self.modify_config, "modify filter"),
        ] {
            std::fs::write(
                path,
                format!("# EveBox oneshot {description} configuration.\n"),
            )
            .with_context(|| format!("failed to write {}", path.display()))?;
        }

        let mut disable_filter = String::from("# EveBox oneshot disable filter configuration.\n");
        if cfg!(windows) {
            // The Windows Suricata builds do not include libmagic, so rules
            // using the file.magic or filemagic keywords fail to load there.
            disable_filter.push_str("re:file\\.magic\nre:filemagic\n");
        }
        std::fs::write(&self.disable_config, disable_filter)
            .with_context(|| format!("failed to write {}", self.disable_config.display()))?;
        Ok(())
    }
}

fn utf8_path(path: &Path) -> Result<&str> {
    path.to_str().with_context(|| {
        format!(
            "local Suricata workspace path is not valid Unicode: {}",
            path.display()
        )
    })
}

pub(super) struct EveGenerator {
    programs: Programs,
    layout: WorkspaceLayout,
}

impl EveGenerator {
    pub(super) async fn prepare(programs: Programs, workspace: &Path) -> Result<Self> {
        Self::prepare_with(programs, workspace, &TokioProcessExecutor).await
    }

    async fn prepare_with(
        programs: Programs,
        workspace: &Path,
        executor: &dyn ProcessExecutor,
    ) -> Result<Self> {
        let updater_cache = if programs.suricata_update.is_some() {
            match updater_cache_directory().and_then(|path| {
                std::fs::create_dir_all(&path).with_context(|| {
                    format!(
                        "failed to create persistent suricata-update cache {}",
                        path.display()
                    )
                })?;
                Ok(path)
            }) {
                Ok(path) => Some(path),
                Err(err) => {
                    warn!(
                        "Persistent suricata-update cache is unavailable; using the temporary \
                         workspace cache: {err:#}"
                    );
                    None
                }
            }
        } else {
            None
        };
        let generator = Self {
            programs,
            layout: WorkspaceLayout::create(workspace, updater_cache)?,
        };
        generator.update_rules(executor).await?;
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
        let output_directory = output
            .parent()
            .context("local Suricata output path has no parent directory")?;
        let command = self.replay_command(pcap, output_directory);
        info!(
            "Processing {} with local Suricata {}",
            pcap.display(),
            self.programs.version
        );
        execute_checked(
            executor,
            "running local Suricata against the PCAP",
            command,
            ExecutionMode::Interruptible,
        )
        .await?;
        validate_eve_output(output)
    }

    async fn update_rules(&self, executor: &dyn ProcessExecutor) -> Result<()> {
        if self.programs.suricata_update.is_none() {
            info!("Downloading ET/Open, PAW Patrules, and The Hunters Ledger rules directly");
            let summary = rules::download_and_merge(
                &self.programs.version,
                &self.layout.rules,
                &self.layout.rule_file,
            )
            .await?;
            info!(
                "Merged {} rule files while preserving {} archive files in {}",
                summary.rule_files,
                summary.archive_files,
                self.layout.rules.display()
            );
            return Ok(());
        }

        info!("Updating ET/Open, PAW Patrules, and The Hunters Ledger rules locally");
        execute_checked(
            executor,
            "updating the Suricata rule source index",
            self.source_command("update-sources", None),
            ExecutionMode::Interruptible,
        )
        .await?;
        for source in RULE_SOURCES {
            execute_checked(
                executor,
                &format!("enabling Suricata rule source {source}"),
                self.source_command("enable-source", Some(source)),
                ExecutionMode::Interruptible,
            )
            .await?;
        }
        execute_checked(
            executor,
            "downloading and preparing local Suricata rules",
            self.update_command(),
            ExecutionMode::Interruptible,
        )
        .await
    }

    fn updater_base_args(&self, subcommand: &str) -> Vec<OsString> {
        vec![
            OsString::from(subcommand),
            OsString::from("--data-dir"),
            path(&self.layout.updater_data),
            OsString::from("--config"),
            path(&self.layout.updater_config),
            OsString::from("--suricata"),
            path(&self.programs.suricata),
            OsString::from("--suricata-conf"),
            path(&self.layout.suricata_config),
        ]
    }

    fn source_command(&self, subcommand: &str, source: Option<&str>) -> CommandSpec {
        let mut args = self.updater_base_args(subcommand);
        if let Some(source) = source {
            args.push(OsString::from(source));
        }
        CommandSpec::new(path(self.suricata_update()), args)
    }

    fn update_command(&self) -> CommandSpec {
        let mut args = self.updater_base_args("update");
        for (option, value) in [
            ("--output", &self.layout.rules),
            ("--disable-conf", &self.layout.disable_config),
            ("--enable-conf", &self.layout.enable_config),
            ("--drop-conf", &self.layout.drop_config),
            ("--modify-conf", &self.layout.modify_config),
        ] {
            args.push(OsString::from(option));
            args.push(path(value));
        }
        args.extend([
            OsString::from("--no-test"),
            OsString::from("--no-reload"),
            OsString::from("--fail"),
        ]);
        CommandSpec::new(path(self.suricata_update()), args)
    }

    fn suricata_update(&self) -> &Path {
        self.programs
            .suricata_update
            .as_deref()
            .expect("updater command requested without suricata-update")
    }

    fn replay_command(&self, pcap: &Path, output_directory: &Path) -> CommandSpec {
        CommandSpec::new(
            path(&self.programs.suricata),
            [
                OsString::from("--runmode"),
                OsString::from("single"),
                OsString::from("-c"),
                path(&self.layout.suricata_config),
                OsString::from("-r"),
                path(pcap),
                OsString::from("-S"),
                path(&self.layout.rule_file),
                OsString::from("-k"),
                OsString::from("none"),
                OsString::from("-l"),
                path(output_directory),
            ],
        )
        .with_current_dir(&self.layout.rules)
    }
}

/// Persistent suricata-update cache so repeated runs do not re-download
/// unchanged rule feeds.
fn updater_cache_directory() -> Result<PathBuf> {
    let directories = directories::ProjectDirs::from("org", "evebox", "evebox")
        .context("failed to determine a cache directory for suricata-update")?;
    Ok(directories
        .cache_dir()
        .join("oneshot-suricata")
        .join("local")
        .join("suricata-update"))
}

async fn execute_checked(
    executor: &dyn ProcessExecutor,
    phase: &str,
    command: CommandSpec,
    mode: ExecutionMode,
) -> Result<()> {
    debug!("Running local command: {}", command.display());
    match executor.execute(&command, mode).await {
        Err(ExecuteError::Interrupted) => anyhow::bail!("{phase} was interrupted"),
        Err(err) => anyhow::bail!("failed while {phase}: {err}"),
        Ok(result) if !result.success => failed_command(phase, &command, result),
        Ok(_) => Ok(()),
    }
}

fn failed_command<T>(phase: &str, command: &CommandSpec, result: CommandResult) -> Result<T> {
    let detail = if !result.stderr.trim().is_empty() {
        result.stderr.trim()
    } else {
        result.stdout.trim()
    };
    if !detail.is_empty() {
        anyhow::bail!("{phase} failed: {detail}");
    }
    if let Some(code) = result.code {
        anyhow::bail!(
            "{phase} failed: {} exited with status {code}",
            command.program.to_string_lossy()
        );
    }
    anyhow::bail!(
        "{phase} failed: {} exited unsuccessfully",
        command.display()
    )
}
