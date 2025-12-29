//! Configuration module for QwenCoder Bot
//!
//! Loads and validates configuration from TOML files.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub models: ModelsConfig,
    pub polling: PollingConfig,
    pub vm: VmConfig,
    pub gog_profiles: GogProfilesConfig,
    #[serde(default)]
    pub repositories: RepositoriesConfig,
    #[serde(default)]
    pub assessment: AssessmentConfig,
    #[serde(default)]
    pub implementation: ImplementationConfig,
    #[serde(default)]
    pub testing: TestingConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub error_handling: ErrorHandlingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

/// Server endpoints configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub gogs_url: String,
    pub ollama_endpoint: String,
    #[serde(default = "default_ollama_timeout")]
    pub ollama_timeout: u64,
}

fn default_ollama_timeout() -> u64 {
    300
}

/// Model configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ModelsConfig {
    pub assessor: String,
    pub coder: String,
    #[serde(default = "default_model_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub parameters: ModelParameters,
}

fn default_model_timeout() -> u64 {
    300
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct ModelParameters {
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_top_p")]
    pub top_p: f32,
    pub max_tokens: Option<u32>,
}

fn default_temperature() -> f32 {
    0.3
}

fn default_top_p() -> f32 {
    0.9
}

/// Polling configuration
#[derive(Debug, Deserialize, Clone)]
pub struct PollingConfig {
    #[serde(default = "default_polling_interval")]
    pub interval_secs: u64,
    pub trigger_label: String,
    #[serde(default)]
    pub additional_labels: Vec<String>,
    #[serde(default)]
    pub stop_labels: Vec<String>,
    #[serde(default)]
    pub require_assignee: bool,
}

fn default_polling_interval() -> u64 {
    60
}

/// VM configuration
#[derive(Debug, Deserialize, Clone)]
pub struct VmConfig {
    pub base_snapshot: PathBuf,
    #[serde(default = "default_vm_ram")]
    pub ram_mb: u32,
    #[serde(default = "default_vm_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_ssh_user")]
    pub ssh_user: String,
    #[serde(default = "default_ssh_port_base")]
    pub ssh_port_base: u16,
    pub ssh_key: Option<PathBuf>,
    #[serde(default = "default_hypervisor")]
    pub hypervisor: String,
    #[serde(default)]
    pub qemu: QemuConfig,
    #[serde(default)]
    pub limits: VmLimits,
}

fn default_vm_ram() -> u32 {
    4096
}

fn default_vm_timeout() -> u64 {
    600
}

fn default_ssh_user() -> String {
    "qwencoder".to_string()
}

fn default_ssh_port_base() -> u16 {
    2222
}

fn default_hypervisor() -> String {
    "qemu".to_string()
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct QemuConfig {
    #[serde(default = "default_enable_kvm")]
    pub enable_kvm: bool,
    #[serde(default = "default_cpu_cores")]
    pub cpu_cores: u32,
}

fn default_enable_kvm() -> bool {
    true
}

fn default_cpu_cores() -> u32 {
    2
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct VmLimits {
    #[serde(default = "default_max_concurrent_vms")]
    pub max_concurrent_vms: u32,
    #[serde(default = "default_max_disk_size")]
    pub max_disk_size_mb: u32,
    #[serde(default = "default_cpu_quota")]
    pub cpu_quota_percent: u32,
}

fn default_max_concurrent_vms() -> u32 {
    3
}

fn default_max_disk_size() -> u32 {
    10240
}

fn default_cpu_quota() -> u32 {
    200
}

/// Gog CLI profiles configuration
#[derive(Debug, Deserialize, Clone)]
pub struct GogProfilesConfig {
    pub assessor: String,
    pub coder: String,
    pub gog_binary: Option<PathBuf>,
    #[serde(default = "default_gog_timeout")]
    pub gog_timeout_secs: u64,
}

fn default_gog_timeout() -> u64 {
    30
}

/// Repositories configuration
#[derive(Debug, Deserialize, Clone, Default)]
pub struct RepositoriesConfig {
    pub repos: Option<Vec<String>>,
    pub exclude_repos: Option<Vec<String>>,
    pub cache_dir: Option<PathBuf>,
    #[serde(default = "default_max_cache_size")]
    pub max_cache_size_mb: u32,
}

fn default_max_cache_size() -> u32 {
    5120
}

/// Assessment configuration
#[derive(Debug, Deserialize, Clone)]
pub struct AssessmentConfig {
    pub prompt_file: Option<PathBuf>,
    #[serde(default = "default_min_confidence")]
    pub min_confidence: u32,
    #[serde(default = "default_track_outcomes")]
    pub track_outcomes: bool,
    pub outcomes_file: Option<PathBuf>,
    #[serde(default = "default_retry_on_failure")]
    pub retry_on_failure: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for AssessmentConfig {
    fn default() -> Self {
        Self {
            prompt_file: None,
            min_confidence: default_min_confidence(),
            track_outcomes: default_track_outcomes(),
            outcomes_file: None,
            retry_on_failure: default_retry_on_failure(),
            max_retries: default_max_retries(),
        }
    }
}

fn default_min_confidence() -> u32 {
    60
}

fn default_track_outcomes() -> bool {
    true
}

fn default_retry_on_failure() -> bool {
    true
}

fn default_max_retries() -> u32 {
    2
}

/// Implementation configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ImplementationConfig {
    pub prompt_file: Option<PathBuf>,
    #[serde(default = "default_branch_prefix")]
    pub branch_prefix: String,
    pub commit_template: Option<String>,
    #[serde(default = "default_auto_push")]
    pub auto_push: bool,
    #[serde(default)]
    pub create_pr: bool,
}

impl Default for ImplementationConfig {
    fn default() -> Self {
        Self {
            prompt_file: None,
            branch_prefix: default_branch_prefix(),
            commit_template: None,
            auto_push: default_auto_push(),
            create_pr: false,
        }
    }
}

fn default_branch_prefix() -> String {
    "qwen-fix-".to_string()
}

fn default_auto_push() -> bool {
    true
}

/// Testing configuration
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TestingConfig {
    #[serde(default)]
    pub rust: LanguageTestConfig,
    #[serde(default)]
    pub csharp: LanguageTestConfig,
    #[serde(default)]
    pub python: LanguageTestConfig,
    #[serde(default)]
    pub shell: LanguageTestConfig,
    #[serde(default)]
    pub custom: Vec<CustomTestConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LanguageTestConfig {
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default = "default_test_timeout")]
    pub timeout_secs: u64,
}

impl Default for LanguageTestConfig {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            timeout_secs: default_test_timeout(),
        }
    }
}

