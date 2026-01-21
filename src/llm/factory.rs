use crate::config::{Config, LlmConfig, LlmProvider};
use crate::llm::anthropic::AnthropicClient;
use crate::llm::LlmClient;
use anyhow::{bail, Result};
use std::sync::Arc;

pub fn create_client(config: &Config, llm_config: &LlmConfig) -> Result<Arc<dyn LlmClient>> {
    match llm_config.provider {
        LlmProvider::Anthropic => {
            let anthropic_config = config
                .anthropic
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Anthropic provider selected but [anthropic] config missing"))?;

            Ok(Arc::new(AnthropicClient::new(
                anthropic_config.api_key.clone(),
                llm_config.model.clone(),
                llm_config.max_tokens,
            )))
        }
        LlmProvider::OpenAi => {
            bail!("OpenAI provider not yet implemented")
        }
        LlmProvider::Ollama => {
            bail!("Ollama provider not yet implemented")
        }
    }
}
