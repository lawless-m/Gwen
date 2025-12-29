# QwenCoder Bot - Implementation Guide

This guide walks through setting up the QwenCoder bot from scratch. Follow the steps in order.

## Prerequisites

Before starting, ensure you have:

- [x] Gogs server accessible with API
- [x] Gog CLI installed and working
- [x] Ollama server running (tested with `ollama list`)
- [x] NVIDIA 3090 GPU with proper drivers
- [x] VM hypervisor available (QEMU/KVM, VirtualBox, etc.)
- [x] Rust toolchain installed (`rustc`, `cargo`)
- [x] At least one Rust repository in Gogs to test with

## Step 1: Pull Qwen Models

Pull both required models to your Ollama server:

```bash
# Pull assessment model (7B)
ollama pull qwen2.5-coder:7b

# Pull implementation model (14B)  
ollama pull qwen2.5-coder:14b

# Verify both models are available
ollama list
```

Expected output:
```
NAME                    ID              SIZE      MODIFIED
qwen2.5-coder:14b      a1b2c3d4e5f6    8.7 GB    2 minutes ago
qwen2.5-coder:7b       f6e5d4c3b2a1    4.7 GB    5 minutes ago
```

**Important**: The first time you use each model, it will take a few seconds to load into VRAM. Subsequent uses are faster if within the `OLLAMA_KEEP_ALIVE` timeout.

## Step 2: Configure Ollama Auto-Unload

Edit Ollama's systemd service to ensure models unload after inactivity:

```bash
sudo mkdir -p /etc/systemd/system/ollama.service.d
sudo nano /etc/systemd/system/ollama.service.d/override.conf
```

Add:
```ini
[Service]
Environment="OLLAMA_HOST=0.0.0.0:11434"
Environment="OLLAMA_KEEP_ALIVE=30s"
Environment="OLLAMA_MAX_LOADED_MODELS=1"
```

Reload and restart:
```bash
sudo systemctl daemon-reload
sudo systemctl restart ollama

# Verify it's running
systemctl status ollama
```

**Why**: This ensures the 7B model unloads before the 14B model loads, preventing VRAM exhaustion.

## Step 3: Create Gogs Bot Accounts

Create two bot user accounts in your Gogs server:

### Account 1: Assessor Bot
- **Username**: `bot-qwen-assess`
- **Email**: `qwen-assess@localhost` (or your domain)
- **Role**: Regular user

### Account 2: Coder Bot
- **Username**: `bot-qwen-coder`
- **Email**: `qwen-coder@localhost`
- **Role**: Regular user

For each account:
1. Log into Gogs as the bot user
2. Go to **Settings → Applications**
3. Generate new token
4. Copy token for use in next step

**Permissions**: Ensure both bots have:
- Read access to repositories you want them to work on
- Ability to create branches
- Ability to comment on issues
- Ability to manage labels

## Step 4: Configure Gog CLI Profiles

Edit your gog CLI config (`~/.config/gogs-cli/config.toml`):

```toml
[server]
url = "https://your-gogs-server.com"

[defaults]
profile = "default"  # Your human profile

# Your existing human profile
[profiles.default]
gogs_user = "your-username"
token = "your-human-token"
role = "Human Developer"
signature = "[Human]"

# Add these bot profiles
[profiles.qwen-assessor]
gogs_user = "bot-qwen-assess"
token = "ASSESSOR_TOKEN_FROM_STEP_3"
role = "Issue Assessment"
signature = "[QwenAssess]"

[profiles.qwen-coder]
gogs_user = "bot-qwen-coder"
token = "CODER_TOKEN_FROM_STEP_3"
role = "Code Implementation"
signature = "[QwenCoder]"
```

**Test the profiles**:
```bash
# Test assessor profile
gog --profile qwen-assessor repo list

# Test coder profile
gog --profile qwen-coder repo list
```

Both should return your accessible repositories.

## Step 5: Set Up VM Base Snapshot

Create a base VM with all required toolchains.

### Using QEMU/KVM (Recommended for Linux)

**Create base VM**:
```bash
# Download Debian netinst ISO
wget https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-12.X.X-amd64-netinst.iso

# Create disk image (20GB)
qemu-img create -f qcow2 qwencoder-base.qcow2 20G

# Install Debian (interactive)
qemu-system-x86_64 \
  -m 4096 \
  -cpu host \
  -enable-kvm \
  -cdrom debian-12.X.X-amd64-netinst.iso \
  -hda qwencoder-base.qcow2 \
  -boot d
```

**Inside the VM, install toolchains**:
```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install Git
sudo apt install -y git

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup component add clippy rustfmt

# Install C# (if needed)
wget https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb -O packages-microsoft-prod.deb
sudo dpkg -i packages-microsoft-prod.deb
rm packages-microsoft-prod.deb
sudo apt update
sudo apt install -y dotnet-sdk-8.0

# Install Python tooling (if needed)
sudo apt install -y python3 python3-pip python3-venv
pip3 install pytest mypy black

# Install shell script validation
sudo apt install -y shellcheck

# Configure git identity (for commits)
git config --global user.name "QwenCoder Bot"
git config --global user.email "qwen-coder@localhost"

# Install SSH for bot access
sudo apt install -y openssh-server

# Shutdown cleanly
sudo shutdown -h now
```

