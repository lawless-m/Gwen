//! Implementation module for QwenCoder Bot
//!
//! Uses the 14B model to generate code changes for assessed issues.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::assessment::AssessmentResult;
use crate::config::Config;
use crate::gogs::Issue;
use crate::ollama::{GenerateOptions, OllamaClient};

/// File change action
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    Create,
    Modify,
    Delete,
}

/// A file change from the implementation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub action: FileAction,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub reasoning: String,
}

/// Full implementation result from the 14B model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationResult {
    pub files: Vec<FileChange>,
    pub commit_message: String,
    #[serde(default)]
    pub test_commands: Vec<String>,
    #[serde(default)]
    pub breaking_changes: bool,
    #[serde(default)]
    pub migration_notes: String,
    #[serde(default)]
    pub implementation_notes: String,
}

/// Error response from the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationError {
    pub error: String,
    pub reason: String,
    #[serde(default)]
    pub suggestions: String,
}

/// Context for implementation
pub struct ImplementationContext {
    pub issue: Issue,
    pub assessment: AssessmentResult,
    pub relevant_files: Vec<(String, String, String)>, // (path, language, content)
    pub existing_tests: Option<String>,
    pub similar_code: Vec<(String, String)>, // (path, snippet)
    pub test_framework: String,
    pub test_directory: String,
    pub test_pattern: String,
    pub example_test: Option<String>,
    pub build_command: String,
    pub test_command: String,
    pub lint_command: String,
    pub format_command: String,
}

/// The system prompt for implementation
const IMPLEMENTATION_SYSTEM_PROMPT: &str = r#"You are an expert software engineer implementing code changes for a specific issue. You have been given a clear, assessed-as-implementable issue to solve.

YOUR TASK:

1. Read and understand the issue requirements
2. Review the provided code context
3. Generate code changes that solve the issue
4. Write or update tests to verify the solution
5. Provide a clear commit message

CODE QUALITY STANDARDS:

