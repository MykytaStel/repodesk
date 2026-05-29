use crate::errors::RepoDeskResult;

pub mod ollama;

pub trait LlmProvider {
    /// Send a prompt to the LLM provider and return the string completion.
    fn generate(&self, prompt: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = RepoDeskResult<String>> + Send>>;
    
    /// Health check
    fn is_available(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>;
}
