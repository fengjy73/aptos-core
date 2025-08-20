# （增量升级到 BlockSTMv2）

> **目标**：在不破坏你现有日志与文件布局的前提下，对 logger 与插桩进行补强，完整覆盖 **SchedulerV2 的 stall/abort/水位/提交-后处理** 语义，并让 `row_tx_mapping.csv` 能稳定把 **CSV 行→交易（incarnation/内核状态/依赖链）** 对上。
> **注意**：以下所有新增事件与字段均为**向后兼容**（维持你现有文件名与核心字段），仅追加或细化。已有字段保持不变（例如你现在的 `TransactionStart/Finish`、`StateTransition`、`AbortRecovery` 等）。

---

## A. 需要新增/细化的 **事件与字段**（按你现有文件分类）

> 说明里都标出**字段定义**、**插桩位置（v2 源码名）**、**为什么要记（对应 V2 语义）**，以及**示例**。

### 1) `execution_flow.ndjson`（保留）— 执行与回放主线

**新增事件**

* `TaskPicked` / `TaskFinished`

  * **字段**：

    * `task_kind`: `"Execute"` | `"PostCommit"`（V2 引入 post-commit 任务队列）
    * `tx_index`: 交易索引（0-based，与你现有 `transaction_id` 同值，冗余书写便于后续迁移）
    * `incarnation`: 当前转世号（u32）
    * `picked_ts_us` / `finished_ts_us`: 微秒时间戳（保留你现在的 ISO 时间戳同时再加一个微秒整形，便于作图）
  * **插桩**：`scheduler_v2.rs` 中**取任务**与**完成任务**路径。
  * **原因**：V2 任务分两类：执行与提交后的后处理（post-commit），调度器会优先派发 post-commit 以缩尾（PR 描述）。([GitHub][1])

**新增/细化字段（补到你现有 `TransactionStart/Finish`）**

* `is_reexecution`: bool（是否回放执行）
* `first_reexecution_deferred`: bool（是否命中**首次回放延迟**策略）
* `defer_reason`: `"Waterline"` | `"Stalled"` | `"QueueBackoff"`
* `executed_once_watermark`: `u32`（当时的“已至少执行过一次”的前缀水位 `executed_once_max_idx`）
* **插桩**：取自 `ExecutionQueueManager`/`SchedulerV2` 取任务与排队逻辑；当回放第一次被延迟时置位。
* **原因**：V2 会把**首轮回放**延后到水位（前缀都执行过一次）越过自身再执行，减少再次 abort（PR/说明）。([GitHub][1])

**示例**

```ndjson
{"event_type":"TaskPicked","task_kind":"Execute","tx_index":123,"incarnation":0,"picked_ts_us":1754474710127000}
{"event_type":"TaskFinished","task_kind":"Execute","tx_index":123,"incarnation":0,"finished_ts_us":1754474711127000,"is_reexecution":false,"first_reexecution_deferred":false,"executed_once_watermark":48000}
```

---

### 2) `scheduler_states.ndjson`（保留）— 状态机与 **stall** 传播

**保留**你现有的 `StateTransition`（Ready→Executing→Executed…）并**新增**以下事件：

* `StallAdd` / `StallRemove`

  * **字段**：

    * `tx_index`, `incarnation`
    * `by_tx`: 触发 stall 的**上游**交易索引（如果由“传播”触发，可置为下游被记忆的上游）
    * `stall_count_after`: 对该 txn 的**当前 stall 计数**（加一/减一后）
  * **定义**：V2 中**stall 必须配对**（像括号配对）：每次 `add_stall` 必须最终 `remove_stall`，计数为 0 才算“unstalled”。
  * **插桩**：`scheduler_status.rs` 的 `add_stall/remove_stall`。
  * **依据**：V2 在状态层实现可配平的 stall 语义，并由 SchedulerV2 管理跨交易传播。([GitHub][2])

* `StallPropagateTick`

  * **字段**：`source_tx`（作为传播起点的低序号 txn）、`affected_range`（如 `[j_min, j_max]`）、`propagated_count`
  * **插桩**：`SchedulerV2::propagate()` 批次传播完成处。
  * **意义**：显式记录本轮传播把多少**下游依赖**压住或释放，后续分析“并发度被谁卡住”。([GitHub][1])

