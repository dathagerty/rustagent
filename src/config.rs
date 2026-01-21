use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAi,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_max_tokens() -> u32 {
    8192
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiConfig {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustagentConfig {
    pub spec_dir: String,
    pub max_iterations: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeConfig {
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ShellPolicy {
    Allowlist,
    Blocklist,
    Unrestricted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_shell_policy")]
    pub shell_policy: ShellPolicy,

    #[serde(default = "default_allowed_commands")]
    pub allowed_commands: Vec<String>,

    #[serde(default)]
    pub blocked_patterns: Vec<String>,

    #[serde(default = "default_max_file_size_mb")]
    pub max_file_size_mb: u64,

    #[serde(default = "default_allowed_paths")]
    pub allowed_paths: Vec<String>,
}

fn default_shell_policy() -> ShellPolicy {
    ShellPolicy::Allowlist
}

fn default_allowed_commands() -> Vec<String> {
    vec![
        "git".to_string(),
        "cargo".to_string(),
        "npm".to_string(),
        "ls".to_string(),
        "cat".to_string(),
        "grep".to_string(),
        "find".to_string(),
        "echo".to_string(),
        "pwd".to_string(),
        "mkdir".to_string(),
        "touch".to_string(),
    ]
}

fn default_max_file_size_mb() -> u64 {
    10
}

fn default_allowed_paths() -> Vec<String> {
    vec![".".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub anthropic: Option<AnthropicConfig>,
    pub openai: Option<OpenAiConfig>,
    pub ollama: Option<OllamaConfig>,
    pub planning: Option<ModeConfig>,
    pub ralph: Option<ModeConfig>,
    pub rustagent: RustagentConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).context("Failed to read config file")?;

        // Expand environment variables
        let expanded = Self::expand_env_vars(&content);

        let config: Config = toml::from_str(&expanded).context("Failed to parse config file")?;

        // Validate provider configuration exists
        match config.llm.provider {
            LlmProvider::Anthropic if config.anthropic.is_none() => {
                anyhow::bail!("Anthropic provider selected but [anthropic] config missing")
            }
            LlmProvider::OpenAi if config.openai.is_none() => {
                anyhow::bail!("OpenAI provider selected but [openai] config missing")
            }
            LlmProvider::Ollama if config.ollama.is_none() => {
                anyhow::bail!("Ollama provider selected but [ollama] config missing")
            }
            _ => {}
        }

        Ok(config)
    }

    fn expand_env_vars(content: &str) -> String {
        let mut result = content.to_string();

        // Find all ${VAR} patterns
        while let Some(start) = result.find("${") {
            if let Some(end) = result[start..].find('}') {
                let var_name = &result[start + 2..start + end];
                let value = std::env::var(var_name).unwrap_or_default();
                result.replace_range(start..start + end + 1, &value);
            } else {
                break;
            }
        }

        result
    }

    pub fn planning_llm(&self) -> &LlmConfig {
        self.planning.as_ref().map(|m| &m.llm).unwrap_or(&self.llm)
    }

    pub fn ralph_llm(&self) -> &LlmConfig {
        self.ralph.as_ref().map(|m| &m.llm).unwrap_or(&self.llm)
    }
}

impl Default for RustagentConfig {
    fn default() -> Self {
        Self {
            spec_dir: "specs".to_string(),
            max_iterations: None,
        }
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            shell_policy: default_shell_policy(),
            allowed_commands: default_allowed_commands(),
            blocked_patterns: vec![],
            max_file_size_mb: default_max_file_size_mb(),
            allowed_paths: default_allowed_paths(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_expand_env_vars_multichar() {
        // Set a multi-character environment variable
        unsafe {
            env::set_var("MY_TEST_VAR", "test_value");
        }

        let input = "api_key = \"${MY_TEST_VAR}\"";
        let result = Config::expand_env_vars(input);

        assert_eq!(result, "api_key = \"test_value\"");

        // Clean up
        unsafe {
            env::remove_var("MY_TEST_VAR");
        }
    }

    #[test]
    fn test_expand_env_vars_multiple() {
        unsafe {
            env::set_var("VAR1", "value1");
            env::set_var("VAR2", "value2");
        }

        let input = "key1 = \"${VAR1}\"\nkey2 = \"${VAR2}\"";
        let result = Config::expand_env_vars(input);

        assert_eq!(result, "key1 = \"value1\"\nkey2 = \"value2\"");

        unsafe {
            env::remove_var("VAR1");
            env::remove_var("VAR2");
        }
    }

    #[test]
    fn test_expand_env_vars_missing() {
        let input = "api_key = \"${NONEXISTENT_VAR}\"";
        let result = Config::expand_env_vars(input);

        // Should expand to empty string when var doesn't exist
        assert_eq!(result, "api_key = \"\"");
    }

    #[test]
    fn test_provider_validation_anthropic_missing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[llm]
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"

[rustagent]
spec_dir = "specs"
        "#
        )
        .unwrap();

        let result = Config::load(file.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Anthropic provider selected but [anthropic] config missing")
        );
    }

    #[test]
    fn test_provider_validation_openai_missing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[llm]
provider = "openai"
model = "gpt-4"

[rustagent]
spec_dir = "specs"
        "#
        )
        .unwrap();

        let result = Config::load(file.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("OpenAI provider selected but [openai] config missing")
        );
    }

    #[test]
    fn test_provider_validation_ollama_missing() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[llm]
provider = "ollama"
model = "llama2"

[rustagent]
spec_dir = "specs"
        "#
        )
        .unwrap();

        let result = Config::load(file.path());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Ollama provider selected but [ollama] config missing")
        );
    }

    #[test]
    fn test_provider_validation_success() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        unsafe {
            env::set_var("TEST_API_KEY", "test_key");
        }

        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"
[llm]
provider = "anthropic"
model = "claude-3-5-sonnet-20241022"

[anthropic]
api_key = "${{TEST_API_KEY}}"

[rustagent]
spec_dir = "specs"
        "#
        )
        .unwrap();

        let result = Config::load(file.path());
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.llm.provider, LlmProvider::Anthropic);
        assert_eq!(config.anthropic.unwrap().api_key, "test_key");

        unsafe {
            env::remove_var("TEST_API_KEY");
        }
    }
}
