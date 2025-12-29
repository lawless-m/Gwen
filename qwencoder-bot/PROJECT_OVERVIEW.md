# QwenCoder Bot - Project Overview

## What Is This?

QwenCoder Bot is an autonomous AI coding agent that integrates with your existing Gogs multi-agent development workflow. It uses local Qwen 2.5 Coder models via Ollama to automatically implement fixes and features from issue tickets, operating as another profile in your `gog` CLI system.

## Core Concept

The bot operates as a two-tier autonomous agent:

1. **Assessment Tier (7B model)** - Fast triage to determine if an issue is straightforward enough to attempt
2. **Implementation Tier (14B model)** - Attempts the actual code changes, runs tests, and reports results

If both tiers fail or the issue is too complex, it escalates to Claude Code (you) for manual intervention.

## How It Works

```
┌─────────────────────────────────────────────────────────────┐
│ Gogs Issue Tracker                                          │
│ Issue #42: "Fix off-by-one error in parser.rs"             │
│ Labels: [rust-issue, qwen-candidate]                        │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│ QwenCoder Bot (Main Process)                                │
│ • Polls for issues with "qwen-candidate" label              │
│ • Fetches issue details and relevant code files             │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│ Assessment Phase (qwen2.5-coder:7b via Ollama)              │
│ • Reads issue description                                   │
│ • Examines affected code files                              │
│ • Decides: "trivial", "worth-attempting", or "too-complex"  │
│ • Posts assessment as comment via gog CLI                   │
└─────────────────────────────────────────────────────────────┘
                           │
                ┌──────────┴──────────┐
                │                     │
        "too-complex"          "worth-attempting"
                │                     │
                ▼                     ▼
┌───────────────────────┐  ┌─────────────────────────────────┐
│ Escalate              │  │ Implementation Phase (14B)       │
│ • Add label:          │  │ • Spawns clean VM snapshot       │
│   "needs-claude-code" │  │ • Checks out code                │
│ • Comment: reason     │  │ • Applies changes                │
└───────────────────────┘  │ • Runs tests (cargo test, etc.) │
                           │ • Commits if tests pass          │
                           └──────────┬──────────────────────┘
                                      │
                        ┌─────────────┴─────────────┐
                        │                           │
                   Tests Pass                  Tests Fail
                        │                           │
                        ▼                           ▼
        ┌───────────────────────────┐  ┌──────────────────────┐
        │ Success                   │  │ Failure              │
        │ • Create branch/PR        │  │ • Comment: error log │
        │ • Label: "needs-review"   │  │ • Add label:         │
        │ • Comment: summary        │  │   "needs-claude-code"│
        └───────────────────────────┘  └──────────────────────┘
```

## Key Design Principles

### Human-in-the-Loop (Initially)

The bot doesn't automatically merge anything. It:
- Comments its assessment and reasoning
- Creates branches/PRs for review
- Labels issues for human attention
- Provides detailed logs for debugging

You review all changes before they go into main branches.

### Conservative Escalation

The 7B assessment model errs on the side of caution. Better to escalate unnecessarily than waste 14B model time on impossible tasks.

### Language-Aware

The bot understands different toolchains:
- **Rust**: `cargo test`, `cargo clippy`, `cargo check`
- **C#**: `dotnet test`, `dotnet build`
- **Python**: `pytest`, `mypy`
- **Shell scripts**: `shellcheck`, basic syntax validation

It adjusts test execution based on detected project type.

### Isolated Execution

All code changes happen in disposable VM snapshots. Failed attempts are simply destroyed. No risk to your main development environment.

## Integration with Existing Infrastructure

### Uses Your Gog CLI

The bot operates as standard `gog` profiles:

```toml
[profiles.qwen-assessor]
gogs_user = "bot-qwen-assess"
token = "assessor-token"
role = "Issue Assessment"
signature = "[QwenAssess]"

[profiles.qwen-coder]
gogs_user = "bot-qwen-coder"
token = "coder-token"
role = "Code Implementation"
signature = "[QwenCoder]"
```

It follows the same patterns as your opus-planning and sonnet-backend agents.

