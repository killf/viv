use crate::agent::message::{PromptCache, SystemBlock, hash_str};
use crate::memory::retrieval::RetrievalResult;

const BASE_SYSTEM_PROMPT: &str = r#"You are viv, a self-evolving AI programming agent.

You help users with software engineering tasks: writing code, fixing bugs, refactoring, explaining code, and running commands.

You have access to tools to read/write files, execute commands, search the web, and more. Use them to accomplish tasks effectively.

Be concise and direct. Default to action over explanation. When a task is ambiguous, make a reasonable assumption and proceed."#;

pub struct SystemPrompt {
    pub blocks: Vec<SystemBlock>,
}

/// Build environment info block (cwd, git, platform, OS).
/// Mirrors Claude Code's computeSimpleEnvInfo().
pub fn build_env_info(cwd: &str) -> String {
    let is_git = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(cwd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let platform = std::env::consts::OS;
    let arch = std::env::consts::ARCH;

    format!(
        r#"Here is useful information about the environment you are running in:
<env>
Primary working directory: {cwd}
Is a git repository: {is_git}
Platform: {platform}
Architecture: {arch}
</env>"#
    )
}

/// Build cache-optimized system prompt: Base → EnvInfo → Tools → Skills → Memory
pub fn build_system_prompt(
    cwd: &str,
    tool_descriptions: &str,
    skill_contents: &str,
    memory_results: &[RetrievalResult],
    cache: &mut PromptCache,
) -> SystemPrompt {
    let mut blocks = vec![];

    // Block 1: Base (most stable, highest cache hit rate)
    let base_hash = hash_str(BASE_SYSTEM_PROMPT);
    if cache.base_hash != base_hash {
        cache.base_hash = base_hash;
        cache.base_text = BASE_SYSTEM_PROMPT.to_string();
    }
    blocks.push(SystemBlock::cached(&cache.base_text));

    // Block 2: EnvInfo (includes cwd, invalidated when cwd changes)
    let env_info = build_env_info(cwd);
    let env_hash = hash_str(&env_info);
    if cache.env_hash != env_hash {
        cache.env_hash = env_hash;
        cache.env_text = env_info;
    }
    blocks.push(SystemBlock::cached(&cache.env_text));

    // Block 3: Tools (invalidated when tool set changes)
    if !tool_descriptions.is_empty() {
        let tools_hash = hash_str(tool_descriptions);
        if cache.tools_hash != tools_hash {
            cache.tools_hash = tools_hash;
            cache.tools_text = tool_descriptions.to_string();
        }
        blocks.push(SystemBlock::cached(&cache.tools_text));
    }

    // Block 4: Skills (invalidated when skill set changes)
    if !skill_contents.is_empty() {
        let skills_hash = hash_str(skill_contents);
        if cache.skills_hash != skills_hash {
            cache.skills_hash = skills_hash;
            cache.skills_text = skill_contents.to_string();
        }
        blocks.push(SystemBlock::cached(&cache.skills_text));
    }

    // Block 5: Memory (dynamic per request, not cached)
    let memory_text = crate::memory::retrieval::format_memory_injection(memory_results);
    if !memory_text.is_empty() {
        blocks.push(SystemBlock::dynamic(memory_text));
    }

    SystemPrompt { blocks }
}
