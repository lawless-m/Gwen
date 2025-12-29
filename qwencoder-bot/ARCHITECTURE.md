# QwenCoder Bot - Architecture

## System Components

### 1. Main Bot Process (Rust)

**Purpose**: Orchestrates the entire workflow from issue detection to result reporting.

**Responsibilities**:
- Polls Gogs API for issues with trigger label
- Fetches issue details and related code files
- Calls Ollama API for LLM inference
- Manages VM lifecycle
- Updates Gogs via `gog` CLI
- Logs all operations for debugging

**Key Dependencies**:
```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
git2 = "0.19"  # For repository operations
```

**Configuration File** (`~/.config/qwencoder-bot/config.toml`):
```toml
[server]
gogs_url = "https://gogs.example.com"
ollama_endpoint = "http://localhost:11434"

[models]
assessor = "qwen2.5-coder:7b"
coder = "qwen2.5-coder:14b"
timeout_secs = 300

[polling]
interval_secs = 60
trigger_label = "qwen-candidate"

[vm]
base_snapshot = "qwencoder-base"
ram_mb = 4096
timeout_secs = 600

[gog_profiles]
assessor = "qwen-assessor"
coder = "qwen-coder"

[repositories]
# Repositories to monitor (optional, defaults to all accessible)
# repos = ["owner/repo1", "owner/repo2"]
```

### 2. Ollama Server (Existing)

**Purpose**: Serves Qwen 2.5 Coder models via REST API.

**Models Required**:
- `qwen2.5-coder:7b` - Assessment (4-5GB VRAM quantized)
- `qwen2.5-coder:14b` - Implementation (8-9GB VRAM quantized)

**Configuration** (`/etc/systemd/system/ollama.service`):
```ini
[Service]
Environment="OLLAMA_HOST=0.0.0.0:11434"
Environment="OLLAMA_KEEP_ALIVE=30s"
Environment="OLLAMA_MAX_LOADED_MODELS=1"
```

**Note**: With `KEEP_ALIVE=30s`, models auto-unload after 30 seconds, freeing VRAM for the next model.

### 3. Gog CLI (Existing)

**Purpose**: Interface to Gogs API for issue management.

**Bot Profiles** (`~/.config/gogs-cli/config.toml`):
```toml
[profiles.qwen-assessor]
gogs_user = "bot-qwen-assess"
token = "..." 
role = "Issue Assessment"
signature = "[QwenAssess]"

[profiles.qwen-coder]
gogs_user = "bot-qwen-coder"
token = "..."
role = "Code Implementation"
signature = "[QwenCoder]"
```

**Required Permissions**:
- Read issues (all monitored repos)
- Write comments
- Manage labels
- Create branches (for PRs)

### 4. VM Environment

**Purpose**: Isolated execution environment for code changes and testing.

**Base Snapshot Requirements**:
- Linux (Debian preferred)
- Git
- Rust toolchain (rustc, cargo, clippy)
- C# toolchain (dotnet SDK)
- Python (pytest, mypy)
- Shell script validators (shellcheck)

**Network Configuration**:
- Access to private Gogs server
- No internet access (optional security measure)
- SSH access for bot process

**Snapshot Strategy**:
```
qwencoder-base (read-only base)
    ├── job-001 (ephemeral, destroyed after completion)
    ├── job-002 (ephemeral, destroyed after completion)
    └── job-003 (ephemeral, destroyed after completion)
```

## Data Flow

### Issue Detection and Fetching

```
┌──────────────┐
│ Bot Process  │
│ (every 60s)  │
└──────┬───────┘
       │
       ├─────► gog issue list --label qwen-candidate --json
       │
       ├─────► Parse JSON, extract issue numbers
       │
       └─────► For each issue:
               gog issue show <num> --repo <repo> --json
               └─► Extract: title, body, labels, comments, repo
```

### Repository and Code Fetching

```
┌──────────────┐
│ Bot Process  │
└──────┬───────┘
       │
       ├─────► Identify repository from issue
       │
       ├─────► Clone or update local cache:
       │       git clone <gogs-repo-url>
       │
       ├─────► Identify relevant files:
       │       • Parse issue for file mentions
       │       • Search codebase for related files
       │       • Include tests for affected modules
       │
       └─────► Read file contents into context
```

### Assessment Phase (7B Model)