* `WaterlineAdvance`

  * **字段**：`executed_once_max_idx_after`、`advanced_by_tx`
  * **插桩**：推进“已至少执行过一次”水位的地方（排队管理器/调度器）。
  * **意义**：支撑“首次回放延迟”的诊断与可视化（水位带来的解耦）。([GitHub][1])

**示例**

```ndjson
{"event_type":"StallAdd","tx_index":910,"incarnation":0,"by_tx":432,"stall_count_after":1,"timestamp":1754474710127475}
{"event_type":"StallPropagateTick","source_tx":432,"affected_range":[433,1200],"propagated_count":768}
{"event_type":"WaterlineAdvance","executed_once_max_idx_after":50000,"advanced_by_tx":49999}
```

> 兼容：你之前自带的 `validation_wave` 字段在 V1 时代有意义，V2 里可置 `null` 或保留但不再使用（避免破坏已有消费脚本）。

---

### 3) `dependencies.ndjson`（保留）— **因果链**与 AbortManager 输出

**新增事件**

* `InvalidationEdge`

  * **字段**：`by_tx`（低序号写者）、`to_tx`（被无效化的高序号 txn）、`to_incarnation`（被判定无效的具体 incarnation，如未知填 `null`）、`key`（冲突键；可选）
  * **插桩**：`AbortManager` 收集到 invalidations 时先缓存，`finish_execution` 批量上交调度器前记录一次边。
  * **意义**：把“谁让谁 abort”的**静态因果边**固化，便于画依赖 DAG 与热交易剖面。([GitHub][1])

* `DependencyBlocked` / `DependencyUnblocked`

  * **字段**：`dependent_tx`, `on_tx`（所依赖的上游），`reason`: `"EstimateRead"`|`"InFlightWrite"` 等
  * **插桩**：遇到依赖（读到上游估计版本或未完成写）与解除依赖处。
  * **意义**：与 `StallAdd/Remove` 对应，前者表达**数据层**阻断，后者表达**调度层**抑制。

**示例**

```ndjson
{"event_type":"InvalidationEdge","by_tx":432,"to_tx":910,"to_incarnation":0,"key":"0x1::coin::balances<...>"}
{"event_type":"DependencyBlocked","dependent_tx":910,"on_tx":432,"reason":"EstimateRead"}
```

---

### 4) `abort_recovery.ndjson`（保留）— 两段式 abort

把你现有的单事件 `AbortRecovery`（原因/对手方/重试计数…）细化为两段式：

* `AbortStart` / `AbortFinish`

  * **字段**：

    * `tx_index`, `incarnation`（被 abort 的目标）
    * `by_tx`（谁触发的 abort，通常是低序号写者）
    * `started`: bool（`AbortStart` 固定为 true）
    * `result`: `"AlreadyAborted"` | `"Started"`（start 阶段） / `"EnqueuedForReexec"` | `"NoReexec"`（finish 阶段是否回到执行队列）
  * **插桩**：`SchedulerV2::start_abort` / `finish_abort` 返回之后立即记录。
  * **依据**：V2 定义**两段式 abort** + `AbortManager` 本地收集 + 在 `finish_execution` 批量提交与传播（PR 讨论里也有“为什么需要重试 start\_abort/读集不清理”的注释）。([GitHub][1])

* `IncarnationBump`

  * **字段**：`tx_index`, `old_incarnation`, `new_incarnation`
  * **插桩**：目标 txn 再次入队前。
  * **意义**：把回放转世与 abort 串成闭环，便于统计“谁 thrash”。

**示例**

```ndjson
{"event_type":"AbortStart","tx_index":910,"incarnation":0,"by_tx":432,"result":"Started"}
{"event_type":"AbortFinish","tx_index":910,"incarnation":0,"by_tx":432,"result":"EnqueuedForReexec"}
{"event_type":"IncarnationBump","tx_index":910,"old_incarnation":0,"new_incarnation":1}
```

---

### 5) `system_operations.ndjson`（保留）— 提交与 Post-Commit 段

**新增事件**

