# Skill 系统设计

> 对标 Claude Code 的 Skill 机制：Markdown 文件定义能力，Agent 自动匹配或用户 `/name` 显式调用。

## 目标

为 viv 实现可扩展的 Skill 系统。Skill 是预定义的指令模板（Markdown 文件），Agent 根据任务自动判断是否调用，也支持用户通过 `/skill-name` 显式触发。

## 非目标

- Skill 市场/插件系统（先做本地文件加载）
- Skill 之间的依赖管理
- Skill 版本控制
- 运行时动态添加/删除 Skill（启动时加载一次）

---

## 1. Skill 文件约定

### 目录结构

每个 Skill 是一个目录，包含必需的 `SKILL.md` 和可选的辅助文件：

```
skill-name/              ← kebab-case 目录名
├── SKILL.md             ← 主文件（必需）
├── helper-prompt.md     ← 辅助文件（可选）
└── scripts/             ← 脚本目录（可选）
    └── run.sh
```

### SKILL.md 格式

```markdown
---
name: skill-name
description: Use when [triggering conditions]
---

# Skill Title

[Skill 内容：指令、工作流、检查清单等]
```

Frontmatter 字段：
- `name` — kebab-case 标识符，也是 `/name` 调用名
- `description` — 以 "Use when" 开头，描述触发条件（不描述工作流本身）

### 存储位置（两级合并）

```
~/.viv/skills/           ← 用户级（全局共享）
  ├── commit/SKILL.md
  └── review/SKILL.md

.viv/skills/             ← 项目级（项目特定）
  ├── deploy/SKILL.md
  └── test/SKILL.md
```

项目级同名 Skill 覆盖用户级。

---

## 2. SkillRegistry

### 数据结构

```rust
pub struct SkillEntry {
    pub name: String,           // frontmatter name
    pub description: String,    // frontmatter description
    pub content: String,        // SKILL.md frontmatter 之后的全部内容
    pub base_dir: String,       // skill 目录的绝对路径
    pub source: SkillSource,
}

pub enum SkillSource {
    User,      // ~/.viv/skills/
    Project,   // .viv/skills/
}

pub struct SkillRegistry {
    skills: Vec<SkillEntry>,
}
```

### 加载流程

Agent 启动时一次性加载：

1. 扫描 `~/.viv/skills/*/SKILL.md`，解析 frontmatter，加入 registry（source = User）
2. 扫描 `.viv/skills/*/SKILL.md`，解析 frontmatter，加入 registry（source = Project）
3. 项目级同名 skill 替换用户级

加载失败（缺少 frontmatter、文件损坏）的 skill 跳过并打日志，不影响其他 skill。

### Frontmatter 解析

不需要完整 YAML parser。简单逐行解析 `---` 分隔块内的 `key: value` 对：

```rust
pub fn parse_frontmatter(content: &str) -> Option<(HashMap<String, String>, String)>
```

返回 (frontmatter 字段, frontmatter 之后的内容)。

### 公开方法

```rust
impl SkillRegistry {
    pub fn load(user_dir: &str, project_dir: &str) -> Self
    pub fn get(&self, name: &str) -> Option<&SkillEntry>
    pub fn list(&self) -> &[SkillEntry]
    pub fn format_for_prompt(&self) -> String   // 生成 system prompt 注入文本
    pub fn is_empty(&self) -> bool
}
```

### format_for_prompt 输出

```
Available skills (invoke via Skill tool):
- commit: Use when the user asks to commit changes or create a git commit
- review: Use when completing tasks or before merging to verify work
- deploy: Use when deploying to production environment
```

---

## 3. Skill Tool

注册到 ToolRegistry 的新 Tool，Agent 通过 tool_use 调用。

```rust
pub struct SkillTool {
    registry: Arc<SkillRegistry>,
}
```

### Tool trait 实现

- `name()` → `"Skill"`
- `description()` → `"Invoke a skill by name. Returns the skill's full content for you to follow."`
- `input_schema()` → `{ "skill": string (required), "args": string (optional) }`
- `permission_level()` → `ReadOnly`
- `execute(input)`:
  1. 从 input 提取 `skill` 字段
  2. `registry.get(skill_name)` 查找
  3. 找到 → 返回: `"Base directory for this skill: {base_dir}\n\n{content}"`
  4. 找不到 → 返回错误: `"Skill '{name}' not found. Available: commit, review, deploy"`

### 返回格式

返回内容前缀 `base_dir`，让 Agent 知道辅助文件位置：

```
Base directory for this skill: /home/user/.viv/skills/commit

# Commit Skill

When committing:
1. Run `git status` to see changes
...
```

Agent 可以用 Read tool 读取 `base_dir` 下的辅助文件。

---

## 4. Agent 集成

### System Prompt 注入

在 `agent.rs` 的 `handle_input` 中，将 skill 列表传入已有的基础设施：

```rust
// 之前:
let system = build_system_prompt("", "", &memories, &mut self.prompt_cache);

// 之后:
let skill_list = self.skill_registry.format_for_prompt();
let system = build_system_prompt("", &skill_list, &memories, &mut self.prompt_cache);
```

Skill 列表通过已有的 `PromptCache.skills_hash` 缓存，只在 skill 内容变化时重新计算。

### 用户显式调用（/name）

用户输入 `/xxx` 时，TerminalUI 不做特殊处理，直接作为普通文本发送给 Agent。

System prompt 中包含指令告诉 Agent：

```
"/<skill-name>" (e.g., /commit) is shorthand to invoke a skill.
When you see user input starting with /, use the Skill tool to load and follow it.
```

Agent 收到 `/commit -m 'fix bug'` 后：
1. 识别 `/commit` 是 skill 调用
2. 调用 Skill tool: `{ "skill": "commit", "args": "-m 'fix bug'" }`
3. 获得 SKILL.md 内容
4. 按内容指示执行

这样 TerminalUI 不需要访问 SkillRegistry，保持 UI 与逻辑的分离。

### ToolRegistry 注册

在 `default_tools()` 中注册 SkillTool：

```rust
reg.register(Box::new(SkillTool::new(skill_registry.clone())));
```

---

## 5. 文件组织

```
src/
├── skill/
│   ├── mod.rs              // SkillRegistry + SkillEntry + SkillSource + 加载/解析
│   └── tool.rs             // SkillTool (impl Tool)
├── agent/
│   ├── agent.rs            // 改：初始化 SkillRegistry，传 skill_list 到 build_system_prompt
│   └── prompt.rs           // 改：在 skill block 中添加 /name 使用说明
└── tools/
    └── mod.rs              // 改：注册 SkillTool

tests/
├── skill/
│   ├── mod.rs
│   ├── registry_test.rs    // 加载、合并、查找、frontmatter 解析
│   └── tool_test.rs        // SkillTool execute 行为
```
