# 07 · Memory（跨会话记忆）

> 子系统：embedding 语义搜索 + LLM sidecar 验证 + 图结构存储的三层管线，实现记忆自动提取、相关性检索、上下文注入与图谱维护。
> 回指：[CLAUDE.md](../CLAUDE.md) · [doc_ref README](README.md)

## 职责一句话

Memory 子系统为 AI Agent 提供跨会话持久化记忆能力，通过 embedding 语义搜索 + LLM sidecar 验证 + 图结构存储的三层管线，实现记忆的自动提取、相关性检索、注入上下文以及图谱维护。

## 内存按 scope 划分

| Scope | 存储路径 | 说明 |
|---|---|---|
| **Project（per-workdir）** | `~/.ssc_tui/memory/projects/{hash}.json`，hash 由工作目录路径经 `DefaultHasher` 计算 | 项目级记忆，绑定特定代码仓库 |
| **Global（user-level）** | `~/.ssc_tui/memory/global.json` | 用户级偏好、跨项目事实 |
| **Test** | `~/.ssc_tui/memory/test/test_project.json` / `test_global.json` | 测试隔离存储（`MemoryManager::new_test()`） |

两个 scope 均以 `MemoryGraph`（JSON）持久化，legacy `MemoryStore` 格式在加载时自动迁移为 graph 格式。

### 存储布局

> **来源**：原生 jcode 设计文档 `docs/MEMORY_ARCHITECTURE.md`。

```
~/.ssc_tui/memory/
├── graph.json                    # 序列化 MemoryGraph（非 petgraph，用 HashMap 替代）
├── projects/
│   └── <project_hash>.json       # 各工作目录项目记忆
├── global.json                   # 用户级全局记忆
├── embeddings/
│   └── <memory_id>.vec           # 各记忆条目 embedding 向量
├── clusters/
│   └── cluster_metadata.json     # 自动聚类质心与元数据
└── tags/
    └── tag_index.json            # Tag 到 memory ID 的映射索引
```

> **注意**：实际实现使用 `HashMap` 而非 `petgraph::DiGraph`（简化 JSON 序列化），但图语义（节点/边/标签）保持不变。

## 关键文件清单

| 文件路径 | 职责 |
|---|---|
| `src/memory.rs` | 入口，`MemoryManager` 提供增删改查、sidecar 集成、graph 操作、嵌入检索管线 |
| `src/memory/activity.rs` | 全局 `MemoryActivity` 状态机，管理 UI 展示用的管线步骤状态和事件流 |
| `src/memory/cache.rs` | 进程级 `MemoryGraph` 缓存（按文件路径 + mtime 校验） |
| `src/memory/pending.rs` | 异步记忆注入队列（per-session），含去重、抑制重入、过期淘汰 |
| `src/memory_agent.rs` | 独立的 Haiku-powered Memory Agent，后台运行，topic change 检测、增量提取、图维护 |
| `src/memory_graph.rs` | 兼容性 re-export，转发到 `memory_types` 中的 `MemoryGraph` |
| `src/memory_log.rs` | 持久化 JSONL 事件日志（`~/.ssc_tui/logs/memory-events-*.jsonl`），14 天保留 |
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

## Memory Tools（CLI 接口）

> **来源**：原生 jcode 设计文档 `docs/MEMORY_ARCHITECTURE.md`。

Agent 可用的 memory 操作（由 sidecar/tool 路由到 `MemoryManager`）：

```
memory { action: "remember", content: "...", category: "fact|preference|correction",
         scope: "project|global", tags: ["tag1", "tag2"] }
memory { action: "recall" }                    # 获取当前上下文相关记忆
memory { action: "search", query: "..." }      # 语义搜索
memory { action: "list", tag: "..." }          # 按标签列举
memory { action: "forget", id: "..." }         # 停用记忆
memory { action: "link", from: "id1", to: "id2", relation: "relates_to" }
memory { action: "tag", id: "...", tags: ["new", "tags"] }
```

## 级联检索 (Cascade Retrieval) 参数

> **来源**：原生 jcode `docs/MEMORY_ARCHITECTURE.md`。

`cascade_retrieve()` 算法分三步：embedding similarity 搜索 → BFS 沿图边遍历（HasTag/InCluster/RelatesTo/Supersedes）→ 去重排序。各边类型权重：

| 边类型 | 权重 | 含义 |
|---|---|---|
| `Supersedes` | 0.9 | 后续记忆替代旧记忆，最强信号 |
| `HasTag` | 0.8 | 相同标签，强关联 |
| `InCluster` | 0.6 | 同一自动聚类，中等关联 |
| `RelatesTo { weight }` | weight | 语义关系权重由边携带 |
| `DerivedFrom` / `Contradicts` | 0.3 | 弱关联 |

