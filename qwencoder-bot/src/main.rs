//! QwenCoder Bot - Autonomous AI coding agent
//!
//! An autonomous coding bot that uses Qwen 2.5 Coder models via Ollama
//! to automatically implement fixes and features from issue tickets.

mod assessment;
mod config;
mod gogs;
mod implementation;
mod ollama;
mod vm;

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::fmt::format::FmtSpan;

use crate::assessment::{assess_issue, determine_action, AssessmentAction, AssessmentContext};
use crate::config::Config;
use crate::gogs::{
    format_assessment_comment, format_failure_comment, format_success_comment, GogClient, Issue,
};
use crate::implementation::{
    apply_changes, implement_issue, summarize_changes, validate_implementation,
    ImplementationContext, ParsedImplementation,
};
use crate::ollama::OllamaClient;
use crate::vm::VmManager;

/// Command line arguments
struct Args {
    config_path: Option<PathBuf>,
    check_config: bool,
    once: bool,
    verbose: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        config_path: None,
        check_config: false,
        once: false,
        verbose: false,
    };

    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                args.config_path = iter.next().map(PathBuf::from);
            }
            "--check-config" => {
                args.check_config = true;
            }
            "--once" => {
                args.once = true;
            }
            "--verbose" | "-v" => {
                args.verbose = true;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                std::process::exit(1);
            }
        }
    }

    args
}

fn print_help() {
    println!(
        r#"QwenCoder Bot - Autonomous AI coding agent

USAGE:
    qwencoder-bot [OPTIONS]

OPTIONS:
    -c, --config <PATH>    Path to configuration file
    --check-config         Validate configuration and exit
    --once                 Process issues once and exit (no polling)
    -v, --verbose          Enable verbose logging
    -h, --help             Print help information

ENVIRONMENT VARIABLES:
    QWENCODER_CONFIG       Configuration file path
    QWENCODER_LOG_LEVEL    Logging level (error, warn, info, debug, trace)
    QWENCODER_NO_VM        Disable VM operations (for testing)

CONFIGURATION:
    Default config locations:
    - $QWENCODER_CONFIG
    - ~/.config/qwencoder-bot/config.toml
    - ./config.toml
"#
    );
}

fn init_logging(verbose: bool, config: &Config) {
    let level = if verbose {
        Level::DEBUG
    } else {
        match config.logging.level.to_lowercase().as_str() {
            "error" => Level::ERROR,
            "warn" => Level::WARN,
            "debug" => Level::DEBUG,
            "trace" => Level::TRACE,
            _ => Level::INFO,
        }
    };

    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(false);

    if config.logging.log_to_stdout {
        subscriber.init();
    } else {
        subscriber.with_ansi(false).init();
    }
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();

    // Load configuration
    let config = if let Some(path) = &args.config_path {
        Config::load(path)?
    } else {
        Config::load_default()?
    };

    // Check config mode
    if args.check_config {
        println!("Configuration valid");
        println!("  Gogs URL: {}", config.server.gogs_url);
        println!("  Ollama endpoint: {}", config.server.ollama_endpoint);
        println!("  Assessor model: {}", config.models.assessor);
        println!("  Coder model: {}", config.models.coder);
        println!("  Trigger label: {}", config.polling.trigger_label);
        println!("  Polling interval: {}s", config.polling.interval_secs);
        return Ok(());
    }

    // Initialize logging
    init_logging(args.verbose, &config);

    info!("QwenCoder Bot starting");
    info!("Using trigger label: {}", config.polling.trigger_label);

    // Initialize clients
    let ollama = OllamaClient::new(
        &config.server.ollama_endpoint,
        config.models.timeout_secs,
    );

    let gog = GogClient::new(
        config.gog_profiles.gog_binary.clone(),
        config.gog_profiles.gog_timeout_secs,
    );

    // Check Ollama connection
    if !ollama.health_check().await? {
        error!("Cannot connect to Ollama at {}", config.server.ollama_endpoint);
        std::process::exit(1);
    }
    info!("Connected to Ollama");

    // Check required models
    for model in [&config.models.assessor, &config.models.coder] {
        if !ollama.has_model(model).await? {
            error!("Required model not found: {}", model);
            error!("Please run: ollama pull {}", model);
            std::process::exit(1);
        }
    }
    info!("Required models available");

    // Main loop
    if args.once {
        info!("Running in single-pass mode");
        process_issues(&config, &ollama, &gog).await?;
    } else {
        info!(
            "Starting polling loop (interval: {}s)",
            config.polling.interval_secs
        );

        loop {
            if let Err(e) = process_issues(&config, &ollama, &gog).await {
                error!("Error processing issues: {}", e);
            }

            tokio::time::sleep(Duration::from_secs(config.polling.interval_secs)).await;
        }
    }

    Ok(())
}