* `CommitMarkerTransition`

  * **字段**：`tx_index`, `from_marker`: `"NotCommitted"|"CommitStarted"|"Committed"`, `to_marker`
  * **插桩**：顺序提交路径（带“arming”的提交锁）改变标志位时。
  * **意义**：V2 把**提交阶段三态化**并行化后处理队列。([GitHub][1])

* `PostCommitStart` / `PostCommitFinish`

  * **字段**：`tx_index`, `hook_kind`（聚合器/延迟字段等），`duration_us`
  * **插桩**：post-commit 钩子执行处（队列消费）。
  * **意义**：覆盖**并行 post-commit** 对尾延迟的影响。([GitHub][1])

---

### 6) `block_summary.ndjson`（保留）— 区块级汇总补栏

在你已有汇总上，**新增**：

* `stall_metrics`: `{ "adds": u64, "removes": u64, "max_stall_depth": u32 }`
* `abort_v2`: `{ "start_count": u64, "finish_count": u64, "inc_bumps": u64 }`
* `waterline`: `{ "executed_once_max_idx": u32, "deferral_first_reexec": u64 }`
* `post_commit`: `{ "tasks": u64, "avg_duration_us": f64 }`

---

### 7) `mvhashmap_ops.ndjson`（可选增强）

如果你想把**读到估计版本**与**读来源**一并记下，增加：

* `MVRead`：`{"key","by_tx","incarnation","source":"Committed|EstimateOf(tx)","result":"Ok|Dependency"}`
  配合 `DependencyBlocked` 一起看，能更快锁定高冲突键。

---

## B. **CSV 行→交易** 映射升级（`row_tx_mapping.csv`）

你现有映射列为：`csv_row,transaction_id,from_address,to_address,value,tx_hash`。建议**追加**以下列（保持向后兼容）：

* `tx_index`：= `transaction_id`（冗余，未来迁移到更语义化的名字）
* `incarnation_final`: 该交易最终成功的转世号
* `aborted_times`: 被 abort 次数
* `stalled_times`: 被加 stall 次数（`StallAdd` 次）
* `first_reexec_deferred`: true/false（是否经历过“首次回放延迟”）
* `hotspot_flag`: true/false（如果在 `InvalidationEdge` 里作为 **by\_tx** 出现次数超过阈值或因其导致的下游总 stall 超过阈值）
* `dataset`: `ETH` | `USDT`（方便做跨数据集对比）
* `row_hash`: 对 `from,to,value` 做稳定 hash（便于去重/复现实验）

**示例**

```csv
csv_row,transaction_id,tx_index,from_address,to_address,value,tx_hash,incarnation_final,aborted_times,stalled_times,first_reexec_deferred,hotspot_flag,dataset,row_hash
0,0,0,account_1234,account_5678,100,0xabcd...,1,0,3,false,true,ETH,0x8e1c...
```

---

## C. **插桩位置清单（v2 源码锚点）**

> 具体函数名/模块基于 PR 描述与文件命名（`scheduler_status.rs` / `scheduler_v2.rs`），先 `grep`/`code search` 定位，再在**状态改变/返回点**写日志调用。

* **取/完成任务**：`SchedulerV2` 的任务获取与完成路径 → 记 `TaskPicked/TaskFinished` 与执行/后处理时长。([GitHub][1])
* **stall 加/减**：`scheduler_status.rs` 的 `add_stall/remove_stall` → 记 `StallAdd/StallRemove`。([GitHub][2])
* **stall 传播**：`SchedulerV2::propagate()` 批次末尾 → 记 `StallPropagateTick`。([GitHub][1])
* **水位推进**：推进 `executed_once_max_idx` 的位置 → 记 `WaterlineAdvance`，并在取任务处根据水位决定 `first_reexecution_deferred`。([GitHub][1])
* **两段式 abort**：`start_abort` / `finish_abort` 返回后 → 记 `AbortStart/AbortFinish`；incarnation 增加时 → 记 `IncarnationBump`。([GitHub][2])
* **AbortManager 汇总**：执行完成、提交给调度器前 → 记 `InvalidationEdge` 批。([GitHub][1])
* **提交三态与 post-commit**：提交路径的标志位变化与钩子执行 → 记 `CommitMarkerTransition`、`PostCommitStart/Finish`。([GitHub][1])

