# ufox-arc：面向 AI 编程工具的分步实施文档

## 1. 文档目的

本文不是纯功能说明，而是给 Claude、Cursor 等 AI 编程工具使用的实施文档。
目标是让 AI 按固定顺序逐步实现 `ufox-arc`，每一阶段都能形成可运行、可测试、可继续迭代的结果，而不是一次性生成一套难以落地的大而全设计。

项目目标：基于 `ufox-llm`，用 Rust 构建一个轻量、可扩展、生产就绪的 AI Agent 运行时。

核心设计目标：

- **类型安全**：借助 Rust 类型系统保证 Agent 执行可靠性
- **高性能**：零开销抽象、async/await、最小运行时开销
- **可扩展**：工具、记忆后端可插拔
- **多模态**：统一处理文本、图片、音频、视频、文档输入
- **类型复用**：优先复用 `ufox-llm` 的消息与多模态内容定义
- **可观测**：具备日志、链路追踪与指标采集能力

## 2. AI 实施方式

### 2.1 推荐工作模式

AI 工具实现本项目时，必须遵循以下顺序：

1. 先完成一个阶段的最小闭环，再进入下一阶段。
2. 每一阶段都要给出明确交付物，而不是只补接口。
3. 优先实现能跑通示例的主路径，再补错误处理和扩展点。
4. 除非当前阶段明确需要，否则不要提前引入多 Agent、复杂调度、热重载等高级能力。
5. 每次改动都要说明：改了哪些文件、实现了什么、缺了什么、下一步做什么。

### 2.2 每阶段统一输出格式

AI 在完成任一阶段时，输出应至少包含：

- 本阶段完成内容
- 涉及文件列表
- 公开 API 变化
- 运行方式或示例
- 已补充的测试
- 尚未覆盖的边界与后续建议

### 2.3 实现约束

- 外部消息、多模态内容、工具调用结果优先复用 `ufox-llm`
- 默认采用异步实现，避免阻塞 I/O
- 错误处理使用结构化错误，避免在主链路 `panic`
- 可序列化的数据结构尽量都支持 `serde`
- 先保证最小正确性，再做性能优化
- 没有必要时，不要为"未来可能会用到"的场景过度抽象
- 多轮上下文与恢复围绕 `session_id` 建模，长期偏好与跨会话记忆围绕 `user_id` 建模
- 第一版默认同一 `session_id` 不允许并发写；不同 `session_id` 可以并发
- 对外会话 API 统一使用 `chat()` / `chat_stream()`
- 写代码时必须遵守第 10.8 节的注释与编码规范，尤其是 `pub` 项文档注释、`unsafe` 的 `// SAFETY:` 说明，以及"只注释为什么、不注释是什么"

## 3. 总体验收标准

项目最终完成时，至少满足以下标准：

- ✅ Agent 能自主完成多步骤任务
- ✅ 工具调用可以可靠执行，并具备错误恢复能力
- ✅ 支持文本、图片、音频、视频、文档等多模态输入
- ✅ 记忆支持跨会话持久化，并区分 `user memory` 与 `session memory`
- ✅ HITL 中断在需要人工确认时正常触发
- ✅ 生产能力完善：日志、指标、错误处理可用
- ✅ 单步额外运行时开销目标小于 `100ms`（不含外部模型与工具耗时）
- ✅ 测试覆盖核心路径，包含单元测试与集成测试
- ✅ 文档完整，能指导外部接入与二次开发

**注意：** 多 Agent 协作和技能系统推迟到第二版，第一版专注核心能力。

## 4. 建议阅读与实现顺序

AI 工具应按下面顺序阅读和实施：

1. **第 5 节：分阶段实施计划**
   先明确每个阶段的目标、边界、输入输出和完成定义。
2. **第 6 节：核心架构要求**
   再理解运行时中的 Session、Agent 循环、工具、记忆、HITL。
3. **第 7 节：目录结构与模块落点**
   确认代码应该放在哪里，模块边界如何划分。
4. **第 8 节：公开 API 示例**
   按外部使用方式校验 API 设计是否合理。
5. **第 9 节：配置约束**
   确认运行时如何落地配置。
6. **第 10 节：工程质量要求**
   最后补齐质量、安全、可观测性、测试和性能要求。

## 5. 分阶段实施计划

本节是 AI 工具的主执行入口。默认按阶段顺序推进，除非用户明确要求跳阶段。

### 阶段 1：最小可运行 Agent 与 Session

**目标**：先做出一个能调用 `ufox-llm` 完成单轮问答，并具备最小 Session 的 Agent。

**本阶段必须实现：**

- `Agent` 主结构体与 `Agent::builder()`
- 基础 `AgentConfig`
- `Session` 的最小结构
- `agent.session(user_id, session_id).await?`
- `session.chat()` 与 `session.chat_stream()`
- 最小可运行示例 `examples/simple_agent.rs`
- 基础错误类型与结果类型

**本阶段不要做：**

- 复杂工具系统
- 持久化 checkpoint
- 多 Agent 协作
- 复杂并发控制
- `UserHandle` 中间层（直接用 `user_id` 字符串）

**完成定义：**

- 可以构造一个 Agent，并打开某个用户下的会话
- 同一个 `session` 能连续执行多轮 `chat()`
- 支持普通输出与流式输出
- 外部调用路径简洁，示例可直接运行
- 至少有基础单元测试或示例级验证

**建议落点：**

- `src/lib.rs`
- `src/agent/mod.rs`
- `src/agent/config.rs`
- `src/session/mod.rs`
- `src/error.rs`
- `examples/simple_agent.rs`

### 阶段 2：推理循环与执行轨迹

**目标**：实现可追踪的 Agent 执行循环，参考 ReAct 和 LangChain 的成熟实践。

**关于 5 步循环的设计决策：**

参考社区实践（LangChain、AutoGPT、ReAct 论文），推荐实现灵活的 5 步模型：

1. **Perceive（感知）**：从记忆/环境检索上下文（可选，简单场景跳过）
2. **Think（思考）**：LLM 推理，生成回复或工具调用
3. **Act（行动）**：执行工具调用
4. **Observe（观察）**：格式化工具结果，提取关键信息（可选，简单场景直接用原始结果）
5. **Reflect（反思）**：自我评估，判断是否需要重试（可选，复杂任务启用）

**核心原则：**
- 简单场景：`Think → Act → Think → Completion`（3 步）
- 复杂场景：`Perceive → Think → Act → Observe → Reflect → ...`（5 步）
- 通过配置控制是否启用 Perceive/Observe/Reflect

**本阶段必须实现：**

- `StepKind` 枚举：`Perceive | Think | Act | Observe | Reflect | Completion`
- `ExecutionStep`、`ExecutionTrace`、`ExecutionResult`
- 执行步数限制与超时控制
- 中间步骤记录与返回
- 基础重试或失败终止机制
- 配置项：`enable_perceive`、`enable_observe`、`enable_reflect`

**完成定义：**

- 一次 `session.chat()` 至少能输出完整步骤轨迹
- 每一步都可序列化或具备清晰的数据结构
- 简单场景下只有 Think/Act/Completion 步骤
- 复杂场景下可以启用完整 5 步
- 运行结果能带上 `user_id` 与 `session_id`

**建议落点：**

- `src/agent/loop_.rs`
- `src/agent/step.rs`
- `src/session/mod.rs`

**实现建议：**

```rust
pub enum StepKind {
    /// 感知：从记忆/环境检索上下文（可选）
    Perceive,
    /// 思考：LLM 推理，生成回复或工具调用
    Think,
    /// 行动：执行工具调用
    Act,
    /// 观察：格式化工具结果（可选）
    Observe,
    /// 反思：自我评估，判断是否重试（可选）
    Reflect,
    /// 完成：最终响应
    Completion,
}

pub struct ExecutionConfig {
    pub max_iterations: usize,
    pub timeout: Duration,
    /// 是否启用 Perceive 步骤（从记忆检索）
    pub enable_perceive: bool,
    /// 是否启用 Observe 步骤（格式化工具结果）
    pub enable_observe: bool,
    /// 是否启用 Reflect 步骤（自我评估）
    pub enable_reflect: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            timeout: Duration::from_secs(300),
            // 默认简单模式：只有 Think/Act/Completion
            enable_perceive: false,
            enable_observe: false,
            enable_reflect: false,
        }
    }
}
```
- `src/agent/step.rs`
- `src/session/mod.rs`

