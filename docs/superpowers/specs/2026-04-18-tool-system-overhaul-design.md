# Tool System Overhaul Design

完善 viv 的 tool 支持，对齐 Claude Code 实现。

## 目标

| 维度 | 内容 |
|------|------|
| A 名称对齐 | `FileRead→Read`, `FileWrite→Write`, `FileEdit→Edit` |
| B Description | 所有 tool 对齐 Claude Code 级别的详细描述 |
| C 参数补全 | 修复 Glob 排序、实现 Read pages、Grep 可移植性、TodoWrite 格式对齐等 |
| D 新增 tool | NotebookEdit、Agent（SubAgent 并发）、WebSearch（Tavily） |

## 实现策略

Top-Down：先改框架（Tool trait / ToolRegistry / Agent loop），再逐个填充 tool 细节。

---

## Part 1: 框架层变更

### 1.1 Tool trait

保持不变：

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> JsonValue;
    fn execute(&self, input: &JsonValue)
        -> Pin<Box<dyn Future<Output = Result<String>> + Send + '_>>;
    fn permission_level(&self) -> PermissionLevel;
}
```

SubAgent 不需要扩展 trait — 它内部创建轻量 agent loop，不暴露新接口。

### 1.2 ToolRegistry — Agent 不变，改进 to_api_json

Agent 中 `tools: ToolRegistry` 保持不变（不需要 Arc）。SubAgent 是临时的，用时创建自己的 ToolRegistry，跑完销毁。

ToolRegistry 改进 `to_api_json()`：内部用 `JsonValue` 构建代替字符串拼接（避免转义问题），仍返回 `String`。调用方（`llm.rs`）无需变更。

新增 `default_tools_without(exclude, llm)` 工厂方法，供 SubAgent 创建不含指定 tool 的 registry：

```rust
impl ToolRegistry {
    pub fn default_tools_without(exclude: &str, llm: Arc<LLMClient>) -> Self {
        let mut reg = Self::default_tools(llm);
        reg.tools.retain(|t| t.name() != exclude);
        reg
    }
}
```

### 1.4 Tool 名称重映射

| 之前 | 之后 |
|------|------|
| `FileRead` | `Read` |
| `FileWrite` | `Write` |
| `FileEdit` | `Edit` |

`agent.rs` 中 LSP 通知判断同步更新：

```rust
// 之前
if matches!(name.as_str(), "FileEdit" | "FileWrite" | "MultiEdit")
// 之后
if matches!(name.as_str(), "Edit" | "Write" | "MultiEdit")
```

### 1.5 Agent Loop 并发改造

当 LLM 返回多个 tool_use 时，Agent tool 并发执行，普通 tool 串行执行：

```rust
let (agent_tasks, normal_tasks): (Vec<_>, Vec<_>) =
    tool_uses.iter().partition(|tu| tu.name == "Agent");

// 1. 串行执行普通 tool
for tu in &normal_tasks {
    let result = tool.execute(input).await;
    tool_results.push(result);
}

// 2. 并发执行 Agent tool
let agent_futures: Vec<_> = agent_tasks.iter()
    .map(|tu| tool.execute(input))
    .collect();
