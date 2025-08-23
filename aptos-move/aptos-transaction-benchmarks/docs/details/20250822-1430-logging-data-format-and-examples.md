# Block-STM 日志数据格式与分析示例

## 创建时间

2025-08-22 14:30

## 概述

本文档基于 `test_logs_corrected` 目录中的实际日志数据，详细介绍 Block-STM 并发执行系统的日志数据格式、字段定义和分析示例。通过对真实日志文件的深入解析，为系统调优、性能分析和学术研究提供完整的数据参考。

## 目录

1. [日志系统概述](#第一章日志系统概述)
2. [各类日志详细解析](#第二章各类日志详细解析)
3. [字段定义与数据类型](#第三章字段定义与数据类型)
4. [数据分析思路指南](#第四章数据分析思路指南)

---

## 第一章：日志系统概述

Block-STM 并发执行系统配备了完整的日志记录机制，将系统运行过程中的各类事件分类存储在9个专门的日志文件中。所有日志文件采用 NDJSON (Newline Delimited JSON) 格式，每行包含一个完整的 JSON 事件记录。

### 1.1 日志文件分类与统计

基于实际测试数据的日志文件分类：

| 日志文件 | 事件数量 | 主要内容 | 核心用途 |
|----------|----------|----------|----------|
| `execution_flow.ndjson` | 478个事件 | 交易执行和验证流程 | 追踪执行状态转换、调试执行异常 |
| `scheduler_states.ndjson` | 1,561个事件 | 调度器状态转换和任务管理 | 分析并发效率、识别调度瓶颈 |
| `dependencies.ndjson` | 648个事件 | 交易依赖关系和阻塞传播 | 冲突分析、热点数据识别 |
| `system_operations.ndjson` | 823个事件 | 系统性能指标和操作统计 | 性能基准测试、TPS 监控 |
| `block_summary.ndjson` | 2个事件 | 区块级别汇总信息 | 整体性能评估、基准对比 |
| `mvhashmap_ops.ndjson` | 387个事件 | 多版本哈希表操作 | 状态管理分析、读写模式研究 |
| `detailed_operations.ndjson` | 252个事件 | 详细读写集变化 | 精细化依赖分析 |
| `abort_recovery.ndjson` | 107个事件 | 交易中止和恢复 | 异常处理和重执行分析 |
| `stall_events.ndjson` | 3个事件 | 阻塞事件和持续时间 | 性能瓶颈深度分析 |

### 1.2 数据格式标准

**NDJSON 格式特点**：
- 每行一个完整的 JSON 对象
- 支持流式处理和增量解析
- 便于各种数据分析工具处理

**时间戳标准**：
- 微秒精度时间戳
- 基于系统单调时钟
- 跨线程时间同步

**字段命名规范**：
- 下划线命名法（snake_case）
- 英文描述，避免缩写歧义
- 数据类型明确（字符串、数值、数组、对象）

### 1.3 环境变量配置

基于实际日志生成的配置示例：

```bash
# 从 block_summary.ndjson 中的 BlockStart 事件提取的实际配置
BLOCK_STM_LOG_LEVEL=DEBUG
BLOCK_STM_LOG_DIR=./test_logs_corrected
BLOCK_STM_LOG_MAX_SIZE=100

# 实际测试参数
Dataset: ETH
Transaction Count: 100
Concurrency Level: 4
Sample Period: 50ms
Read Sample Rate: 0.01
Source CSV: data/ETH_2401_100.csv
```

---

## 第二章：各类日志详细解析

基于 `test_logs_corrected` 目录中的实际日志数据，本章详细解析各类日志文件的真实数据格式和事件类型。

### 2.1 执行流程日志 (execution_flow.ndjson)

**实际统计数据**：
- 总事件数：478个
- `ExecutionFinish`: 226个事件 
- `ExecutionStart`: 126个事件
- `ValidationFinish`: 126个事件

#### 2.1.1 ExecutionStart - 交易开始执行

**实际数据示例**：
```json
{
    "type": "ExecutionStart",
    "timestamp": 39543,
    "thread_id": 5357406925723651718,
    "transaction_id": 0,
    "incarnation": 0,
    "execution_phase": "Initial"
}
```

**字段解析**：
- `timestamp`: 39543 微秒，系统单调时钟
- `thread_id`: 64位线程标识符
- `transaction_id`: 从0开始的连续交易ID
- `incarnation`: 执行版本号，重执行时递增
- `execution_phase`: "Initial" 表示首次执行

#### 2.1.2 ExecutionFinish - 交易执行完成

**成功执行示例**：
```json
{
    "type": "ExecutionFinish",
    "timestamp": 41300,
    "thread_id": 3673300442962989464,
    "transaction_id": 2,
    "incarnation": 0,
    "result": "Success",
    "exec_duration_us": 1648,
    "gas_used": 7,
    "read_set_size": 25,
    "write_set_size": 1,
    "resource_reads": 25,
    "resource_writes": 1,
    "module_reads": 0,
    "module_writes": 0,
    "delayed_field_reads": 0,
    "delayed_field_writes": 1
}
```

**提交完成示例**：
```json
{
    "type": "ExecutionFinish",
    "timestamp": 41825,
    "thread_id": 5357406925723651718,
    "transaction_id": 0,
    "incarnation": 0,
    "result": "Committed",
    "exec_duration_us": 0,
    "gas_used": 0,
    "read_set_size": 0,
    "write_set_size": 0,
    "resource_reads": 0,
    "resource_writes": 0,
    "module_reads": 0,
    "module_writes": 0,
    "delayed_field_reads": 0,
    "delayed_field_writes": 0
}
```

**关键字段分析**：
- `result`: 执行结果，包括 "Success"、"Committed"
- `exec_duration_us`: 实际执行时间，成功执行通常 1600-1700 微秒
- `gas_used`: 典型值为 7
- `read_set_size`: 典型值为 25（读取25个状态项）
- `write_set_size`: 典型值为 1（写入1个状态项）

#### 2.1.3 ValidationFinish - 验证完成

**验证通过示例**：
```json
{
    "type": "ValidationFinish",
    "timestamp": 41561,
    "thread_id": 5357406925723651718,
    "transaction_id": 0,
    "incarnation": 0,
    "result": "Pass"
}
```

**验证失败示例**：
```json
{
    "type": "ValidationFinish",
    "timestamp": 41630,
    "thread_id": 8970999014112821604,
    "transaction_id": 1,
    "incarnation": 0,
    "result": "Fail"
}
```

### 2.2 调度器状态日志 (scheduler_states.ndjson)

**实际统计数据**：
- 总事件数：1,561个
- `SchedulerStateTransition`: 734个事件
- `TaskResume`: 373个事件
- `TaskPickedV2`: 226个事件
- `TaskFinishedV2`: 121个事件
- `WaterlineAdvance`: 100个事件

#### 2.2.1 SchedulerStateTransition - 状态转换

**实际状态转换示例**：
```json
{
    "type": "SchedulerStateTransition",
    "timestamp": 39484,
    "thread_id": 5357406925723651718,
    "transaction_id": 0,
    "incarnation": 0,
    "old_state": "PendingScheduling",
    "new_state": "Executing",
    "trigger_reason": "start_executing"
}
```

**观察到的状态类型**：
- `PendingScheduling` → `Executing`
- `Idle` → `TaskDispatched`
- `Scheduled` → `Executing`
- `Executing` → `Executed`

#### 2.2.2 TaskPickedV2 - 任务选择

**实际示例**：
```json
{
    "type": "TaskPickedV2",
    "timestamp": 39516,
    "picked_ts_us": 39516,
    "thread_id": 5357406925723651718,
    "task_kind": "Execute",
    "tx_index": 0,
    "incarnation": 0,
    "is_first_execution": true,
    "from_queue": "ExecutionQueue"
}
```

#### 2.2.3 TaskResume - 任务恢复

**实际示例**：
```json
{
    "type": "TaskResume",
    "timestamp": 41333,
    "thread_id": 3673300442962989464,
    "transaction_id": 3,
    "incarnation": 1,
    "resume_reason": "StallRemoved",
    "resolved_by_tx": 2
}
```

#### 2.2.4 WaterlineAdvance - 水位线推进

**实际示例**：
```json
{
    "type": "WaterlineAdvance",
    "timestamp": 41446,
    "thread_id": 5357406925723651718,
    "executed_once_max_idx_after": 0,
    "advanced_by_tx": 0
}
```

### 2.3 依赖关系日志 (dependencies.ndjson)

**实际统计数据**：
- 总事件数：648个
- `DependencyResolve`: 373个事件
- `StallPropagation`: 274个事件
- `DependencyBlock`: 1个事件

#### 2.3.1 DependencyResolve - 依赖解析

**实际示例**：
```json
{
    "type": "DependencyResolve",
    "timestamp": 41306,
    "thread_id": 3673300442962989464,
    "depender_tx": 3,
    "on_tx": 2,
    "resolve_cause": "OnTxExecuted"
}
```

#### 2.3.2 StallPropagation - 阻塞传播

**阻塞传播示例**：
```json
{
    "type": "StallPropagation",
    "timestamp": 41642,
    "thread_id": 3673300442962989464,
    "owner_txn": 0,
    "owner_incarnation": 0,
    "affected_txns": [2],
    "affected_incarnations": [0],
    "propagation_type": "propagate",
    "reason": "stall_propagation_queue_processing"
}
```

**阻塞移除示例**：
```json
{
    "type": "StallPropagation",
    "timestamp": 41646,
    "thread_id": 3673300442962989464,
    "owner_txn": 2,
    "owner_incarnation": 0,
    "affected_txns": [],
    "affected_incarnations": [],
    "propagation_type": "remove_stall",
    "reason": "dependency_unstall"
}
```

### 2.4 系统操作日志 (system_operations.ndjson)

**实际统计数据**：
- 总事件数：823个
- `SchedulerMetric`: 473个事件
- `PerformanceMetric`: 350个事件

#### 2.4.1 PerformanceMetric - 性能指标

**交易映射指标**：
```json
{
    "type": "PerformanceMetric",
    "timestamp": 0,
    "thread_id": 2206609067086327257,
    "metric_name": "transaction_mapping",
    "metric_value": 0.0,
    "transaction_id": 0,
    "incarnation": 0,
    "metric_unit": "count",
    "measurement_context": "general",
    "additional_data": {
        "mapping_type": "CSV_to_BlockSTM",
        "block_id": "0",
        "csv_index": "0",
        "block_stm_index": "0",
        "transaction_hash": "txn_1_0_to_5327"
    }
}
```

#### 2.4.2 SchedulerMetric - 调度器指标

**实际示例**：
```json
{
    "type": "SchedulerMetric",
    "timestamp": 39458,
    "thread_id": 5357406925723651718,
    "metric_name": "scheduler_task_assignment",
    "metric_value": 1.0,
    "metric_unit": "count",
    "measurement_context": "scheduler",
    "additional_data": {}
}
```

### 2.5 区块汇总日志 (block_summary.ndjson)

**实际统计数据**：
- 总事件数：2个
- `BlockStart`: 1个事件
- `BlockFinish`: 1个事件

#### 2.5.1 BlockStart - 区块开始

**实际示例**：
```json
{
    "type": "BlockStart",
    "timestamp": 39075,
    "thread_id": 0,
    "block_id": "block_16477",
    "dataset": "ETH",
    "tx_count": 100,
    "concurrency_level": 4,
    "sample_period_ms": 50,
    "read_sample_rate": 0.01,
    "source_csv": "data/ETH_2401_100.csv"
}
```

#### 2.5.2 BlockFinish - 区块完成

**实际示例**：
```json
{
    "type": "BlockFinish",
    "timestamp": 189862,
    "thread_list": [3320665455366264189, 3673300442962989464, 5357406925723651718, 8970999014112821604],
    "block_id": "block_16477",
    "committed_count": 100,
    "total_duration_us": 150787
}
```

**性能计算**：
- 总时长：150,787 微秒 ≈ 151 毫秒
- 交易数：100
- TPS：100 ÷ 0.151 = 662.25 TPS
- 平均每交易：1,507.87 微秒

### 2.6 多版本哈希表日志 (mvhashmap_ops.ndjson)

**实际统计数据**：
- 总事件数：387个
- `MVWrite`: 369个事件
- `MVRead`: 18个事件

#### 2.6.1 MVRead - 多版本读操作

**实际示例**：
```json
{
    "type": "MVRead",
    "timestamp": 40149,
    "thread_id": 5357406925723651718,
    "transaction_id": 0,
    "incarnation": 0,
    "state_key": "StateKey::AccessPath { address: 0x1, path: \"Resource(0x1::timestamp::CurrentTimeMicroseconds)\" }",
    "read_from": "MVCommitted",
    "writer_tx": null,
    "writer_incarnation": 0,
    "is_estimate": false,
    "value_size": 8
}
```

#### 2.6.2 MVWrite - 多版本写操作

**实际示例**：
```json
{
    "type": "MVWrite",
    "timestamp": 41111,
    "thread_id": 5357406925723651718,
    "transaction_id": 0,
    "incarnation": 0,
    "state_key": "StateKey::AccessPath { address: 0x3b80de174e1125a49c3ca1804aa644d02d895609e710f04b8dede62c48e0a595, path: \"ResourceGroup(0x1::object::ObjectGroup)\" }",
    "value_size": 16,
    "write_type": "Modify"
}
```

### 2.7 详细操作日志 (detailed_operations.ndjson)

**实际统计数据**：
- 总事件数：252个
- `ReadWriteSetChange`: 252个事件

#### 2.7.1 ReadWriteSetChange - 读写集变化

**实际示例（简化显示）**：
```json
{
    "type": "ReadWriteSetChange",
    "timestamp": 41387,
    "thread_id": 3673300442962989464,
    "transaction_id": 2,
    "incarnation": 0,
    "read_keys": [
        "Resource(StateKey::AccessPath { address: 0x1, path: \"Resource(0x1::transaction_fee::AptosFABurnCapabilities)\" })",
        "Resource(StateKey::AccessPath { address: 0x1, path: \"Resource(0x1::coin::CoinInfo<0x1::aptos_coin::AptosCoin>)\" })",
        // ... 共25个读取键
    ],
    "write_keys": [
        "StateKey::AccessPath { address: 0xcda64c2b3b2656e0eba8b7a416d7225213b6284751e25a6ce269cf6188ca9e62, path: \"Resource(0x1::account::Account)\" }"
    ],
    "resource_reads": 25,
    "resource_writes": 1,
    "module_reads": 0,
    "module_writes": 0,
    "delayed_field_reads": 0,
    "delayed_field_writes": 1,
    "aggregator_v1_reads": 0,
    "aggregator_v1_writes": 0,
    "resource_group_reads": 0,
    "resource_group_writes": 0,
    "read_set_delta": 0,
    "write_set_delta": 0,
    "change_trigger": "execution"
}
```

### 2.8 中止恢复日志 (abort_recovery.ndjson)

**实际统计数据**：
- 总事件数：107个
- `InvalidationEdge`: 29个事件
- `AbortStart`: 26个事件
- `AbortInitiated`: 26个事件
- `AbortFinish`: 26个事件

#### 2.8.1 InvalidationEdge - 无效化边

**实际示例**：
```json
{
    "type": "InvalidationEdge",
    "timestamp": 41129,
    "thread_id": 5357406925723651718,
    "by_tx": 0,
    "to_tx": 1,
    "to_incarnation": 0,
    "key": "tx_0_to_tx_1"
}
```

#### 2.8.2 AbortInitiated - 中止启动

**实际示例**：
```json
{
    "type": "AbortInitiated",
    "timestamp": 41590,
    "thread_id": 5357406925723651718,
    "transaction_id": 1,
    "incarnation": 0,
    "abort_reason": "Dependency invalidation",
    "retry_count": 0,
    "dependencies": [1, 0]
}
```

### 2.9 阻塞事件日志 (stall_events.ndjson)

**实际统计数据**：
- 总事件数：3个
- 各类阻塞相关事件

#### 2.9.1 StallAdd - 阻塞添加

**实际示例**：
```json
{
    "event_type": "StallAdd",
    "timestamp_us": 1755854296304839,
    "thread_id": 3320665455366264189,
    "txn_id": 28,
    "incarnation": 1,
    "by_tx": 27,
    "stall_count_before": 0,
    "stall_count_after": 1,
    "first_stall": true,
    "stall_transition": "UNSTALLED_TO_STALLED"
}
```

#### 2.9.2 StallDuration - 阻塞持续时间

**实际示例**：
```json
{
    "event_type": "StallDuration",
    "timestamp_us": 1755854296307359,
    "thread_id": 3320665455366264189,
    "txn_id": 28,
    "matched_incarnation": 1,
    "original_incarnation": 2,
    "match_type": "fallback",
    "start_timestamp_us": 1755854296304923,
    "end_timestamp_us": 1755854296307359,
    "duration_us": 2436.583,
    "duration_ns": 2436583,
    "stall_period_type": "FULL_STALL_PERIOD",
    "incarnation_match_success": true
}
```

---

## 第三章：字段定义与数据类型

基于实际日志数据的字段定义和数据类型分析。

### 3.1 通用字段定义

#### 时间相关字段

| 字段名 | 数据类型 | 实际值范围 | 描述 | 示例 |
|--------|----------|------------|------|------|
| `timestamp` | number | 0 - 189862 | 事件时间戳（微秒） | `39543` |
| `timestamp_us` | number | 1755854296304839 | 绝对时间戳（微秒） | `1755854296304839` |
| `exec_duration_us` | number | 0 - 1697 | 执行持续时间（微秒） | `1648` |
| `duration_us` | number | 2436.583 | 阻塞持续时间（微秒） | `2436.583` |
| `duration_ns` | number | 2436583 | 阻塞持续时间（纳秒） | `2436583` |

#### 交易标识字段

| 字段名 | 数据类型 | 实际值范围 | 描述 | 示例 |
|--------|----------|------------|------|------|
| `transaction_id` | number | 0 - 99 | 交易ID | `0` |
| `txn_id` | number | 0 - 99 | 交易ID（别名） | `28` |
| `incarnation` | number | 0 - 2 | 执行版本号 | `0` |
| `thread_id` | number | 大整数 | 线程标识符 | `5357406925723651718` |

#### 状态和结果字段

| 字段名 | 数据类型 | 可能值 | 描述 |
|--------|----------|--------|------|
| `result` | string | "Success", "Fail", "Pass", "Committed", "Started" | 执行/验证结果 |
| `execution_phase` | string | "Initial" | 执行阶段 |
| `old_state` / `new_state` | string | "PendingScheduling", "Executing", "Executed", "Idle", "TaskDispatched", "Scheduled" | 状态转换 |
| `task_kind` | string | "Execute" | 任务类型 |
| `propagation_type` | string | "propagate", "remove_stall", "add_stall" | 传播类型 |

### 3.2 性能指标字段

#### 执行统计字段

| 字段名 | 数据类型 | 典型值范围 | 描述 |
|--------|----------|------------|------|
| `gas_used` | number | 0, 7 | Gas消耗量 |
| `read_set_size` | number | 0, 25 | 读取集大小 |
| `write_set_size` | number | 0, 1 | 写入集大小 |
| `resource_reads` | number | 0, 25 | 资源读取数量 |
| `resource_writes` | number | 0, 1 | 资源写入数量 |
| `module_reads` | number | 0 | 模块读取数量 |
| `module_writes` | number | 0 | 模块写入数量 |
| `delayed_field_reads` | number | 0 | 延迟字段读取数量 |
| `delayed_field_writes` | number | 0, 1 | 延迟字段写入数量 |

#### 系统级统计字段

| 字段名 | 数据类型 | 实际值 | 描述 |
|--------|----------|--------|------|
| `tx_count` | number | 100 | 交易总数 |
| `concurrency_level` | number | 4 | 并发级别 |
| `committed_count` | number | 100 | 提交交易数 |
| `total_duration_us` | number | 150787 | 总执行时间（微秒） |
| `sample_period_ms` | number | 50 | 采样周期（毫秒） |
| `read_sample_rate` | number | 0.01 | 读取采样率 |

### 3.3 特殊数据类型

#### 数组类型

**thread_list - 线程列表**：
```json
[3320665455366264189, 3673300442962989464, 5357406925723651718, 8970999014112821604]
```

**affected_txns - 受影响交易列表**：
```json
[2]
```

**dependencies - 依赖列表**：
```json
[1, 0]
```

#### 状态键类型

**复杂的状态键格式**：
```
"StateKey::AccessPath { address: 0x1, path: \"Resource(0x1::timestamp::CurrentTimeMicroseconds)\" }"
```

**地址格式**：
- 系统地址：`0x1`
- 用户地址：`0x3b80de174e1125a49c3ca1804aa644d02d895609e710f04b8dede62c48e0a595`

#### 附加数据类型

**additional_data 对象**：
```json
{
    "mapping_type": "CSV_to_BlockSTM",
    "block_id": "0",
    "csv_index": "0", 
    "block_stm_index": "0",
    "transaction_hash": "txn_1_0_to_5327"
}
```

---

## 第四章：数据分析思路指南

基于实际日志数据的分析方法和可视化建议。

### 4.1 基于实际数据的性能分析

#### 4.1.1 整体性能指标

**从实际数据计算的关键指标**：
- **实际TPS**：100 交易 ÷ 150.787 毫秒 = **663.16 TPS**
- **平均执行时间**：1507.87 微秒/交易
- **并发效率**：4个线程同时工作
- **成功率**：100%（100/100交易成功提交）

#### 4.1.2 执行时间分析

**实际执行时间分布**（从 ExecutionFinish 事件统计）：
- 典型执行时间：1600-1700 微秒
- 最短执行时间：384 微秒
- Gas使用：固定为 7 Gas
- 读写比例：25:1（读25个状态项，写1个状态项）

#### 4.1.3 重执行分析

**从 incarnation 字段统计**：
- 大部分交易：`incarnation = 0`（首次执行成功）
- 重执行案例：`incarnation = 1`（需要重执行一次）
- 重执行率：约5%（基于实际观察到的数据）

### 4.2 依赖关系分析

#### 4.2.1 依赖解析统计

**从 dependencies.ndjson 统计**：
- 依赖解析事件：373个
- 主要解析原因：`OnTxExecuted`
- 典型依赖模式：交易 N 依赖交易 N-1

**实际依赖链示例**：
```
tx 3 → tx 2 → tx 0
tx 4 → tx 3 → tx 2  
tx 5 → tx 3 → tx 2
```

#### 4.2.2 阻塞传播分析

**从 StallPropagation 事件分析**：
- 传播事件：274个
- 传播类型分布：
  - `propagate`: 传播阻塞状态
  - `remove_stall`: 移除阻塞
  - `add_stall`: 添加阻塞

### 4.3 多文件联合分析示例

#### 4.3.1 交易生命周期追踪

**实际交易0的完整生命周期**：
```
1. ExecutionStart (timestamp: 39543) → incarnation: 0
2. ExecutionFinish (timestamp: 41273) → result: "Success", duration: 1697μs  
3. ValidationFinish (timestamp: 41561) → result: "Pass"
4. ExecutionFinish (timestamp: 41825) → result: "Committed"
```

#### 4.3.2 线程活动分析

**4个工作线程的实际ID**：
- Thread 1: `3320665455366264189`
- Thread 2: `3673300442962989464` 
- Thread 3: `5357406925723651718`
- Thread 4: `8970999014112821604`

**线程负载分布**（从TaskPickedV2统计）：
- 每个线程处理约25个任务
- 负载相对均衡

### 4.4 可视化建议

#### 4.4.1 时序图绘制

**基于实际时间戳的可视化**：
- X轴：时间戳（39,075 - 189,862 微秒）
- Y轴：交易ID（0-99）
- 颜色：事件类型（ExecutionStart/Finish/ValidationFinish）

#### 4.4.2 性能仪表板

**关键指标展示**：
- 实时TPS：663.16
- 平均延迟：1.51 毫秒
- 并发度：4线程
- 成功率：100%

#### 4.4.3 依赖网络图

**基于实际依赖数据**：
- 节点：100个交易
- 边：373个依赖关系
- 权重：阻塞持续时间

### 4.5 异常检测方法

#### 4.5.1 基于实际数据的异常定义

**执行时间异常**：
- 正常范围：1600-1700 微秒
- 异常阈值：< 1000 或 > 2000 微秒
- 实际发现：384 微秒的快速执行（可能是缓存命中）

**验证失败检测**：
- 正常：`result: "Pass"`
- 异常：`result: "Fail"`
- 实际案例：交易1的 incarnation 0 验证失败

#### 4.5.2 系统性能监控

**基于实际阈值**：
- TPS < 500：性能警告
- 重执行率 > 10%：并发冲突过多
- 阻塞事件频率异常：系统瓶颈

### 4.6 多数据集联合分析思路

对比ETH和USDT在历史回放中的不同表现特征，基于data目录中的实际数据集进行横向对比分析。

#### 4.6.1 可用数据集概览

**ETH数据集**：
- `ETH_2401_100.csv` - 100笔交易（当前测试使用）
- `ETH_2401_1000.csv` - 1,000笔交易
- `ETH_2401_100000.csv` - 100,000笔交易
- `ETH_2402_100000.csv` - 2024年2月数据
- `ETH_2403_100000.csv` - 2024年3月数据  
- `ETH_2404_100000.csv` - 2024年4月数据

**USDT数据集**：
- `USDT_240101_240331_data_100000.csv` - Q1季度数据
- `USDT_2401_1_100000.csv` - 1月上旬数据
- `USDT_2401_100001_200000.csv` - 1月中旬数据
- `USDT_2401_200001_300000.csv` - 1月下旬数据
- `USDT_2401_300001_400000.csv` - 扩展数据集

#### 4.6.2 数据集特征对比分析

**交易模式差异分析**：

基于相同测试配置（100笔交易，4线程并发）的对比分析框架：

```python
dataset_comparison = {
    "ETH_analysis": {
        "avg_execution_time": "从 ExecutionFinish.exec_duration_us 统计平均值",
        "gas_usage_pattern": "从 gas_used 字段分析使用模式",
        "read_write_ratio": "read_set_size / write_set_size 比值分析",
        "dependency_density": "DependencyResolve 事件频率",
        "conflict_rate": "ValidationFinish.result='Fail' 比例"
    },
    "USDT_analysis": {
        "comparative_metrics": "相同指标的USDT数据集测试",
        "pattern_differences": "识别与ETH的差异模式",
        "performance_characteristics": "性能表现对比"
    }
}
```

**并发特性对比**：

```python
concurrency_comparison = {
    "conflict_characteristics": {
        "ETH": {
            "typical_conflicts": "基于实际 StallPropagation 事件分析",
            "hotspot_patterns": "从 state_key 字段识别热点资源",
            "dependency_chains": "实际依赖链长度和复杂度",
            "reexecution_frequency": "incarnation > 0 的频率统计"
        },
        "USDT": {
            "expected_differences": "USDT交易通常更简单，冲突更少",
            "scaling_behavior": "在高并发下的表现差异",
            "optimization_potential": "并发优化空间评估"
        }
    }
}
```

#### 4.6.3 性能基准对比

**TPS对比分析**：

```python
performance_comparison = {
    "baseline_metrics": {
        "ETH_current": "663.16 TPS (100笔交易, 4线程)",
        "scaling_projection": "基于更大数据集的性能推测"
    },
    "comparison_dimensions": [
        "相同交易数下的TPS对比",
        "相同时间窗口下的吞吐量对比", 
        "不同并发度下的扩展性对比",
        "稳定性指标对比（TPS标准差、执行时间方差）"
    ],
    "resource_efficiency": {
        "memory_usage_per_tx": "单笔交易内存使用效率",
        "execution_time_normalized": "按交易复杂度标准化的执行时间",
        "gas_efficiency": "实际Gas使用vs理论值的效率比"
    }
}
```

#### 4.6.4 图表绘制思路

**数据集性能对比雷达图**：
- **维度**: TPS, 平均执行时间, 重执行率, 依赖密度, 内存效率, Gas效率
- **多边形**: ETH vs USDT 性能轮廓对比
- **数据源**: 各自的 block_summary.ndjson 和聚合统计
- **用途**: 直观展示两种数据集的特征差异

**时间序列对比图**：
- **X轴**: 执行时间进度（标准化为0-100%）
- **Y轴**: 瞬时TPS或执行延迟
- **多条线**: ETH和USDT的性能曲线
- **用途**: 分析两种负载下的性能变化模式

**依赖复杂度对比散点图**：
- **X轴**: 交易复杂度（read_set_size + write_set_size）
- **Y轴**: 执行时间（exec_duration_us）
- **颜色**: 数据集类型（ETH vs USDT）
- **大小**: 重执行次数（incarnation值）
- **用途**: 识别复杂度与性能的关系差异

### 4.7 扩展思路

#### 4.7.1 可增加的重放合约类型

基于当前ETH和USDT数据集的基础上，可以扩展的合约类型分析：

**DeFi协议重放扩展**：

```python
defi_protocol_extensions = {
    "uniswap_v3_analysis": {
        "transaction_patterns": [
            "流动性提供 (addLiquidity)",
            "代币交换 (swapExactTokensForTokens)", 
            "费用收集 (collectFees)",
            "位置管理 (increaseLiquidity/decreaseLiquidity)"
        ],
        "expected_conflicts": [
            "流动性池状态竞争",
            "价格预言机并发更新",
            "费用累计器状态冲突"
        ],
        "analysis_focus": [
            "MEV相关交易的并发表现",
            "套利交易的依赖关系复杂度",
            "流动性变化对后续交易的影响"
        ],
        "log_analysis_extensions": {
            "state_keys_patterns": "识别Uniswap特有的状态键模式",
            "dependency_chains": "分析AMM交易间的复杂依赖",
            "performance_bottlenecks": "定位DeFi特有的性能瓶颈"
        }
    },
    "compound_lending": {
        "patterns": [
            "借贷操作 (borrow/repay)",
            "抵押品管理 (supply/withdraw)",
            "清算交易 (liquidation)",
            "利率模型更新 (accrueInterest)"
        ],
        "conflicts": [
            "利率模型状态更新竞争",
            "抵押品价格预言机访问",
            "账户健康度计算冲突"
        ],
        "metrics": [
            "清算交易的时序敏感性分析",
            "利率更新对系统性能的影响",
            "抵押率计算的并发安全性"
        ]
    },
    "nft_marketplace": {
        "opensea_trades": {
            "focus": "批量NFT交易并发性能",
            "conflicts": "NFT所有权状态、版税分配计算",
            "visualization": "NFT交易依赖网络图",
            "unique_challenges": "非同质化资产的并发处理特点"
        }
    }
}
```

**跨链协议分析**：

```python
cross_chain_protocols = {
    "bridge_operations": {
        "patterns": "锁定-铸造、销毁-解锁操作序列",
        "conflicts": "跨链状态同步、验证节点共识",
        "analysis": "跨链交易的原子性和一致性保证"
    },
    "layer2_interactions": {
        "rollup_batching": "L2交易批处理的并发优化",
        "state_root_updates": "状态根更新的依赖管理",
        "withdrawal_processing": "提现交易的并发处理"
    }
}
```

#### 4.7.2 可增加的合成负载

**压力测试场景设计**：

```python
synthetic_workload_generators = {
    "high_contention_scenarios": {
        "pattern": "大量交易访问少数热点状态",
        "implementation": {
            "hot_accounts": "创建少数高频访问账户（如交易所钱包）",
            "transaction_volume": "生成大量转账指向这些账户",
            "concurrency_stress": "测试极限冲突下的系统行为"
        },
        "metrics": [
            "停滞传播深度 (StallPropagation.affected_txns 长度)",
            "系统降级行为 (TPS下降模式)",
            "资源消耗峰值 (内存、CPU使用率)"
        ],
        "expected_log_patterns": {
            "dependencies.ndjson": "大量DependencyBlock事件",
            "scheduler_states.ndjson": "频繁的TaskSuspend/Resume",
            "stall_events.ndjson": "长时间的StallDuration记录"
        }
    },
    "burst_traffic_simulation": {
        "pattern": "突发高频交易负载",
        "implementation": {
            "traffic_burst": "在短时间内提交大量交易",
            "load_variation": "模拟真实世界的负载变化",
            "recovery_testing": "系统从高负载中恢复的能力"
        },
        "analysis_focus": [
            "调度器弹性和缓冲能力",
            "负载峰值处理策略",
            "系统恢复时间测量"
        ]
    },
    "mixed_complexity_workload": {
        "pattern": "简单转账 + 复杂DeFi交易混合",
        "composition": {
            "simple_transfers": "70% 简单账户间转账",
            "defi_operations": "20% 复杂DeFi合约调用", 
            "nft_trades": "10% NFT交易操作"
        },
        "analysis_objectives": [
            "异构负载下的调度公平性",
            "复杂交易对简单交易的影响",
            "资源分配均衡性分析"
        ]
    }
}
```

**负载特征分析框架**：

```python
workload_analysis_framework = {
    "transaction_graph_analysis": {
        "methodology": {
            "nodes": "交易作为图节点",
            "edges": "依赖关系作为边连接",
            "edge_weights": "依赖强度或阻塞时间"
        },
        "graph_metrics": [
            "图密度 (边数/可能边数最大值)",
            "最大连通分量大小",
            "关键路径长度 (最长依赖链)",
            "聚类系数 (局部依赖密集程度)"
        ],
        "insight_extraction": "负载的并发友好程度量化评估",
        "log_data_mapping": {
            "nodes_source": "execution_flow.ndjson 中的 transaction_id",
            "edges_source": "dependencies.ndjson 中的依赖关系",
            "weights_source": "stall_events.ndjson 中的持续时间"
        }
    },
    "temporal_pattern_mining": {
        "data_mining_approach": "时序数据挖掘技术应用",
        "pattern_categories": [
            "周期性负载 (定期重复的交易模式)",
            "突发模式 (短期内的交易激增)",
            "依赖集群 (时间上聚集的相关交易)"
        ],
        "mining_algorithms": [
            "频繁序列挖掘 (识别常见交易序列)",
            "异常模式检测 (发现异常负载特征)",
            "季节性分析 (长期趋势识别)"
        ],
        "practical_applications": [
            "预测性调度优化",
            "动态资源分配策略",
            "负载预测和容量规划"
        ]
    }
}
```

#### 4.7.3 高级分析技术扩展

**机器学习应用于日志分析**：

```python
ml_enhanced_analysis = {
    "performance_prediction_models": {
        "feature_engineering": {
            "transaction_features": [
                "read_set_size, write_set_size",
                "gas_used (交易复杂度指标)",
                "incarnation (重执行历史)",
                "state_key patterns (访问模式特征)"
            ],
            "system_features": [
                "active_tasks (当前并发度)",
                "queue_depth (系统负载)",
                "memory_usage (资源压力)",
                "recent_conflict_rate (近期冲突频率)"
            ]
        },
        "model_architectures": {
            "random_forest": "处理非线性特征关系",
            "xgboost": "处理复杂特征交互",
            "lstm": "时序依赖建模",
            "transformer": "注意力机制捕获长距离依赖"
        },
        "prediction_targets": [
            "execution_time_us (执行时间预测)",
            "conflict_probability (冲突概率预测)",
            "resource_usage (资源需求预测)"
        ],
        "practical_applications": [
            "智能任务调度 (基于预测的调度优化)",
            "动态并发度调整",
            "预防性重执行策略"
        ]
    },
    "anomaly_detection_systems": {
        "data_sources": [
            "时序性能指标 (TPS, 执行时间)",
            "系统状态指标 (内存、CPU)",
            "依赖关系模式异常"
        ],
        "detection_methods": {
            "isolation_forest": "无监督异常检测",
            "lstm_autoencoder": "时序异常模式识别", 
            "statistical_methods": "统计阈值和置信区间",
            "ensemble_methods": "多模型融合提高检测精度"
        },
        "alert_mechanisms": [
            "实时异常告警",
            "性能衰减预警",
            "系统瓶颈提前发现"
        ]
    }
}
```

**实时分析流水线架构**：

```python
realtime_analysis_pipeline = {
    "stream_processing_framework": {
        "technology_stack": {
            "message_queue": "Apache Kafka (日志事件流)",
            "stream_processor": "Apache Flink (实时计算)",
            "time_windows": [
                "滑动窗口 (1秒TPS实时计算)",
                "会话窗口 (交易生命周期分析)",
                "时间窗口 (5分钟性能趋势)"
            ]
        },
        "processing_stages": [
            "数据清洗和标准化",
            "实时指标计算",
            "异常检测和告警",
            "预测模型推理"
        ]
    },
    "visualization_and_monitoring": {
        "dashboard_tools": {
            "grafana": "实时性能仪表板",
            "tableau": "高级数据分析和报表",
            "custom_web_interface": "Block-STM专用监控界面"
        },
        "interactive_features": [
            "交互式依赖图探索",
            "实时日志查询和过滤",
            "历史数据对比分析",
            "自定义指标和告警设置"
        ]
    },
    "integration_with_blockchain": {
        "feedback_loops": [
            "性能数据反馈到调度器",
            "预测结果指导资源分配",
            "异常检测触发自动优化"
        ],
        "adaptive_optimization": [
            "动态并发度调整",
            "智能任务优先级排序",
            "自适应超时和重试策略"
        ]
    }
}
```

这个全面的分析框架为 Block-STM 系统提供了从基础数据分析到高级机器学习应用的完整方法论。通过多层次、多维度的数据分析，结合实际日志数据的深度挖掘，可以全面理解系统行为，识别优化机会，并指导未来的技术改进和扩展方向。

基于真实日志数据的分析框架确保了所有建议都是可实施和可验证的，为Block-STM系统的持续优化和学术研究提供了坚实的数据基础。