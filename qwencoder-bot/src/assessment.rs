//! Assessment module for QwenCoder Bot
//!
//! Uses the 7B model to assess whether an issue is suitable for automated implementation.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::gogs::Issue;
use crate::ollama::{GenerateOptions, OllamaClient};

/// Assessment decision from the 7B model
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    Trivial,
    WorthAttempting,
    TooComplex,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Trivial => write!(f, "trivial"),
            Decision::WorthAttempting => write!(f, "worth-attempting"),
            Decision::TooComplex => write!(f, "too-complex"),
        }
    }
}

/// Complexity factors from assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityFactors {
    pub requirements_clarity: u32,
    pub testability: u32,
    pub pattern_match: u32,
    pub risk_level: u32,
    pub scope: String,
}

/// Full assessment result from the 7B model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessmentResult {
    pub decision: Decision,
    pub confidence: u32,
    pub reasoning: String,
    #[serde(default)]
    pub complexity_factors: Option<ComplexityFactors>,
    #[serde(default)]
    pub affected_files: Vec<String>,
    #[serde(default)]
    pub test_strategy: String,
    #[serde(default)]
    pub implementation_guidance: String,
    #[serde(default)]
    pub estimated_difficulty: String,
}

/// Context for assessment
pub struct AssessmentContext {
    pub issue: Issue,
    pub repo_structure: String,
    pub affected_files_content: Vec<(String, String, String)>, // (path, language, content)
    pub test_content: Option<String>,
    pub primary_language: String,
    pub test_framework: String,
    pub build_tool: String,
    pub has_ci: bool,
    pub coverage_percent: Option<u32>,
    pub similar_issues: Vec<(u64, String, String)>, // (number, title, outcome)
}

/// The system prompt for assessment
const ASSESSMENT_SYSTEM_PROMPT: &str = r#"You are an expert code reviewer and triager for an autonomous coding system. Your job is to assess whether a GitHub/Gogs issue can be automatically implemented by an AI coding agent.

ASSESSMENT CRITERIA:

Classify issues into one of three categories:

1. TRIVIAL (0-30% complexity)
   - Typos in code or comments
   - Simple variable renames
   - Documentation updates
   - Adding/removing import statements
   - One-line fixes with obvious solutions

2. WORTH-ATTEMPTING (30-70% complexity)
   - Bug fixes with clear reproduction steps
   - Implementing functions that follow existing patterns
   - Adding straightforward tests
   - Refactoring that doesn't change behavior
   - Simple feature additions to existing code

3. TOO-COMPLEX (70-100% complexity)
   - Architecture decisions or design choices
   - Security-sensitive changes
   - Complex algorithms without clear specifications
   - Changes requiring domain expertise
   - Ambiguous requirements
   - Multi-file refactoring affecting many systems

ASSESSMENT FACTORS:

Consider these when making your decision:
- Clarity of requirements (is it obvious what needs to change?)
- Testability (are there tests, or is it clear how to test?)
- Code patterns (does similar code exist to follow?)
- Risk level (could this break critical systems?)
- Scope (how many files/lines affected?)

OUTPUT FORMAT:

You must respond with valid JSON only (no markdown, no explanations outside JSON):

{
  "decision": "trivial" | "worth-attempting" | "too-complex",
  "confidence": <0-100>,
  "reasoning": "<brief explanation of your decision>",
  "complexity_factors": {
    "requirements_clarity": <0-100>,
    "testability": <0-100>,
    "pattern_match": <0-100>,
    "risk_level": <0-100>,
    "scope": "small" | "medium" | "large"
  },
  "affected_files": ["list", "of", "file", "paths"],
  "test_strategy": "<how to test the changes>",
  "implementation_guidance": "<specific guidance for implementation phase>",
  "estimated_difficulty": "easy" | "moderate" | "hard"
}

RULES:

