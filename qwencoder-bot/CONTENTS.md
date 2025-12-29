# QwenCoder Bot - Documentation Contents

Welcome to the QwenCoder Bot documentation. This file guides you through all available documentation and helps you find what you need.

## Quick Start Path

**If you're setting up the bot for the first time**, follow this order:

1. **PROJECT_OVERVIEW.md** (10 min read)
   - Understand what the bot does
   - See how it fits into your workflow
   - Review the high-level architecture

2. **IMPLEMENTATION_GUIDE.md** (2-3 hours to complete)
   - Step-by-step setup instructions
   - From installing models to running your first test
   - Includes troubleshooting for common issues

3. **PROMPTS.md** (20 min read, ongoing tuning)
   - Understand how the bot makes decisions
   - Initial prompts that you'll tune over time
   - Examples of good and bad assessments

4. **CONFIGURATION.md** (30 min read)
   - Detailed configuration options
   - Security best practices
   - Performance tuning

5. **VM_SETUP.md** (1-2 hours to complete)
   - Creating the VM environment
   - Installing toolchains
   - Snapshot management

6. **ARCHITECTURE.md** (Reference as needed)
   - Deep technical details
   - Component interactions
   - Extension points

## Document Overview

### PROJECT_OVERVIEW.md
**Purpose**: High-level introduction to the system  
**Audience**: Anyone evaluating or understanding the bot  
**Key Topics**:
- What is QwenCoder Bot?
- How does it work?
- Integration with your existing tools (gog CLI, Ollama)
- Workflow stages (experiment → production)
- Success metrics

**Read this if**: You're new to the project or explaining it to others

---

### ARCHITECTURE.md
**Purpose**: Technical deep-dive into system design  
**Audience**: Developers implementing or extending the bot  
**Key Topics**:
- Component breakdown (bot process, Ollama, gog CLI, VMs)
- Data flow through assessment and implementation phases
- Error handling strategies
- Performance characteristics
- Future enhancement ideas

**Read this if**: You're implementing the bot, debugging issues, or planning extensions

---

### IMPLEMENTATION_GUIDE.md
**Purpose**: Hands-on setup instructions  
**Audience**: You, right now, setting up the bot  
**Key Topics**:
- Prerequisites checklist
- Pull Qwen models
- Configure Ollama
- Create bot Gogs accounts
- Set up gog CLI profiles
- Build and configure the bot
- Initial test run
- Deploy as systemd service

**Read this if**: You're ready to actually set up the bot

---

### PROMPTS.md
**Purpose**: System prompts and tuning guidance  
**Audience**: You, continuously improving the bot  
**Key Topics**:
- Assessment prompt (7B model) - decides what to attempt
- Implementation prompt (14B model) - generates code
- Prompt tuning methodology
- Example conversations
- Version history tracking

**Read this if**: You want to understand or improve bot decision-making

---

### VM_SETUP.md
**Purpose**: VM environment creation and management  
**Audience**: System administrators or VM-focused developers  
**Key Topics**:
- Why VMs for isolation
- Hypervisor options (QEMU/KVM, VirtualBox)
- Creating base VM with toolchains
- Snapshot strategies
- VM management from bot code
- Resource limits and monitoring

**Read this if**: You need to set up or troubleshoot the VM infrastructure

---

### CONFIGURATION.md
**Purpose**: Complete configuration reference  
**Audience**: Operators and administrators  
**Key Topics**:
- Main bot config file (config.toml)
- Gog CLI profiles
- Ollama settings
- Systemd service configuration
- Security hardening
- Common configuration patterns

**Read this if**: You need to configure or tune the bot's behavior

---

## File Relationships

```
PROJECT_OVERVIEW.md
    ↓
    Provides context for everything else
    
IMPLEMENTATION_GUIDE.md
    ↓
    References → CONFIGURATION.md (for config details)
    References → VM_SETUP.md (for VM creation)
    References → PROMPTS.md (for understanding decisions)
    
ARCHITECTURE.md
    ↑
    Deep-dive from overview
    Informs implementation decisions
    
PROMPTS.md
    ↔
    Iterated based on experience
    Affects bot behavior
    
VM_SETUP.md
    ↔
    Supports implementation
    Referenced by architecture
    
CONFIGURATION.md
    ↔
    Used throughout implementation
    Referenced by all operational docs
```

## Getting Started Checklist

Before diving into setup, ensure you have:

- [ ] Read PROJECT_OVERVIEW.md completely
- [ ] Gogs server with API access
- [ ] Gog CLI installed and working
- [ ] Ollama server running
- [ ] NVIDIA GPU with proper drivers
- [ ] VM hypervisor available (QEMU/KVM or VirtualBox)
- [ ] At least one Rust project in Gogs to test with
- [ ] 2-3 hours to complete initial setup

Once you have these, start with **IMPLEMENTATION_GUIDE.md step 1**.

## Common Questions & Relevant Docs

**"How do I install the bot?"**  
→ IMPLEMENTATION_GUIDE.md

**"What models do I need?"**  
→ PROJECT_OVERVIEW.md (overview), IMPLEMENTATION_GUIDE.md (installation)

**"How does the bot make decisions?"**  
→ PROMPTS.md

**"The bot is making bad decisions, how do I fix it?"**  
→ PROMPTS.md (tuning section)

**"How do I configure timeouts/polling/labels?"**  
→ CONFIGURATION.md