let agent_results = join_all(agent_futures).await;
tool_results.extend(agent_results);
```

### 1.6 join_all 实现

在 `core::runtime` 中添加零依赖的 `join_all`：

```rust
pub async fn join_all<F, T>(futures: Vec<F>) -> Vec<T>
where F: Future<Output = T> + Send
```

基于 viv 已有的单线程 async runtime，轮询所有 future 直到全部完成。

---

## Part 2: SubAgent 设计

### 2.1 结构

```rust
pub struct SubAgentTool {
    llm: Arc<LLMClient>,
}
```

文件位置：`src/tools/agent.rs`

SubAgent 是临时的、轻量的 — 用时创建，跑完销毁。每次 execute() 内部：
1. 创建新的 `ToolRegistry`（`default_tools_without("Agent", llm)`）
2. 创建新的 `Vec<Message>` 消息历史
3. 跑 agentic loop
4. 返回文本结果
5. 一切 drop

### 2.2 Tool 接口

- **名称**: `"Agent"`
- **权限**: `ReadOnly`（子 Agent 本身不直接写文件；内部 tool 各自控制权限，全部 auto-approve）

### 2.3 参数

```json
{
  "prompt": "string, required — 子 Agent 执行的任务描述",
  "model": "string, optional — fast|medium|slow, 默认 fast",
  "max_iterations": "number, optional — 最大迭代次数, 默认 20"
}
```

### 2.4 执行流程

```rust
async fn run_sub_agent(llm: &LLMClient, prompt: &str, tier: ModelTier, max_iter: usize)
    -> Result<String>
{
    let tools = ToolRegistry::default_tools_without("Agent", Arc::clone(&llm));
    let tools_json = tools.to_api_json();
    let system = vec![SystemBlock::dynamic("You are a sub-agent. Complete the task and report back.")];
    let mut messages = vec![Message::user_text(prompt)];
    let mut collected_text = String::new();

    for _ in 0..max_iter {
        let result = llm.stream_agent_async(
            &system, &messages, &tools_json, tier,
            |_| { /* 不转发到 UI */ }
        ).await?;

        for block in &result.text_blocks {
            if let ContentBlock::Text(t) = block { collected_text.push_str(t); }
        }

        if result.tool_uses.is_empty() || result.stop_reason == "end_turn" {
            break;
        }

        // 构建 assistant message
        let mut assistant_blocks = result.text_blocks;
        assistant_blocks.extend(result.tool_uses.clone());
        messages.push(Message::Assistant(assistant_blocks));

        // 执行 tool（全部 auto-approve，串行）
        let mut tool_results = Vec::new();
        for tu in &result.tool_uses {
            if let ContentBlock::ToolUse { id, name, input } = tu {
                let r = match tools.get(name) {
                    Some(tool) => tool.execute(input).await,
                    None => Err(Error::Tool(format!("unknown tool: {}", name))),
                };
                // 收集结果 ...
            }
        }
        messages.push(Message::User(tool_results));
    }

    Ok(collected_text)
}
```

### 2.5 UI 通知

- 启动时：`AgentMessage::ToolStart { name: "Agent", input: "prompt=..." }`
- 完成时：`AgentMessage::ToolEnd { name: "Agent", output: "..." }`
- 子 Agent 内部的 tool 调用不单独通知 UI

### 2.6 递归防护

`default_tools_without("Agent")` 自然排除了 Agent tool，防止无限递归。

---

## Part 3: 现有 Tool 改造

### 3.1 Read（原 FileRead）

**名称变更**: `FileRead` → `Read`

**Description**: 对齐 Claude Code — 强调绝对路径、cat -n 格式、默认 2000 行、offset/limit 使用指导。

**参数补全**:
- `pages`: 检测 `.pdf` 后缀，调用 `pdftotext -f <start> -l <end> <file> -` 命令。`pdftotext` 不存在时返回友好错误
- 二进制文件检测：读前 512 字节检查 NUL 字节，是二进制则返回提示信息而非崩溃

### 3.2 Write（原 FileWrite）

**名称变更**: `FileWrite` → `Write`

**Description**: 对齐 Claude Code — 强调先 Read 再 Write、偏好 Edit、不创建 md。

**实现改进**: 返回信息包含行数。

### 3.3 Edit（原 FileEdit）

**名称变更**: `FileEdit` → `Edit`

**Description**: 对齐 Claude Code — 强调先 Read、保留缩进、old_string 唯一性、replace_all 场景。

无参数变更。

### 3.4 MultiEdit

保留，description 补充原子性说明。无参数变更。

### 3.5 Bash

**Description**: 对齐 Claude Code 详细使用指南 — 不用 bash 做 grep/cat、引号处理、git 注意事项、timeout 说明。

**实现修复**: 从 schema 中移除 `dangerouslyDisableSandbox`（viv 无沙箱机制）。

### 3.6 Glob

**Description 修正**: 移除"sorted by modification time"的错误描述，改为真正按修改时间排序。

**实现修复**:
- `matches.sort()` 改为按 `metadata().modified()` 排序（最近修改的排前面）
- 添加默认忽略：`.git/`, `node_modules/`, `target/`

### 3.7 Grep

**Description**: 对齐 Claude Code — regex 语法说明、output_mode 用途、head_limit 说明。

**参数补全**:
- `context` 字段：添加为 `-C` 的别名（Claude Code 两个都有）
- `multiline` 可移植性：`-z` 是 GNU 扩展，在 macOS 上不可用。改为检测平台：Linux 用 `grep -Pz`，其他平台回退到 `grep -E`（multiline 降级为警告）
- `type` 扩展：添加常用语言的 type→glob 映射表：
  - `js` → `*.{js,jsx,mjs,cjs}`
  - `ts` → `*.{ts,tsx,mts,cts}`
  - `py` → `*.{py,pyi}`
  - `rs` → `*.rs`
  - `go` → `*.go`
  - 等

### 3.8 LS

保留，description 丰富。无实现变更。

### 3.9 TodoWrite

**格式对齐 Claude Code**:
- 移除 `id` 字段
- 移除 `priority` 字段
- 添加 `activeForm` 字段（进行时描述，如 "Running tests"）
- 保持 `content` + `status`（pending | in_progress | completed）

### 3.10 TodoRead

保留（viv 独有），无变更。

### 3.11 WebFetch

**Description**: 对齐 Claude Code — 不支持认证 URL、HTTP→HTTPS 升级。

**实现改进**:
- HTML → Markdown 转换器替代 `strip_html()`：处理 `<h1-6>` → `#`、`<a>` → `[text](url)`、`<code>/<pre>` → `` ` ``/` ``` `、`<ul>/<li>` → `- `、`<strong>` → `**`、`<em>` → `*`、`<p>` → 换行
- 截断阈值 8000 → 16000 字符

