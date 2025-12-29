# QwenCoder Bot - Configuration Guide

This guide details all configuration options for the QwenCoder bot system.

## Configuration Files Overview

The bot uses multiple configuration files:

```
~/.config/qwencoder-bot/
├── config.toml              # Main bot configuration
├── prompts/
│   ├── assessment.txt       # 7B model system prompt
│   └── implementation.txt   # 14B model system prompt
└── logs/
    └── bot.log              # Main log file

~/.config/gogs-cli/
└── config.toml              # Gog CLI profiles (bot accounts)

/etc/systemd/system/
├── ollama.service.d/
│   └── override.conf        # Ollama VRAM settings
└── qwencoder-bot.service    # Bot systemd service
```

## Main Bot Configuration

**Location**: `~/.config/qwencoder-bot/config.toml`

### Complete Configuration Template

```toml
# ============================================================================
# QwenCoder Bot Configuration
# ============================================================================

# ----------------------------------------------------------------------------
# Server Endpoints
# ----------------------------------------------------------------------------
[server]
# Gogs server URL (no trailing slash)
gogs_url = "https://gogs.example.com"

# Ollama API endpoint
ollama_endpoint = "http://localhost:11434"

# Optional: Ollama timeout (seconds)
# Default: 300 (5 minutes)
ollama_timeout = 300

# ----------------------------------------------------------------------------
# Model Configuration
# ----------------------------------------------------------------------------
[models]
# Assessment model (lightweight, fast)
assessor = "qwen2.5-coder:7b"

# Implementation model (heavier, better quality)
coder = "qwen2.5-coder:14b"

# Generation timeout (seconds)
# How long to wait for model response
# Increase for complex prompts
timeout_secs = 300

# Optional: Model parameters
[models.parameters]
# Temperature (0.0 = deterministic, 1.0 = creative)
temperature = 0.3

# Top-p sampling (nucleus sampling)
top_p = 0.9

# Number of tokens to generate
# (usually not needed as models detect completion)
# max_tokens = 4096

# ----------------------------------------------------------------------------
# Polling Configuration
# ----------------------------------------------------------------------------
[polling]
# How often to check for new issues (seconds)
interval_secs = 60

# Label that triggers bot processing
trigger_label = "qwen-candidate"

# Optional: Additional labels to monitor
# additional_labels = ["auto-fix", "bot-candidate"]

# Optional: Stop labels (if present, don't process)
# stop_labels = ["wip", "blocked"]

# Optional: Only process issues with assignee
# require_assignee = false

# ----------------------------------------------------------------------------
# VM Configuration
# ----------------------------------------------------------------------------
[vm]
# Path to base VM snapshot
base_snapshot = "/var/lib/qwencoder-bot/qwencoder-base.qcow2"

# VM RAM allocation (MB)
ram_mb = 4096

# Maximum time for VM job (seconds)
# After this, VM is killed
timeout_secs = 600

# SSH connection details
ssh_user = "qwencoder"
ssh_port_base = 2222
ssh_key = "/home/user/.ssh/qwencoder_vm"

# Optional: VM hypervisor type
# Options: "qemu", "virtualbox", "docker"
hypervisor = "qemu"

# Optional: QEMU-specific options
[vm.qemu]
enable_kvm = true
cpu_cores = 2

# Optional: Resource limits
[vm.limits]
max_concurrent_vms = 3
max_disk_size_mb = 10240
cpu_quota_percent = 200  # 200% = 2 cores

# ----------------------------------------------------------------------------
# Gog CLI Integration
# ----------------------------------------------------------------------------
[gog_profiles]
# Profile name for assessment phase
assessor = "qwen-assessor"

# Profile name for implementation phase
coder = "qwen-coder"

# Optional: Path to gog binary
# gog_binary = "/usr/local/bin/gog"

# Optional: gog CLI timeout
# gog_timeout_secs = 30

# ----------------------------------------------------------------------------
# Repository Configuration
# ----------------------------------------------------------------------------
[repositories]
# Optional: Limit to specific repositories
# If not specified, monitors all accessible repos
# repos = [
#     "owner/repo1",
#     "owner/repo2",
# ]

# Optional: Exclude specific repositories
# exclude_repos = [
#     "owner/experimental",
#     "owner/archived-project",
# ]

# Optional: Cache directory for cloned repos
cache_dir = "/var/cache/qwencoder-bot/repos"

# Optional: Maximum cache size (MB)
max_cache_size_mb = 5120

# ----------------------------------------------------------------------------
# Assessment Configuration
# ----------------------------------------------------------------------------
[assessment]
# Path to assessment system prompt
prompt_file = "~/.config/qwencoder-bot/prompts/assessment.txt"

# Minimum confidence threshold (0-100)
# Issues below this confidence are escalated
min_confidence = 60

# Optional: Historical data for learning
# Store assessment outcomes for analysis
track_outcomes = true
outcomes_file = "~/.config/qwencoder-bot/assessment-history.json"

# Optional: Retry failed assessments
retry_on_failure = true
max_retries = 2

# ----------------------------------------------------------------------------
# Implementation Configuration
# ----------------------------------------------------------------------------
[implementation]
# Path to implementation system prompt
prompt_file = "~/.config/qwencoder-bot/prompts/implementation.txt"

# Branch naming pattern
# {issue} will be replaced with issue number
branch_prefix = "qwen-fix-"

# Optional: Commit message template
# Variables: {issue_number}, {issue_title}, {summary}
commit_template = """
{type}: {summary}

{detailed_description}

Fixes #{issue_number}
"""

# Optional: Auto-push branches
auto_push = true

# Optional: Create PRs automatically (requires Gogs API support)
create_pr = false

# ----------------------------------------------------------------------------
# Testing Configuration
# ----------------------------------------------------------------------------
[testing]
# Default test commands by language
[testing.rust]
commands = [
    "cargo test --all",
    "cargo clippy -- -D warnings",
    "cargo fmt -- --check"
]
timeout_secs = 300

[testing.csharp]
commands = [
    "dotnet test",
    "dotnet format --verify-no-changes"
]
timeout_secs = 180

[testing.python]
commands = [
    "pytest",
    "mypy .",
    "black --check ."
]
timeout_secs = 120

[testing.shell]
commands = [
    "shellcheck *.sh"
]
timeout_secs = 30

# Optional: Custom commands for specific repos
[[testing.custom]]
repo = "owner/special-project"
commands = ["make test", "make lint"]
timeout_secs = 600

# ----------------------------------------------------------------------------
# Logging Configuration
# ----------------------------------------------------------------------------
[logging]
# Log level: "error", "warn", "info", "debug", "trace"
level = "info"

# Main log file
file = "/var/log/qwencoder-bot/bot.log"

# Optional: Separate log files for phases
assessment_log = "/var/log/qwencoder-bot/assessments.log"
implementation_log = "/var/log/qwencoder-bot/implementations.log"
vm_log_dir = "/var/log/qwencoder-bot/vms/"

# Optional: Log rotation
max_size_mb = 100
max_age_days = 30
max_backups = 10

# Optional: Also log to stdout
log_to_stdout = false

# ----------------------------------------------------------------------------
# Error Handling
# ----------------------------------------------------------------------------
[error_handling]
# What to do on various failures

# When assessment fails (network, parse error, etc.)
# Options: "skip", "retry", "escalate"
on_assessment_failure = "retry"

# When implementation fails (tests don't pass)
# Options: "escalate", "retry", "comment"
on_implementation_failure = "escalate"

# When VM fails to spawn
# Options: "skip", "retry", "alert"
on_vm_failure = "skip"

# Maximum retries before giving up
max_retries = 3

# Backoff strategy: "linear", "exponential"
retry_backoff = "exponential"

# Initial retry delay (seconds)
retry_delay_secs = 10

# ----------------------------------------------------------------------------
# Notifications (Optional)
# ----------------------------------------------------------------------------
[notifications]
enabled = false

# Email notifications
[notifications.email]
enabled = false
smtp_host = "smtp.example.com"
smtp_port = 587
smtp_user = "bot@example.com"
smtp_password = "password"
from_addr = "qwencoder-bot@example.com"
to_addrs = ["admin@example.com"]

# When to send notifications
notify_on_success = false
notify_on_failure = true
notify_on_escalation = true

# Slack notifications
[notifications.slack]
enabled = false
webhook_url = "https://hooks.slack.com/services/..."
channel = "#bot-alerts"

# ----------------------------------------------------------------------------
# Metrics and Monitoring (Optional)
# ----------------------------------------------------------------------------
[metrics]
enabled = true

# Metrics file (JSON)
file = "/var/lib/qwencoder-bot/metrics.json"

# What to track
track = [
    "issues_processed",
    "assessment_outcomes",
    "implementation_success_rate",
    "average_time_per_phase",
    "model_inference_time",
    "vm_spawn_time"
]

# Optional: Export to Prometheus
[metrics.prometheus]
enabled = false
port = 9090

# ----------------------------------------------------------------------------
# Security Settings
# ----------------------------------------------------------------------------
[security]
# Verify SSL certificates for Gogs
verify_ssl = true

# Optional: Path to custom CA bundle
# ca_bundle = "/etc/ssl/certs/custom-ca.pem"

# Restrict file operations in VM
restrict_file_ops = true

# Maximum files that can be modified per issue
max_files_per_issue = 10

# Disallow certain file patterns
forbidden_patterns = [
    ".git/*",
    ".env",
    "secrets.toml",
    "*.key",
    "*.pem"
]

# ----------------------------------------------------------------------------
# Experimental Features
# ----------------------------------------------------------------------------
[experimental]
# Use multiple models for implementation
multi_model = false

# Parallel VM processing
parallel_vms = false

# Auto-merge trivial fixes (DANGEROUS)
auto_merge = false

# Learn from feedback
feedback_loop = false
```

