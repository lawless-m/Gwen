# QwenCoder Bot - VM Setup Guide

This guide covers creating and managing the VM environment where the bot executes code changes safely.

## Why VMs?

LLM-generated code can contain:
- Bugs that crash processes
- Infinite loops
- Resource exhaustion
- Unintended file operations
- Network requests

Running code in a disposable VM ensures:
- Host system is protected
- Failed attempts are cleanly destroyed
- Each job starts from a known-good state
- Resource limits can be enforced

## VM Requirements

### Minimum Specifications
- **RAM**: 2GB (4GB recommended)
- **Disk**: 10GB (20GB recommended)
- **CPU**: 2 cores
- **OS**: Debian 12 (Bookworm) or Ubuntu 24.04

### Required Software
- Git
- Language toolchains (Rust, C#, Python, etc.)
- SSH server (for bot access)
- Test frameworks

### Network Requirements
- Access to your Gogs server (for git operations)
- No internet access (optional security measure)
- SSH accessible from bot host

## Hypervisor Options

### Option 1: QEMU/KVM (Recommended for Linux)

**Pros**:
- Fast snapshot creation/restoration
- Copy-on-write disk images (efficient storage)
- Full KVM hardware acceleration
- Scriptable via command line

**Cons**:
- Linux host required
- Requires KVM kernel module

**Installation**:
```bash
# Debian/Ubuntu
sudo apt install qemu-system-x86 qemu-kvm libvirt-daemon-system

# Enable KVM
sudo modprobe kvm
sudo modprobe kvm-intel  # or kvm-amd for AMD

# Verify KVM is available
lsmod | grep kvm
```

### Option 2: VirtualBox

**Pros**:
- Cross-platform (Linux, Windows, macOS)
- Good UI for manual management
- Well-documented

**Cons**:
- Slower snapshot operations
- Larger disk usage
- Less scriptable

**Installation**:
```bash
# Debian/Ubuntu
sudo apt install virtualbox virtualbox-ext-pack

# Verify installation
VBoxManage --version
```

### Option 3: Docker (Experimental)

**Pros**:
- Lightweight
- Very fast startup
- Easy resource limits

**Cons**:
- Shared kernel (less isolation)
- Cannot run different init systems
- May need privileged mode for full toolchains

**Not recommended for production** but viable for experimentation.

## Creating the Base VM

### Step 1: Download OS Image

**Debian 12 (Recommended)**:
```bash
wget https://cdimage.debian.org/debian-cd/current/amd64/iso-cd/debian-12.5.0-amd64-netinst.iso
```

**Ubuntu 24.04 (Alternative)**:
```bash
wget https://releases.ubuntu.com/24.04/ubuntu-24.04-live-server-amd64.iso
```

### Step 2: Create VM Disk

**QEMU/KVM**:
```bash
# Create 20GB qcow2 image
qemu-img create -f qcow2 qwencoder-base.qcow2 20G
```

**VirtualBox**:
```bash
# Create 20GB VDI image
VBoxManage createhd --filename qwencoder-base.vdi --size 20480
```

### Step 3: Install Operating System

**QEMU/KVM**:
```bash
qemu-system-x86_64 \
  -m 4096 \
  -cpu host \
  -enable-kvm \
  -cdrom debian-12.5.0-amd64-netinst.iso \
  -hda qwencoder-base.qcow2 \
  -boot d \
  -net nic \
  -net user
```

**VirtualBox**:
```bash
# Create VM
VBoxManage createvm --name qwencoder-base --ostype Debian_64 --register

# Add storage
VBoxManage storagectl qwencoder-base --name "SATA" --add sata
VBoxManage storageattach qwencoder-base --storagectl "SATA" --port 0 \
  --device 0 --type hdd --medium qwencoder-base.vdi

# Add ISO
VBoxManage storagectl qwencoder-base --name "IDE" --add ide
VBoxManage storageattach qwencoder-base --storagectl "IDE" --port 0 \
  --device 0 --type dvddrive --medium debian-12.5.0-amd64-netinst.iso

# Configure RAM and CPU
VBoxManage modifyvm qwencoder-base --memory 4096 --cpus 2

# Start installation
VBoxManage startvm qwencoder-base
```

**Installation settings**:
- Hostname: `qwencoder-vm`
- User: `qwencoder`
- Password: (generate strong password)
- Partitioning: Use entire disk, single partition
- Software: Standard system utilities, SSH server

### Step 4: Configure Base System

Once OS is installed and you've SSH'd in:

```bash
# Update system
sudo apt update
sudo apt upgrade -y

# Install essential tools
sudo apt install -y \
  git \
  curl \
  wget \
  build-essential \
  pkg-config \
  libssl-dev \
  openssh-server \
  vim \
  htop

# Configure SSH for bot access
sudo nano /etc/ssh/sshd_config
# Ensure these are set:
#   PermitRootLogin no
#   PasswordAuthentication yes (or use keys)
#   PubkeyAuthentication yes

sudo systemctl restart sshd

# Set up git identity
git config --global user.name "QwenCoder Bot"
git config --global user.email "qwen-coder@localhost"
```

### Step 5: Install Language Toolchains

#### Rust

```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install components
rustup component add clippy rustfmt rust-src

# Verify installation
rustc --version
cargo --version
clippy-driver --version
```

#### C# / .NET

```bash
# Add Microsoft package repository
wget https://packages.microsoft.com/config/debian/12/packages-microsoft-prod.deb
sudo dpkg -i packages-microsoft-prod.deb
rm packages-microsoft-prod.deb

sudo apt update
sudo apt install -y dotnet-sdk-8.0

# Verify installation
dotnet --version
```

#### Python

```bash
# Install Python and tools
sudo apt install -y \
  python3 \
  python3-pip \
  python3-venv \
  python3-dev

# Install common testing/linting tools
pip3 install --user \
  pytest \
  pytest-cov \
  mypy \
  black \
  ruff \
  pylint

# Verify installation
python3 --version
pytest --version
mypy --version
```

#### Shell Script Validation

```bash
sudo apt install -y shellcheck

# Verify
shellcheck --version
```

#### JavaScript/TypeScript (Optional)

```bash
# Install Node.js
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo bash -
sudo apt install -y nodejs

# Install common tools
sudo npm install -g \
  typescript \
  eslint \
  prettier \
  jest

# Verify
node --version
npm --version
tsc --version
```

### Step 6: Configure Git Access to Gogs

The VM needs to access your private Gogs server:

#### Option A: SSH Key (Recommended)

```bash
# Generate SSH key in VM
ssh-keygen -t ed25519 -C "qwen-coder@vm" -f ~/.ssh/gogs_key -N ""

# Display public key
cat ~/.ssh/gogs_key.pub
```

Copy this public key and add it to both bot Gogs accounts:
1. Log into Gogs as `bot-qwen-assess`
2. Settings → SSH Keys → Add Key
3. Paste public key
4. Repeat for `bot-qwen-coder`

Configure SSH in VM:
```bash
nano ~/.ssh/config
```

```
Host gogs.example.com
    HostName gogs.example.com
    User git
    IdentityFile ~/.ssh/gogs_key
    StrictHostKeyChecking no
```

Test access:
```bash
ssh -T git@gogs.example.com
```

#### Option B: HTTPS with Token

```bash
# Configure git credential helper
git config --global credential.helper store

# On first clone, you'll be prompted for username/token
# Username: bot-qwen-coder
# Password: <bot API token>
```

**Note**: Less secure than SSH keys, but simpler to set up.

### Step 7: Test Toolchains

Create test scripts to verify everything works:

**test-rust.sh**:
```bash
#!/bin/bash
set -e

echo "Testing Rust toolchain..."

# Create temp project
cd /tmp
cargo new test-project
cd test-project

# Build
cargo build

# Test
cargo test

# Lint
cargo clippy

# Format check
cargo fmt -- --check

echo "✓ Rust toolchain OK"
```

**test-dotnet.sh**:
```bash
#!/bin/bash
set -e

echo "Testing .NET toolchain..."

cd /tmp
dotnet new console -n TestProject
cd TestProject

dotnet build
dotnet test
dotnet format --verify-no-changes

echo "✓ .NET toolchain OK"
```

**test-python.sh**:
```bash
#!/bin/bash
set -e

echo "Testing Python toolchain..."

cd /tmp
mkdir test-project
cd test-project

cat > test_example.py << 'EOF'
def add(a, b):
    return a + b

def test_add():
    assert add(2, 3) == 5
EOF

python3 -m pytest test_example.py
python3 -m mypy test_example.py

echo "✓ Python toolchain OK"
```

Run all tests:
```bash
chmod +x test-*.sh
./test-rust.sh
./test-dotnet.sh
./test-python.sh
```

If all pass, the base VM is ready.

### Step 8: Clean Up and Prepare for Snapshot

```bash
# Clear bash history
history -c
rm ~/.bash_history

# Clear any cached packages
sudo apt clean

# Remove installation logs
sudo rm -rf /var/log/installer

# Clear temp files
sudo rm -rf /tmp/*

# Shutdown cleanly
sudo shutdown -h now
```

## Creating the Snapshot

### QEMU/KVM

The `.qcow2` file is already a snapshot-capable format:

```bash
# The base VM is now in qwencoder-base.qcow2
# To create ephemeral instances, use snapshot mode

# Test ephemeral instance
qemu-system-x86_64 \
  -m 4096 \
  -cpu host \
  -enable-kvm \
  -hda qwencoder-base.qcow2 \
  -snapshot  # <-- Important: changes not saved

# For persistent snapshot, create overlay
qemu-img create -f qcow2 -b qwencoder-base.qcow2 -F qcow2 job-001.qcow2
```

**Overlay disks** are copy-on-write: they store only changes from the base, so they're small and fast to create.

### VirtualBox

```bash
# Take snapshot
VBoxManage snapshot qwencoder-base take "base-v1" \
  --description "Base VM with all toolchains"

# List snapshots
VBoxManage snapshot qwencoder-base list

# Clone from snapshot for ephemeral use
VBoxManage clonevm qwencoder-base \
  --snapshot "base-v1" \
  --name "job-001" \
  --register
```

## VM Management from Bot

The bot needs to script VM operations. Here are reference implementations:

### QEMU/KVM Management Script

**vm-manager.sh**:
```bash
#!/bin/bash

BASE_IMAGE="/path/to/qwencoder-base.qcow2"
VM_DIR="/var/lib/qwencoder-bot/vms"
SSH_PORT_BASE=2222

function create_vm() {
    local job_id="$1"
    local ssh_port=$((SSH_PORT_BASE + job_id))
    local vm_image="$VM_DIR/job-${job_id}.qcow2"
    
    # Create overlay disk
    qemu-img create -f qcow2 -b "$BASE_IMAGE" -F qcow2 "$vm_image"
    
    # Start VM in background
    qemu-system-x86_64 \
      -m 4096 \
      -cpu host \
      -enable-kvm \
      -hda "$vm_image" \
      -net nic \
      -net user,hostfwd=tcp::${ssh_port}-:22 \
      -daemonize \
      -pidfile "$VM_DIR/job-${job_id}.pid"
    
    # Wait for SSH to be ready
    for i in {1..30}; do
        if ssh -p "$ssh_port" -o StrictHostKeyChecking=no \
               qwencoder@localhost "echo ready" &>/dev/null; then
            echo "VM job-${job_id} ready on port ${ssh_port}"
            return 0
        fi
        sleep 2
    done
    
    echo "VM failed to start" >&2
    return 1
}

function destroy_vm() {
    local job_id="$1"
    local pid_file="$VM_DIR/job-${job_id}.pid"
    
    if [ -f "$pid_file" ]; then
        kill $(cat "$pid_file") 2>/dev/null
        rm "$pid_file"
    fi
    
    rm -f "$VM_DIR/job-${job_id}.qcow2"
    echo "VM job-${job_id} destroyed"
}

function run_command() {
    local job_id="$1"
    local ssh_port=$((SSH_PORT_BASE + job_id))
    shift
    
    ssh -p "$ssh_port" -o StrictHostKeyChecking=no \
        qwencoder@localhost "$@"
}

case "$1" in
    create)
        create_vm "$2"
        ;;
    destroy)
        destroy_vm "$2"
        ;;
    exec)
        run_command "$2" "${@:3}"
        ;;
    *)
        echo "Usage: $0 {create|destroy|exec} job_id [command...]"
        exit 1
        ;;
esac
```

Usage from Rust bot:
```rust
use std::process::Command;

fn spawn_vm(job_id: u32) -> Result<(), Error> {
    let output = Command::new("/usr/local/bin/vm-manager.sh")
        .args(&["create", &job_id.to_string()])
        .output()?;
    
    if !output.status.success() {
        return Err(Error::VMSpawnFailed);
    }
    
    Ok(())
}

fn run_in_vm(job_id: u32, command: &str) -> Result<String, Error> {
    let output = Command::new("/usr/local/bin/vm-manager.sh")
        .args(&["exec", &job_id.to_string(), "bash", "-c", command])
        .output()?;
    
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn destroy_vm(job_id: u32) -> Result<(), Error> {
    let output = Command::new("/usr/local/bin/vm-manager.sh")
        .args(&["destroy", &job_id.to_string()])
        .output()?;
    
    Ok(())
}
```

## Resource Limits

Prevent VMs from consuming all host resources:

### CPU Limits

**QEMU**:
```bash
# Limit to 2 CPUs
qemu-system-x86_64 -smp 2 ...
```

**cgroups** (systemd):
```bash
# Create cgroup for VM processes
sudo systemctl set-property qemu-system-x86_64 CPUQuota=200%
```

### Memory Limits

Already set by `-m` parameter in QEMU (4096MB in examples).

### Disk I/O Limits

**QEMU**:
```bash
# Limit disk throughput
qemu-system-x86_64 \
  -drive file=vm.qcow2,throttling.iops-total=1000 ...
```

### Network Limits

For enhanced security:
```bash
# Restrict to only Gogs server
# Use iptables or nftables to block all except:
# - Outbound SSH to Gogs (port 22)
# - Outbound HTTPS to Gogs (port 443)
```

## Monitoring VMs

Track VM resource usage:

```bash
#!/bin/bash
# vm-monitor.sh

while true; do
    for pid_file in /var/lib/qwencoder-bot/vms/*.pid; do
        if [ -f "$pid_file" ]; then
            pid=$(cat "$pid_file")
            ps -p "$pid" -o pid,pcpu,pmem,etime,cmd --no-headers || echo "Dead: $pid_file"
        fi
    done
    sleep 10
done
```

## Troubleshooting

### VM Won't Start

**Check KVM**:
```bash
lsmod | grep kvm
# Should show kvm_intel or kvm_amd
```

**Check permissions**:
```bash
# User must be in kvm group
sudo usermod -a -G kvm $USER
# Log out and back in
```

**Check disk image**:
```bash
qemu-img info qwencoder-base.qcow2
# Verify format and virtual size
```

### SSH Connection Refused

**Check VM is running**:
```bash
ps aux | grep qemu
```

**Check port forwarding**:
```bash
netstat -tlnp | grep 2222
```

**Test from VM console** (if you can access it):
```bash
sudo systemctl status sshd
```

### Tests Fail in VM But Pass Locally

**Check toolchain versions**:
```bash
# In VM
rustc --version
dotnet --version
python3 --version

# Compare to your local versions
```

**Check dependencies**:
```bash
# Some projects need external services
# E.g., database, redis, etc.
```

**Check file paths**:
```bash
# Absolute paths in tests may differ
```

### VM Performance is Slow

**Enable KVM acceleration**:
```bash
# Verify KVM is being used
ps aux | grep qemu | grep -i kvm
```

**Increase RAM**:
```bash
# Change -m 4096 to -m 8192
```

**Use virtio drivers**:
```bash
qemu-system-x86_64 \
  -drive file=vm.qcow2,if=virtio \
  -net nic,model=virtio \
  ...
```

## Maintenance

### Updating Base Snapshot

Periodically update toolchains:

```bash
# Boot base VM (not in snapshot mode)
qemu-system-x86_64 -m 4096 -cpu host -enable-kvm -hda qwencoder-base.qcow2

# Inside VM:
sudo apt update && sudo apt upgrade -y
rustup update
dotnet tool update --global

# Test everything still works
./test-rust.sh
./test-dotnet.sh

# Shutdown
sudo shutdown -h now

# Back up old version
cp qwencoder-base.qcow2 qwencoder-base-v1.qcow2

# New version is now the active base
```

### Cleaning Up Stale VMs

```bash
#!/bin/bash
# cleanup-vms.sh

# Kill any VMs older than 1 hour
find /var/lib/qwencoder-bot/vms -name "*.pid" -mmin +60 | while read pid_file; do
    echo "Cleaning up stale VM: $pid_file"
    pid=$(cat "$pid_file")
    kill -9 "$pid" 2>/dev/null
    rm "$pid_file"
    rm "${pid_file%.pid}.qcow2"
done
```

Run as cron job:
```cron
*/15 * * * * /usr/local/bin/cleanup-vms.sh
```

## Security Hardening

### Minimal VM Privileges

- No sudo access for qwencoder user
- Read-only base image
- No outbound internet (except Gogs)
- No access to host filesystem

### Audit Logging

Enable comprehensive logging in VM:

```bash
# /etc/rsyslog.conf
*.* @@bot-host:514  # Forward all logs to bot host
```

On bot host, collect VM logs:
```bash
# /etc/rsyslog.conf
$ModLoad imudp
$UDPServerRun 514
```

### Network Isolation

Use separate network namespace:
```bash
# Create isolated network
sudo ip netns add qwencoder-net

# Run VM in namespace
sudo ip netns exec qwencoder-net qemu-system-x86_64 ...
```

## Next Steps

With VMs configured:
1. Test VM creation/destruction from bot
2. Verify toolchains work for your projects
3. Test git access to your Gogs server
4. Run end-to-end test with known issue
5. Monitor resource usage under load

See `IMPLEMENTATION_GUIDE.md` for integrating VMs with the bot.
