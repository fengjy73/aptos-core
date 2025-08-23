# Stall计数不一致问题分析

## 发现时间
2025-08-22 08:00

## 问题描述

运行Block-STM基准测试时发现：
- **程序输出**: "停滞次数:99"
- **日志记录**: test_logs_recovery/stall_events.ndjson中只有9条记录
- **数据不匹配**: 99 vs 9，相差90条记录

## 具体现象

### 程序输出显示
```
执行次数:133, 验证次数:366, 中止次数:33, 停滞次数:99, 平均停滞时间:29.50 us, 总停滞时间:2920.79 us
```

### 日志文件内容
stall_events.ndjson中只包含9条记录：
- 3条StallAdd事件 (txn_id: 23, 24, 25)
- 3条StallRemove事件 
- 3条StallDuration事件

## 可能原因分析

### 1. 统计方法不一致
**程序统计**: 可能统计的是所有`add_stall`调用的次数
**日志记录**: 只记录了部分stall事件，可能存在记录遗漏

### 2. 不同类型的Stall操作
Block-STM v2中存在多种stall触发路径：
- **直接stall**: `ExecutionStatuses::add_stall`
- **传播stall**: `AbortedDependencies::add_stall`
- **批量stall**: propagation过程中的递归stall

### 3. 日志插桩不完整
可能的遗漏点：
- 某些stall操作没有调用`log_stall_add`
- 传播过程中的stall没有被记录
- 测试代码中的`add_stall_legacy`调用

### 4. 线程竞争导致的记录丢失
- 多线程并发写入可能导致部分记录丢失
- 文件缓冲区刷新不及时
- 锁竞争导致的记录跳过

### 5. 统计计数器位置问题
`increment_stall_events()`可能被调用的位置：
- `log_stall_add`中每次调用都递增
- 但可能存在其他地方直接递增计数器而不记录日志

## 关键代码路径分析

### 统计计数器更新
```rust
// 在log_stall_add中
pub fn log_stall_add(...) {
    self.increment_stall_events();  // 递增计数器
    self.log_detailed_stall_event(...); // 记录日志
}
```

### 可能的遗漏路径
1. **scheduler_status.rs**: `add_stall_internal`直接操作状态
2. **scheduler_v2.rs**: `AbortedDependencies::add_stall`批量处理
3. **propagate**: 递归传播过程中的stall操作
4. **测试场景**: `add_stall_legacy`调用

## 验证假设

需要检查：
1. 所有`add_stall`调用点是否都记录日志
2. `increment_stall_events`是否在其他地方被调用
3. 多线程环境下的日志写入完整性
4. 测试数据的复杂度是否足够触发大量stall

## 调试策略

1. **添加调试日志**: 在所有stall相关函数中添加计数日志
2. **统计验证**: 分析日志文件中StallAdd事件的实际数量
3. **代码审计**: 检查所有可能触发stall的代码路径
4. **压力测试**: 使用更复杂的测试数据验证记录完整性