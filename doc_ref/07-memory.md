# 07 · Memory（跨会话记忆）

> 子系统：embedding 语义搜索 + LLM sidecar 验证 + 图结构存储的三层管线，实现记忆自动提取、相关性检索、上下文注入与图谱维护。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Memory 子系统为 AI Agent 提供跨会话持久化记忆能力，通过 embedding 语义搜索 + LLM sidecar 验证 + 图结构存储的三层管线，实现记忆的自动提取、相关性检索、注入上下文以及图谱维护。

## 内存按 scope 划分

| Scope | 存储路径 | 说明 |
|---|---|---|
| **Project（per-workdir）** | `~/.jcode/memory/projects/{hash}.json`，hash 由工作目录路径经 `DefaultHasher` 计算 | 项目级记忆，绑定特定代码仓库 |
| **Global（user-level）** | `~/.jcode/memory/global.json` | 用户级偏好、跨项目事实 |
| **Test** | `~/.jcode/memory/test/test_project.json` / `test_global.json` | 测试隔离存储（`MemoryManager::new_test()`） |

两个 scope 均以 `MemoryGraph`（JSON）持久化，legacy `MemoryStore` 格式在加载时自动迁移为 graph 格式。

## 关键文件清单

| 文件路径 | 职责 |
|---|---|
| `src/memory.rs` | 入口，`MemoryManager` 提供增删改查、sidecar 集成、graph 操作、嵌入检索管线 |
| `src/memory/activity.rs` | 全局 `MemoryActivity` 状态机，管理 UI 展示用的管线步骤状态和事件流 |
| `src/memory/cache.rs` | 进程级 `MemoryGraph` 缓存（按文件路径 + mtime 校验） |
| `src/memory/pending.rs` | 异步记忆注入队列（per-session），含去重、抑制重入、过期淘汰 |
| `src/memory_agent.rs` | 独立的 Haiku-powered Memory Agent，后台运行，topic change 检测、增量提取、图维护 |
| `src/memory_graph.rs` | 兼容性 re-export，转发到 `memory_types` 中的 `MemoryGraph` |
| `src/memory_log.rs` | 持久化 JSONL 事件日志（`~/.jcode/logs/memory-events-*.jsonl`），14 天保留 |
| `src/memory_prompt.rs` | 对话消息格式化为 context 字符串，分 relevance 和 extraction 两种窗口 |
| `src/memory_types.rs` | 核心类型：`MemoryEntry`/`MemoryCategory`/`MemoryScope`/`MemoryStore`/`MemoryActivity`/`PipelineState`/ranking |
| `src/runtime_memory_log.rs` | 进程级运行时内存采样日志（PSS、JSON 字节、allocator 统计） |
| `src/sidecar.rs` | 轻量 LLM 客户端，relevance 检查、矛盾检测、记忆提取，自动选 OpenAI/Claude 后端 |
| `src/embedding.rs` | Embedding facade，封装 `jcode-embedding`，进程级 LRU 缓存和 idle unload |
| `src/embedding_stub.rs` | `embeddings` feature 关闭时的 stub 实现 |
| `crates/jcode-memory-types/src/lib.rs` | Re-export `graph` 模块的所有公共类型 |
| `crates/jcode-memory-types/src/graph.rs` | `MemoryGraph` 数据结构、`EdgeKind`/`TagEntry`/`ClusterEntry`、BFS cascade retrieval、legacy 迁移 |
| `crates/jcode-embedding/src/lib.rs` | ONNX 推理：all-MiniLM-L6-v2 加载、tokenize、mean-pooling、cosine similarity、模型自动下载 |

## 核心类型与关键函数

**`MemoryGraph`** (`crates/jcode-memory-types/src/graph.rs`)：
```rust
pub struct MemoryGraph {
    pub graph_version: u32,                    // 当前值 2
    pub memories: HashMap<String, MemoryEntry>,
    pub tags: HashMap<String, TagEntry>,
    pub clusters: HashMap<String, ClusterEntry>,
    pub edges: HashMap<String, Vec<Edge>>,       // forward edges
    pub reverse_edges: HashMap<String, Vec<String>>, // reverse edges for BFS
    pub metadata: GraphMetadata,
}
```
关键函数：`add_memory()`/`remove_memory()`（自动维护 tag 节点和边）、`tag_memory()`/`untag_memory()`、`link_memories()`/`supersede()`/`mark_contradiction()`、`cascade_retrieve(seed_ids, seed_scores, max_depth, max_results)`（BFS 级联检索，沿 HasTag/RelatesTo/Supersedes 遍历，score 衰减 `0.7^depth`）、`from_legacy_store()`。

