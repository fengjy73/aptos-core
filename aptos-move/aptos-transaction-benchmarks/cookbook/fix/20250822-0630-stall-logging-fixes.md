# Block-STM v2 Stall Logging 修复方案

## 1. 问题分析

### 当前问题
1. **字段冗余**: `first_stall":null` 在StallRemove事件中无意义
2. **记录不完整**: 部分stall过程未被记录，特别是传播过程
3. **线程ID缺失**: 操作stall的线程信息丢失
4. **事件分散**: stall相关数据分布在不同文件中

### 根本原因
- 日志插桩不够精确，没有覆盖所有stall状态变化点
- 字段设计不合理，StallAdd和StallRemove使用了相同的结构
- 缺乏对传播过程的细粒度跟踪

## 2. 修复策略

### 2.1 字段清理
```rust
// StallAdd事件
{
    "event_type": "StallAdd",
    "timestamp_us": u64,
    "thread_id": u64,
    "txn_id": TxnIndex,
    "incarnation": Incarnation,
    "by_tx": TxnIndex,
    "stall_count_before": u32,
    "stall_count_after": u32,
    "first_stall": bool,  // 只在StallAdd中有效
    "stall_transition": String // UNSTALLED_TO_STALLED | STALLED_TO_MORE_STALLED
}

// StallRemove事件
{
    "event_type": "StallRemove",
    "timestamp_us": u64,
    "thread_id": u64,
    "txn_id": TxnIndex,
    "incarnation": Incarnation,
    "by_tx": TxnIndex,
    "stall_count_before": u32,
    "stall_count_after": u32,
    "became_unstalled": bool,  // 只在StallRemove中有效
    "stall_transition": String // STALLED_TO_UNSTALLED | MORE_STALLED_TO_STALLED
}
```

### 2.2 完整插桩点
1. **ExecutionStatuses::add_stall_internal**: 直接stall操作
2. **AbortedDependencies::add_stall**: 批量依赖stall
3. **AbortedDependencies::remove_stall**: 批量依赖unstall
4. **SchedulerV2::propagate**: 传播过程跟踪
5. **所有shortcut_executed_and_not_stalled检查点**

### 2.3 线程ID跟踪
确保每个stall事件都记录执行操作的线程ID：
- 在scheduler_status.rs中的add_stall_internal/remove_stall
- 在scheduler_v2.rs中的传播方法
- 在所有日志记录点使用`Self::current_thread_id()`

## 3. 具体修复方案

### 3.1 修改log_detailed_stall_event方法
```rust
pub fn log_detailed_stall_event(
    &self,
    event_type: &str,  // "StallAdd" | "StallRemove" | "StallPropagation"
    txn_id: TxnIndex,
    incarnation: Incarnation,
    by_tx: TxnIndex,
    stall_count_after: u32,
    additional_fields: Option<serde_json::Map<String, serde_json::Value>>,
) {
    let mut base_record = serde_json::json!({
        "event_type": event_type,
        "timestamp_us": Self::current_timestamp_us(),
        "thread_id": Self::current_thread_id(),
        "txn_id": txn_id,
        "incarnation": incarnation,
        "by_tx": by_tx,
        "stall_count_before": match event_type {
            "StallAdd" => stall_count_after.saturating_sub(1),
            "StallRemove" => stall_count_after + 1,
            _ => stall_count_after
        },
        "stall_count_after": stall_count_after,
    });
    
    // 添加事件特定字段
    if let Some(fields) = additional_fields {
        if let Some(obj) = base_record.as_object_mut() {
            obj.extend(fields);
        }
    }
}
```

### 3.2 增强传播跟踪
在`SchedulerV2::propagate`中添加详细日志：
```rust
// 传播开始
logger.log_stall_propagation_start(queue_size, affected_txns);

// 每个交易的处理决策
logger.log_stall_decision(task_idx, decision, reason);

// 传播结束
logger.log_stall_propagation_end(processed_count, final_queue_size);
```

### 3.3 修复stall计数统计
确保停滞次数统计包含所有incarnation的所有stall事件：
- 每次StallAdd事件都递增计数器
- 区分per-transaction和per-incarnation计数
- 在输出中明确说明统计范围

## 4. 验证方案

### 4.1 数据完整性检查
- 每个StallAdd必须有对应的StallRemove
- stall_count_before + 1 = stall_count_after (for StallAdd)
- stall_count_before - 1 = stall_count_after (for StallRemove)
- 同一(txn_id, incarnation)的最终stall_count必须为0

### 4.2 线程一致性验证
- 同一stall周期内的Add/Remove可能在不同线程
- 传播操作应该记录triggering thread
- 时间戳应该单调递增

### 4.3 性能影响评估
- 日志记录不应显著影响执行性能
- 文件I/O应该异步或批量处理
- 内存使用应该受控

## 5. 实现优先级

1. **高优先级**: 修复字段冗余，清理无意义字段
2. **高优先级**: 确保所有stall操作都被记录
3. **中优先级**: 增强传播过程跟踪
4. **低优先级**: 性能优化和批量处理