/// Process all issues with the trigger label
async fn process_issues(
    config: &Config,
    ollama: &OllamaClient,
    gog: &GogClient,
) -> Result<()> {
    debug!("Checking for issues with label: {}", config.polling.trigger_label);

    // Get issues with trigger label
    let issues = gog.list_issues_by_label(
        &config.gog_profiles.assessor,
        &config.polling.trigger_label,
        None,
    )?;

    if issues.is_empty() {
        debug!("No issues found");
        return Ok(());
    }

    info!("Found {} issue(s) to process", issues.len());

    for issue in issues {
        // Check for stop labels
        let has_stop_label = issue.labels.iter().any(|l| {
            config.polling.stop_labels.contains(&l.name)
        });

        if has_stop_label {
            debug!("Skipping issue #{} (has stop label)", issue.number);
            continue;
        }

        // Process this issue
        if let Err(e) = process_issue(config, ollama, gog, &issue).await {
            error!("Failed to process issue #{}: {}", issue.number, e);

            // Comment the error
            let error_comment = format_failure_comment(&e.to_string(), None);
            let _ = gog.add_comment(
                &config.gog_profiles.assessor,
                &format!("{}/{}", issue.owner, issue.repo),
                issue.number,
                &format!("[QwenAssess]\n\n{}", error_comment),
            );
        }
    }

    Ok(())
}

/// Process a single issue
async fn process_issue(
    config: &Config,
    ollama: &OllamaClient,
    gog: &GogClient,
    issue: &Issue,
) -> Result<()> {
    let repo_full = format!("{}/{}", issue.owner, issue.repo);
    info!("Processing issue #{}: {}", issue.number, issue.title);

    // Get full issue details
    let full_issue = gog.get_issue(
        &config.gog_profiles.assessor,
        &repo_full,
        issue.number,
    )?;

    // Clone/update repository
    let cache_dir = config.repositories.cache_dir.clone()
        .unwrap_or_else(|| PathBuf::from("/var/cache/qwencoder-bot/repos"));
    let repo_dir = cache_dir.join(&issue.owner).join(&issue.repo);

    gog.clone_or_update(&config.gog_profiles.assessor, &repo_full, &repo_dir)?;

    // Detect language and get relevant files
    let language = gog.detect_language(&repo_dir)?;
    debug!("Detected language: {}", language);

    // Build assessment context
    let context = build_assessment_context(&full_issue, &repo_dir, &language, gog)?;

    // Perform assessment
    let assessment = assess_issue(ollama, config, &context).await?;

    // Determine action
    let action = determine_action(&assessment, config);

    // Post assessment comment
    let assessment_comment = format_assessment_comment(
        &assessment.decision.to_string(),
        assessment.confidence,
        &assessment.reasoning,
        &assessment.affected_files,
        &assessment.test_strategy,
    );

    gog.add_comment(
        &config.gog_profiles.assessor,
        &repo_full,
        issue.number,
        &format!("[QwenAssess]\n\n{}", assessment_comment),
    )?;

    // Handle based on action
    match action {
        AssessmentAction::Proceed => {
            info!("Proceeding with implementation");

            // Add in-progress label
            let _ = gog.add_label(
                &config.gog_profiles.assessor,
                &repo_full,
                issue.number,
                "qwen-in-progress",
            );

            // Run implementation
            let result = run_implementation(
                config, ollama, gog, &full_issue, &assessment, &repo_dir, &language,
            ).await;

            // Remove in-progress label
            let _ = gog.remove_label(
                &config.gog_profiles.coder,
                &repo_full,
                issue.number,
                "qwen-in-progress",
            );

            match result {
                Ok(branch) => {
                    // Success - add needs-review label
                    let _ = gog.add_label(
                        &config.gog_profiles.coder,
                        &repo_full,
                        issue.number,
                        "needs-review",
                    );

                    info!("Implementation successful: branch {}", branch);
                }
                Err(e) => {
                    error!("Implementation failed: {}", e);

                    // Escalate
                    let _ = gog.add_label(
                        &config.gog_profiles.coder,
                        &repo_full,
                        issue.number,
                        "needs-claude-code",
                    );
                }
            }
        }
        AssessmentAction::Escalate { reason } => {
            info!("Escalating issue: {}", reason);

            gog.add_label(
                &config.gog_profiles.assessor,
                &repo_full,
                issue.number,
                "needs-claude-code",
            )?;
        }
        AssessmentAction::Skip { reason } => {
            info!("Skipping issue: {}", reason);
        }
    }

    // Remove trigger label
    let _ = gog.remove_label(
        &config.gog_profiles.assessor,
        &repo_full,
        issue.number,
        &config.polling.trigger_label,
    );

    Ok(())
}