### 阶段 3：工具系统

**目标**：让 Agent 能安全地注册、选择和执行工具，形成完整闭环。

**本阶段必须实现：**

- `Tool` trait
- `ToolRegistry`
- 基于 JSON Schema 的参数定义
- 工具执行前校验
- 工具执行结果标准化
- 至少一组内置工具，建议先做文件读、文件写、Shell

**完成定义：**

- Agent 能生成工具调用请求并执行工具
- 工具结果能回注到当前 `session` 的消息历史中
- 工具错误能结构化返回，而不是直接中断整个进程
- 至少有一个"问答 -> 工具调用 -> 观察结果 -> 最终回复"的示例

**建议落点：**

- `src/tools/mod.rs`
- `src/tools/result.rs`
- `src/tools/builtin/file.rs`
- `src/tools/builtin/shell.rs`
- `examples/tool_agent.rs`

### 阶段 4：记忆系统

**目标**：支持跨轮次、跨会话的上下文补充与持久化，并明确区分用户级与会话级记忆。

**本阶段必须实现：**

- `MemoryStore` trait（统一接口，通过 `MemoryScope` 区分用户/会话）
- `Memory`、`MemoryFilter`、`MemoryScope`
- `MemoryScope::User` 与 `MemoryScope::Session` 两层作用域
- 开发期内存后端
- 本地持久化后端，优先 SQLite
- 执行前检索、执行后写回

**完成定义：**

- 同一 `session` 的第二轮运行能利用第一轮保存的信息
- 同一 `user` 的新 `session` 能读取长期偏好
- 记忆支持按标签、时间或文本查询进行过滤
- 至少有跨会话示例

**建议落点：**

- `src/memory/mod.rs`
- `src/memory/backend/in_memory.rs`
- `src/memory/backend/sqlite.rs`
- `src/memory/strategy.rs`
- `examples/memory_agent.rs`

### 阶段 5：多模态输入与提取

**目标**：让运行时统一接受文本、图片、音频、视频、文档输入，并在需要时进行提取。

**本阶段必须实现：**

- `ufox-llm::Message` / `ContentPart` 直接接入
- 统一通过 `session.chat(message)` 进入多模态输入
- 区分原始模态与派生模态
- 文档、音频、视频的基础提取入口
- 来源信息记录

**完成定义：**

- `Session` 可以接收多模态输入消息
- 文档输入能转成后续推理可消费的文本或图片片段
- `session memory` 中能保留来源、页码范围、时间片等上下文
- 同一 `session` 的后续追问不必重复上传同一份媒体

**建议落点：**

- `src/memory/mod.rs`
- `src/session/mod.rs`
- `examples/multimodal_agent.rs`

### 阶段 6：HITL 人机协同（简化版）

**目标**：在高风险操作前支持人工确认。

**本阶段必须实现：**

- `InterruptReason` 枚举
- `InterruptHandler` trait
- `InterruptDecision` 枚举
- 工具执行前的确认拦截
- CLI 版本确认处理器

**完成定义：**

- 高风险工具在执行前可以被拦截
- 用户可继续、取消或修改参数后继续
- 中断上下文能定位到具体 `user_id` 与 `session_id`
- 存在最小 CLI 示例

**建议落点：**

- `src/interrupt/mod.rs`
- `src/interrupt/cli.rs`
- `examples/hitl_agent.rs`

**注意：** 第一版只做工具确认，不做低置信度判断、错误恢复决策等复杂场景。

### 阶段 7：生产加固

**目标**：补齐配置、观测、测试、限流、会话恢复等生产能力。

**本阶段必须实现：**

- 配置加载
- tracing 日志与指标埋点
- 通过 `user_id + session_id` 打开并继续会话
- 基础限流、超时、预算控制
- 单元测试、集成测试、基准测试骨架

**完成定义：**

- 能观测一次完整执行链路
- 能通过 `user_id + session_id` 恢复会话状态
- 关键模块有测试覆盖
- 工程具备继续迭代的生产基础

**注意：** 多 Agent 协作和技能系统推迟到第二版，第一版专注核心能力。

## 6. 核心架构要求

本节描述最终目标架构，但 AI 工具实现时应始终服从第 5 节的阶段边界。

### 6.1 用户与会话模型

`Agent` 只负责模型、工具、配置等静态能力。
多轮对话、恢复状态、会话级上下文和用户长期偏好，统一围绕 `Session` 建模。

推荐数据结构：

```rust
pub struct UserId(pub String);
pub struct SessionId(pub String);

pub struct Session {
    user_id: UserId,
    session_id: SessionId,
    agent: Arc<Agent>,
    messages: Vec<Message>,
    state: SessionState,
}

pub enum SessionState {
    Idle,
    Running { started_at: Instant },
}

pub enum SessionInput {
    Text(String),
    Message(Message),
}

impl Agent {
    /// 打开或创建会话。`session_id` 不传时由框架生成 UUID。
    pub async fn session(
        &self,
        user_id: impl Into<UserId>,
        session_id: impl Into<Option<SessionId>>,
    ) -> Result<Session>;
}

impl Session {
    pub async fn chat<I>(&mut self, input: I) -> Result<ExecutionResult>
    where
        I: Into<SessionInput>;

    pub async fn chat_stream<I>(&mut self, input: I) -> Result<ExecutionEventStream>
    where
        I: Into<SessionInput>;
}
```

推荐外部调用方式：

```rust
// 指定 session_id，用于恢复已有会话
let mut session = agent.session("user_123", Some("design-review")).await?;
let result = session.chat("继续刚才的话题").await?;

// 不指定 session_id，框架自动生成新会话
let mut session = agent.session("user_123", None).await?;
let result = session.chat("开始新的对话").await?;
```

设计要求：

- `session_id` 是多轮会话与恢复的主键
- `session_id` 可选，不传时框架自动生成 UUID
- `user_id` 是长期偏好和跨会话记忆的归属键
- 第一版不引入复杂身份体系，只保留 `user_id + session_id`
- 同一 `session_id` 通过 `SessionState` 防止并发写，新请求等待而不是直接失败
- 多模态输入与文本输入共用 `chat()` / `chat_stream()` 入口
- **不要引入 `UserHandle` 中间层**，直接 `agent.session(user_id, session_id)` 即可

### 6.2 Agent 推理循环

实现灵活的 5 步执行循环，参考 ReAct 和 LangChain 的成熟实践：

```text
Perceive（可选）→ Think → Act → Observe（可选）→ Reflect（可选）→ ... → Completion
```

推荐数据结构：

```rust
use ufox_llm::{
    ChatChunk, ChatResponse, ContentPart, FinishReason, Message, ToolCall, ToolResult, Usage,
};

pub struct ExecutionConfig {
    pub max_iterations: usize,
    pub timeout: Duration,
    /// 是否启用 Perceive 步骤（从记忆检索）
    pub enable_perceive: bool,
    /// 是否启用 Observe 步骤（格式化工具结果）
    pub enable_observe: bool,
    /// 是否启用 Reflect 步骤（自我评估）
    pub enable_reflect: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            timeout: Duration::from_secs(300),
            // 默认简单模式：只有 Think/Act/Completion
            enable_perceive: false,
            enable_observe: false,
            enable_reflect: false,
        }
    }
}

pub enum StepKind {
    /// 感知：从记忆/环境检索上下文（可选）
    Perceive,
    /// 思考：LLM 推理，生成回复或工具调用
    Think,
    /// 行动：执行工具调用
    Act,
    /// 观察：格式化工具结果（可选）
    Observe,
    /// 反思：自我评估，判断是否重试（可选）
    Reflect,
    /// 完成：最终响应
    Completion,
}

pub struct ExecutionStep {
    pub index: usize,
    pub kind: StepKind,
    pub input: StepInput,
    pub output: StepOutput,
    pub duration: Duration,
    pub usage: Option<Usage>,
}

pub enum StepInput {
    Query(String),
    Messages(Vec<Message>),
    ToolCalls(Vec<ToolCall>),
    ToolResults(Vec<ToolResult>),
}

pub enum StepOutput {
    MemoryHits(Vec<Memory>),
    Response {
        message: Message,
        finish_reason: FinishReason,
        tool_calls: Vec<ToolCall>,
    },
    ToolResults(Vec<ToolResult>),
    FormattedObservation(String),
    ReflectionDecision {
        should_retry: bool,
        reason: String,
    },
    Final {
        message: Message,
        finish_reason: FinishReason,
    },
}

pub struct ExecutionTrace {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub steps: Vec<ExecutionStep>,
    pub state: ExecutionState,
    pub total_duration: Duration,
    pub total_usage: Usage,
}

pub enum ExecutionState {
    Running,
    Completed,
    Failed { error: String },
    Interrupted { reason: InterruptReason },
    TimedOut,
    MaxIterationsReached,
}

pub struct ExecutionResult {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub response: ChatResponse,
    pub trace: ExecutionTrace,
}

pub struct ExecutionEvent {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub chunk: Option<ChatChunk>,
    pub step: Option<ExecutionStep>,
    pub state_change: Option<ExecutionState>,
}
```