```
┌──────────────┐
│ Bot Process  │
└──────┬───────┘
       │
       ├─────► Build assessment prompt:
       │       • Issue description
       │       • Relevant code files
       │       • Repository structure context
       │
       ├─────► POST to Ollama API:
       │       {
       │         "model": "qwen2.5-coder:7b",
       │         "prompt": "<assessment_prompt>",
       │         "stream": false
       │       }
       │
       ├─────► Parse response:
       │       {
       │         "decision": "worth-attempting" | "too-complex" | "trivial",
       │         "reasoning": "...",
       │         "confidence": 0-100,
       │         "affected_files": [...],
       │         "test_strategy": "..."
       │       }
       │
       └─────► Update Gogs:
               gog --profile qwen-assessor issue comment <num> \
                 "[QwenAssess] Assessment: <decision>\n<reasoning>"
               
               If too-complex:
                   gog issue label <num> needs-claude-code
               If worth-attempting:
                   gog issue label <num> qwen-in-progress
```

### Implementation Phase (14B Model)

```
┌──────────────┐
│ Bot Process  │
└──────┬───────┘
       │
       ├─────► Spawn VM from base snapshot
       │       VM ID: job-<timestamp>-<issue-num>
       │
       ├─────► Inside VM:
       │       • Clone repository
       │       • Create working branch: qwen-fix-<issue-num>
       │       • Checkout branch
       │
       ├─────► Build implementation prompt:
       │       • Issue description
       │       • Assessment recommendations
       │       • Full code context
       │       • Test requirements
       │
       ├─────► POST to Ollama API:
       │       {
       │         "model": "qwen2.5-coder:14b",
       │         "prompt": "<implementation_prompt>",
       │         "stream": false
       │       }
       │
       ├─────► Parse response:
       │       {
       │         "files": [
       │           {"path": "src/parser.rs", "content": "..."},
       │           {"path": "tests/parser_test.rs", "content": "..."}
       │         ],
       │         "commit_message": "...",
       │         "test_commands": ["cargo test", "cargo clippy"]
       │       }
       │
       ├─────► Apply changes in VM:
       │       • Write files
       │       • Stage changes: git add <files>
       │
       ├─────► Run tests in VM:
       │       For each test command:
       │           • Execute command
       │           • Capture stdout/stderr
       │           • Record exit code
       │
       └─────► Process results:
               If all tests pass:
                   • Commit: git commit -m "<message>"
                   • Push: git push origin qwen-fix-<issue-num>
                   • Comment success on Gogs
                   • Label: needs-review
               If tests fail:
                   • Comment failure with logs
                   • Label: needs-claude-code
               
               Always:
                   • Destroy VM snapshot
```

### Result Reporting

```
┌──────────────┐
│ Bot Process  │
└──────┬───────┘
       │
       └─────► Update Gogs issue:
       
               Success case:
               gog --profile qwen-coder issue comment <num> \
                 "[QwenCoder] Implementation complete ✓
                  Branch: qwen-fix-<num>
                  Tests passing:
                  • cargo test: PASS
                  • cargo clippy: PASS
                  
                  Changes:
                  • src/parser.rs: Fixed off-by-one error
                  • tests/parser_test.rs: Added edge case test
                  
                  Ready for review."
               
               gog issue label <num> needs-review
               gog issue unlabel <num> qwen-in-progress
               
               Failure case:
               gog --profile qwen-coder issue comment <num> \
                 "[QwenCoder] Implementation failed ✗
                  
                  Error: Tests failed
                  
                  cargo test output:
                  <error log>
                  
                  Escalating to human developer."
               
               gog issue label <num> needs-claude-code
               gog issue unlabel <num> qwen-in-progress
```

## Error Handling

### Network Errors

**Ollama Timeout**:
```rust
match client.generate(prompt).await {
    Ok(response) => process_response(response),
    Err(OllamaError::RequestError(_)) => {
        log::error!("Ollama request failed, retrying...");
        // Retry with exponential backoff
    }
    Err(e) => {
        log::error!("Unrecoverable error: {}", e);
        // Comment on issue and escalate
    }
}
```

**Gogs API Failure**:
```rust
// If gog CLI fails (network, auth, etc.)
// Log error, skip this polling cycle, retry next cycle
// Never crash the bot process
```

### VM Failures

**VM Won't Start**:
- Log error
- Comment on issue: "VM spawn failed"
- Add "infrastructure-issue" label
- Move to next issue

**VM Timeout**:
- Kill VM after configured timeout (default 600s)
- Comment: "Implementation timed out"
- Escalate to human

**VM Destroyed Unexpectedly**:
- Treat as failure
- Log diagnostics
- Comment on issue with error details

### Malformed LLM Responses