BFS 深度每步 score 衰减：`decayed_score = edge_weight * 0.7^(depth + 1)`。

| 参数 | 默认值 | 说明 |
|---|---|---|
| `similarity_threshold` | 0.4 | 初始 embedding 搜索最低相似度 |
| `max_initial_hits` | 10 | embedding 搜索结果数 |
| `max_depth` | 2 | BFS 遍历深度限制 |
| `max_results` | 10 | 最终返回上限 |
| `edge_decay` | 0.7 | 每层遍历的分数衰减因子 |

## 高级特性

> **来源**：原生 jcode `docs/MEMORY_ARCHITECTURE.md`。部分特性尚未完全实现（标注 📋）。

### 1. 时间感知 (Temporal Awareness)

回忆近期访问的记忆时会获得 boost：

```
recency_boost = 1.0 + 0.5 * e^(-hours_since_access / 24)
```

### 2. 置信度衰减 (Confidence Decay)

各 memory type 半衰期不同：

| Memory Type | 半衰期 | 理由 |
|---|---|---|
| Correction | 365 天 | 用户修正价值高，长期保留 |
| Preference | 90 天 | 偏好可能变化 |
| Fact | 30 天 | 代码库事实可能过时 |
| Procedure | 60 天 | 流程变化频率中等 |
| Inferred | 7 天 | 低级推理不可靠 |

衰减公式：`confidence = initial * e^(-age_days / half_life) * (1 + 0.1 * log(access_count + 1)) * trust_weight`

### 3. 负向记忆 (Negative Memories) 📋

不应做的事，带触发模式匹配：
```rust
MemoryEntry {
    content: "Never use println! for logging in production code",
    memory_type: MemoryType::Negative,
    trigger_patterns: vec!["println!", "print!", "dbg!"],
}
```

触发模式匹配当前上下文时优先展现代理。

### 4. 过程性记忆 (Procedural Memories) 📋

结构化步骤知识：
```rust
pub struct Procedure {
    pub name: String,
    pub trigger: String,        // "deploy to production"
    pub steps: Vec<String>,
    pub prerequisites: Vec<String>,
    pub warnings: Vec<String>,
}
```

### 5. 反馈回路 (Feedback Loops)

`MemoryEntry::on_used(helpful: bool)` 在每次使用后更新：

- **helpful = true**：`strength++`，`confidence = min(confidence + 0.05, 1.0)`
- **helpful = false**：`confidence = max(confidence - 0.1, 0.0)`

### 6. 溯源追踪 (Provenance Tracking)

每段记忆记录来源链：
- `Provenance`：`UserStated | UserCorrected | Observed | Inferred | Extracted`
- 每次 reinforcement 记录 `Vec<Reinforcement>`（session_id, message_index, timestamp）

### 7. 检索后维护 (Post-Retrieval Maintenance)

`MemoryAgent::post_retrieval_maintenance()` 在每轮检索后异步运行：

| 任务 | 触发条件 | 动作 |
|---|---|---|
| Link Discovery | 2+ 条记忆被验证相关 | 在同时相关的记忆间建/增强 RelatesTo 边 |
| Confidence Boost | 记忆被验证相关 | 增加 access_count、boost confidence |
| Confidence Decay | 记忆被取出但被拒绝 | 小幅 decay confidence（0.02） |
| Gap Detection | context 无相关记忆 | 记录 memory gap 用于后续提取 |
| Tag Inference | 多条记忆共享 context | 自动推断共同标签 |
| Cluster Update | 每 50 次检索 | 重跑聚类，合并邻近聚类 |

### 8. Scope 层级

```
Global (user-level, permanent) → Project (codebase, 永久) → Session (会话内)
```

- **Global**：用户级偏好、跨项目事实，存 `~/.ssc_tui/memory/global.json`
- **Project**：项目级知识，存 `~/.ssc_tui/memory/projects/{hash}.json`
- **Session**：当前对话上下文，不持久化跨会话

### 9. 隐私与安全

**不应记录的内容**：
- API key、密码、token 等凭证
- 个人身份信息（PII）
- 标记为敏感的文件内容

**过滤机制**：
- 正则模式匹配（secret、API key、password）
- `.gitignore` / `.secretsignore` 文件
- `.env` 文件内容跳过

**用户控制**：
- 所有记忆以 human-readable JSON 存储
- CLI 提供查看/编辑/删除能力
- 可通过配置完全关闭记忆系统
- 支持导出/导入备份

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