### Uses Your Ollama Setup

The bot connects to your existing Ollama server running on the 3090. It leverages the automatic VRAM management you've already configured via `OLLAMA_KEEP_ALIVE`.

No new infrastructure needed—it's just another client of your existing services.

## What Gets Automated?

### Good Candidates for Automation

- Simple bug fixes (off-by-one errors, null checks, typos)
- Adding straightforward tests to untested code
- Implementing functions that follow existing patterns
- Documentation updates
- Refactoring that doesn't change behavior
- Configuration changes

### Escalates to Human

- Architecture decisions
- API design changes
- Complex algorithms
- Security-sensitive code
- Anything requiring domain knowledge
- Ambiguous requirements

## Resource Requirements

### GPU (NVIDIA 3090 - 24GB VRAM)

With `OLLAMA_KEEP_ALIVE=30s`:
- **7B model**: ~5GB VRAM (quantized), loads in 2-3 seconds
- **14B model**: ~9GB VRAM (quantized), loads in 5-6 seconds
- Models auto-unload after 30 seconds of inactivity
- Cannot run both simultaneously, but sequential operation works fine

### CPU/RAM (Host System)

- **Minimum**: 8GB RAM for bot process
- **Recommended**: 16GB+ for comfortable operation
- CPU usage is minimal (mostly waiting on Ollama)

### VM Environment

- **Per-job VM**: 2-4GB RAM, 10-20GB disk
- Snapshots created from base image with toolchains
- Destroyed after job completion (success or failure)

## Workflow Stages

### Stage 1: Initial Experimentation (You Start Here)

- Run bot manually on selected issues
- Review every assessment and implementation
- Tune prompts based on results
- Build confidence in success patterns

### Stage 2: Semi-Automated

- Bot runs automatically but all results reviewed
- Successful patterns get merged
- Failed attempts inform prompt improvements
- Establish baseline success rates

### Stage 3: Selective Automation

- Bot auto-merges for specific issue types (e.g., "trivial-fix" label)
- Everything else still requires review
- Regular audits of auto-merged changes

### Stage 4: Full Automation (Optional, Future)

- Bot operates autonomously within defined boundaries
- Alerts on unusual patterns or repeated failures
- You focus on complex problems

## File Structure

This project provides:

```
qwencoder-bot/
├── PROJECT_OVERVIEW.md      ← You are here
├── ARCHITECTURE.md          ← Technical design and components
├── IMPLEMENTATION_GUIDE.md  ← Step-by-step setup instructions
├── PROMPTS.md               ← System prompts (7B and 14B models)
├── VM_SETUP.md              ← VM configuration and snapshots
├── CONFIGURATION.md         ← Gog profiles and Ollama settings
└── CONTENTS.md              ← This file (guide to documentation)
```

## Success Metrics

To evaluate if the bot is working well:

### Assessment Quality (7B Model)
- **Accuracy**: Does it correctly identify trivial vs complex issues?
- **Precision**: How often does "worth-attempting" lead to 14B success?
- **Recall**: Is it escalating too conservatively?

### Implementation Success (14B Model)
- **Test Pass Rate**: Percentage of attempts where tests pass
- **Code Quality**: Are the changes sensible and maintainable?
- **False Positives**: How often do "passing" implementations break in review?

### Overall Efficiency
- **Time Saved**: How many issues resolved without human coding?
- **False Start Rate**: How often does bot work get discarded?
- **Escalation Quality**: Are escalated issues genuinely complex?

## Current State

This is an **experimental system**. The goal is to:
1. Prove the concept on your Rust codebase
2. Learn what works and what doesn't
3. Gradually expand to other languages/codebases
4. Iterate on prompts and assessment criteria

You're the only developer using it initially, so all results flow through you for review and learning.

## Next Steps

1. Read through all documentation files
2. Set up Gog profiles for the bot
3. Configure Ollama with qwen2.5-coder models
4. Create VM base snapshots with toolchains
5. Run first test with a simple known-good issue
6. Iterate on prompts based on results

See `IMPLEMENTATION_GUIDE.md` for detailed setup instructions.