/// Build assessment context from issue and repository
fn build_assessment_context(
    issue: &Issue,
    repo_dir: &std::path::Path,
    language: &str,
    gog: &GogClient,
) -> Result<AssessmentContext> {
    // Get repository structure (top-level files and directories)
    let files = gog.list_files(repo_dir, None)?;
    let repo_structure = files
        .iter()
        .take(50)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");

    // Try to find affected files mentioned in issue
    let mut affected_files_content = Vec::new();

    // Look for file paths in issue body
    let file_patterns = extract_file_patterns(&issue.body);
    for pattern in file_patterns.iter().take(5) {
        if let Ok(content) = gog.read_file(repo_dir, pattern) {
            let lang = detect_file_language(pattern);
            affected_files_content.push((pattern.clone(), lang, content));
        }
    }

    // Determine test framework
    let test_framework = match language {
        "rust" => "cargo test".to_string(),
        "python" => "pytest".to_string(),
        "csharp" => "dotnet test".to_string(),
        _ => "unknown".to_string(),
    };

    // Determine build tool
    let build_tool = match language {
        "rust" => "cargo".to_string(),
        "python" => "pip/setuptools".to_string(),
        "csharp" => "dotnet".to_string(),
        _ => "unknown".to_string(),
    };

    Ok(AssessmentContext {
        issue: issue.clone(),
        repo_structure,
        affected_files_content,
        test_content: None,
        primary_language: language.to_string(),
        test_framework,
        build_tool,
        has_ci: repo_dir.join(".github").exists() || repo_dir.join(".gitlab-ci.yml").exists(),
        coverage_percent: None,
        similar_issues: Vec::new(),
    })
}

/// Run the implementation phase
async fn run_implementation(
    config: &Config,
    ollama: &OllamaClient,
    gog: &GogClient,
    issue: &Issue,
    assessment: &assessment::AssessmentResult,
    repo_dir: &std::path::Path,
    language: &str,
) -> Result<String> {
    let repo_full = format!("{}/{}", issue.owner, issue.repo);

    // Build implementation context
    let context = build_implementation_context(issue, assessment, repo_dir, language, gog)?;

    // Get implementation from model
    let result = implement_issue(ollama, config, &context).await?;

    match result {
        ParsedImplementation::Success(impl_result) => {
            // Validate the implementation
            validate_implementation(&impl_result, config)?;

            // Create branch name
            let branch_name = format!("{}{}", config.implementation.branch_prefix, issue.number);

            // Apply changes locally first for testing
            apply_changes(repo_dir, &impl_result.files)?;

            // Create branch
            gog.create_branch(repo_dir, &branch_name)?;

            // Get test commands
            let test_commands = if impl_result.test_commands.is_empty() {
                config.get_test_commands(language, Some(&repo_full))
            } else {
                impl_result.test_commands.clone()
            };

            // Run tests
            let mut test_results = Vec::new();
            let mut all_passed = true;

            for cmd in &test_commands {
                info!("Running test: {}", cmd);

                let output = std::process::Command::new("bash")
                    .current_dir(repo_dir)
                    .args(["-c", cmd])
                    .output()
                    .context("Failed to run test command")?;

                let passed = output.status.success();
                test_results.push((cmd.clone(), passed));

                if !passed {
                    all_passed = false;
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    error!("Test failed: {}\n{}", cmd, stderr);

                    // Post failure comment
                    let failure_comment = format_failure_comment(
                        &format!("Test failed: {}", cmd),
                        Some(&stderr),
                    );
                    gog.add_comment(
                        &config.gog_profiles.coder,
                        &repo_full,
                        issue.number,
                        &format!("[QwenCoder]\n\n{}", failure_comment),
                    )?;

                    anyhow::bail!("Tests failed");
                }
            }

            if all_passed {
                // Commit changes
                gog.commit(repo_dir, &impl_result.commit_message)?;

                // Push if configured
                if config.implementation.auto_push {
                    gog.push_branch(repo_dir, &branch_name)?;
                }

                // Post success comment
                let changes = summarize_changes(&impl_result);
                let success_comment = format_success_comment(&branch_name, &test_results, &changes);
                gog.add_comment(
                    &config.gog_profiles.coder,
                    &repo_full,
                    issue.number,
                    &format!("[QwenCoder]\n\n{}", success_comment),
                )?;

                return Ok(branch_name);
            }

            anyhow::bail!("Tests failed")
        }
        ParsedImplementation::Error(error) => {
            // Post error comment
            let failure_comment = format_failure_comment(&error.reason, Some(&error.suggestions));
            gog.add_comment(
                &config.gog_profiles.coder,
                &repo_full,
                issue.number,
                &format!("[QwenCoder]\n\n{}", failure_comment),
            )?;

            anyhow::bail!("Model reported error: {}", error.reason)
        }
    }
}