## Memory 内存预算 (Regression Budget)

> **来源**：原生 jcode `docs/MEMORY_BUDGET.md`（2026-04-18）。当前为原生 jcode 基准，SAITEC-TUI 可能有调整。

### 硬上限（Hard caps）

Markdown 高亮缓存（`src/tui/markdown.rs`）：

| 指标 | 上限 | 原因 |
|---|---|---|
| `highlight_cache_entries` | ≤ 256 | `HIGHLIGHT_CACHE_LIMIT` |

Mermaid 缓存（`src/tui/mermaid.rs` / `mermaid_cache_render.rs`）：

| 指标 | 上限 | 原因 |
|---|---|---|
| `render_cache_entries` | ≤ 64 | `RENDER_CACHE_MAX` |
| `image_state_entries` | ≤ 12 | `IMAGE_STATE_MAX` |
| `source_cache_entries` | ≤ 8 | `SOURCE_CACHE_MAX` |
| `active_diagrams` | ≤ 128 | `ACTIVE_DIAGRAMS_MAX` |
| `cache_disk_png_bytes` | ≤ 50 MiB | `CACHE_MAX_SIZE_BYTES` |
| `cache_disk_max_age_secs` | ≤ 259200 (3 天) | `CACHE_MAX_AGE_SECS` |

### 棘轮期望（Ratchet expectations）

| 指标关系 | 期望 |
|---|---|
| `provider_messages_cache.count` vs `messages.count` | 同一数量级，通常紧跟 transcript |
| `session_provider_cache_json_bytes` vs `canonical_transcript_json_bytes` | 对普通对话流应可比，不独立膨胀 |
| `transient_provider_materialization_json_bytes` | 活跃 materialization 路径外应接近零 |
| `display_large_tool_output_bytes` | 大量值需合理解释（通常意味着 UI 中 tool output 过于激进地保留） |

### Review 检查清单

改动内存相关代码时需记录：

1. **哪些计数器变化了？** 用 `:debug memory` 等内置命令验证。
2. **硬上限被修改了吗？** 如是，解释旧上限为何不足。
3. **重复数据增加了吗？** 检查 canonical transcript、provider cache、materialized provider view、display copy、side-panel copy。
4. **可观测性是否仍然足够？** 内存增长后，日志/Profile 能解释去向吗？

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

## 未来规划

> **来源**：原生 jcode 设计文档 `docs/MEMORY_ARCHITECTURE.md` 和 `docs/MEMORY_BUDGET.md`。以下为原生 jcode 规划，SAITEC-TUI 路线图可能不同。

### Phase 8: 深度记忆巩固（Deep Memory Consolidation，Ambient Garden）

全图范围的巩固，在 ambient 模式后台周期运行：

- [ ] 全图相似度合并（embedding similarity ≥ 0.95）
- [ ] 冗余检测与去重（超出 sidecar 本地范围）
- [ ] 矛盾解决（全局图，不限取出集合）
- [ ] 事实验证（对照代码库检查记忆是否仍有效）
- [ ] 回溯会话提取（crash/missed 会话）
- [ ] 聚类重组
- [ ] 弱记忆剪枝（confidence < 0.05 AND strength ≤ 1）
- [ ] 跨会话关系发现
- [ ] 缺失 embedding 的回填
- [ ] 知识图谱优化

### Sleep-Like 巩固方案（概念阶段）

四种架构候选：
1. **定期守护进程**：每 N 小时运行一次
2. **空闲触发**：无活跃会话 M 分钟后运行
3. **容量触发**：记忆条目超阈值时运行
4. **手动命令**：用户 `/consolidate` 触发

### 未解决问题

1. **多机器同步**：记忆应经加密备份跨设备同步吗？
2. **团队共享**：某些记忆可跨团队共享吗？
3. **聚类算法**：HDBSCAN vs k-means vs hierarchical？
4. **图持久化**：JSON 序列化 vs SQLite（大图）？

## 关联模块

| 模块 | 路径 | 职责 | 规模 |
|---|---|---|---|
| `src/goal.rs` + `src/goal_tests.rs` | 持久化用户目标（Goal）CRUD、里程碑跟踪、进度百分比；Goal 与 MemoryGraph 集成——goal 自动 mirror 为 memory graph entry | ~804 行 |

## 回指
- memory 相关 bus/protocol 事件：[11-bus-message-protocol.md](11-bus-message-protocol.md)
- `jcode-embedding` 重量级依赖（163+ crate）：[12-workspace-build-ci.md](12-workspace-build-ci.md)
