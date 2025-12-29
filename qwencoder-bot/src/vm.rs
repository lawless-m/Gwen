//! VM management module for QwenCoder Bot
//!
//! Handles creating, managing, and destroying ephemeral VMs for code execution.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use crate::config::VmConfig;

/// Represents an active VM instance
pub struct VmInstance {
    pub job_id: u32,
    pub ssh_port: u16,
    pub disk_image: PathBuf,
    pub pid_file: PathBuf,
    ssh_user: String,
    ssh_key: Option<PathBuf>,
}

/// VM Manager for creating and destroying VMs
pub struct VmManager {
    config: VmConfig,
    vm_dir: PathBuf,
    active_vms: Vec<u32>,
}

/// Result of running a command in VM
#[derive(Debug)]
pub struct VmCommandResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl VmManager {
    /// Create a new VM manager
    pub fn new(config: VmConfig) -> Result<Self> {
        let vm_dir = PathBuf::from("/var/lib/qwencoder-bot/vms");

        // Ensure VM directory exists
        std::fs::create_dir_all(&vm_dir)
            .with_context(|| format!("Failed to create VM directory: {:?}", vm_dir))?;

        Ok(Self {
            config,
            vm_dir,
            active_vms: Vec::new(),
        })
    }

    /// Create a new VM instance
    pub async fn create_vm(&mut self, job_id: u32) -> Result<VmInstance> {
        // Check concurrent VM limit
        if self.active_vms.len() >= self.config.limits.max_concurrent_vms as usize {
            anyhow::bail!(
                "Maximum concurrent VMs reached ({})",
                self.config.limits.max_concurrent_vms
            );
        }

        let ssh_port = self.config.ssh_port_base + job_id as u16;
        let disk_image = self.vm_dir.join(format!("job-{}.qcow2", job_id));
        let pid_file = self.vm_dir.join(format!("job-{}.pid", job_id));

        info!("Creating VM for job {} on port {}", job_id, ssh_port);

        // Create overlay disk image
        self.create_overlay_disk(&disk_image)?;

        // Start the VM
        self.start_vm(job_id, &disk_image, &pid_file, ssh_port)?;

        // Wait for SSH to be ready
        self.wait_for_ssh(ssh_port, Duration::from_secs(60)).await?;

        self.active_vms.push(job_id);

        let instance = VmInstance {
            job_id,
            ssh_port,
            disk_image,
            pid_file,
            ssh_user: self.config.ssh_user.clone(),
            ssh_key: self.config.ssh_key.clone(),
        };

        info!("VM for job {} is ready", job_id);
        Ok(instance)
    }

    /// Create an overlay disk from the base snapshot
    fn create_overlay_disk(&self, disk_path: &Path) -> Result<()> {
        debug!("Creating overlay disk: {:?}", disk_path);

        let output = Command::new("qemu-img")
            .args([
                "create",
                "-f", "qcow2",
                "-b", self.config.base_snapshot.to_str().unwrap(),
                "-F", "qcow2",
                disk_path.to_str().unwrap(),
            ])
            .output()
            .context("Failed to execute qemu-img")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create overlay disk: {}", stderr);
        }