**Invalid JSON**:
```rust
match serde_json::from_str::<AssessmentResult>(&response) {
    Ok(result) => result,
    Err(_) => {
        // Try to extract key information with regex/parsing
        // If that fails, comment: "Could not parse model response"
        // Escalate
    }
}
```

**Missing Required Fields**:
- Use sensible defaults where possible
- If critical field missing, treat as failure and escalate

### Code Won't Compile

**Build Errors**:
- Capture full error output
- Include in failure comment
- Label: "needs-claude-code"

**Test Failures**:
- Same as build errors
- Include test output in comment

## Security Considerations

### VM Isolation

**Why**: LLM-generated code could contain:
- Infinite loops
- Resource exhaustion
- Accidental data deletion
- Network attacks

**How**: All code execution in disposable VMs with:
- No access to host filesystem
- Limited network access (only Gogs)
- CPU/RAM limits enforced
- Automatic timeout and termination

### API Token Security

**Gog Profiles**:
- Bot accounts have minimal permissions
- Cannot merge to main branches
- Cannot delete repositories
- Can only comment and label

**Storage**:
- Tokens in config file with restricted permissions (600)
- Never logged or printed
- Separate tokens for assessor and coder profiles

### Code Review Requirement

**No Auto-Merge**:
- All bot changes require human review
- PRs created but not merged
- "needs-review" label mandates approval

## Performance Characteristics

### Timing Estimates

**Per Issue Processing** (Rust project, simple fix):
- Issue detection: < 1s
- Repository clone/update: 2-5s
- Assessment (7B): 10-20s (includes model load)
- VM spawn: 3-5s
- Implementation (14B): 30-60s (includes model load)
- Test execution: 5-30s (depends on test suite)
- Result reporting: 1-2s

**Total**: 1-2 minutes for simple fixes

**Bottlenecks**:
- LLM inference time (largest component)
- Test suite execution time
- VM spawn/destroy overhead

### Throughput

**Sequential Processing**:
- With 60s polling interval
- Average 2 minutes per issue
- ~25-30 issues per hour (single bot instance)

**Scaling**:
- Multiple bot instances possible
- Each needs own VM pool
- Share same Ollama server (queued inference)

### Resource Usage

**Steady State**:
- Bot process: ~50MB RAM, minimal CPU
- Ollama: Loaded model in VRAM, minimal CPU
- VM: 2-4GB RAM per active job

**Peak**:
- During inference: High GPU utilization
- During test runs: High CPU in VM
- During git operations: Disk I/O spike

## Logging and Observability

### Log Levels

**ERROR**: Critical failures requiring attention
- VM infrastructure failures
- Ollama server down
- Gogs authentication failures

**WARN**: Issues that don't stop operation
- Single issue processing failure
- Timeout on specific job
- Malformed LLM response

**INFO**: Normal operations
- Issue detected and queued
- Assessment completed
- Implementation started
- Tests running
- Results posted

**DEBUG**: Detailed diagnostics
- Full prompts sent to LLM
- Complete LLM responses
- Git operations
- VM commands executed

### Log Files

**Location**: `/var/log/qwencoder-bot/`

**Files**:
- `bot.log` - Main bot process log
- `assessments.log` - All assessment decisions
- `implementations.log` - All implementation attempts
- `vm-<job-id>.log` - Per-VM execution logs

**Rotation**: Daily, keep 30 days

### Metrics for Monitoring

**Track**:
- Issues processed per hour
- Assessment outcomes (trivial/worth-attempting/complex)
- Implementation success rate
- Average time per phase
- VM spawn failures
- Ollama API errors
- Model load times

**Store**: JSON metrics file or simple database for analysis

## Future Enhancements

### Multi-Model Strategy
- Use different models for different languages
- Specialized fine-tuned models for specific codebases

### Feedback Loop
- Track which assessments led to successful implementations
- Adjust assessment prompts based on historical accuracy

### Parallel Processing
- Multiple VMs processing different issues simultaneously
- Queue management for Ollama API

### Cost Tracking
- GPU time per issue
- Success rate per issue type
- ROI analysis (bot time vs human time)

### Enhanced Assessment
- Static analysis integration (clippy, pylint)
- Complexity metrics
- Code coverage analysis
- Historical success patterns

## Integration Points

### Required Existing Systems
- Gogs server with API access
- Gog CLI installed and configured
- Ollama server with qwen2.5-coder models
- VM hypervisor (QEMU/KVM, VirtualBox, or similar)

### Optional Integrations
- Metrics dashboard (Grafana)
- Alert system (for repeated failures)
- Code quality tools (SonarQube)
- CI/CD pipeline hooks