- Follow existing code style and patterns exactly
- Write clear, readable code with appropriate comments
- Handle edge cases and error conditions
- Ensure backward compatibility unless explicitly changing behavior
- Use idiomatic patterns for the language (Rust: Result/Option, C#: LINQ, etc.)
- Add logging where appropriate
- Consider performance implications

TESTING REQUIREMENTS:

- Every change must have corresponding tests
- Tests must cover:
  * Happy path (expected usage)
  * Edge cases (empty inputs, boundaries)
  * Error conditions (invalid inputs, failure modes)
- Use existing test patterns and frameworks
- Ensure tests are deterministic (no flaky tests)

LANGUAGE-SPECIFIC GUIDELINES:

Rust:
- Use idiomatic Result<T, E> for error handling
- Leverage the type system for correctness
- Add appropriate derives (Debug, Clone, etc.)
- Run clippy rules: no warnings allowed
- Use rustfmt style

C#:
- Follow .NET naming conventions
- Use LINQ for collections
- Async/await for I/O operations
- XML doc comments for public APIs

Python:
- Type hints for all function signatures
- Docstrings for classes and functions
- Follow PEP 8 style guide
- Use pytest for tests

Shell Scripts:
- Use shellcheck rules
- Quote variables properly
- Handle errors with set -e
- Add usage documentation

OUTPUT FORMAT:

You must respond with valid JSON only (no markdown, no preamble, no postamble):

{
  "files": [
    {
      "path": "relative/path/to/file.rs",
      "action": "create" | "modify" | "delete",
      "content": "<full file content for create/modify>",
      "reasoning": "<why this change is necessary>"
    }
  ],
  "commit_message": "<conventional commit format>\n\n<detailed description>",
  "test_commands": [
    "<command to run tests>",
    "<command to run linter>",
    "<command to run formatter check>"
  ],
  "breaking_changes": true | false,
  "migration_notes": "<if breaking changes, how to migrate>",
  "implementation_notes": "<any important details for reviewer>"
}

COMMIT MESSAGE FORMAT:

Follow conventional commits:
- fix: for bug fixes
- feat: for new features
- docs: for documentation
- test: for test additions
- refactor: for code refactoring
- style: for formatting changes
- chore: for maintenance tasks

Example:
fix: correct off-by-one error in parser

The count_items() function was returning length instead of length-1,
causing incorrect results in edge cases with empty collections.

Fixes #123

CRITICAL RULES:

1. Output ONLY valid JSON - any other format causes system failure
2. Include full file content - no diffs or patches (system applies full files)
3. Only modify files directly related to the issue
4. Always include tests unless explicitly a documentation-only change
5. Ensure code compiles/passes syntax check
6. Follow existing patterns - don't introduce new architectural patterns
7. Keep changes minimal - solve the issue, don't over-engineer
8. If you cannot implement correctly, output JSON with "error" field explaining why

ERROR HANDLING:

If you cannot implement the issue, respond with:

{
  "error": "Cannot implement",
  "reason": "<specific explanation>",
  "suggestions": "<what would be needed to implement this>"
}"#;

/// Build the user prompt for implementation
fn build_user_prompt(context: &ImplementationContext) -> String {
    let mut prompt = String::new();

    // Issue information
    prompt.push_str("ISSUE TO IMPLEMENT:\n\n");
    prompt.push_str(&format!(
        "Repository: {}/{}\n",
        context.issue.owner, context.issue.repo
    ));
    prompt.push_str(&format!("Issue Number: {}\n", context.issue.number));
    prompt.push_str(&format!("Title: {}\n\n", context.issue.title));
    prompt.push_str("Issue Description:\n");
    prompt.push_str(&context.issue.body);
    prompt.push_str("\n\n");

    // Assessment guidance
    prompt.push_str("Assessment Guidance:\n");
    prompt.push_str(&context.assessment.implementation_guidance);
    prompt.push_str("\n\n");
    prompt.push_str(&format!(
        "Estimated Difficulty: {}\n\n",
        context.assessment.estimated_difficulty
    ));

    prompt.push_str("---\n\nFULL CODE CONTEXT:\n\n");

    // Relevant files
    for (path, language, content) in &context.relevant_files {
        prompt.push_str(&format!("File: {}\n", path));
        prompt.push_str(&format!("Current Content:\n```{}\n{}\n```\n\n", language, content));
    }

    // Existing tests
    if let Some(tests) = &context.existing_tests {
        prompt.push_str("Existing Tests for affected files:\n");
        prompt.push_str(&format!("```\n{}\n```\n\n", tests));
    }

    prompt.push_str("---\n\nRELATED CODE PATTERNS:\n\n");

    // Similar code patterns
    if !context.similar_code.is_empty() {
        prompt.push_str("Similar Functions/Classes in Codebase:\n\n");
        for (path, snippet) in &context.similar_code {
            prompt.push_str(&format!("File: {}\n", path));
            prompt.push_str(&format!("Example Pattern:\n```\n{}\n```\n\n", snippet));
        }
    }

    prompt.push_str("---\n\nTEST FRAMEWORK DETAILS:\n\n");
    prompt.push_str(&format!("Framework: {}\n", context.test_framework));
    prompt.push_str(&format!("Test Location: {}\n", context.test_directory));
    prompt.push_str(&format!("Test Naming: {}\n\n", context.test_pattern));

    if let Some(example) = &context.example_test {
        prompt.push_str("Example Test:\n");
        prompt.push_str(&format!("```\n{}\n```\n\n", example));
    }

    prompt.push_str("---\n\nBUILD AND TEST COMMANDS:\n\n");
    prompt.push_str(&format!("Build: {}\n", context.build_command));
    prompt.push_str(&format!("Test: {}\n", context.test_command));
    prompt.push_str(&format!("Lint: {}\n", context.lint_command));
    prompt.push_str(&format!("Format Check: {}\n\n", context.format_command));

    prompt.push_str("---\n\nIMPLEMENT THE SOLUTION:\n\n");
    prompt.push_str(
        "Generate the complete code changes to solve this issue. Include:\n\
         1. All modified files with full content\n\
         2. New or updated tests\n\
         3. Clear commit message\n\
         4. Commands to verify the implementation\n\n\
         Output your implementation in the required JSON format.",
    );

    prompt
}

/// Result of parsing implementation response
pub enum ParsedImplementation {
    Success(ImplementationResult),
    Error(ImplementationError),
}

/// Parse the implementation result from model response
fn parse_implementation_result(response: &str) -> Result<ParsedImplementation> {
    // Try to extract JSON from response
    let json_str = OllamaClient::extract_json(response)
        .ok_or_else(|| anyhow::anyhow!("No JSON found in response"))?;

    debug!("Extracted JSON: {}", json_str);

    // Check if it's an error response
    if json_str.contains("\"error\"") && json_str.contains("\"reason\"") {
        let error: ImplementationError = serde_json::from_str(json_str)
            .with_context(|| "Failed to parse error response")?;
        return Ok(ParsedImplementation::Error(error));
    }

    // Parse as success
    let result: ImplementationResult = serde_json::from_str(json_str)
        .with_context(|| format!("Failed to parse implementation JSON: {}", json_str))?;

    Ok(ParsedImplementation::Success(result))
}

/// Perform implementation using the 14B model
pub async fn implement_issue(
    client: &OllamaClient,
    config: &Config,
    context: &ImplementationContext,
) -> Result<ParsedImplementation> {
    let model = &config.models.coder;
    info!(
        "Implementing issue #{} with model {}",
        context.issue.number, model
    );

    // Build the prompt
    let user_prompt = build_user_prompt(context);
    debug!("Implementation prompt length: {} chars", user_prompt.len());

    // Generate options - slightly higher temperature for implementation
    let options = GenerateOptions {
        temperature: Some(config.models.parameters.temperature + 0.1),
        top_p: Some(config.models.parameters.top_p),
        num_predict: config.models.parameters.max_tokens,
        ..Default::default()
    };

    // Call the model
    let response = client
        .generate_with_retry(
            model,
            &user_prompt,
            Some(IMPLEMENTATION_SYSTEM_PROMPT),
            Some(options),
            config.error_handling.max_retries,
        )
        .await
        .context("Failed to get implementation from model")?;

    debug!("Model response length: {} chars", response.response.len());

    // Parse the result
    let result = parse_implementation_result(&response.response)?;

    match &result {
        ParsedImplementation::Success(impl_result) => {
            info!(
                "Implementation complete: {} files changed",
                impl_result.files.len()
            );
        }
        ParsedImplementation::Error(error) => {
            warn!("Model reported error: {}", error.reason);
        }
    }

    Ok(result)
}

/// Validate implementation result before applying
pub fn validate_implementation(result: &ImplementationResult, config: &Config) -> Result<()> {
    // Check file count limit
    if result.files.len() > config.security.max_files_per_issue as usize {
        anyhow::bail!(
            "Too many files changed ({} > {})",
            result.files.len(),
            config.security.max_files_per_issue
        );
    }

    // Check for forbidden patterns
    for file in &result.files {
        for pattern in &config.security.forbidden_patterns {
            // Simple glob matching
            if matches_pattern(&file.path, pattern) {
                anyhow::bail!("File '{}' matches forbidden pattern '{}'", file.path, pattern);
            }
        }
    }

    // Check commit message is not empty
    if result.commit_message.trim().is_empty() {
        anyhow::bail!("Commit message cannot be empty");
    }

    // Check that modified/created files have content
    for file in &result.files {
        if file.action != FileAction::Delete && file.content.is_empty() {
            anyhow::bail!("File '{}' has no content", file.path);
        }
    }

    Ok(())
}

/// Simple glob pattern matching
fn matches_pattern(path: &str, pattern: &str) -> bool {
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 2];
        path.starts_with(prefix)
    } else if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        path.ends_with(ext)
    } else {
        path == pattern || path.ends_with(&format!("/{}", pattern))
    }
}

