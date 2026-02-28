use krew_config::AgentConfig;
use krew_llm::LlmClient;
use krew_tools::Tool;

/// Runtime state for a single agent in a session.
pub struct AgentRuntime {
    /// Agent configuration from settings.
    pub config: AgentConfig,
    /// LLM client for this agent's provider.
    pub client: Box<dyn LlmClient>,
    /// Tools available to this agent.
    pub tools: Vec<Box<dyn Tool>>,
    /// Whether the agent is currently generating a response.
    pub is_responding: bool,
}