> 参考资料：Block-STM 论文（V1 的协作调度/验证节拍）、Aptos 官方执行文档（Block-STM 动态并行执行背景），与 BlockSTMv2 的两则 PR（状态层引入 stall + 两段式 abort；SchedulerV2 接管队列与传播/提交-后处理）。([arXiv][3], [Aptos][4], [GitHub][2])

---

## D. **字段定义（统一口径）**

> 下面仅列新增或容易歧义的字段；你已有字段不变（见你文档“记录字段”节）。

* `tx_index`：交易在区块内的固定序号（0-based）。与现有 `transaction_id` 等值，保留两者用于向后兼容。
* `incarnation` / `incarnation_final`：同一 `tx_index` 的第 N 次执行尝试编号，从 0 递增。
* `task_kind`：`"Execute"`（主执行）或 `"PostCommit"`（提交后的并行钩子）。
* `stall_count_after`：对某 txn 在状态层累计的 stall 计数（`add_stall`+1，`remove_stall`-1）。
* `executed_once_max_idx`（watermark）：已**至少执行过一次**的前缀最大索引。
* `first_reexecution_deferred`：该 txn 的**第一次回放**是否因水位/被 stall 延后。
* `by_tx` / `dependent_tx` / `on_tx`：分别表示**施加影响的上游**、**受影响的下游**、**被依赖的上游**。
* `CommitMarker`: `"NotCommitted"|"CommitStarted"|"Committed"`（V2 的三态提交标志）。

---

## E. **与现有日志风格的兼容性**

* **时间戳**：保留你现有的 ISO8601 `timestamp`，**额外**写入整型 `*_ts_us` 便于高精度对齐（你给的 `SchedulerStateTransition` 微秒样例也能原样写到 `scheduler_states.ndjson`）。
* **文件名**：全部沿用你现有命名，不新建新文件夹（读取脚本零修改即可工作）。
* **老字段**：如 `validation_wave` 在 V2 设置为 `null`（或 0），将来可在可视化侧隐藏。

---

## F. **落地顺序建议（很快能跑通）**

1. 在 `scheduler_status.rs` 打点 `StallAdd/Remove`；在 `scheduler_v2.rs` 打点 `TaskPicked/TaskFinished`、`StallPropagateTick`、`WaterlineAdvance`。([GitHub][2])
2. 在 `start_abort/finish_abort` 与 `finish_execution`（提交 `AbortManager` 的地方）加 `AbortStart/AbortFinish/InvalidationEdge/IncarnationBump`。([GitHub][1])
3. 在提交路径加 `CommitMarkerTransition` 与 `PostCommit*`。([GitHub][1])
4. 扩展 `row_tx_mapping.csv` 列并写入数据（不移除旧列）。
5. 在 `execution_flow.ndjson` 上补 `is_reexecution / first_reexecution_deferred / executed_once_watermark`。([GitHub][1])

---

## G. 小样例（合并你现有风格）

```ndjson
{"event_type":"StateTransition","timestamp": "2025-08-09T05:09:58.488956Z","tx_index":910,"incarnation":0,"from_state":"Executing","to_state":"Executed","thread_id":1}
{"event_type":"StallAdd","tx_index":910,"incarnation":0,"by_tx":432,"stall_count_after":1,"timestamp":1754474710127475}
{"event_type":"AbortStart","tx_index":910,"incarnation":0,"by_tx":432,"result":"Started","timestamp":1754474710130000}
{"event_type":"AbortFinish","tx_index":910,"incarnation":0,"by_tx":432,"result":"EnqueuedForReexec"}
{"event_type":"IncarnationBump","tx_index":910,"old_incarnation":0,"new_incarnation":1}
{"event_type":"TaskPicked","task_kind":"Execute","tx_index":910,"incarnation":1,"picked_ts_us":1754474711127000,"first_reexecution_deferred":true,"executed_once_watermark":50000}
{"event_type":"StallPropagateTick","source_tx":432,"affected_range":[433,1200],"propagated_count":768}
{"event_type":"CommitMarkerTransition","tx_index":432,"from_marker":"NotCommitted","to_marker":"CommitStarted"}
{"event_type":"PostCommitStart","tx_index":432,"hook_kind":"aggregator"}
{"event_type":"PostCommitFinish","tx_index":432,"hook_kind":"aggregator","duration_us":210}
```