**`MemoryAgent`** (`src/memory_agent.rs`)：
```rust
pub struct MemoryAgent {
    rx: mpsc::Receiver<AgentMessage>,
    sidecar: Option<Sidecar>,
    sessions: HashMap<String, SessionState>,  // per-session state
}
```
关键函数：`process_context()`（每轮主流程：embed context → topic change 检测 → 周期性提取 → embedding 检索 → sidecar 验证 → 格式化 pending memory → post-retrieval maintenance）、`evaluate_candidates()`（并发调 sidecar 检查相关性）、`extract_from_context()`（增量提取，含去重 embedding similarity>=0.90、矛盾检测、supersede 建图）、`post_retrieval_maintenance()`（后台：link discovery、confidence boost/decay、cluster refinement、tag inference、GC 低置信度）、`trigger_final_extraction()`（会话结束全量提取，fire-and-forget）。

**`MemoryLog`** / **`RuntimeMemoryLog`** / **`Sidecar`** / **`MemoryPrompt`**：见文件清单与职责。

`Sidecar` 关键函数：`check_relevance(memory_content, context)` → `(is_relevant, reason)`、`check_contradiction(new, existing)`、`extract_memories(transcript)` / `extract_memories_with_existing(transcript, existing)`（输出 `CATEGORY|CONTENT|TRUST` 格式）。

`MemoryPrompt`：`format_context_for_relevance(messages)`（最近 12 条，8000 字符上限）、`format_context_for_extraction(messages)`（最近 40 条，24000 字符上限）。

## GRAPH_VERSION schema 版本机制

`GRAPH_VERSION` 当前值 **2**（`crates/jcode-memory-types/src/graph.rs:16`）。

加载逻辑（`MemoryManager::load_project_graph`/`load_global_graph`）：
1. 先查进程级 cache（路径 + mtime 校验）。
2. 尝试按 `MemoryGraph` 反序列化，`graph_version == GRAPH_VERSION` 则直接用。
3. 版本不匹配或反序列化失败 → 退回 `MemoryStore`（旧扁平 JSON）反序列化 → `MemoryGraph::from_legacy_store()` 迁移。
4. 迁移时创建 `.json_bak` 备份，写回 graph 格式。

**无显式 1→2 迁移逻辑**：只要 `graph_version != GRAPH_VERSION` 就走 legacy 路径。

## 内存流程：extract → sidecar verify → store in graph → inject by relevance

**Extract（提取）**：触发条件为 topic change 检测（cosine similarity < 0.3）、每 12 轮周期提取、会话结束时 final extraction。`MemoryAgent::extract_from_context()` → `Sidecar::extract_memories_with_existing(transcript, existing)`；sidecar prompt 含已知记忆列表（最多 80 条）避免重复；输出 `CATEGORY|CONTENT|TRUST` 解析为 `ExtractedMemory`；去重（`find_similar(content, 0.90, 1)`，>=0.90 则 reinforce 而非新建）；矛盾处理（>=0.5 同类记忆经 `sidecar.check_contradiction()` 命中则建 Contradicts 边并 supersede 旧记忆）；同批提取多条间建 DerivedFrom 关系。

**Sidecar Verify（验证）**：embedding search → sidecar verify → inject → maintain。`find_retrieval_candidates_similar_scoped()` 对 project + global 两 graph 做 cosine similarity 搜索（阈值 0.5，最多 10 候选）；gap filter 按 score 分布自然断裂点截断；`Sidecar::check_relevance()` 并发批次 5；`is_memory_injected_any()` 排除已注入记忆。

**Store in Graph（存储）**：`remember_project()`/`remember_global()`；storage-layer dedup（embedding similarity>=0.85 视为重复，reinforce 而非新增）；cross-store dedup（project 写入时检查 global，反之亦然）；`MemoryEntry::ensure_embedding()` 写入前调 `embedding::embed(&content)`；`MemoryGraph::add_memory()` 自动创建 TagEntry 节点和 HasTag 边。

**Inject by Relevance（按相关性注入）**：`format_relevant_prompt()` 按 category 分组（Correction > Fact > Preference > Entity）生成 `# Memory\n## Category\n1. content`；结果存 per-session `PENDING_MEMORY` HashMap；去重抑制（90s 内相同 prompt signature 抑制、180s 内 80%+ memory ID 重叠抑制）；主 agent 下一轮对话前调 `take_pending_memory(session_id)` 获取并注入。

## Sidecar 如何选择 OpenAI / Claude 后端

`Sidecar::new()`（`src/sidecar.rs:64-101`）：
1. **配置覆盖**：`config.agents.memory_model` 非空则经 `provider::provider_for_model()` 判断属于 OpenAI 还是 Claude。
2. **凭据检测**（无覆盖时）：优先 OpenAI（`auth::codex::load_credentials()` 成功用 `gpt-5.3-codex-spark`）→ 退而 Claude（`auth::claude::load_credentials()` 成功用 `claude-haiku-4-5-20241022`）→ 都没有则默认 Claude（调用时报错）。
3. **ChatGPT OAuth 模式**：`codex-spark` 不可用（404/403/400 + 模型相关错误）时自动降级到 `gpt-5.4` 并设 `reasoning: "low"`。