**"VMs won't start, what's wrong?"**  
→ VM_SETUP.md (troubleshooting section)

**"How does the bot integrate with my existing tools?"**  
→ ARCHITECTURE.md (integration points)

**"What resources does this need?"**  
→ PROJECT_OVERVIEW.md (resource requirements), VM_SETUP.md (VM specs)

**"Is this secure?"**  
→ ARCHITECTURE.md (security considerations), CONFIGURATION.md (security best practices)

**"Can I run multiple bots simultaneously?"**  
→ ARCHITECTURE.md (performance/scaling), CONFIGURATION.md (concurrent VMs)

**"How do I track success rates?"**  
→ PROJECT_OVERVIEW.md (success metrics), PROMPTS.md (tracking effectiveness)

## Usage Patterns

### First-Time Setup
1. PROJECT_OVERVIEW.md
2. IMPLEMENTATION_GUIDE.md (follow all steps)
3. Test with simple issue
4. Review results, note any problems

### Daily Operations
- Monitor logs (`journalctl -u qwencoder-bot -f`)
- Review bot comments on issues
- Merge successful PRs
- Escalate failures to yourself

### Periodic Tuning (Weekly/Monthly)
1. Review success/failure metrics
2. Read PROMPTS.md tuning section
3. Adjust prompts based on patterns
4. Update version history in PROMPTS.md
5. Test changes with known issues

### VM Maintenance (Monthly)
1. Read VM_SETUP.md maintenance section
2. Update base VM toolchains
3. Test updated snapshot
4. Replace production snapshot

### Configuration Changes
1. Read CONFIGURATION.md for relevant option
2. Update config.toml
3. Validate: `qwencoder-bot --check-config`
4. Restart: `sudo systemctl restart qwencoder-bot`
5. Monitor logs for issues

## When Things Go Wrong

### Assessment Phase Issues
**Symptoms**: Bot not detecting issues, or escalating everything  
**Docs**: PROMPTS.md, CONFIGURATION.md (polling settings)

### Implementation Phase Issues
**Symptoms**: Tests failing, code won't compile  
**Docs**: PROMPTS.md (implementation prompt), VM_SETUP.md (toolchains)

### VM Issues
**Symptoms**: VMs won't start, timeout errors  
**Docs**: VM_SETUP.md (troubleshooting), CONFIGURATION.md (VM settings)

### Performance Issues
**Symptoms**: Bot is slow, using too much resources  
**Docs**: ARCHITECTURE.md (performance), CONFIGURATION.md (tuning patterns)

### Integration Issues
**Symptoms**: Can't connect to Gogs, Ollama, or gog CLI problems  
**Docs**: IMPLEMENTATION_GUIDE.md (testing components), CONFIGURATION.md

## Recommended Reading Order by Role

### If you're the sole developer (most likely):
1. PROJECT_OVERVIEW.md
2. IMPLEMENTATION_GUIDE.md
3. PROMPTS.md
4. CONFIGURATION.md (skim, reference as needed)
5. VM_SETUP.md (when setting up VMs)
6. ARCHITECTURE.md (when extending or debugging)

### If you're explaining to someone else:
1. PROJECT_OVERVIEW.md (show them the big picture)
2. Example bot comments on Gogs issues (show real output)
3. ARCHITECTURE.md (if they want technical details)

### If you're troubleshooting:
1. Identify which phase is failing
2. Check relevant section in ARCHITECTURE.md
3. Review configuration in CONFIGURATION.md
4. Check logs and correlate with documentation
5. Adjust prompts if decision-making issue (PROMPTS.md)

## Documentation Maintenance

As you use the bot:

**Track successful patterns** in PROMPTS.md:
- Issues that worked well
- Effective prompt adjustments
- Version history of prompts

**Note configuration changes** in CONFIGURATION.md:
- Settings that improved performance
- Security hardening steps
- Custom configurations for your setup

**Document VM changes** in VM_SETUP.md:
- Toolchain updates
- New dependencies added
- Snapshot management procedures

**Add architecture insights** to ARCHITECTURE.md:
- Bottlenecks discovered
- Extension points you've used
- Integration patterns that work

## Version Control

These docs should live in your project repository:

```bash
git clone your-repo
cd your-repo
cp -r /path/to/qwencoder-bot-docs docs/qwencoder-bot/
git add docs/qwencoder-bot/
git commit -m "docs: add QwenCoder bot documentation"
```

Track changes as you tune the system:
- Prompt updates (PROMPTS.md)
- Config optimizations (CONFIGURATION.md)
- VM improvements (VM_SETUP.md)
- Lessons learned (PROJECT_OVERVIEW.md)

## Getting Help

If you're stuck:

1. **Check the relevant doc** (use this contents guide to find it)
2. **Search the doc** for your error message or symptom
3. **Review the troubleshooting section** in that doc
4. **Check logs**: `journalctl -u qwencoder-bot -n 100`
5. **Test components individually** (Ollama, gog CLI, VMs)

## Next Steps

**Ready to start?** → IMPLEMENTATION_GUIDE.md Step 1

**Need more context first?** → PROJECT_OVERVIEW.md

**Want to understand the design?** → ARCHITECTURE.md

**Have specific configuration questions?** → CONFIGURATION.md

**VM setup questions?** → VM_SETUP.md

**Want to tune prompts?** → PROMPTS.md

Good luck with your autonomous coding agent!