1. Be conservative - better to escalate than to waste implementation time
2. Always provide specific reasoning
3. If requirements are ambiguous, classify as too-complex
4. Consider the language and toolchain (Rust's compiler catches many errors)
5. Factor in existing test coverage
6. Look for similar solved issues in the codebase
7. Must output valid JSON - any other format will cause system failure"#;

/// Build the user prompt for assessment
fn build_user_prompt(context: &AssessmentContext) -> String {
    let mut prompt = String::new();

    // Issue information
    prompt.push_str("ISSUE INFORMATION:\n\n");
    prompt.push_str(&format!(
        "Repository: {}/{}\n",
        context.issue.owner, context.issue.repo
    ));
    prompt.push_str(&format!("Issue Number: {}\n", context.issue.number));
    prompt.push_str(&format!("Title: {}\n", context.issue.title));

    let labels: Vec<&str> = context.issue.labels.iter().map(|l| l.name.as_str()).collect();
    prompt.push_str(&format!("Labels: {}\n\n", labels.join(", ")));

    prompt.push_str("Issue Description:\n");
    prompt.push_str(&context.issue.body);
    prompt.push_str("\n\n");

    // Comments if any
    if !context.issue.comments.is_empty() {
        prompt.push_str("Recent Comments:\n");
        for comment in context.issue.comments.iter().take(5) {
            prompt.push_str(&format!("@{}: {}\n", comment.user, comment.body));
        }
        prompt.push_str("\n");
    }

    prompt.push_str("---\n\nCODE CONTEXT:\n\n");

    // Repository structure
    prompt.push_str("Repository Structure:\n");
    prompt.push_str(&context.repo_structure);
    prompt.push_str("\n\n");

    // Affected files
    prompt.push_str("Affected Files (based on issue description):\n\n");
    for (path, language, content) in &context.affected_files_content {
        prompt.push_str(&format!("File: {}\n", path));
        prompt.push_str(&format!("Language: {}\n", language));
        prompt.push_str(&format!("Lines: {}\n\n", content.lines().count()));
        prompt.push_str(&format!("Content:\n```{}\n{}\n```\n\n", language, content));
    }

    // Test content if available
    if let Some(test_content) = &context.test_content {
        prompt.push_str("Related Tests:\n");
        prompt.push_str(&format!("```\n{}\n```\n\n", test_content));
    }

    prompt.push_str("---\n\nPROJECT METADATA:\n\n");
    prompt.push_str(&format!("Language: {}\n", context.primary_language));
    prompt.push_str(&format!("Test Framework: {}\n", context.test_framework));
    prompt.push_str(&format!("Build Tool: {}\n", context.build_tool));
    prompt.push_str(&format!("Has CI/CD: {}\n", context.has_ci));

    if let Some(coverage) = context.coverage_percent {
        prompt.push_str(&format!("Test Coverage: {}%\n", coverage));
    }

    prompt.push_str("\n---\n\n");

    // Similar issues if available
    if !context.similar_issues.is_empty() {
        prompt.push_str("HISTORICAL DATA:\n\nSimilar Issues:\n");
        for (number, title, outcome) in &context.similar_issues {
            prompt.push_str(&format!("- Issue #{}: {} ({})\n", number, title, outcome));
        }
        prompt.push_str("\n---\n\n");
    }

    prompt.push_str(
        "ASSESS THIS ISSUE:\n\n\
         Determine if this issue should be:\n\
         1. Attempted automatically (trivial or worth-attempting)\n\
         2. Escalated to human developer (too-complex)\n\n\
         Provide your assessment in the required JSON format.",
    );

    prompt
}

/// Parse the assessment result from model response
fn parse_assessment_result(response: &str) -> Result<AssessmentResult> {
    // Try to extract JSON from response
    let json_str = OllamaClient::extract_json(response)
        .ok_or_else(|| anyhow::anyhow!("No JSON found in response"))?;

    debug!("Extracted JSON: {}", json_str);

    // Parse the JSON
    let result: AssessmentResult = serde_json::from_str(json_str)
        .with_context(|| format!("Failed to parse assessment JSON: {}", json_str))?;

    Ok(result)
}

/// Perform assessment using the 7B model
pub async fn assess_issue(
    client: &OllamaClient,
    config: &Config,
    context: &AssessmentContext,
) -> Result<AssessmentResult> {
    let model = &config.models.assessor;
    info!(
        "Assessing issue #{} with model {}",
        context.issue.number, model
    );

    // Build the prompt
    let user_prompt = build_user_prompt(context);
    debug!("Assessment prompt length: {} chars", user_prompt.len());

    // Generate options
    let options = GenerateOptions {
        temperature: Some(config.models.parameters.temperature),
        top_p: Some(config.models.parameters.top_p),
        num_predict: config.models.parameters.max_tokens,
        ..Default::default()
    };

    // Call the model
    let response = client
        .generate_with_retry(
            model,
            &user_prompt,
            Some(ASSESSMENT_SYSTEM_PROMPT),
            Some(options),
            config.assessment.max_retries,
        )
        .await
        .context("Failed to get assessment from model")?;

    debug!("Model response: {}", response.response);

    // Parse the result
    let result = parse_assessment_result(&response.response)?;

    info!(
        "Assessment complete: {} (confidence: {}%)",
        result.decision, result.confidence
    );

    Ok(result)
}

/// Check if the assessment result meets minimum confidence threshold
pub fn meets_confidence_threshold(result: &AssessmentResult, min_confidence: u32) -> bool {
    result.confidence >= min_confidence
}

/// Determine the next action based on assessment
pub enum AssessmentAction {
    Proceed,
    Escalate { reason: String },
    Skip { reason: String },
}

pub fn determine_action(result: &AssessmentResult, config: &Config) -> AssessmentAction {
    // Check confidence threshold
    if !meets_confidence_threshold(result, config.assessment.min_confidence) {
        return AssessmentAction::Escalate {
            reason: format!(
                "Confidence {}% is below threshold {}%",
                result.confidence, config.assessment.min_confidence
            ),
        };
    }

    // Check decision
    match result.decision {
        Decision::TooComplex => AssessmentAction::Escalate {
            reason: result.reasoning.clone(),
        },
        Decision::Trivial | Decision::WorthAttempting => AssessmentAction::Proceed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_assessment_result() {
        let json = r#"{
            "decision": "worth-attempting",
            "confidence": 85,
            "reasoning": "Simple fix with clear requirements",
            "affected_files": ["src/lib.rs"],
            "test_strategy": "Run cargo test",
            "implementation_guidance": "Change the return value",
            "estimated_difficulty": "easy"
        }"#;

        let result = parse_assessment_result(json).unwrap();
        assert_eq!(result.decision, Decision::WorthAttempting);
        assert_eq!(result.confidence, 85);
    }

    #[test]
    fn test_parse_assessment_in_code_block() {
        let response = r#"Here is my assessment:

```json
{
    "decision": "trivial",
    "confidence": 95,
    "reasoning": "Typo fix"
}
```"#;

        let result = parse_assessment_result(response).unwrap();
        assert_eq!(result.decision, Decision::Trivial);
    }

    #[test]
    fn test_decision_display() {
        assert_eq!(Decision::Trivial.to_string(), "trivial");
        assert_eq!(Decision::WorthAttempting.to_string(), "worth-attempting");
        assert_eq!(Decision::TooComplex.to_string(), "too-complex");
    }

    #[test]
    fn test_meets_confidence_threshold() {
        let result = AssessmentResult {
            decision: Decision::WorthAttempting,
            confidence: 75,
            reasoning: String::new(),
            complexity_factors: None,
            affected_files: Vec::new(),
            test_strategy: String::new(),
            implementation_guidance: String::new(),
            estimated_difficulty: String::new(),
        };

        assert!(meets_confidence_threshold(&result, 60));
        assert!(meets_confidence_threshold(&result, 75));
        assert!(!meets_confidence_threshold(&result, 80));
    }
}
