use std::sync::{Arc, Mutex};
use crate::llm::{LLMClient, ModelTier};
use crate::agent::message::{Message, PromptCache};
use crate::memory::store::MemoryStore;
use crate::memory::index::MemoryIndex;
use crate::tools::ToolRegistry;
use crate::permissions::PermissionManager;

pub struct AgentContext {
    pub messages: Vec<Message>,
    pub prompt_cache: PromptCache,
    pub llm: Arc<LLMClient>,
    pub store: Arc<MemoryStore>,
    pub index: Arc<Mutex<MemoryIndex>>,
    pub config: AgentConfig,
    pub tool_registry: ToolRegistry,
    pub permission_manager: PermissionManager,
}

#[derive(Clone)]
pub struct AgentConfig {
    pub model_tier: ModelTier,
    pub max_iterations: usize,
    pub top_k_memory: usize,
    pub permission_mode: PermissionMode,
}

#[derive(Clone, PartialEq)]
pub enum PermissionMode {
    Default,
    Auto,
    Bypass,
}

impl Default for AgentConfig {
    fn default() -> Self {
        AgentConfig {
            model_tier: ModelTier::Medium,
            max_iterations: 50,
            top_k_memory: 5,
            permission_mode: PermissionMode::Default,
        }
    }
}

impl AgentContext {
    pub fn new(llm: Arc<LLMClient>, base_dir: std::path::PathBuf) -> crate::Result<Self> {
        let store = Arc::new(MemoryStore::new(base_dir)?);
        let index = Arc::new(Mutex::new(MemoryIndex::load(&store)?));
        let tool_registry = ToolRegistry::default_tools(Arc::clone(&llm));
        Ok(AgentContext {
            messages: vec![],
            prompt_cache: PromptCache::default(),
            llm,
            store,
            index,
            config: AgentConfig::default(),
            tool_registry,
            permission_manager: PermissionManager::default(),
        })
    }
}
