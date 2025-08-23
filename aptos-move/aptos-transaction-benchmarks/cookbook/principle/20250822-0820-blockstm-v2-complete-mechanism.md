# Block-STM v2 完整机制分析

## 创建时间
2025-08-22 08:20

## 1. Block-STM v1 vs v2 架构对比

### v1 (scheduler.rs)
- **传统调度器**: 基于ArmedLock机制的经典Block-STM实现
- **任务类型**: 主要是ExecutionTaskType枚举
- **依赖管理**: 通过DependencyStatus进行简单的依赖跟踪
- **调度逻辑**: Wave-based调度，相对简单的任务分发

### v2 (scheduler_v2.rs + scheduler_status.rs)  
- **下一代调度器**: 更精细的任务管理和状态跟踪
- **任务类型**: TaskKind枚举，包含Execute/PostCommitProcessing/NextTask
- **依赖管理**: 复杂的AbortedDependencies传播机制
- **调度逻辑**: 基于ExecutionStatuses的细粒度状态管理

## 2. Block-STM v2 核心架构

### 2.1 执行器组件
```
executor.rs (主执行引擎)
    ↓ 使用
SchedulerV2 (scheduler_v2.rs) 
    ↓ 管理
ExecutionStatuses (scheduler_status.rs)
    ↓ 跟踪
Transaction Status Lifecycle
```

### 2.2 关键数据结构

#### SharedSyncParams
```rust
struct SharedSyncParams<'a, 'b, T, E, S> {
    base_view: &'a S,                    // 基础状态视图
    scheduler: &'a SchedulerV2,          // v2调度器实例
    versioned_cache: &'a MVHashMap,      // 多版本哈希表
    global_module_cache: &'a GlobalModuleCache, // 全局模块缓存
    last_input_output: &'a TxnLastInputOutput,   // 交易I/O缓存
    // ... 其他共享参数
}
```

#### TaskKind (v2任务类型)
```rust
pub enum TaskKind {
    Execute(TxnIndex, Incarnation),      // 执行交易
    PostCommitProcessing(TxnIndex),     // 后提交处理
    NextTask,                           // 无可用任务，需要等待
}
```

## 3. 执行流程详解

### 3.1 主执行循环 (executor.rs:1750-1900)
```
1. 尝试获取commit_hooks锁
2. 处理ready-to-commit交易的顺序提交
3. 调用scheduler.next_task()获取下一个任务
4. 根据TaskKind分发处理：
   - Execute: 执行具体交易
   - PostCommitProcessing: 处理已提交交易的后处理
   - NextTask: 处理无任务可用的情况
```

### 3.2 任务分发机制

#### Execute任务
- **触发条件**: 交易状态为PendingScheduling且无stall
- **执行过程**: 调用execute_v2进行并行执行
- **状态变更**: PendingScheduling → Executing → (Executed | Aborted)

#### PostCommitProcessing任务
- **触发条件**: 交易已提交但需要后处理
- **执行过程**: 调用materialize_txn_commit
- **目的**: 清理资源、更新缓存、执行commit hooks

#### NextTask任务
- **触发条件**: 当前无可用任务，所有交易都在等待依赖
- **含义**: 系统级调度等待状态
- **处理**: 记录系统级stall事件，等待依赖解除

## 4. Stall机制深度分析

### 4.1 Transaction-Level Stall
**定义**: 特定交易由于依赖关系被暂停执行

**触发点**:
- `ExecutionStatuses::add_stall(txn_idx, by_tx)`: 直接交易stall
- `AbortedDependencies::add_stall()`: 批量依赖传播stall
- `SchedulerV2::propagate()`: 递归依赖传播stall

**特征**:
- 有明确的txn_id和incarnation
- 有明确的引起stall的上游交易(by_tx)
- 在MVHashMap中有对应的读写依赖记录
- 通过remove_stall可以精确恢复

**日志记录**: StallAdd/StallRemove事件

### 4.2 System-Level Stall  
**定义**: 整个调度系统无可用任务的等待状态

**触发点**:
- `TaskKind::NextTask`处理中的`increment_stall_events()`
- 所有交易都在等待依赖，调度器暂时无事可做

**特征**:
- 没有特定的txn_id (使用0作为占位符)
- 没有具体的by_tx关系
- 表示系统整体的调度瓶颈
- 不对应特定的MVHashMap操作

**日志记录**: StallPropagation事件(不是StallAdd)

## 5. 两种Stall的本质区别

### 5.1 语义层面
- **Transaction Stall**: 具体交易间的依赖等待
- **System Stall**: 调度器层面的全局等待

### 5.2 技术实现
- **Transaction Stall**: 在ExecutionStatuses中维护stall计数
- **System Stall**: 在executor主循环中检测到的调度空窗期

### 5.3 性能影响
- **Transaction Stall**: 反映并发冲突的程度，影响吞吐量
- **System Stall**: 反映调度效率，影响CPU利用率

### 5.4 恢复机制
- **Transaction Stall**: 通过上游交易完成自动恢复
- **System Stall**: 通过新任务变为可调度自动恢复

## 6. 计数统计的合理性

### 6.1 当前实现问题
目前两种stall都被计入`stall_events_count`，但：
- Transaction stall有详细的日志记录
- System stall只有传播日志，缺少对应的StallAdd事件

### 6.2 统计意义
两种stall都是系统性能的重要指标：
- **Transaction stall**: 衡量依赖冲突频率
- **System stall**: 衡量调度饥饿程度

### 6.3 应保持的统计方式
应该统计所有类型的stall，但需要：
1. 为System stall创建对应的日志事件
2. 或者分别统计两种stall类型
3. 确保日志记录与计数器一致

