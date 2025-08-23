# Block-STM v2 Stall Mechanism 原理分析

## 1. Transaction Status Lifecycle (交易状态生命周期)

### 状态转换图

```
PendingScheduling(i)
    |
    | start_executing
    |
    ↓                       finish_execution
Executing(i) ------------------------------> Executed(i)
    |                                           |
    | start_abort(i) + finish_abort(i)            | start_abort(i) + finish_abort(i)
    |                                           |
    ↓                    finish_execution       ↓
Aborted(i) ------------------------------> PendingScheduling(i+1)
```

### 关键状态

- **PendingScheduling**: 等待调度执行
- **Executing**: 正在执行中
- **Executed**: 已完成执行
- **Aborted**: 被中止，需要重新执行

## 2. Stall Mechanism (停滞机制)

### 核心概念

- Stall机制类似于平衡括号：`add_stall` = '('，`remove_stall` = ')'
- 每个successful `add_stall`必须有对应的 `remove_stall`
- 当括号平衡时，交易变为"unstalled"

### 目的

1. 记录有依赖关系的交易，避免过早重新调度
2. 限制乐观并发，减少级联中止
3. 通过best-effort方式引导依赖处理

## 3. AbortedDependencies Structure

### 数据结构

```rust
struct AbortedDependencies {
    is_stalled: bool,                    // owner交易是否被stalled
    not_stalled_deps: BTreeSet<TxnIndex>, // 未传播stall的依赖
    stalled_deps: BTreeSet<TxnIndex>,     // 已传播stall的依赖
}
```

### 不变式

- `stalled_deps` 和 `not_stalled_deps` 必须不相交
- 依赖关系总是具有比owner更高的索引

## 4. Stall Propagation (停滞传播)

### add_stall流程

1. 遍历 `not_stalled_deps`中的所有交易
2. 对每个依赖调用 `statuses.add_stall(dep_idx, owner_txn)`
3. 如果返回true（unstalled → stalled），加入传播队列
4. 将所有 `not_stalled_deps`移到 `stalled_deps`
5. 设置 `is_stalled = true`

### remove_stall流程

1. 遍历 `stalled_deps`中的所有交易
2. 对每个依赖调用 `statuses.remove_stall(dep_idx, owner_txn)`
3. 如果返回true（stalled → unstalled），加入传播队列
4. 将所有 `stalled_deps`移到 `not_stalled_deps`
5. 设置 `is_stalled = false`

### propagate方法

1. 从队列中弹出transaction index
2. 检查状态：
   - 如果 `shortcut_executed_and_not_stalled`：调用 `remove_stall`
   - 否则：调用 `add_stall`
3. 递归处理直到队列为空

## 5. 关键执行点

### 主要stall触发点

1. **AbortManager.finish_execution**: 当交易完成时处理依赖中止
2. **依赖传播**: 通过 `propagate`方法递归传播状态变化
3. **状态检查**: 通过 `shortcut_executed_and_not_stalled`决定传播方向

### 线程安全

- 使用 `ArmedLock`保护每个交易的状态
- 依赖锁按索引升序获取，避免死锁
- 原子操作管理stall计数

## 6. 日志记录需求

### 必须记录的事件

1. **StallAdd**: 每次add_stall调用
2. **StallRemove**: 每次remove_stall调用
3. **StallPropagation**: 传播过程开始/结束
4. **DependencyStall**: 个体依赖的stall状态变化
5. **StallDuration**: 完整的stall周期时长

### 关键字段

- `txn_id`, `incarnation`: 被stall的交易
- `by_tx`: 引起stall的上游交易
- `thread_id`: 执行stall操作的线程
- `stall_count_before/after`: stall计数变化
- `first_stall/became_unstalled`: 状态转换标识