/// Apply file changes to a directory
pub fn apply_changes(
    repo_dir: &std::path::Path,
    changes: &[FileChange],
) -> Result<Vec<String>> {
    let mut applied = Vec::new();

    for change in changes {
        let file_path = repo_dir.join(&change.path);

        match change.action {
            FileAction::Create | FileAction::Modify => {
                // Ensure parent directory exists
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)
                        .with_context(|| format!("Failed to create directory: {:?}", parent))?;
                }

                // Write the file
                std::fs::write(&file_path, &change.content)
                    .with_context(|| format!("Failed to write file: {:?}", file_path))?;

                info!("Applied {:?} to {}", change.action, change.path);
                applied.push(change.path.clone());
            }
            FileAction::Delete => {
                if file_path.exists() {
                    std::fs::remove_file(&file_path)
                        .with_context(|| format!("Failed to delete file: {:?}", file_path))?;
                    info!("Deleted {}", change.path);
                    applied.push(change.path.clone());
                } else {
                    warn!("File to delete does not exist: {}", change.path);
                }
            }
        }
    }

    Ok(applied)
}

/// Get summary of changes for reporting
pub fn summarize_changes(result: &ImplementationResult) -> Vec<(String, String)> {
    result
        .files
        .iter()
        .map(|f| {
            let description = if f.reasoning.is_empty() {
                format!("{:?}", f.action)
            } else {
                f.reasoning.clone()
            };
            (f.path.clone(), description)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_implementation_success() {
        let json = r#"{
            "files": [
                {
                    "path": "src/lib.rs",
                    "action": "modify",
                    "content": "fn main() {}",
                    "reasoning": "Fixed the bug"
                }
            ],
            "commit_message": "fix: correct bug",
            "test_commands": ["cargo test"],
            "breaking_changes": false,
            "implementation_notes": "Simple fix"
        }"#;

        let result = parse_implementation_result(json).unwrap();
        match result {
            ParsedImplementation::Success(impl_result) => {
                assert_eq!(impl_result.files.len(), 1);
                assert_eq!(impl_result.files[0].path, "src/lib.rs");
            }
            _ => panic!("Expected success"),
        }
    }

    #[test]
    fn test_parse_implementation_error() {
        let json = r#"{
            "error": "Cannot implement",
            "reason": "Requirements unclear",
            "suggestions": "Add more details"
        }"#;

        let result = parse_implementation_result(json).unwrap();
        match result {
            ParsedImplementation::Error(error) => {
                assert_eq!(error.reason, "Requirements unclear");
            }
            _ => panic!("Expected error"),
        }
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern(".git/config", ".git/*"));
        assert!(matches_pattern("secrets.key", "*.key"));
        assert!(matches_pattern(".env", ".env"));
        assert!(!matches_pattern("src/lib.rs", ".git/*"));
    }

    #[test]
    fn test_summarize_changes() {
        let result = ImplementationResult {
            files: vec![
                FileChange {
                    path: "src/lib.rs".to_string(),
                    action: FileAction::Modify,
                    content: "code".to_string(),
                    reasoning: "Fixed bug".to_string(),
                },
                FileChange {
                    path: "src/new.rs".to_string(),
                    action: FileAction::Create,
                    content: "code".to_string(),
                    reasoning: String::new(),
                },
            ],
            commit_message: String::new(),
            test_commands: Vec::new(),
            breaking_changes: false,
            migration_notes: String::new(),
            implementation_notes: String::new(),
        };

        let summary = summarize_changes(&result);
        assert_eq!(summary.len(), 2);
        assert_eq!(summary[0].1, "Fixed bug");
        assert_eq!(summary[1].1, "Create");
    }
}