## Environment Variables

Some settings can be overridden via environment variables:

```bash
# Configuration file path
export QWENCODER_CONFIG="/custom/path/config.toml"

# Logging level
export QWENCODER_LOG_LEVEL="debug"

# Disable VM operations (for testing)
export QWENCODER_NO_VM="true"

# Override Ollama endpoint
export OLLAMA_HOST="http://other-server:11434"

# Gog CLI path
export GOG_BINARY="/custom/path/gog"
```

## Gog CLI Configuration

**Location**: `~/.config/gogs-cli/config.toml`

```toml
[server]
url = "https://gogs.example.com"

[defaults]
profile = "default"  # Your personal profile

# Your personal profile
[profiles.default]
gogs_user = "your-username"
token = "your-personal-token"
role = "Human Developer"
signature = "[Human]"

# Bot assessment profile
[profiles.qwen-assessor]
gogs_user = "bot-qwen-assess"
token = "ASSESSMENT_BOT_TOKEN_HERE"
role = "Issue Assessment"
signature = "[QwenAssess]"

# Bot implementation profile
[profiles.qwen-coder]
gogs_user = "bot-qwen-coder"
token = "IMPLEMENTATION_BOT_TOKEN_HERE"
role = "Code Implementation"
signature = "[QwenCoder]"
```

