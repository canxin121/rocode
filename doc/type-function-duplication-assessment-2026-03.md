# rocode 类型/函数重复定义与放置不合理评估报告（2026-03）

## 1. 背景与目标

当前仓库存在较明显的“同类类型/函数多处重复定义”和“模块放置职责不清”问题。该问题会带来：

- 行为漂移（同名函数语义不一致）
- 修复成本放大（N 处同步改动）
- 分层耦合加深（server 依赖 command 等跨层倒置）

本报告目标：

1. 给出证据驱动的问题清单（路径 + 行号 + 摘录）
2. 按严重度评估影响
3. 提出可执行、可分阶段落地的整改路线

---

## 2. 范围与方法

### 2.1 审查范围

重点覆盖目录：

- `crates/rocode-core`
- `crates/rocode-content`
- `crates/rocode-command`
- `crates/rocode-server`
- `crates/rocode-tui`
- `crates/rocode-message`
- `crates/rocode-session`
- `crates/rocode-tool`
- `crates/rocode-permission`
- `crates/rocode-types`

### 2.2 审查方法

- 全仓关键模式检索（类型名、函数名、协议枚举、跨 crate use）
- 对高风险点做文件级抽样复核（行号与关键片段）
- 聚合重复模式，避免“同类问题重复计数”

---

## 3. 概览结论（Executive Summary）

### 3.1 关键结论

1. **重复最严重的是 lossy 反序列化 helper**：`deserialize_opt_string_lossy` 在 Rust 源码中出现 **40 处**定义，且语义不一致。
2. **核心域类型存在重复并分叉**：`Role`、Scheduler 状态枚举在不同 crate 中重复，字段/命名兼容策略不同。
3. **同一模型双轨维护**：`rocode-message` 中 canonical 与 v2 DTO 出现高重叠结构并依赖手工双向转换。
4. **分层边界有倒置**：`rocode-server` 直接依赖 `rocode-command` 的协议/展示类型，职责边界不清。

### 3.2 严重度分布（聚合问题）

- 高：4
- 中：3
- 低：1

---

## 4. 详细问题清单（含证据）

> 注：以下为“问题模式”级聚合，每个模式下仅展示最具代表性的证据。

### F1（高）大量重复且语义不一致的 lossy 反序列化函数

**问题描述**

`deserialize_opt_string_lossy` 在多个 crate 重复定义；同名函数的输入容忍策略不一致（有的仅 string，有的额外接受 number/bool）。

**证据**

- 计数证据：
  - 全仓命中：`fn deserialize_opt_string_lossy` 共 **40** 处。
- 语义差异证据：
  - `crates/rocode-command/src/lib.rs:421-429`
    - `Some(serde_json::Value::String(value)) => Some(value)`
    - ` _ => None`
  - `crates/rocode-permission/src/model.rs:9-19`
    - 额外接受 `Number` 与 `Bool` 并转字符串
  - `crates/rocode-tui/src/components/session_tool.rs:550, 656, 1492, 1701`
    - 同一文件内多次重复定义

**影响**

- 相同字段在不同路径解析结果可能不同
- 修复/增强需要多点同步，极易遗漏

**建议**

- 在公共 crate（建议 `rocode-types` 或新增 `rocode-util/serde`）建立**唯一来源**：
  - `deserialize_opt_string_lossy_strict`（仅 string）
  - `deserialize_opt_string_lossy_lenient`（string/number/bool）
- 原分散实现统一替换为公共函数；新增契约测试覆盖两种策略

---

### F2（高）`Role` 类型重复定义且已产生语义分叉

**问题描述**

`Role` 在 `rocode-types` 与 `rocode-content` 双重定义，成员集合不同。

**证据**

- `crates/rocode-types/src/role.rs:6-10`
  - `User / Assistant / System / Tool`
- `crates/rocode-content/src/output_blocks.rs:66-70`
  - `User / Assistant / System`（缺少 `Tool`）

**影响**