OpenAI 用 Responses API（`/v1/responses`），ChatGPT OAuth 模式走 SSE streaming；Claude 用 Messages API（`/v1/messages`）带 OAuth beta headers。

## embeddings feature flag 的角色

`Cargo.toml`：`embeddings = ["dep:jcode-embedding"]`，**不在 `default` features 中**，需显式 `--features embeddings` 或 `JCODE_DEV_FEATURE_PROFILE=full` 启用。

`src/lib.rs` 条件编译：`#[cfg(feature = "embeddings")]` → `src/embedding.rs`（真正 ONNX 推理 facade）；`#[cfg(not(feature = "embeddings"))]` → `src/embedding_stub.rs` 并 re-export 为 `embedding`。

**Stub 行为**：`embed()` 返回 `anyhow::bail!("Embeddings feature not compiled in this build")`；`cosine_similarity`/`find_similar` 有纯数学实现（fallback）；`Embedder::load()`/`get_embedder()` 返回错误。

**影响**：feature 关闭时所有依赖 embedding 的操作（语义搜索、存储层去重、topic change 检测、cascade retrieval）退化为空或 keyword-only 模式；sidecar relevance 仍可用但候选数降为全量 score 排序。

## 依赖关系

- 被 [02 Agent](02-agent-runtime.md)（memory prompt 注入 / memory-agent 管线）、[11 Bus](11-bus-message-protocol.md)（`BusEvent` 中 memory 相关事件、`ServerEvent::MemoryInjected`/`MemoryActivity`）依赖。
- 依赖 [03 Provider](03-provider.md)（`provider_for_model`）、[06 Auth](06-auth-login.md)（sidecar 凭据检测）、[08 Storage](08-storage-session.md)（JSON 持久化）、[12 Workspace](12-workspace-build-ci.md)（`jcode-memory-types`/`jcode-embedding`）。

## 陷阱与设计约束

- **GRAPH_VERSION 无渐进迁移**：仅靠 `graph_version == 2` 判断；未来升版本 3 时，现存版本 2 文件会走 legacy 路径（`from_legacy_store()`），但 `MemoryGraph` 不实现 `MemoryStore` 的 `Deserialize`——该路径假设旧格式是扁平 `MemoryStore`，对 graph 格式的 v2→v3 升级无覆盖。
- **`top_k_scored`/`TopKItem` 在 4 个文件重复实现**：`memory_types.rs`/`memory_graph.rs`/`embedding.rs`/`embedding_stub.rs`/`jcode-embedding/src/lib.rs` 各一份几乎相同实现，未抽到公共模块。
- **`process_context` 中的 borrow checker 绕行**：`memory_agent.rs` 为在 `&mut self.session_state()` 借用期间调 `self.extract_from_context()`，手动 drop `ss` 引用（`let _ = ss;`）。
- **全量内存扫描**：`find_similar_with_embedding()` 收集 project + global 两 graph 所有有 embedding 的 `MemoryEntry::clone()` 到 Vec 再算 cosine similarity，记忆量大时内存/CPU 线性增长，无 ANN 索引。
- **Sidecar 并发调用无速率控制**：`evaluate_candidates()`/`get_relevant_parallel()` 仅靠 `BATCH_SIZE = 5` 控制，无重试/退避/限流，API 限流时可能批量失败。
- **embedding LRU cache 用 `u64` hash 做 key**：`DefaultHasher` 对文本 hash 理论上有 collision 风险（概率极低，LRU 容量 128 时影响可忽略）。
- **`PENDING_MEMORY`/`INJECTED_MEMORY_IDS` 等全局 static Mutex 无清理**：只增不减（仅 `clear_all_pending_memory()` 整体置 None），长运行 server 进程中 session 数持续增长时积累垃圾条目。
- **confidence decay half-life 按 category 硬编码**：`MemoryEntry::effective_confidence()` 中 Correction 半衰期 365 天、Fact 30 天等，无配置化通道。
- **cross-scope link 不支持**：`MemoryManager::link_memories()` 注释「Cross-store links not supported for now」——project 和 global graph 中同义记忆无法建 RelatesTo 边。

## 回指

- Agent 如何注入 memory prompt（作为 user message）：[02-agent-runtime.md](02-agent-runtime.md)
- memory 相关 bus/protocol 事件：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- `jcode-embedding` 重量级依赖（163+ crate）：[12-workspace-build-ci.md](12-workspace-build-ci.md)