## 7. Transaction-Level vs System-Level Stall详细对比

### 7.1 Transaction-Level Stall详细机制

#### 触发条件
基于`executor.rs:1849`的代码分析，Transaction-Level Stall发生在以下情况：

1. **MVHashMap读取失败**: 交易读取依赖时发现需要等待上游交易完成
2. **依赖传播**: `AbortedDependencies::add_stall()`批量传播stall到下游交易
3. **状态冲突**: 交易状态从`PendingScheduling`无法转换到`Executing`

#### 实现路径
```rust
// scheduler_status.rs中的核心实现
ExecutionStatuses::add_stall(txn_idx, by_tx) {
    // 1. 检查当前状态
    // 2. 更新stall计数
    // 3. 标记为stalled状态
    // 4. 记录日志事件
}
```

#### 日志生成
- **StallAdd事件**: 包含具体的txn_id、incarnation、by_tx
- **依赖关系**: 明确的上下游依赖追踪
- **状态转换**: 精确的状态变更记录

### 7.2 System-Level Stall详细机制

#### 触发条件
基于`executor.rs:1841-1868`的代码分析：

```rust
TaskKind::NextTask => {
    // 系统级stall：所有交易都在等待，调度器无任务可分配
    logger.increment_stall_events();  // 直接递增计数器
    logger.log_stall_propagation(...); // 只记录传播事件，不是StallAdd
}
```

#### 特征差异
1. **无具体交易**: 使用txn_id=0作为占位符
2. **无依赖关系**: 没有明确的by_tx关系
3. **系统状态**: 表示整个调度器的等待状态
4. **日志类型**: 生成StallPropagation而非StallAdd事件

### 7.3 计数不一致的根本原因

#### 问题定位
通过代码分析发现了**两个独立的计数路径**：

1. **Transaction Stall路径** (scheduler_status.rs):
   ```rust
   add_stall() -> log_stall_add() -> increment_stall_events() -> log_detailed_stall_event()
   ```
   - 生成StallAdd事件
   - 记录到stall_events.ndjson

2. **System Stall路径** (executor.rs:1849):
   ```rust
   TaskKind::NextTask -> increment_stall_events() -> log_stall_propagation()
   ```
   - 只递增计数器
   - 不生成StallAdd事件

#### 数据验证
从test_logs_recovery/stall_events.ndjson分析：
- **实际StallAdd事件**: 3个 (txn 23, 24, 25)
- **程序报告stall**: 99个
- **差值**: 96个，这些是System-Level stall

### 7.4 两种Stall的价值分析

#### Transaction-Level Stall的价值
- **并发冲突分析**: 准确反映交易间的读写依赖冲突
- **性能优化指导**: 识别热点数据和冲突模式
- **调试支持**: 精确的依赖关系追踪

#### System-Level Stall的价值
- **调度效率监控**: 反映调度器空闲等待的频率
- **负载均衡分析**: 识别工作线程饥饿状态
- **系统瓶颈定位**: 发现整体系统调度问题

### 7.5 合理的解决方案

#### 当前问题
- **统计混乱**: 两种不同类型的stall被混合计数
- **日志不完整**: System stall有计数但无详细日志
- **分析困难**: 无法区分不同类型的性能瓶颈

#### 推荐修复策略
基于分析，应该**保持两种stall的独立性**：

1. **为System Stall创建专门的日志事件**:
   ```rust
   TaskKind::NextTask => {
       logger.increment_system_stall_events();
       logger.log_system_stall_event(
           worker_id,
           scheduler_state,
           "NextTask_no_available_work"
       );
   }
   ```

2. **分离计数统计**:
   ```rust
   pub struct BlockExecutionStats {
       pub transaction_stall_count: AtomicU32,
       pub system_stall_count: AtomicU32,
   }
   ```

3. **完善日志输出**:
   ```rust
   println!("交易停滞次数:{}, 系统停滞次数:{}, 总停滞次数:{}", 
       transaction_stalls, system_stalls, transaction_stalls + system_stalls);
   ```

## 8. Block-STM v2完整执行流程

### 8.1 主循环结构 (executor.rs:1750-1876)
```
1. 尝试获取commit_hooks锁
   ├─ 有锁: 处理ready-to-commit队列
   └─ 无锁: 继续下一步

2. 调用scheduler.next_task()获取任务
   ├─ Execute(txn_idx, incarnation): 执行交易
   ├─ PostCommitProcessing(txn_idx): 后处理
   ├─ NextTask: 系统等待 (System-Level Stall)
   └─ Done: 结束

3. 任务处理完成后循环
```

### 8.2 Transaction-Level Stall触发时机
- **执行阶段**: MVHashMap读取等待
- **验证阶段**: 依赖检查失败
- **传播阶段**: AbortedDependencies批量传播

### 8.3 System-Level Stall触发时机
- **无可用任务**: 所有交易都在等待依赖
- **调度器空闲**: 工作线程无事可做
- **系统级等待**: 整体调度瓶颈

## 9. 结论与建议

### 9.1 机制理解
Block-STM v2中的两种stall机制serve不同的监控目的：
- **Transaction-Level**: 微观的交易依赖冲突分析
- **System-Level**: 宏观的调度器性能监控

### 9.2 修复建议
1. **保持统计分离**: 不要简单地移除System stall计数
2. **完善日志记录**: 为System stall创建对应的日志事件  
3. **改进报告格式**: 区分两种类型的stall统计
4. **增强分析能力**: 利用两种数据进行综合性能分析

### 9.3 长期价值
正确区分和统计两种stall类型，能够：
- 提供更精确的性能分析数据
- 支持更有针对性的优化策略
- 增强Block-STM v2的可观测性
- 为未来的调度器优化提供数据基础