设计要求：

- 支持 5 步完整循环：Perceive → Think → Act → Observe → Reflect
- 通过配置控制是否启用可选步骤（Perceive/Observe/Reflect）
- 默认简单模式：只有 Think/Act/Completion（3 步）
- 每个步骤有明确的输入输出类型，避免 `Option` 嵌套
- 完全复用 `ufox-llm` 的 `Message`/`ToolCall`/`ToolResult`/`ChatResponse`
- 执行循环必须支持超时、最大步数和错误退出
- 所有运行结果都要能回挂到当前 `session`
- **不要引入 `ExecutionEngine` 作为公开 API**，它是 `Session` 的内部实现细节
- **不要引入 `ExecutionContext`**，相关信息直接从 `Session` 获取或作为内部临时变量

### 6.3 工具系统

目标能力：

- Schema 驱动的参数定义
- Rust 类型安全序列化
- 异步执行
- 参数校验与错误上下文
- 可选沙箱隔离

推荐接口：

```rust
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub parameters_schema: serde_json::Value,
    pub requires_confirmation: bool,
    pub timeout: Duration,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> &ToolMetadata;

    async fn execute(
        &self,
        params: serde_json::Value,
    ) -> Result<ufox_llm::ToolResultPayload, ToolError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register(&mut self, tool: impl Tool + 'static) -> Result<()>;
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>>;
    pub fn list_names(&self) -> Vec<String>;
    
    /// 执行工具调用（包含校验、超时、确认逻辑）
    pub async fn execute(
        &self,
        tool_call: &ToolCall,
        interrupt_handler: Option<&dyn InterruptHandler>,
    ) -> Result<ToolResult>;
}
```

工具执行流程：

1. LLM 生成工具调用请求
2. 根据 Schema 校验参数
3. 判断是否需要人工确认（通过 HITL）
4. 带超时执行工具
5. 归一化返回结果与错误
6. 将结果追加进当前 `session` 的消息历史
7. 记录执行指标

设计要求：

- **不要引入 `ToolContext`**，工具执行不需要额外上下文
- 工具确认逻辑统一通过 HITL 处理，不在工具层重复
- 第一版不处理多模态参数引用，简化实现
- 静态配置（`requires_confirmation`、`timeout`）提取到 `ToolMetadata`，避免每次调用都查询

### 6.4 记忆系统

记忆最小分层：

1. **Session Memory**：当前会话中的临时上下文、文档摘要、工具结果、阶段性结论
2. **User Memory**：用户长期偏好、稳定约束、跨会话复用的事实

推荐接口：

```rust
pub enum MemoryScope {
    Session { session_id: SessionId },
    User { user_id: UserId },
}

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn insert(&mut self, memory: Memory) -> Result<MemoryId>;
    async fn find(&self, filter: MemoryFilter) -> Result<Vec<Memory>>;
    async fn replace(&mut self, id: MemoryId, memory: Memory) -> Result<()>;
    async fn remove(&mut self, id: MemoryId) -> Result<()>;
    async fn search_similar(
        &self,
        embedding: Vec<f32>,
        top_k: usize,
    ) -> Result<Vec<Memory>>;
}

pub struct Memory {
    id: MemoryId,
    scope: MemoryScope,
    parts: Vec<ContentPart>,
    metadata: HashMap<String, serde_json::Value>,
    embeddings: Option<Vec<f32>>,
    timestamp: DateTime<Utc>,
    tags: Vec<String>,
}
```

写入规则：

- 文档提取结果、任务中间结论、当前会话摘要写入 `Session Memory`
- 用户风格偏好、长期事实、稳定约束写入 `User Memory`

检索顺序建议：

1. 先检索当前 `session memory`
2. 再补充 `user memory`
3. 最后合并结果进入本轮上下文

要求：

- 统一使用一个 `MemoryStore` trait，通过 `MemoryScope` 区分用户/会话
- **不要拆分成 `UserMemoryStore` 和 `SessionMemoryStore` 两个 trait**
- 默认保存引用与派生结果，而不是大块原始二进制
- 支持按标签、时间范围过滤
- 检索策略综合时效性、相关性与重要性

### 6.5 多模态输入

多模态内容定义：

```rust
// 直接复用 ufox-llm 的类型
pub use ufox_llm::{ContentPart, Image, Audio, Video, MediaSource};

pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
    Document,
}

pub struct ExtractedContent {
    pub modality: Modality,
    pub parts: Vec<ContentPart>,
    pub source: MediaSource,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

要求：

- `Session` 可以接收多模态输入消息
- 文档输入能转成后续推理可消费的文本或图片片段
- `session memory` 中能保留来源、页码范围、时间片等上下文
- 同一 `session` 的后续追问不必重复上传同一份媒体

### 6.6 最小存储约束

第一版不强制完整的执行快照模型，但至少要求能持久化以下信息：

- `sessions`：必须带 `user_id + session_id`
- `messages`：必须按 `session_id` 归档
- `session memory`：必须按 `session_id` 归档
- `user memory`：必须按 `user_id` 归档

实现要求：

- 能按 `user_id + session_id` 打开已有会话
- 消息历史必须按写入顺序可重建
- 大媒体默认存引用，不存整块二进制
- 所有持久化结构优先支持 `serde`

### 6.7 并发语义

第一版采用最简单、最容易落地的规则：

- 同一 `session_id` 不允许并发写
- 同一 `user_id` 下的不同 `session` 可以并发
- 不同用户的不同 `session` 可以并发
- 同一 `session` 正在执行时，新的写请求直接返回 `SessionBusy`

### 6.8 HITL

中断触发点：

- 破坏性操作前
- 工具执行错误后的恢复决策（可选）
- 用户自定义断点（可选）

推荐接口：

```rust
pub enum InterruptReason {
    ToolConfirmation { tool: String, params: serde_json::Value },
    ErrorRecovery { error: String, proposed_action: String },
    UserBreakpoint { condition: String },
}

#[async_trait]
pub trait InterruptHandler: Send + Sync {
    async fn handle_interrupt(
        &self,
        reason: InterruptReason,
        user_id: &UserId,
        session_id: &SessionId,
    ) -> Result<InterruptDecision>;
}

pub enum InterruptDecision {
    Continue,
    Abort,
    Retry,
    ModifyAndContinue(serde_json::Value),
}
```

要求：

- 中断上下文必须能追溯到 `user_id` 与 `session_id`
- 会话恢复后仍能继续人工确认后的执行流程
- 第一版只做工具确认，不做低置信度判断等复杂场景

## 7. 模块目录结构

建议目录如下，AI 工具实现时应尽量遵守，不随意打散模块边界：

```text
ufox-arc/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── agent/
│   │   ├── mod.rs
│   │   ├── loop_.rs
│   │   ├── step.rs
│   │   └── config.rs
│   ├── session/
│   │   └── mod.rs
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── result.rs
│   │   └── builtin/
│   │       ├── mod.rs
│   │       ├── file.rs
│   │       ├── shell.rs
│   │       ├── web.rs
│   │       └── code.rs
│   ├── memory/
│   │   ├── mod.rs
│   │   ├── strategy.rs
│   │   └── backend/
│   │       ├── mod.rs
│   │       ├── in_memory.rs
│   │       ├── sqlite.rs
│   │       └── vector.rs
│   └── interrupt/
│       ├── mod.rs
│       └── cli.rs
└── examples/
    ├── simple_agent.rs
    ├── tool_agent.rs
    ├── stream_agent.rs
    ├── multimodal_agent.rs
    ├── memory_agent.rs
    └── hitl_agent.rs