- 跨层传递时可能发生语义丢失或额外映射逻辑
- 新增角色时易出现“某层忘记更新”的隐患

**建议**

- 以 `rocode-types::Role` 为 canonical
- 其它层如需子集语义，使用显式新类型 + `TryFrom`，避免同名 enum 重定义

---

### F3（中）Scheduler 状态枚举重复（`StageStatus` vs `SchedulerStageStatus`）

**问题描述**

同一域状态存在两份枚举，命名/序列化别名策略不同。

**证据**

- `crates/rocode-content/src/stage_protocol.rs:53-63`
  - `pub enum StageStatus { Running, Waiting, Done, Cancelled, Cancelling, Blocked, Retrying }`
- `crates/rocode-core/src/contracts/scheduler.rs:193-202`
  - `pub enum SchedulerStageStatus { Running, Waiting, Cancelling, Cancelled, Done, Blocked, Retrying }`

**影响**

- parse/as_str 与兼容别名逻辑需要多处维护
- wire 协议一致性风险上升

**建议**

- 收敛到合同层单一定义（建议 `rocode-core/contracts`）
- 其它层仅 re-export 或薄包装，避免再次定义

---

### F4（高）`rocode-message` canonical 与 v2 模型重复度高

**问题描述**

`ToolState`、`RunningTime`、`CompletedTime`、`ErrorTime` 在 canonical 与 v2 各定义一套，并通过手工双向转换保持一致。

**证据**

- canonical：`crates/rocode-message/src/part.rs:110-173`
  - `RunningTime / CompletedTime / ErrorTime / ToolState`
- v2：`crates/rocode-message/src/message_v2.rs:166-220`
  - 再次定义同名结构
- 转换层：`crates/rocode-message/src/message_v2.rs:233+`
  - `v2_tool_state_to_canonical(...)` 等映射函数

**影响**

- 字段变更需双份更新 + 映射函数同步
- 任何遗漏都会造成序列化/反序列化行为偏差

**建议**

- 将 canonical 类型作为内部唯一模型
- v2 层仅保留 wire 差异（必要字段适配）
- 增加 round-trip 与字段完整性测试

---

### F5（中）Decision Render Spec 默认值与解析逻辑分散重复

**问题描述**

相同默认策略在 content/server/tui 三处定义。

**证据**

- `crates/rocode-content/src/output_blocks.rs:303-312`
  - `default_scheduler_decision_render_spec()`
- `crates/rocode-server/src/session_runtime/mod.rs:2801-2810`
  - `default_decision_render_spec()`
- `crates/rocode-tui/src/components/session_text.rs:1049-1054`
  - 本地默认配置（子集）

**影响**

- 默认策略更新后前后端/不同 UI 可能表现不一致

**建议**

- 默认 spec 统一下沉到共享模块（建议 content 或 contracts）
- server/tui 调用共享默认函数，不再硬编码副本

---

### F6（低）`strip_think_tags` 小型工具函数重复

**问题描述**

相同文本清洗函数在 command 与 tui 两处重复。

**证据**

- `crates/rocode-command/src/output_blocks.rs:392-396`
- `crates/rocode-tui/src/components/session_text.rs:1057-1060`

**影响**

- 目前风险低，但扩展规则时易分叉

**建议**

- 抽取为公共工具函数，统一单测

---

### F7（中）Scheduler Stage 渲染拼装逻辑在 CLI/TUI 平行复制

**问题描述**

状态行、token usage 等字符串拼装在多个 UI 层独立实现。

**证据**

- `crates/rocode-command/src/output_blocks.rs:147-163, 927-939`
- `crates/rocode-tui/src/components/session_text.rs:488-529`

**影响**

- 展示规则迭代需要双改，回归概率高

**建议**

- 抽取共享 formatter 核心或中间展示模型（view model）

---

### F8（高）模块放置职责倒置：`rocode-server` 依赖 `rocode-command`

**问题描述**

按命名职责看 `rocode-command` 偏命令/交互层，但 `rocode-server` 对其协议/展示类型形成直接依赖。