**Getting Tokens**:
1. Log into Gogs as each bot user
2. Settings → Applications → Generate New Token
3. Give token a descriptive name: "QwenCoder Bot Access"
4. Copy token to config (you can't view it again)

## Ollama Configuration

**Location**: `/etc/systemd/system/ollama.service.d/override.conf`

```ini
[Service]
# Bind to all interfaces (or specific IP)
Environment="OLLAMA_HOST=0.0.0.0:11434"

# Auto-unload models after 30 seconds
Environment="OLLAMA_KEEP_ALIVE=30s"

# Only keep one model loaded at a time
Environment="OLLAMA_MAX_LOADED_MODELS=1"

# Optional: Store models in custom location
# Environment="OLLAMA_MODELS=/mnt/fast-storage/ollama-models"

# Optional: Number of concurrent requests
# Environment="OLLAMA_NUM_PARALLEL=1"
```

Apply changes:
```bash
sudo systemctl daemon-reload
sudo systemctl restart ollama
```

## Systemd Service Configuration

**Location**: `/etc/systemd/system/qwencoder-bot.service`

```ini
[Unit]
Description=QwenCoder Bot - Autonomous Code Implementation
After=network.target ollama.service
Requires=ollama.service

[Service]
Type=simple
User=qwencoder
Group=qwencoder
WorkingDirectory=/home/qwencoder/qwencoder-bot

# Main executable
ExecStart=/home/qwencoder/qwencoder-bot/target/release/qwencoder-bot

# Restart on failure
Restart=always
RestartSec=10

# Resource limits
MemoryLimit=2G
CPUQuota=200%

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=qwencoder-bot

# Environment variables
Environment="RUST_LOG=info"
Environment="QWENCODER_CONFIG=/home/qwencoder/.config/qwencoder-bot/config.toml"

# Security settings
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

## Configuration Validation

Before running the bot, validate your configuration:

### Check Config Syntax

```bash
# TOML syntax check
cargo run --bin qwencoder-bot -- --check-config

# Should output:
# ✓ Configuration valid
# ✓ All required fields present
# ✓ All file paths exist
# ✓ Gog profiles configured
# ✓ Ollama endpoint reachable
# ✓ VM snapshot exists
```

### Test Individual Components

```bash
# Test Ollama connection
curl http://localhost:11434/api/generate \
  -d '{"model":"qwen2.5-coder:7b","prompt":"test"}' | jq

# Test gog CLI
gog --profile qwen-assessor repo list

# Test VM spawn
./scripts/vm-manager.sh create 999
./scripts/vm-manager.sh exec 999 "echo test"
./scripts/vm-manager.sh destroy 999
```

## Common Configuration Patterns

### High-Throughput Setup

For processing many issues quickly:

```toml
[polling]
interval_secs = 30  # Check more frequently

[vm]
timeout_secs = 300  # Shorter timeout
ram_mb = 2048  # Less RAM per VM

[vm.limits]
max_concurrent_vms = 5  # Run multiple VMs

[models.parameters]
temperature = 0.1  # More deterministic (faster)
```

### Conservative/High-Quality Setup

For maximum code quality:

```toml
[assessment]
min_confidence = 80  # Higher threshold

[models.parameters]
temperature = 0.5  # More creative solutions

[testing]
# Add more comprehensive tests
[testing.rust]
commands = [
    "cargo test --all",
    "cargo clippy -- -D warnings",
    "cargo fmt -- --check",
    "cargo audit",  # Security audit
    "cargo bench"   # Performance tests
]
timeout_secs = 600  # More time for thorough testing
```

### Development/Testing Setup

For experimenting with the bot:

```toml
[polling]
interval_secs = 300  # Check less often

[logging]
level = "debug"  # Verbose logging
log_to_stdout = true

[vm]
timeout_secs = 1200  # More time to debug

[error_handling]
on_assessment_failure = "skip"  # Don't retry failures
on_implementation_failure = "comment"  # Just comment, don't escalate

[experimental]
# Enable all experimental features
multi_model = true
feedback_loop = true
```

## Security Best Practices

### API Tokens

- Use separate tokens for assessor and coder profiles
- Rotate tokens periodically (every 90 days)
- Store tokens with restricted permissions (chmod 600)
- Never commit tokens to git

### VM Isolation

```toml
[vm]
# Use read-only base snapshot
base_snapshot = "/var/lib/qwencoder-bot/base-readonly.qcow2"

[security]
# Restrict operations
restrict_file_ops = true
max_files_per_issue = 5

# Block sensitive paths
forbidden_patterns = [
    ".git/*",
    ".env*",
    "secrets*",
    "*.key",
    "*.pem",
    "~/.ssh/*"
]
```

### Network Restrictions

Configure VM networking to only allow Gogs access:

```bash
# In VM firewall
sudo iptables -A OUTPUT -d <gogs-server-ip> -j ACCEPT
sudo iptables -A OUTPUT -j DROP
```

## Troubleshooting Configuration

### Config Not Found

```bash
# Check expected location
ls -la ~/.config/qwencoder-bot/config.toml

# Use explicit path
qwencoder-bot --config /path/to/config.toml
```

### Invalid TOML Syntax

```bash
# Validate with external tool
cargo install --quiet toml-cli
toml get ~/.config/qwencoder-bot/config.toml
```

### Permission Errors

```bash
# Fix permissions
chmod 600 ~/.config/qwencoder-bot/config.toml
chown $USER:$USER ~/.config/qwencoder-bot/config.toml
```

### Environment Variables Not Working

```bash
# Check they're being read
qwencoder-bot --show-config
# Should display active configuration including env vars
```

## Configuration Migration

When updating the bot:

```bash
# Back up current config
cp ~/.config/qwencoder-bot/config.toml \
   ~/.config/qwencoder-bot/config.toml.backup

# Check for new options
qwencoder-bot --show-config-template > new-config.toml

# Merge changes manually
diff ~/.config/qwencoder-bot/config.toml new-config.toml
```

## Next Steps

After configuring:
1. Validate configuration with `--check-config`
2. Test each component individually
3. Run manual test with `--once` flag
4. Review logs for any errors
5. Enable systemd service for production

See `IMPLEMENTATION_GUIDE.md` for the complete setup process.