fn default_test_timeout() -> u64 {
    300
}

#[derive(Debug, Deserialize, Clone)]
pub struct CustomTestConfig {
    pub repo: String,
    pub commands: Vec<String>,
    #[serde(default = "default_test_timeout")]
    pub timeout_secs: u64,
}

/// Logging configuration
#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    pub file: Option<PathBuf>,
    pub assessment_log: Option<PathBuf>,
    pub implementation_log: Option<PathBuf>,
    pub vm_log_dir: Option<PathBuf>,
    #[serde(default = "default_max_log_size")]
    pub max_size_mb: u32,
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
    #[serde(default = "default_max_backups")]
    pub max_backups: u32,
    #[serde(default)]
    pub log_to_stdout: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            file: None,
            assessment_log: None,
            implementation_log: None,
            vm_log_dir: None,
            max_size_mb: default_max_log_size(),
            max_age_days: default_max_age_days(),
            max_backups: default_max_backups(),
            log_to_stdout: false,
        }
    }
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_max_log_size() -> u32 {
    100
}

fn default_max_age_days() -> u32 {
    30
}

fn default_max_backups() -> u32 {
    10
}

/// Error handling configuration
#[derive(Debug, Deserialize, Clone)]
pub struct ErrorHandlingConfig {
    #[serde(default = "default_on_assessment_failure")]
    pub on_assessment_failure: String,
    #[serde(default = "default_on_implementation_failure")]
    pub on_implementation_failure: String,
    #[serde(default = "default_on_vm_failure")]
    pub on_vm_failure: String,
    #[serde(default = "default_error_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_backoff")]
    pub retry_backoff: String,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_secs: u64,
}

impl Default for ErrorHandlingConfig {
    fn default() -> Self {
        Self {
            on_assessment_failure: default_on_assessment_failure(),
            on_implementation_failure: default_on_implementation_failure(),
            on_vm_failure: default_on_vm_failure(),
            max_retries: default_error_max_retries(),
            retry_backoff: default_retry_backoff(),
            retry_delay_secs: default_retry_delay(),
        }
    }
}

fn default_on_assessment_failure() -> String {
    "retry".to_string()
}

fn default_on_implementation_failure() -> String {
    "escalate".to_string()
}

fn default_on_vm_failure() -> String {
    "skip".to_string()
}

fn default_error_max_retries() -> u32 {
    3
}

fn default_retry_backoff() -> String {
    "exponential".to_string()
}

fn default_retry_delay() -> u64 {
    10
}