/// Build implementation context
fn build_implementation_context(
    issue: &Issue,
    assessment: &assessment::AssessmentResult,
    repo_dir: &std::path::Path,
    language: &str,
    gog: &GogClient,
) -> Result<ImplementationContext> {
    // Get content of affected files
    let mut relevant_files = Vec::new();
    for path in &assessment.affected_files {
        if let Ok(content) = gog.read_file(repo_dir, path) {
            let lang = detect_file_language(path);
            relevant_files.push((path.clone(), lang, content));
        }
    }

    // Determine test framework details
    let (test_framework, test_directory, test_pattern) = match language {
        "rust" => ("cargo test".to_string(), "tests/".to_string(), "test_*.rs".to_string()),
        "python" => ("pytest".to_string(), "tests/".to_string(), "test_*.py".to_string()),
        "csharp" => ("dotnet test".to_string(), "Tests/".to_string(), "*Tests.cs".to_string()),
        _ => ("unknown".to_string(), "tests/".to_string(), "test_*".to_string()),
    };

    // Get build/test commands
    let (build_command, test_command, lint_command, format_command) = match language {
        "rust" => (
            "cargo build".to_string(),
            "cargo test".to_string(),
            "cargo clippy".to_string(),
            "cargo fmt --check".to_string(),
        ),
        "python" => (
            "pip install -e .".to_string(),
            "pytest".to_string(),
            "mypy .".to_string(),
            "black --check .".to_string(),
        ),
        "csharp" => (
            "dotnet build".to_string(),
            "dotnet test".to_string(),
            "dotnet format --verify-no-changes".to_string(),
            "dotnet format --verify-no-changes".to_string(),
        ),
        _ => (
            "make build".to_string(),
            "make test".to_string(),
            "make lint".to_string(),
            "make format-check".to_string(),
        ),
    };

    Ok(ImplementationContext {
        issue: issue.clone(),
        assessment: assessment.clone(),
        relevant_files,
        existing_tests: None,
        similar_code: Vec::new(),
        test_framework,
        test_directory,
        test_pattern,
        example_test: None,
        build_command,
        test_command,
        lint_command,
        format_command,
    })
}

/// Extract file patterns from text
fn extract_file_patterns(text: &str) -> Vec<String> {
    let mut patterns = Vec::new();

    // Match common file path patterns
    let re = regex::Regex::new(r"(?:^|\s|`)([\w\-./]+\.(rs|py|cs|js|ts|sh|md|toml|json|yaml|yml))(?:\s|`|$|:|\))").unwrap();

    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            patterns.push(m.as_str().to_string());
        }
    }

    patterns
}

/// Detect language from file extension
fn detect_file_language(path: &str) -> String {
    if path.ends_with(".rs") {
        "rust".to_string()
    } else if path.ends_with(".py") {
        "python".to_string()
    } else if path.ends_with(".cs") {
        "csharp".to_string()
    } else if path.ends_with(".js") {
        "javascript".to_string()
    } else if path.ends_with(".ts") {
        "typescript".to_string()
    } else if path.ends_with(".sh") || path.ends_with(".bash") {
        "bash".to_string()
    } else if path.ends_with(".toml") {
        "toml".to_string()
    } else if path.ends_with(".json") {
        "json".to_string()
    } else if path.ends_with(".yaml") || path.ends_with(".yml") {
        "yaml".to_string()
    } else {
        "text".to_string()
    }
}