**Create snapshot**:
```bash
# The qcow2 file is now your base snapshot
cp qwencoder-base.qcow2 qwencoder-base-snapshot.qcow2
```

**Test the snapshot**:
```bash
# Create ephemeral copy
cp qwencoder-base-snapshot.qcow2 test-vm.qcow2

# Boot it
qemu-system-x86_64 \
  -m 4096 \
  -cpu host \
  -enable-kvm \
  -hda test-vm.qcow2 \
  -snapshot  # Important: runs in memory, no disk changes

# Test inside VM:
# - cargo --version
# - dotnet --version
# - python3 --version
# - git --version

# Shutdown and delete
rm test-vm.qcow2
```

### Alternative: VirtualBox

If using VirtualBox instead of QEMU:

1. Create new VM with same Debian install
2. Install all toolchains as above
3. Create snapshot: **Machine → Take Snapshot → "base"**
4. Test by cloning snapshot and running tests

## Step 6: Build the Bot

Clone or create the bot codebase (you'll implement this based on the architecture):

```bash
# Create project structure
cargo new qwencoder-bot
cd qwencoder-bot

# Add dependencies to Cargo.toml
```

**Edit `Cargo.toml`**:
```toml
[package]
name = "qwencoder-bot"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
git2 = "0.19"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
toml = "0.8"

# Use your existing OllamaClient
# (Copy from Marvinous project or include as local module)
```

**Project structure**:
```
qwencoder-bot/
├── Cargo.toml
├── src/
│   ├── main.rs              # Main event loop
│   ├── config.rs            # Load configuration
│   ├── ollama.rs            # Ollama API client (reuse from Marvinous)
│   ├── gogs.rs              # Gog CLI wrapper
│   ├── assessment.rs        # 7B assessment logic
│   ├── implementation.rs    # 14B implementation logic
│   ├── vm.rs                # VM management
│   └── prompts.rs           # System prompts (load from PROMPTS.md)
└── config.toml.example      # Example configuration
```

**Build it**:
```bash
cargo build --release
```

## Step 7: Configure the Bot

Create bot configuration:

```bash
mkdir -p ~/.config/qwencoder-bot
nano ~/.config/qwencoder-bot/config.toml
```

**Configuration template**:
```toml
[server]
gogs_url = "https://your-gogs-server.com"
ollama_endpoint = "http://localhost:11434"

[models]
assessor = "qwen2.5-coder:7b"
coder = "qwen2.5-coder:14b"
timeout_secs = 300

[polling]
interval_secs = 60
trigger_label = "qwen-candidate"

[vm]
base_snapshot = "/path/to/qwencoder-base-snapshot.qcow2"
ram_mb = 4096
timeout_secs = 600
ssh_port = 2222

[gog_profiles]
assessor = "qwen-assessor"
coder = "qwen-coder"

[logging]
level = "info"
file = "/var/log/qwencoder-bot/bot.log"

# Optional: limit to specific repositories
# [repositories]
# repos = ["owner/repo1", "owner/repo2"]
```

## Step 8: Initial Test Run

Before running the bot automatically, do a manual test:

### Create a Test Issue

In Gogs, create a simple test issue:

```
Title: Fix off-by-one error in example function

Body:
The `count_items()` function in `src/utils.rs` has an off-by-one error 
causing it to return the wrong count. The test `test_count_items()` is 
currently failing.

Files affected:
- src/utils.rs
- tests/utils_test.rs

Labels: rust-issue, qwen-candidate
```

### Run Bot Manually (First Time)

```bash
# Set verbose logging
export RUST_LOG=debug

# Run bot once (don't loop)
./target/release/qwencoder-bot --once

# Watch the logs
tail -f /var/log/qwencoder-bot/bot.log
```

**Expected behavior**:
1. Bot detects issue with "qwen-candidate" label
2. Fetches issue details and code files
3. Calls 7B model for assessment
4. Posts assessment comment to issue
5. If assessment says "worth-attempting":
   - Spawns VM
   - Calls 14B model
   - Applies changes
   - Runs tests
   - Posts results
6. Updates labels appropriately

### Review Results

Check the Gogs issue for bot comments:

```
[QwenAssess] Assessment: worth-attempting

Confidence: 85%

Reasoning: This is a straightforward fix in a utility function. The 
test is already written and failing, which provides clear validation. 
The fix should be a simple index adjustment.

Files to modify:
- src/utils.rs (main fix)

Test strategy:
- Run existing test: cargo test test_count_items
- Verify no other tests break: cargo test
```

```
[QwenCoder] Implementation complete ✓

Branch: qwen-fix-123
Tests passing:
• cargo test: PASS (all 47 tests)
• cargo clippy: PASS (no warnings)

Changes:
• src/utils.rs: Changed `items.len()` to `items.len() - 1` in 
  return statement

Ready for review.
```

### Verify Changes

```bash
# Check out the bot's branch
git fetch origin qwen-fix-123
git checkout qwen-fix-123

# Review the diff
git diff main

# Run tests yourself
cargo test

# If good, merge to main
git checkout main
git merge qwen-fix-123
git push origin main
```

## Step 9: Run as Service

Once manual testing works, set up the bot as a systemd service:

**Create service file**:
```bash
sudo nano /etc/systemd/system/qwencoder-bot.service
```

```ini
[Unit]
Description=QwenCoder Bot - Autonomous Code Implementation
After=network.target ollama.service

[Service]
Type=simple
User=your-username
WorkingDirectory=/home/your-username/qwencoder-bot
ExecStart=/home/your-username/qwencoder-bot/target/release/qwencoder-bot
Restart=always
RestartSec=10

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=qwencoder-bot

# Environment
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
```

**Enable and start**:
```bash
sudo systemctl daemon-reload
sudo systemctl enable qwencoder-bot
sudo systemctl start qwencoder-bot

# Check status
systemctl status qwencoder-bot

# View logs
journalctl -u qwencoder-bot -f
```

## Step 10: Iterate and Tune

Now that the bot is running:

### Monitor Success Rates

Track in a spreadsheet or simple database:
- Total issues attempted
- Assessment outcomes (trivial/worth-attempting/complex)
- Implementation success rate
- False positives (tests pass but code is wrong)
- False negatives (escalated but could have been automated)

### Tune Prompts

Based on results, adjust the system prompts in `PROMPTS.md`:
- If too many false escalations → make assessment less conservative
- If too many failed implementations → make assessment more conservative
- If code quality is poor → add more constraints to implementation prompt
- If tests don't catch bugs → emphasize edge cases in prompt

### Expand Coverage

As confidence builds:
1. Add more issue labels (e.g., "documentation", "simple-feature")
2. Enable for more repositories
3. Reduce human review for high-confidence cases

## Troubleshooting

### Bot Not Detecting Issues

**Check**:
```bash
# Test gog CLI manually
gog --profile qwen-assessor issue list --label qwen-candidate --json

# Verify label exists and is spelled correctly
gog issue show <num> --repo <repo>
```

**Fix**: Ensure label matches exactly (case-sensitive).

### Ollama Connection Refused

**Check**:
```bash
# Is Ollama running?
systemctl status ollama

# Can you reach it?
curl http://localhost:11434/api/generate -d '{"model":"qwen2.5-coder:7b","prompt":"test"}'
```

**Fix**: 
```bash
sudo systemctl start ollama
```

### Model Not Found

**Check**:
```bash
ollama list
```

**Fix**:
```bash
ollama pull qwen2.5-coder:7b
ollama pull qwen2.5-coder:14b
```

### VM Won't Start

**Check**:
```bash
# Test VM manually
qemu-system-x86_64 -m 4096 -cpu host -enable-kvm \
  -hda /path/to/qwencoder-base-snapshot.qcow2
```

**Fix**: 
- Verify KVM is enabled: `lsmod | grep kvm`
- Check QEMU is installed: `qemu-system-x86_64 --version`
- Verify snapshot file exists and is readable

### Tests Fail in VM But Pass Locally

**Likely causes**:
- Missing dependencies in VM
- Network access required but not available
- Different Rust/toolchain versions

**Fix**:
- Boot VM manually and run tests
- Install missing dependencies
- Update VM base snapshot

### Bot Process Crashes

**Check logs**:
```bash
journalctl -u qwencoder-bot -n 100
```

**Common causes**:
- Config file syntax error
- Network timeout
- Out of memory

**Fix**:
- Validate config with TOML parser
- Increase timeouts in config
- Check system resources: `free -h`

## Next Steps

After successful setup:

1. **Read PROMPTS.md** - Understand and customize the system prompts
2. **Review ARCHITECTURE.md** - Understand the full system design
3. **Experiment** - Try different types of issues and track results
4. **Iterate** - Improve prompts based on what works and what doesn't
5. **Scale** - Add more repositories and issue types as confidence grows

## Getting Help

If you encounter issues during setup:

1. Check logs: `journalctl -u qwencoder-bot -f`
2. Test components individually (Ollama, gog CLI, VMs)
3. Review config file for typos
4. Try manual test first before running as service
5. Start with DEBUG logging: `RUST_LOG=debug`

## Configuration Checklist

Before declaring success, verify:

- [ ] Both Qwen models pulled and tested
- [ ] Ollama auto-unload configured (30s)
- [ ] Both bot Gogs accounts created with API tokens
- [ ] Gog CLI profiles configured and tested
- [ ] VM base snapshot created with all toolchains
- [ ] Bot built successfully (`cargo build --release`)
- [ ] Bot config file created and valid
- [ ] Manual test completed successfully
- [ ] Bot runs as systemd service
- [ ] Logs are accessible and readable
- [ ] First real issue processed correctly

Once all boxes are checked, you're ready to use the bot in production!