---

## Part 4: 新增 Tool — NotebookEdit

### 4.1 结构

文件位置：`src/tools/notebook.rs`

### 4.2 Tool 接口

- **名称**: `"NotebookEdit"`
- **权限**: `Write`

### 4.3 参数

```json
{
  "notebook_path": "string, required — ipynb 文件的绝对路径",
  "cell_id": "string, optional — cell 的 id 字段，用于定位",
  "cell_type": "string, optional — code|markdown, insert 时 required",
  "edit_mode": "string, optional — replace|insert|delete, 默认 replace",
  "new_source": "string, required — cell 的新内容（delete 时忽略）"
}
```

### 4.4 实现要点

- ipynb 是 JSON：`{ cells: [{ cell_type, source, id?, metadata, outputs }] }`
- 用 `JsonValue` 解析
- `cell_id` 匹配 cell 的 `id` 字段（nbformat 4.5+）；如无 id 字段，按数组索引回退
- replace: 替换 `source` 字段（转为 `["line1\n", "line2\n"]` 格式）
- insert: 在目标 cell 之后插入新 cell，生成 minimal metadata
- delete: 移除目标 cell
- 写回时保持原 JSON 结构（metadata, outputs, nbformat 等不动）

---

## Part 5: 新增 Tool — WebSearch（Tavily）

### 5.1 结构

文件位置：`src/tools/search.rs`

### 5.2 环境变量

`VIV_TAVILY_API_KEY` — required。未设置时 tool 仍注册，execute 时返回友好错误。

### 5.3 Tool 接口

- **名称**: `"WebSearch"`
- **权限**: `ReadOnly`

### 5.4 参数