```

**注意：** 删除了 `src/user/`（不需要 UserHandle）、`src/skills/`（推迟到第二版）、`src/team/`（推迟到第二版）。

## 8. 公开 API 示例

以下示例用于约束最终使用体验。AI 工具实现时，应尽量让外部调用方式接近这些示例。

### 8.1 最简 Agent

```rust
use ufox_arc::{Agent, AgentConfig};
use ufox_llm::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let agent = Agent::builder()
        .llm(client)
        .system("你是一位 Rust 专家，回答简洁准确。")
        .config(AgentConfig::default())
        .build()?;

    let mut session = agent.session("user_123", Some("intro")).await?;

    let result = session.chat("解释 Rust 中的所有权规则").await?;
    println!("{}", result.response.text);
    Ok(())
}
```

### 8.2 工具调用闭环

```rust
use serde_json::json;
use ufox_arc::{Agent, Tool, ToolMetadata};
use ufox_llm::{Client, ToolResultPayload};
use std::time::Duration;

struct WeatherTool {
    metadata: ToolMetadata,
}

impl WeatherTool {
    fn new() -> Self {
        Self {
            metadata: ToolMetadata {
                name: "get_weather".to_string(),
                description: "查询指定城市的实时天气".to_string(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "city": { "type": "string", "description": "城市名" }
                    },
                    "required": ["city"]
                }),
                requires_confirmation: false,
                timeout: Duration::from_secs(10),
            },
        }
    }
}

#[async_trait::async_trait]
impl Tool for WeatherTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn execute(
        &self,
        params: serde_json::Value,
    ) -> Result<ToolResultPayload, ufox_arc::ToolError> {
        let city = params["city"].as_str().unwrap_or("unknown");
        Ok(ToolResultPayload::text(format!("{city}：晴，24°C")))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let agent = Agent::builder()
        .llm(client)
        .tool(WeatherTool::new())
        .max_iterations(5)
        .build()?;

    let mut session = agent.session("user_123", "weather").await?;

    let result = session.chat("帮我查询杭州和上海的天气，给出穿衣建议").await?;
    println!("{}", result.response.text);
    Ok(())
}
```

### 8.3 流式输出

```rust
use futures::StreamExt;
use ufox_arc::Agent;
use ufox_llm::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let agent = Agent::builder()
        .llm(client)
        .build()?;

    let mut session = agent.session("user_123", "poem").await?;

    let mut stream = session.chat_stream("写一首关于 Rust 的七言绝句").await?;
    while let Some(event) = stream.next().await {
        let event = event?;
        if let Some(chunk) = event.chunk {
            if let Some(text) = chunk.text {
                print!("{text}");
            }
        }
    }
    println!();
    Ok(())
}
```

### 8.4 记忆读写

```rust
use ufox_arc::{memory::SqliteMemory, Agent};
use ufox_llm::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let memory = SqliteMemory::open("./agent_memory.db").await?;
    let client = Client::from_env()?;

    let agent = Agent::builder()
        .llm(client)
        .memory(memory)
        .build()?;

    // 写入用户长期偏好
    let mut profile = agent.session("user_123", "profile").await?;
    profile.chat("我喜欢简洁的代码风格，不喜欢过度抽象").await?;

    // 在另一个会话中继续利用同一用户的长期偏好
    let mut review = agent.session("user_123", "review").await?;
    let result = review.chat("帮我 review 这段代码是否符合我的风格偏好").await?;

    println!("{}", result.response.text);
    Ok(())
}
```

### 8.5 多模态输入

```rust
use ufox_arc::Agent;
use ufox_llm::{Client, ContentPart, Image, MediaSource, Message, Role};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let agent = Agent::builder()
        .llm(client)
        .build()?;

    let mut session = agent.session("user_123", "panic-analysis").await?;

    let input = Message {
        role: Role::User,
        content: vec![
            ContentPart::text("阅读这张报错截图，分析根因并给出修复建议"),
            ContentPart::Image(Image {
                source: MediaSource::File {
                    path: "./fixtures/panic.png".into(),
                },
                fidelity: None,
            }),
        ],
        name: None,
    };

    let first = session.chat(input).await?;
    println!("first: {}", first.response.text);

    // 继续在同一会话中追问，不需要重复传图片
    let second = session.chat("基于刚才的截图，再给我一个最小修复 patch").await?;
    println!("second: {}", second.response.text);
    Ok(())
}
```

### 8.6 人工确认

```rust
use ufox_arc::{
    interrupt::CliInterruptHandler,
    tools::builtin::ShellTool,
    Agent,
};
use ufox_llm::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;
    let agent = Agent::builder()
        .llm(client)
        .tool(ShellTool::new())
        .interrupt_handler(CliInterruptHandler::default())
        .build()?;

    let mut session = agent.session("user_123", "ops").await?;

    let result = session.chat("列出当前目录下最近修改的 5 个文件").await?;
    println!("{}", result.response.text);
    Ok(())
}
```

## 9. 配置

建议配置结构如下：

```toml
[agent]
max_iterations = 10
timeout_seconds = 300
temperature = 0.7
enable_multimodal = true

[sessions]
enabled = true
backend = "sqlite"
idle_ttl_seconds = 86400

[tools]
enabled = ["file_read", "file_write", "shell", "web_search"]
confirmation_required = ["file_write", "shell"]

[memory]
backend = "sqlite"
path = "./agent_memory.db"
max_entries = 10000
enable_user_memory = true
enable_session_memory = true

[multimodal]
max_image_bytes = 10485760
max_document_bytes = 20971520
max_audio_seconds = 600
max_video_seconds = 300
extract_text_from_documents = true
transcribe_audio = true
sample_video_frames = true