/// Security configuration
#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    #[serde(default = "default_verify_ssl")]
    pub verify_ssl: bool,
    pub ca_bundle: Option<PathBuf>,
    #[serde(default = "default_restrict_file_ops")]
    pub restrict_file_ops: bool,
    #[serde(default = "default_max_files_per_issue")]
    pub max_files_per_issue: u32,
    #[serde(default)]
    pub forbidden_patterns: Vec<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            verify_ssl: default_verify_ssl(),
            ca_bundle: None,
            restrict_file_ops: default_restrict_file_ops(),
            max_files_per_issue: default_max_files_per_issue(),
            forbidden_patterns: vec![
                ".git/*".to_string(),
                ".env".to_string(),
                "secrets.toml".to_string(),
                "*.key".to_string(),
                "*.pem".to_string(),
            ],
        }
    }
}

fn default_verify_ssl() -> bool {
    true
}

fn default_restrict_file_ops() -> bool {
    true
}

fn default_max_files_per_issue() -> u32 {
    10
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;

        Ok(config)
    }

    /// Load configuration from default location or environment variable
    pub fn load_default() -> Result<Self> {
        // Check environment variable first
        if let Ok(path) = std::env::var("QWENCODER_CONFIG") {
            return Self::load(&path);
        }

        // Try default locations
        let home = dirs::home_dir().context("Could not determine home directory")?;
        let config_path = home.join(".config/qwencoder-bot/config.toml");

        if config_path.exists() {
            return Self::load(&config_path);
        }

        // Try current directory
        let cwd_config = PathBuf::from("config.toml");
        if cwd_config.exists() {
            return Self::load(&cwd_config);
        }

        anyhow::bail!(
            "No configuration file found. Expected at:\n\
             - $QWENCODER_CONFIG\n\
             - ~/.config/qwencoder-bot/config.toml\n\
             - ./config.toml"
        )
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        // Validate URLs
        if !self.server.gogs_url.starts_with("http://") && !self.server.gogs_url.starts_with("https://") {
            anyhow::bail!("gogs_url must start with http:// or https://");
        }

        if !self.server.ollama_endpoint.starts_with("http://") {
            anyhow::bail!("ollama_endpoint must start with http://");
        }

        // Validate trigger label
        if self.polling.trigger_label.is_empty() {
            anyhow::bail!("trigger_label cannot be empty");
        }

        // Validate profiles
        if self.gog_profiles.assessor.is_empty() {
            anyhow::bail!("gog_profiles.assessor cannot be empty");
        }

        if self.gog_profiles.coder.is_empty() {
            anyhow::bail!("gog_profiles.coder cannot be empty");
        }

        Ok(())
    }

    /// Get default test commands for a language
    pub fn get_test_commands(&self, language: &str, repo: Option<&str>) -> Vec<String> {
        // Check for custom repo config first
        if let Some(repo_name) = repo {
            for custom in &self.testing.custom {
                if custom.repo == repo_name {
                    return custom.commands.clone();
                }
            }
        }

        // Fall back to language defaults
        match language.to_lowercase().as_str() {
            "rust" => {
                if self.testing.rust.commands.is_empty() {
                    vec![
                        "cargo test --all".to_string(),
                        "cargo clippy -- -D warnings".to_string(),
                        "cargo fmt -- --check".to_string(),
                    ]
                } else {
                    self.testing.rust.commands.clone()
                }
            }
            "csharp" | "c#" => {
                if self.testing.csharp.commands.is_empty() {
                    vec![
                        "dotnet test".to_string(),
                        "dotnet format --verify-no-changes".to_string(),
                    ]
                } else {
                    self.testing.csharp.commands.clone()
                }
            }
            "python" => {
                if self.testing.python.commands.is_empty() {
                    vec![
                        "pytest".to_string(),
                        "mypy .".to_string(),
                        "black --check .".to_string(),
                    ]
                } else {
                    self.testing.python.commands.clone()
                }
            }
            "shell" | "bash" | "sh" => {
                if self.testing.shell.commands.is_empty() {
                    vec!["shellcheck *.sh".to_string()]
                } else {
                    self.testing.shell.commands.clone()
                }
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config_str = r#"
            [server]
            gogs_url = "https://gogs.example.com"
            ollama_endpoint = "http://localhost:11434"

            [models]
            assessor = "qwen2.5-coder:7b"
            coder = "qwen2.5-coder:14b"

            [polling]
            trigger_label = "qwen-candidate"

            [vm]
            base_snapshot = "/path/to/snapshot.qcow2"

            [gog_profiles]
            assessor = "qwen-assessor"
            coder = "qwen-coder"
        "#;

        let config: Config = toml::from_str(config_str).unwrap();

        assert_eq!(config.polling.interval_secs, 60);
        assert_eq!(config.vm.ram_mb, 4096);
        assert_eq!(config.models.parameters.temperature, 0.3);
    }
}