```json
{
  "query": "string, required — 搜索关键词",
  "max_results": "number, optional — 默认 10, 最大 20",
  "search_depth": "string, optional — basic|advanced, 默认 basic",
  "topic": "string, optional — general|news, 默认 general",
  "include_domains": "array of string, optional",
  "exclude_domains": "array of string, optional"
}
```

### 5.5 实现要点

- POST `https://api.tavily.com/search`
- 复用 `AsyncTlsStream` + `HttpRequest`
- Request body: `{ api_key, query, max_results, search_depth, topic, include_domains, exclude_domains }`
- Response 解析：提取 `results[].{title, url, content}`
- 格式化输出：编号列表，每项 title + url + content snippet

---

## Part 6: default_tools 注册顺序

```rust
pub fn default_tools(llm: Arc<LLMClient>) -> Self {
    let mut reg = ToolRegistry::new();
    // 核心文件操作
    reg.register(Box::new(ReadTool));
    reg.register(Box::new(WriteTool));
    reg.register(Box::new(EditTool));
    reg.register(Box::new(MultiEditTool));
    reg.register(Box::new(NotebookEditTool));
    // 搜索
    reg.register(Box::new(GlobTool));
    reg.register(Box::new(GrepTool));
    reg.register(Box::new(LsTool));
    // 执行
    reg.register(Box::new(BashTool));
    // 任务管理
    reg.register(Box::new(TodoWriteTool::new(todo_path.clone())));
    reg.register(Box::new(TodoReadTool::new(todo_path)));
    // 网络
    reg.register(Box::new(WebFetchTool::new(Arc::clone(&llm))));
    reg.register(Box::new(WebSearchTool));
    // SubAgent — 轻量，用时创建自己的 ToolRegistry
    reg.register(Box::new(SubAgentTool::new(Arc::clone(&llm))));
    reg
}
```

SubAgent 没有循环引用问题 — 它只持有 `Arc<LLMClient>`，execute() 时临时创建自己的 `ToolRegistry`（通过 `default_tools_without("Agent", llm)`），跑完即销毁。

---

## 文件变更清单

| 文件 | 变更类型 |
|------|---------|
| `src/tools/mod.rs` | 修改 — to_api_json 用 JsonValue 构建, 新增 default_tools_without |
| `src/tools/bash.rs` | 修改 — description 丰富, 移除 dangerouslyDisableSandbox |
| `src/tools/file/read.rs` | 修改 — 改名 Read, description, PDF pages 实现, 二进制检测 |
| `src/tools/file/write.rs` | 修改 — 改名 Write, description, 返回行数 |
| `src/tools/file/edit.rs` | 修改 — 改名 Edit, description (EditTool + MultiEditTool) |
| `src/tools/file/glob.rs` | 修改 — description 修正, 按修改时间排序, 默认忽略目录 |
| `src/tools/file/grep.rs` | 修改 — description, context 别名, type 映射表, multiline 可移植性 |
| `src/tools/file/ls.rs` | 修改 — description 丰富 |
| `src/tools/todo.rs` | 修改 — TodoWrite 格式对齐 (移除 id/priority, 加 activeForm) |
| `src/tools/web.rs` | 修改 — description, HTML→Markdown 转换器, 截断 16000 |
| `src/tools/notebook.rs` | 新增 — NotebookEditTool |
| `src/tools/search.rs` | 新增 — WebSearchTool (Tavily) |
| `src/tools/agent.rs` | 新增 — SubAgentTool |
| `src/agent/agent.rs` | 修改 — LSP 名称更新, Agent tool 并发执行 |
| `src/core/runtime/mod.rs` | 修改 — 新增 join_all |
| `tests/tools/` | 修改 — 所有引用旧名称的测试更新 |
| `tests/tools/notebook_test.rs` | 新增 |
| `tests/tools/search_test.rs` | 新增 |
| `tests/tools/agent_test.rs` | 新增 |
