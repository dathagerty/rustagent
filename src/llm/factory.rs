use crate::config::{Config, LlmConfig, LlmProvider};
use crate::llm::LlmClient;
use crate::llm::anthropic::AnthropicClient;
use crate::llm::ollama::OllamaClient;
use crate::llm::openai::OpenAiClient;
use anyhow::Result;
use std::sync::Arc;

pub fn create_client(config: &Config, llm_config: &LlmConfig) -> Result<Arc<dyn LlmClient>> {
    match llm_config.provider {
        LlmProvider::Anthropic => {
            let anthropic_config = config.anthropic.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Anthropic provider selected but [anthropic] config missing")
            })?;

            Ok(Arc::new(AnthropicClient::new(
                anthropic_config.api_key.clone(),
                llm_config.model.clone(),
                llm_config.max_tokens,
            )))
        }
        LlmProvider::OpenAi => {
            let openai_config = config.openai.as_ref().ok_or_else(|| {
                anyhow::anyhow!("OpenAI provider selected but [openai] config missing")
            })?;

            Ok(Arc::new(OpenAiClient::new(
                openai_config.api_key.clone(),
                llm_config.model.clone(),
                llm_config.max_tokens,
            )))
        }
        LlmProvider::Ollama => {
            let ollama_config = config.ollama.as_ref().ok_or_else(|| {
                anyhow::anyhow!("Ollama provider selected but [ollama] config missing")
            })?;

            Ok(Arc::new(OllamaClient::new(
                ollama_config.base_url.clone(),
                llm_config.model.clone(),
            )))
        }
    }
}