[observability]
log_level = "info"
enable_tracing = true
metrics_port = 9090
```

配置要求：

- Agent、Session、工具、记忆、多模态、观测配置分离
- 默认值要能支撑本地开发
- 支持从 TOML、环境变量或 Builder 注入覆盖
- 配置文件中的 `confirmation_required` 可以覆盖工具的 `requires_confirmation` 声明

## 10. 工程质量要求

### 10.1 错误处理

- 所有公开 API 返回 `Result<T, E>`，不在主链路 `panic`
- 错误类型必须实现 `std::error::Error`
- 错误信息应包含足够上下文，便于定位问题
- 区分可恢复错误与不可恢复错误

推荐错误结构：

```rust
#[derive(Debug, thiserror::Error)]
pub enum ArcError {
    #[error("LLM error: {0}")]
    Llm(#[from] ufox_llm::LlmError),
    
    #[error("Tool error: {tool} - {message}")]
    Tool { tool: String, message: String },
    
    #[error("Memory error: {0}")]
    Memory(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
    
    #[error("Max iterations reached: {0}")]
    MaxIterations(usize),
}
```

### 10.2 日志与观测

- 使用 `tracing` 进行结构化日志
- 关键路径必须有 `span` 覆盖
- 错误日志必须包含 `user_id` 与 `session_id`
- 支持按 `session_id` 过滤日志

推荐埋点位置：

- Agent 初始化
- Session 创建与恢复
- 每个执行步骤的开始与结束
- 工具调用前后
- 记忆读写
- HITL 中断触发

### 10.3 测试

测试覆盖要求：

- 单元测试：核心逻辑与数据结构
- 集成测试：完整执行流程
- 示例测试：所有 `examples/` 下的示例必须可运行

推荐测试结构：

```text
tests/
├── unit/
│   ├── agent_test.rs
│   ├── session_test.rs
│   ├── tools_test.rs
│   └── memory_test.rs
├── integration/
│   ├── simple_flow_test.rs
│   ├── tool_calling_test.rs
│   └── memory_persistence_test.rs
└── fixtures/
    ├── test_image.png
    └── test_document.pdf
```

### 10.4 性能

性能目标：

- 单步额外运行时开销 < 100ms（不含 LLM 与工具耗时）
- 记忆检索延迟 < 50ms（本地 SQLite）
- 支持流式输出，首 token 延迟 < 200ms

性能测试：

- 使用 `criterion` 进行基准测试
- 关键路径必须有性能测试覆盖

### 10.5 安全

安全要求：

- 工具执行必须支持沙箱隔离（可选）
- 敏感信息（API Key）不得记录到日志
- 文件操作必须校验路径，防止目录穿越
- Shell 命令必须转义，防止注入

### 10.6 文档

文档要求：

- 所有公开 API 必须有文档注释
- 复杂逻辑必须有内联注释说明
- `README.md` 必须包含快速开始与示例
- `ARCHITECTURE.md` 必须说明核心设计决策

### 10.7 依赖管理

依赖要求：

- 优先使用成熟、维护活跃的 crate
- 避免引入过多依赖，控制编译时间
- 所有依赖必须在 `Cargo.toml` 中明确版本

推荐依赖：

```toml
[dependencies]
ufox-llm = { path = "../ufox-llm" }
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
sqlx = { version = "0.7", features = ["sqlite", "runtime-tokio"] }
chrono = { version = "0.4", features = ["serde"] }
futures = "0.3"

[dev-dependencies]
criterion = "0.5"
tempfile = "3"
```

### 10.8 代码风格

代码风格要求：

- 遵循 Rust 官方风格指南
- 使用 `rustfmt` 格式化代码
- 使用 `clippy` 检查代码质量
- 所有 `pub` 项必须有文档注释
- 复杂函数必须有注释说明意图
- 避免过长函数，单个函数不超过 100 行
- 避免过深嵌套，嵌套层级不超过 4 层

**注释规范：**

- **只注释为什么（Why），不注释是什么（What）**
- 代码本身应该是自解释的，通过良好的命名和结构表达"是什么"
- 注释应该解释设计决策、权衡、限制、非显而易见的行为
- `unsafe` 代码必须有 `// SAFETY:` 注释说明为什么是安全的
- 复杂算法必须有注释说明思路
- Workaround 必须有注释说明原因和临时性

示例：

```rust
// ❌ 不好的注释（重复代码）
// 创建一个新的 Agent
let agent = Agent::new();

// ✅ 好的注释（解释为什么）
// 使用 Arc 共享 Agent，因为多个 Session 需要访问同一个工具注册表
let agent = Arc::new(Agent::new());

// ✅ 好的注释（说明限制）
// 注意：当前实现不支持并发写同一个 session_id，
// 因为 SQLite 的 WAL 模式在高并发下会有性能问题
async fn write_session(&mut self) -> Result<()> {
    // ...
}

// ✅ 好的注释（说明 unsafe 的安全性）
// SAFETY: 这里使用 unsafe 是安全的，因为：
// 1. ptr 来自 Box::into_raw，保证非空且对齐
// 2. 生命周期由 PhantomData 保证
// 3. 不会发生数据竞争，因为只有一个所有者
unsafe {
    Box::from_raw(ptr)
}
```

## 11. 实施检查清单

AI 工具在完成每个阶段后，应对照以下清单自检：

### 阶段 1 检查清单

- [ ] `Agent::builder()` 可以正常构造
- [ ] `agent.session(user_id, session_id)` 返回 `Session`
- [ ] `session.chat()` 可以调用 LLM 并返回结果
- [ ] `session.chat_stream()` 可以流式输出
- [ ] `examples/simple_agent.rs` 可以运行
- [ ] 基础错误类型已定义
- [ ] 至少有一个单元测试

### 阶段 2 检查清单

- [ ] `ExecutionStep` 可以记录每一步
- [ ] `ExecutionTrace` 可以追踪完整执行流程
- [ ] 支持最大步数限制
- [ ] 支持超时控制
- [ ] `ExecutionResult` 包含完整轨迹
- [ ] 至少有一个集成测试

### 阶段 3 检查清单

- [ ] `Tool` trait 已定义
- [ ] `ToolRegistry` 可以注册和执行工具
- [ ] 至少实现了 3 个内置工具
- [ ] 工具参数校验正常
- [ ] 工具错误可以结构化返回
- [ ] `examples/tool_agent.rs` 可以运行

### 阶段 4 检查清单

- [ ] `MemoryStore` trait 已定义
- [ ] `MemoryScope` 可以区分用户/会话
- [ ] 内存后端可以正常读写
- [ ] SQLite 后端可以持久化
- [ ] 跨会话记忆可以复用
- [ ] `examples/memory_agent.rs` 可以运行

### 阶段 5 检查清单

- [ ] `Session` 可以接收多模态输入
- [ ] 文档提取可以正常工作
- [ ] 多模态内容可以保存到记忆
- [ ] `examples/multimodal_agent.rs` 可以运行

### 阶段 6 检查清单

- [ ] `InterruptHandler` trait 已定义
- [ ] 工具确认可以正常触发
- [ ] CLI 确认处理器可以正常工作
- [ ] `examples/hitl_agent.rs` 可以运行

### 阶段 7 检查清单

- [ ] 配置加载正常
- [ ] tracing 日志可以输出
- [ ] 会话恢复正常
- [ ] 测试覆盖核心路径
- [ ] 所有示例可以运行
- [ ] 文档完整

## 12. 常见问题

### Q1: 为什么不引入 `UserHandle` 中间层？

**A:** `UserHandle` 只是 `user_id` 的包装，除了作为跳板没有实际价值。直接 `agent.session(user_id, session_id)` 更简洁。

### Q2: 为什么不拆分 `UserMemoryStore` 和 `SessionMemoryStore`？

**A:** 统一使用一个 `MemoryStore` trait，通过 `MemoryScope` 区分用户/会话，可以简化实现和使用。

### Q3: 为什么不引入 `ExecutionEngine` 作为公开 API？

**A:** `ExecutionEngine` 是 `Session` 的内部实现细节，不应该暴露给外部。外部只需要关心 `Session` 的 `chat()` 和 `chat_stream()` 方法。

### Q4: 为什么工具的 `requires_confirmation` 要提取到 `ToolMetadata`？

**A:** 静态配置不应该是方法，每次调用都查询会增加开销。提取到 `ToolMetadata` 后，注册时就确定了这些属性。

### Q5: 为什么第一版不做多 Agent 和技能系统？

**A:** 多 Agent 和技能系统过于复杂，第一版应该专注核心能力：Agent、Loop、Tool、Memory、HITL。等核心能力稳定后，再考虑扩展。

### Q6: 示例代码中的 `Client::from_env()` 为什么要先赋值？

**A:** `Client::from_env()` 返回 `Result<Client, LlmError>`，不能直接传给 `.llm()`。正确写法是：

```rust
let client = Client::from_env()?;
let agent = Agent::builder().llm(client).build()?;
```

而不是：

```rust
// ❌ 错误：from_env() 返回 Result，不能直接传入
let agent = Agent::builder().llm(Client::from_env()?).build()?;
```

---

**文档版本：** v1.0  
**最后更新：** 2025-01-XX  
**维护者：** ufox-arc team

---

# ufox-arc 第二版（v2）设计

## 13. v2 定位与升级原则

v2 在 v1 核心能力稳定的基础上，新增两类能力：

1. **技能系统（Skills）**：可复用、可组合的命名能力，将系统提示词 + 工具子集 + 执行配置打包成可调用单元
2. **多 Agent 协作（Multi-Agent）**：多个独立 Agent 通过消息传递协作，完成单 Agent 无法完成的复杂任务

**升级原则：**

- v2 完全向后兼容 v1 API，已有代码无需修改
- 技能和多 Agent 只是 v1 核心能力的"组合"，不引入全新的执行模型
- 保持同样的"不过度设计"原则：第一次就跑通主路径，边界情况后补
- Skills 是轻量的：本质是"带配置的 Agent"，不是全新的运行时
- Multi-Agent 是无共享的：各 Agent 拥有独立 Session，通过消息传递协作

## 14. 技能系统（Skills）

### 14.1 设计思路

Skills 遵循 Anthropic Agent Skills 规范：**技能是自主执行单元，不预设配置，由 LLM 在运行时自主发现所需工具和子技能**。

核心模型：

```text
Skill = name + description

执行时：
  - 框架根据 description 自动生成执行上下文
  - 技能内部 LLM 可访问所有已注册工具
  - 技能内部 LLM 可发现并调用其他技能（调用栈保护）
  - 调用方只需 session.chat()，技能选择和调用完全自主
```

**上下文预算问题：**

技能数量增加后，将全部技能注入每次 LLM 调用会撑爆上下文（每个 skill schema ≈ 100–200 token）。框架采用**按需选择**策略：

```text
注册技能数 ≤ max_skills_per_call（默认 8）→ 全量注入

注册技能数 > max_skills_per_call
  → 按当前输入的语义相关性选 top-K 注入
  → 同时注入 discover_skill 元工具，让 LLM 随时按需发现其余技能
```

`discover_skill` 元工具始终可用，LLM 可随时用它检索未注入上下文的技能。

### 14.2 推荐接口

**Skill 结构（极简）：**

```rust
pub struct Skill {
    pub name: String,
    /// 清晰描述技能的能力边界，LLM 根据这个描述决定何时调用或发现该技能
    pub description: String,
}
```

框架执行技能时，自动构建执行提示：

```text
你正在执行技能「{name}」：{description}。
利用所有可用的工具和技能来完成当前任务。
```

**SkillRegistry：**

```rust
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn register(&mut self, skill: Skill) -> Result<()>;
    /// 替换已有技能定义（不存在时报错）
    pub fn reload(&mut self, skill: Skill) -> Result<()>;
    pub fn unregister(&mut self, name: &str) -> Result<()>;
    pub fn get(&self, name: &str) -> Option<&Skill>;
    pub fn list_names(&self) -> Vec<String>;
}
```

**Agent 动态管理（线程安全，内部持有 `Arc<RwLock<SkillRegistry>>`）：**

```rust
impl AgentBuilder {
    pub fn skill(mut self, skill: Skill) -> Self;
    /// 单次 LLM 调用最多注入几个技能（默认 8）
    pub fn max_skills_per_call(mut self, n: usize) -> Self;
}

impl Agent {
    /// 运行时注册新技能，立即对后续执行生效。
    pub fn add_skill(&self, skill: Skill) -> Result<()>;
    /// 运行时替换技能描述（热更新）。
    pub fn reload_skill(&self, skill: Skill) -> Result<()>;
    pub fn remove_skill(&self, name: &str) -> Result<()>;
}
```

**ExecutionConfig 新增技能配置：**

```rust
pub struct ExecutionConfig {
    // v1 字段不变
    pub max_iterations: usize,
    pub timeout: Duration,
    pub enable_perceive: bool,
    pub enable_observe: bool,
    pub enable_reflect: bool,
    pub max_concurrent_tools: usize,
    /// 单次 LLM 调用注入的最大技能数；超出时按相关性选 top-K（默认 8）
    pub max_skills_per_call: usize,
    /// 技能嵌套最大深度；超出后停止注入技能，LLM 仍可用工具完成任务（默认 3）
    pub max_skill_depth: usize,
}
```

**Session API 不变：**

```rust
// chat() / chat_stream() 完全不变，技能对调用方零感知
let result = session.chat("任务描述").await?;
```

### 14.3 框架内部机制

**Skill → pseudo-tool 转换：**

```json
{
  "name": "<skill.name>",
  "description": "<skill.description>",
  "parameters": {
    "type": "object",
    "properties": {
      "input": { "type": "string", "description": "发给该技能的任务描述" }
    },
    "required": ["input"]
  }
}
```

**discover_skill 元工具（技能数超预算时自动注入）：**

```json
{
  "name": "discover_skill",
  "description": "搜索当前未加载的技能。当现有技能无法满足需求时调用。",
  "parameters": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "描述你需要的能力" }
    },
    "required": ["query"]
  }
}
```

返回匹配的技能名称和描述列表，LLM 可立即调用其中的技能。

**工具列表构建规则：**

```text
每次 LLM 调用前：

1. 取当前输入作为 query
2. 从 SkillRegistry 中排除调用栈已有的技能（防循环）
3. 若剩余技能数 ≤ max_skills_per_call → 全量注入
   否则 → 按 query 相关性选 top-K，并注入 discover_skill 元工具
4. 若当前调用深度 ≥ max_skill_depth → 不注入任何技能（只保留工具）

工具列表 = agent.tools（全量，技能不限制工具访问）
          + 选出的技能 pseudo-tools
          + discover_skill（当技能超预算时）
```

**相关性打分策略（按优先级选用）：**

| 策略 | 条件 | 说明 |
|------|------|------|
| 嵌入向量相似度 | 配置了 embedding 客户端 | 精准，需额外 embed 调用 |
| 关键词匹配 | 默认回退 | 轻量，无需额外调用 |

**调用栈保护：**

- 同名技能已在调用栈 → 从可选列表静默排除（LLM 看不到，不报错）
- 深度 ≥ `max_skill_depth` → 停止注入任何技能 pseudo-tool，LLM 只用工具继续完成任务

两种情况都不中断执行，降级而非报错。

### 14.4 执行流程示例

```text
session.chat("研究 Rust async trait，审查代码，综合成报告")
  │
  ├─ 技能数(3) ≤ max_skills_per_call(8)，全量注入
  ├─ 工具列表：[web_search, file_write, research↗, code_review↗, tech_report↗]
  │
  ├─ LLM Think → 调用 tech_report↗("研究+审查+报告")   depth=0
  │    ├─ 调用栈：[tech_report]，排除 tech_report 自身
  │    ├─ 剩余技能：[research, code_review]，全量注入
  │    ├─ 工具列表：[web_search, file_write, research↗, code_review↗]
  │    │
  │    ├─ tech_report LLM → 调用 research↗("async trait 现状")   depth=1
  │    │    ├─ 调用栈：[tech_report, research]，排除两者
  │    │    ├─ 剩余技能：[code_review]，注入
  │    │    ├─ research LLM 调用 web_search → 整理结果
  │    │    └─ 结果以 ToolResult 返回 tech_report LLM
  │    │
  │    ├─ tech_report LLM → 调用 code_review↗("```rust...```")   depth=1
  │    │    └─ code_review LLM 分析代码 → ToolResult 返回
  │    │
  │    └─ tech_report LLM 综合 → 调用 file_write 保存报告 → Completion
  │
  └─ 主 LLM 收到 tech_report 结果 → Completion
```

### 14.5 使用示例

**示例 1：自动发现与调用**

```rust
use ufox_arc::{Agent, Skill};
use ufox_llm::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;

    let agent = Agent::builder()
        .llm(client)
        .tool(WebSearchTool::new())
        .tool(FileWriteTool::new())
        .skill(Skill {
            name: "research".to_string(),
            description: "深度调研技术话题，搜索并核实信息，输出结构化调研报告".to_string(),
        })
        .skill(Skill {
            name: "code_review".to_string(),
            description: "审查 Rust 代码的内存安全、并发安全、错误处理和性能".to_string(),
        })
        .skill(Skill {
            name: "tech_report".to_string(),
            description: "生成综合技术报告，整合调研结论和代码质量评估，保存到文件".to_string(),
        })
        .build()?;

    let mut session = agent.session("user_123", None).await?;

    // LLM 自主判断：直接用 tech_report，tech_report 再自主调用 research 和 code_review
    let result = session.chat(
        "研究 Rust async trait 最新进展，审查这段代码，综合成报告：\n```rust\n...\n```"
    ).await?;
    println!("{}", result.response.text);

    Ok(())
}
```

**示例 2：大量技能，上下文自动管理**

```rust
let agent = Agent::builder()
    .llm(client)
    .max_skills_per_call(5)  // 超出 5 个时按相关性选择 + discover_skill 兜底
    .skill(Skill { name: "research".to_string(),      description: "调研技术话题...".to_string() })
    .skill(Skill { name: "code_review".to_string(),   description: "审查 Rust 代码...".to_string() })
    .skill(Skill { name: "security_audit".to_string(), description: "安全漏洞审计...".to_string() })
    .skill(Skill { name: "perf_analysis".to_string(), description: "性能瓶颈分析...".to_string() })
    .skill(Skill { name: "doc_writer".to_string(),    description: "编写技术文档...".to_string() })
    .skill(Skill { name: "refactor".to_string(),      description: "代码重构建议...".to_string() })
    .skill(Skill { name: "test_writer".to_string(),   description: "生成测试用例...".to_string() })
    .skill(Skill { name: "deploy_check".to_string(),  description: "部署前置检查...".to_string() })
    // 8 个技能 > max_skills_per_call(5)
    // 框架自动按相关性选 5 个 + 注入 discover_skill
    .build()?;

let mut session = agent.session("user_123", None).await?;
let result = session.chat("对这个 PR 做安全审计和性能分析").await?;
// 框架自动选出 security_audit / perf_analysis 等相关技能注入
println!("{}", result.response.text);
```

**示例 3：运行时动态加载**

```rust
agent.add_skill(Skill {
    name: "k8s_ops".to_string(),
    description: "检查和操作 Kubernetes 集群，排查 Pod 问题、扩缩容、查看日志".to_string(),
})?;

agent.reload_skill(Skill {
    name: "code_review".to_string(),
    description: "审查 Rust 代码，重点关注 unsafe 块、生命周期和异步安全".to_string(),
})?;

agent.remove_skill("deploy_check")?;
```

### 14.6 设计决策

- **Skill 只有 name + description**：行为完全由 LLM 在运行时自主决定，框架不预设工具子集或子技能列表
- **技能对调用方零感知**：`chat()` 不变，技能选择和调用由 LLM 驱动
- **上下文预算是硬约束**：`max_skills_per_call` 限制注入数量，超出时选最相关的而不是全量
- **`discover_skill` 作为安全网**：LLM 永远有办法找到未注入的技能，预算限制不丢失能力
- **循环和超深度都静默降级**：不报错、不中断，调用栈中已有的技能被排除出可选列表，深度超限时停止注入技能
- **技能访问全量工具**：技能内部 LLM 可访问所有注册工具，由 LLM 自行判断使用哪些


## 15. 多 Agent 协作

### 15.1 设计思路

多 Agent 系统的核心挑战是：如何让多个 Agent 协作，同时保持架构简单？

**选定方案：协调器模式（Coordinator Pattern）**

```text
用户请求
    ↓
协调器 Agent（Coordinator）
    ↓ 分析任务，决定分配
    ├→ 成员 Agent A（如：研究员）→ 结果 A
    ├→ 成员 Agent B（如：代码专家）→ 结果 B
    └→ 成员 Agent C（如：审查员）→ 结果 C
    ↓ 汇总所有结果
最终回复
```

**关键决策：**

- 每个成员 Agent 拥有独立的 `Session`，不共享状态
- 协调器与成员之间只通过字符串消息传递，不传递内部结构
- 成员 Agent 的结果以 `ToolResult` 的形式回注到协调器的上下文
- 调用方只感知协调器，对成员 Agent 透明

### 15.2 推荐接口

```rust
pub struct AgentTeamMember {
    pub name: String,
    pub description: String,
    pub agent: Agent,
}

pub struct AgentTeam {
    coordinator: Agent,
    members: HashMap<String, AgentTeamMember>,
}

impl AgentTeam {
    pub fn builder() -> AgentTeamBuilder;

    /// 以协调器视角运行整个任务，返回汇总结果。
    pub async fn run(
        &self,
        user_id: impl Into<UserId>,
        task: impl Into<SessionInput>,
    ) -> Result<ExecutionResult>;

    /// 流式版本，事件中包含每个成员 Agent 的中间步骤。
    pub async fn run_stream(
        &self,
        user_id: impl Into<UserId>,
        task: impl Into<SessionInput>,
    ) -> Result<ExecutionEventStream>;
}

pub struct AgentTeamBuilder {
    coordinator: Option<Agent>,
    members: Vec<AgentTeamMember>,
}

impl AgentTeamBuilder {
    pub fn coordinator(mut self, agent: Agent) -> Self;
    pub fn member(mut self, name: impl Into<String>, description: impl Into<String>, agent: Agent) -> Self;
    pub fn build(self) -> Result<AgentTeam>;
}
```

协调器通过工具调用成员 Agent，框架负责路由：

```rust
/// 框架内部生成的"委派工具"，协调器可以用它来调用成员 Agent。
/// 外部使用者不需要手动创建这个工具，AgentTeam 会自动注入。
struct DelegateTool {
    member_name: String,
    member_description: String,
}
```

### 15.3 使用示例

```rust
use ufox_arc::{Agent, AgentTeam};
use ufox_llm::Client;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = Client::from_env()?;

    // 成员：研究员 Agent
    let researcher = Agent::builder()
        .llm(Client::from_env()?)
        .system("你是一位严谨的研究员，负责收集和核实信息。")
        .tool(WebSearchTool::new())
        .build()?;

    // 成员：代码专家 Agent
    let coder = Agent::builder()
        .llm(Client::from_env()?)
        .system("你是一位 Rust 专家，负责编写和优化代码。")
        .tool(FileReadTool::new())
        .tool(FileWriteTool::new())
        .build()?;

    // 成员：审查员 Agent
    let reviewer = Agent::builder()
        .llm(Client::from_env()?)
        .system("你是一位严谨的审查员，负责找出潜在问题。")
        .build()?;

    // 协调器
    let coordinator = Agent::builder()
        .llm(client)
        .system(
            "你是一位项目协调员。接到任务后，先分析需要哪些专家，\
             然后委派给对应的成员，最后汇总结果给用户。",
        )
        .build()?;

    let team = AgentTeam::builder()
        .coordinator(coordinator)
        .member("researcher", "收集和核实信息", researcher)
        .member("coder", "编写和优化 Rust 代码", coder)
        .member("reviewer", "审查潜在问题", reviewer)
        .build()?;

    let result = team.run("user_123", "研究 Rust 的 async trait 最佳实践，写一个示例，并审查代码质量").await?;
    println!("{}", result.response.text);

    Ok(())
}
```

### 15.4 执行流程说明

```text
1. team.run(user_id, task)
   ├─ 协调器创建新 Session（session_id 自动生成）
   ├─ 框架为协调器注入 delegate_to_researcher / delegate_to_coder / delegate_to_reviewer 工具
   
2. 协调器 Think 步骤
   └─ LLM 输出：调用 delegate_to_researcher("研究 async trait")

3. 框架拦截 delegate_to_* 工具调用
   ├─ 为对应成员 Agent 创建独立 Session（user_id 相同，session_id 新生成）
   ├─ 在成员 Session 中执行完整的 Think/Act 循环
   └─ 将成员结果转为 ToolResult 回注到协调器上下文

4. 协调器继续 Think
   └─ 可继续调用其他成员，或直接生成最终答复

5. 协调器 Completion → 返回给调用方
```

### 15.5 设计决策

- **成员 Agent 对协调器透明**：协调器只看到"委派工具"，不知道背后是 Agent 还是普通工具
- **不共享 Session**：成员 Agent 拥有独立上下文，避免状态污染
- **不引入消息总线（MessageBus）**：v2 不需要异步事件驱动的消息总线，调用方式就足够
- **不引入 `AgentRole` 枚举**：协调器和成员的区别只在于是否被注入了委派工具，不需要专门的角色概念
- **不引入 `AgentLoop` 新类型**：AgentTeam 复用同一个执行循环，不需要单独定义
- **成员 Agent 的 Memory 是独立的**：如果需要共享记忆，在构建时注入同一个 `MemoryStore` 实例即可

### 15.6 共享记忆（可选）

如果协调器和成员需要共享用户级记忆，在构建时注入同一个 `MemoryStore`：

```rust
use std::sync::Arc;

let shared_memory = Arc::new(SqliteMemory::open("./team_memory.db").await?);

let researcher = Agent::builder()
    .llm(Client::from_env()?)
    .memory(Arc::clone(&shared_memory))
    .build()?;

let coder = Agent::builder()
    .llm(Client::from_env()?)
    .memory(Arc::clone(&shared_memory))
    .build()?;

let coordinator = Agent::builder()
    .llm(client)
    .memory(shared_memory)  // 协调器也共享同一个 MemoryStore
    .build()?;
```

## 16. 并行工具执行

v2 支持在单次 LLM 响应中并发执行多个工具调用，而不是串行等待。

### 16.1 推荐接口

```rust
pub struct ExecutionConfig {
    // v1 字段保持不变
    pub max_iterations: usize,
    pub timeout: Duration,
    pub enable_perceive: bool,
    pub enable_observe: bool,
    pub enable_reflect: bool,
    
    // v2 新增
    /// 工具并发执行时的最大并发数；1 表示串行（v1 行为）
    pub max_concurrent_tools: usize,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            timeout: Duration::from_secs(300),
            enable_perceive: false,
            enable_observe: false,
            enable_reflect: false,
            max_concurrent_tools: 4,  // v2 默认并发
        }
    }
}
```

执行逻辑变化：

```rust
// v1：串行执行
for tool_call in tool_calls {
    let result = registry.execute(&tool_call, interrupt_handler).await?;
    results.push(result);
}

// v2：并发执行（受 max_concurrent_tools 限制）
let results = futures::stream::iter(tool_calls)
    .map(|tc| registry.execute(&tc, interrupt_handler))
    .buffer_unordered(config.max_concurrent_tools)
    .collect::<Result<Vec<_>>>()
    .await?;
```

### 16.2 HITL 与并发执行

当工具需要人工确认时，并发执行会暂停等待确认：

- 需要确认的工具单独排队，等待用户响应
- 不需要确认的工具继续并发执行
- 全部完成后再进入下一个 Think 步骤

## 17. v2 模块目录扩展

在 v1 目录结构基础上，新增以下模块：

```text
ufox-arc/
├── src/
│   ├── ...（v1 保持不变）
│   ├── skills/
│   │   ├── mod.rs
│   │   └── registry.rs
│   └── team/
│       ├── mod.rs
│       ├── builder.rs
│       └── delegate.rs
└── examples/
    ├── ...（v1 保持不变）
    ├── skill_agent.rs
    └── multi_agent_team.rs
```

## 18. v2 分阶段实施计划

v2 在 v1 阶段 7 完成后继续推进，阶段编号延续：

### 阶段 8：技能系统

**目标**：实现技能自动发现与调用，LLM 根据任务自主选择和嵌套使用技能，支持运行时动态加载，解决大量技能注册时的上下文预算问题。

**本阶段必须实现：**

- `Skill` 数据结构（只有 `name` + `description`）
- `SkillRegistry`：`register`/`reload`/`unregister`/`get`/`list_names`
- `Agent` 内部持有 `Arc<RwLock<SkillRegistry>>`
- `AgentBuilder::skill()` 和 `AgentBuilder::max_skills_per_call()` 注册入口
- `Agent::add_skill()` / `reload_skill()` / `remove_skill()` 动态管理
- Skill → pseudo-tool 的自动转换（`src/skills/pseudo_tool.rs`）
- 上下文预算选择逻辑：技能数 ≤ `max_skills_per_call` 时全量注入，否则按相关性 top-K
- `discover_skill` 元工具：技能超预算时自动注入，支持 LLM 按需发现剩余技能
- 关键词匹配作为默认相关性打分（无需 embedding 调用）
- 调用栈维护：同名技能静默排除（不报错）
- 深度超限（≥ `max_skill_depth`）时停止注入技能，降级为只用工具
- 示例 `examples/skill_agent.rs`（含多技能、嵌套、动态加载）

**完成定义：**

- `session.chat()` 不变，技能对调用方完全透明
- LLM 根据技能 `description` 自主决定是否调用、嵌套调用哪些技能
- 技能执行时可访问所有注册工具（不受限制）
- 技能可自主发现并调用其他技能（调用栈排除自身，防止直接循环）
- 注册技能数超过 `max_skills_per_call` 时，`discover_skill` 元工具正确工作
- 动态注册/热更新/移除后，后续执行立即感知
- 深度超限时静默降级，执行不中断
- 示例可以运行

**建议落点：**

- `src/skills/mod.rs`
- `src/skills/registry.rs`
- `src/skills/pseudo_tool.rs`（Skill → Tool schema 转换 + discover_skill）
- `src/skills/selector.rs`（top-K 相关性选择逻辑）
- `src/agent/mod.rs`（builder + 动态管理）
- `src/session/mod.rs`（执行时技能注入与调用栈）
- `examples/skill_agent.rs`

### 阶段 9：多 Agent 协作

**目标**：支持协调器 + 成员 Agent 的团队协作模式。

**本阶段必须实现：**

- `AgentTeamMember` 结构体
- `AgentTeam` 与 `AgentTeamBuilder`
- 委派工具（`DelegateTool`）的自动生成与注入
- `team.run()` 的完整执行流程
- 成员 Agent 的独立 Session 创建与管理
- 成员执行结果转换为 `ToolResult` 回注协调器
- 示例 `examples/multi_agent_team.rs`

**本阶段不要做：**

- 成员 Agent 之间直接通信（绕过协调器）
- 成员并发执行（第一版串行，避免复杂度）
- 动态增减团队成员

**完成定义：**

- 协调器能正确路由任务到成员 Agent
- 成员执行结果正确流回协调器
- 协调器能汇总多个成员结果并输出最终回复
- 支持共享 `MemoryStore` 实现团队级记忆
- 至少有一个协调器 + 2 个成员的完整示例

**建议落点：**

- `src/team/mod.rs`
- `src/team/builder.rs`
- `src/team/delegate.rs`
- `examples/multi_agent_team.rs`

## 19. v2 实施检查清单

### 阶段 8 检查清单

- [ ] `Skill` 结构体只有 `name` + `description` 两个字段
- [ ] `SkillRegistry` 支持 `register`/`reload`/`unregister`
- [ ] `Agent` 内部持有 `Arc<RwLock<SkillRegistry>>`
- [ ] `AgentBuilder::skill()` 和 `max_skills_per_call()` 可以正常使用
- [ ] `Agent::add_skill()` 运行时添加技能，立即生效
- [ ] `Agent::reload_skill()` 运行时热更新技能描述
- [ ] `Agent::remove_skill()` 运行时移除技能
- [ ] Skill 正确转换为 pseudo-tool schema 并注入 LLM 工具列表
- [ ] 技能数 ≤ `max_skills_per_call` 时全量注入
- [ ] 技能数 > `max_skills_per_call` 时按关键词相关性选 top-K
- [ ] 超预算时 `discover_skill` 元工具被正确注入且可用
- [ ] 技能调用时当前技能从可选列表中静默排除（防直接循环）
- [ ] 深度 ≥ `max_skill_depth` 时停止注入技能，执行不中断
- [ ] 技能内部 LLM 可访问所有注册工具
- [ ] `examples/skill_agent.rs` 可以运行（含嵌套与动态加载场景）

### 阶段 9 检查清单

- [ ] `AgentTeam::builder()` 可以正常构造
- [ ] 框架自动为协调器注入委派工具
- [ ] `team.run()` 能完整执行协调器 → 成员 → 协调器的流程
- [ ] 成员 Agent 拥有独立 Session
- [ ] 成员执行结果正确转换为 `ToolResult`
- [ ] 共享 `MemoryStore` 可以在团队成员间生效
- [ ] `examples/multi_agent_team.rs` 可以运行
- [ ] 成员不存在时返回清晰错误

## 20. v2 常见问题

### Q7: 技能（Skill）和工具（Tool）的区别是什么？

**A:** 工具是 LLM 调用的原子操作（查天气、读文件），执行结束返回数据；技能是一个完整的 Agent 执行单元，有自己的系统提示词、工具子集和执行循环。技能在框架层被转换为 pseudo-tool，让 LLM 可以自主发现并调用，但执行机制远比普通工具复杂（完整 Think/Act 循环、可进一步调用子技能）。

### Q8: 多 Agent 中的成员 Agent 能不能共享会话上下文？

**A:** 不能，也不应该。成员 Agent 拥有独立 Session，避免状态污染。如果需要共享信息，通过两种方式：（1）协调器在委派时把上下文摘要传给成员；（2）共享同一个 `MemoryStore` 实例（见第 15.6 节）。

### Q9: 为什么多 Agent 不用消息总线（MessageBus）？

**A:** 消息总线适合异步事件驱动的场景，而这里的场景是同步请求-响应：协调器发出委派，等待结果，再继续。直接函数调用更简单、更容易调试、更容易推断执行顺序。

### Q10: 技能执行失败了怎么办？

**A:** 技能执行失败返回 `ArcError`，与普通 `chat()` 失败的错误处理方式相同。调用方可以 `?` 传播，或者 `.unwrap_or_else()` 降级处理。技能执行前已进入 Session 的消息历史不会回滚。

### Q11: v2 还有哪些东西没有做？

**A:** 以下能力有意推迟到 v3 或更后期：

- 成员 Agent 并发执行（AgentTeam 内并发）
- 分布式 Agent（跨进程/跨机器）
- Agent 持久化与迁移

---

**文档版本：** v2.0  
**最后更新：** 2026-04-29  
**维护者：** ufox-arc team