**证据**

- 依赖声明：`crates/rocode-server/Cargo.toml:22`
  - `rocode-command = { path = "../rocode-command" }`
- 使用证据（节选）：
  - `crates/rocode-server/src/runtime_control.rs:2`
  - `crates/rocode-server/src/session_runtime/mod.rs:15`
  - `crates/rocode-server/src/routes/session/scheduler.rs:7`

**影响**

- server 与 command 强耦合，阻碍分层演进
- 协议类型无法稳定沉淀到核心合同层

**建议**

- 将协议/通用类型迁移到 `rocode-core/contracts` 或 `rocode-content`
- `rocode-command` 回归“呈现/交互层”角色

---

## 5. 根因分析

1. **缺少“单一事实来源（SSOT）”约束**：公共类型与 helper 未强制沉淀到公共 crate。
2. **增量开发路径依赖**：为追求交付速度，局部复制粘贴替代了抽象复用。
3. **分层边界治理不足**：未建立/执行 crate 依赖方向规则（例如 lint 或架构守卫测试）。
4. **兼容层缺少清晰生命周期**：v2/legacy 层长期共存，导致“临时映射”演化为常态。

---

## 6. 优先整改清单（Top 10）

1. 合并所有 lossy serde helper 到公共模块（先做）
2. 统一 `Role` 为单一定义并删除重复 enum
3. 统一 Stage/Scheduler 状态枚举到合同层
4. 收敛 `message_v2` 与 canonical 的重复模型
5. 统一 Decision Render Spec 默认值来源
6. 解耦 `rocode-server -> rocode-command` 的协议依赖
7. 抽取 CLI/TUI 共享的 scheduler 渲染 formatter
8. 去重 `strip_think_tags` 等小型文本工具函数
9. 为关键合同类型增加 round-trip 契约测试
10. 对 `session_tool.rs` 等高重复文件先做试点重构

---

## 7. 分阶段落地路线图（建议 3 批次）

### 批次 A（低风险/高收益，1~2 周）

- 目标：先降重复率，最小化业务行为变化
- 内容：
  - 提取 `deserialize_opt_string_lossy*` 公共函数
  - 提取 `strip_think_tags` 公共函数
  - 补充 helper 契约测试
- 验收：
  - 同名 helper 定义数显著下降（目标：40 -> <=5）
  - 相关单元测试通过

### 批次 B（中风险，2~3 周）

- 目标：收敛核心类型与默认策略
- 内容：
  - `Role` 与 Scheduler 状态枚举统一
  - Decision Render Spec 默认值归一
  - 清理重复 parse/as_str 分支
- 验收：
  - 重复 enum 从 2 份收敛至 1 份 canonical
  - CLI/TUI/server 在同输入下渲染基线一致

### 批次 C（中高风险，3~4 周）

- 目标：处理架构边界与历史兼容层
- 内容：
  - `rocode-server` 与 `rocode-command` 解耦
  - `message_v2` 模型去重，保留必要适配层
  - 建立 crate 依赖方向守卫（CI 检查）
- 验收：
  - server 不再直接依赖 command 的协议类型
  - DTO 变更仅需改 canonical + 适配薄层

---

## 8. 统一整改验收标准（Definition of Done）

1. **类型唯一性**：核心域类型在仓库内仅有一份 canonical 定义。
2. **函数唯一性**：通用 helper 不再在业务文件局部重复定义。
3. **分层一致性**：依赖方向符合“contracts/core -> domain -> adapter/UI”。
4. **兼容可信性**：新增 round-trip 与兼容别名测试，覆盖关键协议类型。
5. **可观测性**：建立重复检测基线（例如 CI 中统计特定 helper/enum 的重复定义数）。

---

## 9. 建议立即执行的第一步

建议从 **F1（lossy helper 去重）** 开始：

- 改动面广但语义可控
- 可快速降低重复并建立公共函数迁移模板
- 为后续类型统一/架构解耦提供“可复制的重构路径”
