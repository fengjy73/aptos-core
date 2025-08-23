# Stall事件JSON字段清理修复

## 修复日期
2025-08-22

## 问题描述

在Block-STM v2的stall日志记录中发现以下问题：
1. **无意义null字段**: StallRemove事件中包含`"first_stall":null`，StallAdd事件中包含`"became_unstalled":null`
2. **字段语义混乱**: 不同类型的事件共享相同的JSON结构，导致某些字段在特定事件类型中无意义
3. **日志可读性差**: null值增加了日志文件大小，降低了可读性

## 具体表现

修复前的日志格式：
```json
// StallAdd事件 - became_unstalled字段无意义
{"event_type":"StallAdd","first_stall":true,"became_unstalled":null}

// StallRemove事件 - first_stall字段无意义  
{"event_type":"StallRemove","first_stall":null,"became_unstalled":true}
```

## 修复方案

### 核心思路
为不同类型的stall事件生成专门的JSON结构，只包含相关字段，避免null值。

### 实现细节

修改`block_stm_logger.rs`中的`log_detailed_stall_event`方法：

```rust
// 创建基础记录（共同字段）
let mut stall_event_record = serde_json::json!({
    "event_type": event_type,
    "timestamp_us": timestamp_us,
    "thread_id": Self::current_thread_id(),
    "txn_id": txn_id,
    "incarnation": incarnation,
    "by_tx": by_tx,
    "stall_count_before": if event_type == "StallAdd" { 
        stall_count_after.saturating_sub(1) 
    } else { 
        stall_count_after + 1 
    },
    "stall_count_after": stall_count_after,
});

// 添加事件特定字段，避免null值
if event_type == "StallAdd" {
    stall_event_record["first_stall"] = serde_json::Value::Bool(first_stall);
    stall_event_record["stall_transition"] = serde_json::Value::String(
        if first_stall { "UNSTALLED_TO_STALLED".to_string() } 
        else { "STALLED_TO_MORE_STALLED".to_string() }
    );
} else if event_type == "StallRemove" {
    stall_event_record["became_unstalled"] = serde_json::Value::Bool(became_unstalled);
    stall_event_record["stall_transition"] = serde_json::Value::String(
        if became_unstalled { "STALLED_TO_UNSTALLED".to_string() } 
        else { "MORE_STALLED_TO_STALLED".to_string() }
    );
}
```

## 修复效果

修复后的日志格式：
```json
// StallAdd事件 - 只包含相关字段
{"event_type":"StallAdd","first_stall":true,"stall_transition":"UNSTALLED_TO_STALLED"}

// StallRemove事件 - 只包含相关字段
{"event_type":"StallRemove","became_unstalled":true,"stall_transition":"STALLED_TO_UNSTALLED"}
```

## 优势

1. **语义清晰**: 每种事件类型只包含有意义的字段
2. **减少冗余**: 消除了null值，减少日志文件大小
3. **提高可读性**: 日志更加简洁，便于分析
4. **类型安全**: 避免了字段类型的歧义

## 兼容性

- **向后兼容**: 现有的日志分析工具需要适配新的字段结构
- **字段保留**: 所有核心字段（timestamp_us, thread_id, txn_id等）保持不变
- **新增字段**: stall_transition字段提供了更明确的状态转换信息

## 验证方法

1. 编译检查：`cargo check -p aptos-block-executor`
2. 运行测试观察日志格式
3. 确认无null值出现在stall事件中

## 相关文件

- `aptos-move/block-executor/src/block_stm_logger.rs:2044-2090`