        Ok(())
    }

    /// Start a VM instance
    fn start_vm(
        &self,
        job_id: u32,
        disk_image: &Path,
        pid_file: &Path,
        ssh_port: u16,
    ) -> Result<()> {
        debug!("Starting VM for job {}", job_id);

        let mut args = vec![
            "-m".to_string(),
            format!("{}", self.config.ram_mb),
            "-smp".to_string(),
            format!("{}", self.config.qemu.cpu_cores),
            "-hda".to_string(),
            disk_image.to_str().unwrap().to_string(),
            "-net".to_string(),
            "nic".to_string(),
            "-net".to_string(),
            format!("user,hostfwd=tcp::{}-:22", ssh_port),
            "-daemonize".to_string(),
            "-pidfile".to_string(),
            pid_file.to_str().unwrap().to_string(),
            "-display".to_string(),
            "none".to_string(),
        ];

        if self.config.qemu.enable_kvm {
            args.insert(0, "-enable-kvm".to_string());
            args.insert(1, "-cpu".to_string());
            args.insert(2, "host".to_string());
        }

        let output = Command::new("qemu-system-x86_64")
            .args(&args)
            .output()
            .context("Failed to start QEMU")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to start VM: {}", stderr);
        }

        Ok(())
    }

    /// Wait for SSH to be available on the VM
    async fn wait_for_ssh(&self, port: u16, max_wait: Duration) -> Result<()> {
        debug!("Waiting for SSH on port {}", port);

        let start = std::time::Instant::now();
        let check_interval = Duration::from_secs(2);

        while start.elapsed() < max_wait {
            if self.check_ssh(port) {
                return Ok(());
            }
            tokio::time::sleep(check_interval).await;
        }

        anyhow::bail!("SSH not available after {:?}", max_wait)
    }

    /// Check if SSH is available
    fn check_ssh(&self, port: u16) -> bool {
        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(),
            "ConnectTimeout=5".to_string(),
            "-p".to_string(),
            format!("{}", port),
        ];

        if let Some(key) = &self.config.ssh_key {
            args.push("-i".to_string());
            args.push(key.to_str().unwrap().to_string());
        }

        args.push(format!("{}@localhost", self.config.ssh_user));
        args.push("echo".to_string());
        args.push("ready".to_string());

        let output = Command::new("ssh")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        matches!(output, Ok(status) if status.success())
    }

    /// Destroy a VM instance
    pub fn destroy_vm(&mut self, instance: VmInstance) -> Result<()> {
        info!("Destroying VM for job {}", instance.job_id);

        // Kill the VM process
        if instance.pid_file.exists() {
            if let Ok(pid_str) = std::fs::read_to_string(&instance.pid_file) {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    let _ = Command::new("kill")
                        .args(["-9", &format!("{}", pid)])
                        .status();
                }
            }
            let _ = std::fs::remove_file(&instance.pid_file);
        }

        // Remove disk image
        if instance.disk_image.exists() {
            let _ = std::fs::remove_file(&instance.disk_image);
        }

        // Remove from active VMs
        self.active_vms.retain(|&id| id != instance.job_id);

        info!("VM for job {} destroyed", instance.job_id);
        Ok(())
    }

    /// Clean up stale VMs
    pub fn cleanup_stale(&mut self, max_age: Duration) -> Result<()> {
        info!("Cleaning up stale VMs older than {:?}", max_age);

        let entries = std::fs::read_dir(&self.vm_dir)?;
        let now = std::time::SystemTime::now();

        for entry in entries.flatten() {
            let path = entry.path();

            if let Some(ext) = path.extension() {
                if ext == "pid" {
                    if let Ok(metadata) = entry.metadata() {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(age) = now.duration_since(modified) {
                                if age > max_age {
                                    warn!("Cleaning up stale VM: {:?}", path);

                                    // Kill process
                                    if let Ok(pid_str) = std::fs::read_to_string(&path) {
                                        if let Ok(pid) = pid_str.trim().parse::<i32>() {
                                            let _ = Command::new("kill")
                                                .args(["-9", &format!("{}", pid)])
                                                .status();
                                        }
                                    }

                                    // Remove files
                                    let _ = std::fs::remove_file(&path);
                                    let qcow2 = path.with_extension("qcow2");
                                    let _ = std::fs::remove_file(&qcow2);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl VmInstance {
    /// Run a command in the VM
    pub async fn run_command(
        &self,
        command: &str,
        timeout_secs: u64,
    ) -> Result<VmCommandResult> {
        debug!("Running command in VM: {}", command);

        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(),
            "BatchMode=yes".to_string(),
            "-p".to_string(),
            format!("{}", self.ssh_port),
        ];

        if let Some(key) = &self.ssh_key {
            args.push("-i".to_string());
            args.push(key.to_str().unwrap().to_string());
        }

        args.push(format!("{}@localhost", self.ssh_user));
        args.push("bash".to_string());
        args.push("-c".to_string());
        args.push(command.to_string());

        let child = Command::new("ssh")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn SSH command")?;

        // Wait with timeout
        let output = timeout(
            Duration::from_secs(timeout_secs),
            tokio::task::spawn_blocking(move || child.wait_with_output()),
        )
        .await
        .context("Command timed out")??
        .context("Failed to get command output")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);
        let success = output.status.success();

        debug!(
            "Command completed: exit_code={}, stdout_len={}, stderr_len={}",
            exit_code,
            stdout.len(),
            stderr.len()
        );

        Ok(VmCommandResult {
            exit_code,
            stdout,
            stderr,
            success,
        })
    }

    /// Run multiple commands in sequence
    pub async fn run_commands(
        &self,
        commands: &[String],
        timeout_secs: u64,
    ) -> Result<Vec<VmCommandResult>> {
        let mut results = Vec::new();

        for cmd in commands {
            let result = self.run_command(cmd, timeout_secs).await?;
            let success = result.success;
            results.push(result);

            // Stop on first failure
            if !success {
                break;
            }
        }

        Ok(results)
    }

    /// Copy a file to the VM
    pub async fn copy_to(&self, local_path: &Path, remote_path: &str) -> Result<()> {
        debug!("Copying {:?} to VM:{}", local_path, remote_path);

        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-P".to_string(),
            format!("{}", self.ssh_port),
        ];

        if let Some(key) = &self.ssh_key {
            args.push("-i".to_string());
            args.push(key.to_str().unwrap().to_string());
        }

        args.push(local_path.to_str().unwrap().to_string());
        args.push(format!("{}@localhost:{}", self.ssh_user, remote_path));

        let output = Command::new("scp")
            .args(&args)
            .output()
            .context("Failed to execute scp")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to copy file: {}", stderr);
        }

        Ok(())
    }

    /// Copy a file from the VM
    pub async fn copy_from(&self, remote_path: &str, local_path: &Path) -> Result<()> {
        debug!("Copying VM:{} to {:?}", remote_path, local_path);

        let mut args = vec![
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-P".to_string(),
            format!("{}", self.ssh_port),
        ];

        if let Some(key) = &self.ssh_key {
            args.push("-i".to_string());
            args.push(key.to_str().unwrap().to_string());
        }

        args.push(format!("{}@localhost:{}", self.ssh_user, remote_path));
        args.push(local_path.to_str().unwrap().to_string());

        let output = Command::new("scp")
            .args(&args)
            .output()
            .context("Failed to execute scp")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to copy file: {}", stderr);
        }

        Ok(())
    }

    /// Write content to a file in the VM
    pub async fn write_file(&self, remote_path: &str, content: &str) -> Result<()> {
        // Create a temp file locally
        let temp_file = std::env::temp_dir().join(format!("vm-write-{}", self.job_id));
        std::fs::write(&temp_file, content)?;

        // Copy to VM
        self.copy_to(&temp_file, remote_path).await?;

        // Clean up
        let _ = std::fs::remove_file(&temp_file);

        Ok(())
    }

    /// Read content from a file in the VM
    pub async fn read_file(&self, remote_path: &str) -> Result<String> {
        let result = self.run_command(&format!("cat '{}'", remote_path), 30).await?;

        if !result.success {
            anyhow::bail!("Failed to read file: {}", result.stderr);
        }

        Ok(result.stdout)
    }

    /// Clone a git repository in the VM
    pub async fn git_clone(&self, url: &str, target: &str) -> Result<()> {
        let result = self
            .run_command(&format!("git clone '{}' '{}'", url, target), 120)
            .await?;

        if !result.success {
            anyhow::bail!("Git clone failed: {}", result.stderr);
        }

        Ok(())
    }

    /// Create a branch in the VM repository
    pub async fn git_checkout_branch(&self, repo_dir: &str, branch: &str) -> Result<()> {
        let result = self
            .run_command(
                &format!("cd '{}' && git checkout -b '{}'", repo_dir, branch),
                30,
            )
            .await?;

        if !result.success {
            anyhow::bail!("Git checkout failed: {}", result.stderr);
        }

        Ok(())
    }

    /// Commit changes in the VM
    pub async fn git_commit(&self, repo_dir: &str, message: &str) -> Result<()> {
        // Stage all changes
        let result = self
            .run_command(&format!("cd '{}' && git add -A", repo_dir), 30)
            .await?;

        if !result.success {
            anyhow::bail!("Git add failed: {}", result.stderr);
        }

        // Commit with escaped message
        let escaped_message = message.replace('\'', "'\\''");
        let result = self
            .run_command(
                &format!("cd '{}' && git commit -m '{}'", repo_dir, escaped_message),
                30,
            )
            .await?;

        if !result.success {
            anyhow::bail!("Git commit failed: {}", result.stderr);
        }

        Ok(())
    }

    /// Push changes from the VM
    pub async fn git_push(&self, repo_dir: &str, branch: &str) -> Result<()> {
        let result = self
            .run_command(
                &format!("cd '{}' && git push -u origin '{}'", repo_dir, branch),
                120,
            )
            .await?;

        if !result.success {
            anyhow::bail!("Git push failed: {}", result.stderr);
        }

        Ok(())
    }
}

/// Check if QEMU/KVM is available
pub fn check_qemu_available() -> bool {
    Command::new("qemu-system-x86_64")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Check if KVM is available
pub fn check_kvm_available() -> bool {
    Path::new("/dev/kvm").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_qemu_available() {
        // Just ensure it doesn't panic
        let _ = check_qemu_available();
    }

    #[test]
    fn test_check_kvm_available() {
        let _ = check_kvm_available();
    }
}
