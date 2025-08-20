# Block-STM 源码注释中英文对照翻译

本文档整理了Block-STM源码中的多行注释，并提供中英文对照翻译，按功能模块分类组织。

## 1. 交易状态生命周期管理 (Transaction Status Lifecycle)

### 1.1 状态转换机制

**English:**
```
============================== Transaction Status Lifecycle ==============================

Each transaction status contains an incarnation number (starting with 0) and progresses
through a well-defined lifecycle:

1. Initial State:
   - A transaction begins in the `PendingScheduling` status, meaning it's ready to be picked up
     by the BlockSTMv2 scheduler.
   - When the scheduler selects a transaction, it transitions the status to `Executing` via
     the [ExecutionStatuses::start_executing] method.

2. Abort Process:
   - A transaction incarnation may be aborted if it reads data that is later modified in a way
     that would cause the transaction to read different values if it executed again. This
     signals the need for re-execution with an incremented incarnation number.
   - In BlockSTMv2, a transaction can be aborted while executing or after execution finishes.
   - Abort happens in two distinct phases:

   a) Start Abort Phase:
      - [ExecutionStatuses::start_abort] is called with an incarnation number and succeeds if
        the incarnation has started executing and has not already been aborted.
      - This serves as an efficient test-and-set filter for multiple abort attempts (which
        can occur when a transaction makes multiple reads that may each be invalidated by
        different transactions).
      - Early detection allows the ongoing execution to stop immediately rather than continue
        work that will ultimately be discarded.

   b) Finish Abort Phase:
      - A successful [ExecutionStatuses::start_abort] must be followed by a
        [ExecutionStatuses::finish_abort] call on the status.
        • If the status was 'Executed', it transitions to 'PendingScheduling' for the
          next incarnation.
        • If the status was 'Executing', it transitions to 'Aborted'.
      - When transaction T1 successfully aborts transaction T2 (where T2 > T1):
        • T2 stops executing as soon as possible,
        • Subsequent scheduling of T2 may wait until T1 finishes, since T1 has higher
          priority (lower index),
        • After T1 completes, the worker can process all related aborts in batch. e.g. calling
          [ExecutionStatuses::finish_abort], tracking dependencies, and propagating stalls.

3. Execution Completion:
   - When execution finishes, [ExecutionStatuses::finish_execution] is called on the status.
   - If the status was `Aborted`, it transitions to `PendingScheduling` for the next incarnation.
   - If the status was `Executing`, it transitions to `Executed`.
```

**中文翻译:**
```
============================== 交易状态生命周期 ==============================

每个交易状态包含一个化身编号（从0开始），并按照明确定义的生命周期进行：

1. 初始状态：
   - 交易以`PendingScheduling`状态开始，意味着它准备好被BlockSTMv2调度器选取。
   - 当调度器选择一个交易时，它通过[ExecutionStatuses::start_executing]方法将状态转换为`Executing`。

2. 中止过程：
   - 如果交易读取的数据后来被修改，导致交易重新执行时会读取不同的值，则交易化身可能被中止。
     这表明需要使用递增的化身编号重新执行。
   - 在BlockSTMv2中，交易可以在执行期间或执行完成后被中止。
   - 中止分为两个不同的阶段：

   a) 开始中止阶段：
      - 使用化身编号调用[ExecutionStatuses::start_abort]，如果化身已开始执行且尚未被中止则成功。
      - 这作为多个中止尝试的高效测试和设置过滤器（当交易进行多次读取时可能发生，
        每次读取都可能被不同的交易无效化）。
      - 早期检测允许正在进行的执行立即停止，而不是继续最终会被丢弃的工作。

   b) 完成中止阶段：
      - 成功的[ExecutionStatuses::start_abort]必须跟随对状态的[ExecutionStatuses::finish_abort]调用。
        • 如果状态为'Executed'，则转换为下一个化身的'PendingScheduling'。
        • 如果状态为'Executing'，则转换为'Aborted'。
      - 当交易T1成功中止交易T2（其中T2 > T1）时：
        • T2尽快停止执行，
        • T2的后续调度可能等待T1完成，因为T1具有更高优先级（更低索引），
        • T1完成后，工作线程可以批量处理所有相关中止。例如调用
          [ExecutionStatuses::finish_abort]、跟踪依赖关系和传播停滞。

3. 执行完成：
   - 执行完成时，对状态调用[ExecutionStatuses::finish_execution]。
   - 如果状态为`Aborted`，则转换为下一个化身的`PendingScheduling`。
   - 如果状态为`Executing`，则转换为`Executed`。
```

### 1.2 状态转换图

**English:**
```
Status Transition Diagram:

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

Note: [ExecutionStatuses::start_abort] doesn't change the status directly but marks the
transaction for abort. The actual status change occurs during
[ExecutionStatuses::finish_abort]. Both steps are required to complete the abort process.
```

**中文翻译:**
```
状态转换图：

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

注意：[ExecutionStatuses::start_abort]不直接改变状态，而是标记交易为中止。
实际的状态改变发生在[ExecutionStatuses::finish_abort]期间。
完成中止过程需要两个步骤。
```

## 2. 交易停滞机制 (Transaction Stall Mechanism)

**English:**
```
============================== Transaction Stall Mechanism ==============================

In the BlockSTMv2 scheduler, a transaction status can be "stalled," meaning there have been
more [ExecutionStatuses::add_stall] than [ExecutionStatuses::remove_stall] calls on its status.
Each successful [ExecutionStatuses::add_stall] call requires a guarantee that the
corresponding[ExecutionStatuses::remove_stall] will eventually be performed.

The stall mechanism can be conceptualized as balanced parentheses - `add_stall` represents
an opening bracket '(' and `remove_stall` represents a closing bracket ')'. A status becomes
"unstalled" when the brackets are balanced (equal number of calls).

Key aspects of the stall mechanism:

1. Purpose:
   - Records that a transaction has dependencies that are more likely to cause re-execution
   - Can be used to:
     a) Avoid scheduling transactions for re-execution until stalls are removed
     b) Guide handling when another transaction observes a dependency during execution
   - Helps constrain optimistic concurrency by limiting cascading aborts

2. Behavior:
   - Best-effort approach that allows flexibility in concurrency scenarios, but such that
     high-priority transactions may still be re-executed even in stalled state
```

**中文翻译:**
```
============================== 交易停滞机制 ==============================

在BlockSTMv2调度器中，交易状态可以被"停滞"，意味着对其状态的
[ExecutionStatuses::add_stall]调用多于[ExecutionStatuses::remove_stall]调用。
每个成功的[ExecutionStatuses::add_stall]调用都需要保证最终会执行
相应的[ExecutionStatuses::remove_stall]。

停滞机制可以概念化为平衡括号 - `add_stall`代表开括号'('，
`remove_stall`代表闭括号')'。当括号平衡时（调用次数相等），状态变为"非停滞"。

停滞机制的关键方面：

1. 目的：
   - 记录交易具有更可能导致重新执行的依赖关系
   - 可用于：
     a) 避免调度交易重新执行，直到停滞被移除
     b) 指导当另一个交易在执行期间观察到依赖关系时的处理
   - 通过限制级联中止来帮助约束乐观并发

2. 行为：
   - 尽力而为的方法，在并发场景中允许灵活性，但高优先级交易
     即使在停滞状态下仍可能被重新执行
```

## 3. SchedulerV2 核心设计 (SchedulerV2 Core Design)

**English:**
```
SchedulerV2 as the core of BlockSTMv2, orchestrating parallel transaction execution. Its key responsibilities include:

- Task management (execution/post-commit)
- Transaction lifecycle coordination with `ExecutionStatuses`
- Concurrency control via `AbortManager` (for invalidations) and `AbortedDependencies` (for stall propagation)
- Commit sequencing using `next_to_commit_idx` and `CommitMarkerFlag`s
- Execution flow control with `executed_once_max_idx` and `min_not_scheduled_idx`

It interacts with `ExecutionStatuses`, `ExecutionQueueManager`, `AbortManager`, and worker threads.
The conceptual execution model where workers prioritize post-commit tasks then execution tasks.
```

**中文翻译:**
```
SchedulerV2作为BlockSTMv2的核心，协调并行交易执行。其主要职责包括：

- 任务管理（执行/提交后）
- 与`ExecutionStatuses`协调交易生命周期
- 通过`AbortManager`（用于无效化）和`AbortedDependencies`（用于停滞传播）进行并发控制
- 使用`next_to_commit_idx`和`CommitMarkerFlag`进行提交排序
- 使用`executed_once_max_idx`和`min_not_scheduled_idx`进行执行流控制

它与`ExecutionStatuses`、`ExecutionQueueManager`、`AbortManager`和工作线程交互。
概念执行模型中，工作线程优先处理提交后任务，然后处理执行任务。
```

## 4. 数据读取和验证 (Data Reading and Validation)

### 4.1 CapturedReads 读集合

**English:**
```
CapturedReads as a "read-set" for transaction execution, managing `data_reads`, `group_reads`, and `module_reads`. 
It includes `validate_data_reads` for checking consistency against `VersionedData` and handles speculative failures. 
It also defines `DataRead` enum variants for different types of reads (e.g., `Versioned`, `Resolved`, `Metadata`, `Exists`).
```

**中文翻译:**
```
CapturedReads作为交易执行的"读集合"，管理`data_reads`、`group_reads`和`module_reads`。
它包括用于检查与`VersionedData`一致性的`validate_data_reads`并处理推测失败。
它还定义了用于不同类型读取的`DataRead`枚举变体（例如`Versioned`、`Resolved`、`Metadata`、`Exists`）。
```

### 4.2 DataRead 枚举类型

**English:**
```
/// The enum captures the state that the transaction execution extracted from
/// a read callback to block executor, in order to be validated by Block-STM.
/// The captured state is fine-grained, e.g. it distinguishes between reading
/// a full value, and other kinds of reads that may access only the metadata
/// information, or check whether data exists at a given key.
```

**中文翻译:**
```
/// 该枚举捕获交易执行从读取回调中提取的状态，以便由Block-STM验证。
/// 捕获的状态是细粒度的，例如它区分读取完整值和其他类型的读取，
/// 后者可能只访问元数据信息，或检查给定键是否存在数据。
```

## 5. Block-STM 执行日志模块 (Block-STM Execution Logging Module)

**English:**
```
Block-STM execution logging module's comprehensive features, including:
- Transaction execution status tracking
- Concurrency control monitoring  
- Read/write set changes
- Performance metric collection
- Dependency analysis
- Stall/abort event recording
```

**中文翻译:**
```
Block-STM执行日志模块的全面功能，包括：
- 交易执行状态跟踪
- 并发控制监控
- 读写集变化记录
- 性能指标收集
- 依赖分析
- 停滞/中止事件记录
```

## 6. 方法调用并发性说明 (Method Call Concurrency Notes)

**English:**
```
============================== Notes on Method Call Concurrency ==============================

In general, most methods in this module can be called concurrently with the following exceptions:

1. Each successful [ExecutionStatuses::add_stall] call must be balanced by a
   corresponding [ExecutionStatuses::remove_stall] call that starts after the add_stall
   call completes. Multiple concurrent add_stall and remove_stall calls on the same
   transaction status are supported as long as this balancing property is maintained.

2. While multiple [ExecutionStatuses::start_executing] calls may be attempted
   concurrently, at most one can succeed for a given incarnation. A successful call
   must be followed by exactly one corresponding [ExecutionStatuses::finish_execution]
   call, which can execute concurrently with [ExecutionStatuses::start_abort] calls.
   Only one of these calls can succeed, leading to a single [ExecutionStatuses::finish_abort]
   call being performed for a given incarnation. There may be multiple concurrent
   calls for outdated incarnations
```

**中文翻译:**
```
============================== 方法调用并发性说明 ==============================

一般来说，此模块中的大多数方法可以并发调用，但有以下例外：

1. 每个成功的[ExecutionStatuses::add_stall]调用必须由在add_stall调用完成后开始的
   相应[ExecutionStatuses::remove_stall]调用来平衡。支持对同一交易状态的多个并发
   add_stall和remove_stall调用，只要维持这种平衡属性。

2. 虽然可能并发尝试多个[ExecutionStatuses::start_executing]调用，
   但对于给定化身最多只能有一个成功。成功的调用必须跟随恰好一个相应的
   [ExecutionStatuses::finish_execution]调用，该调用可以与
   [ExecutionStatuses::start_abort]调用并发执行。
   这些调用中只有一个能成功，导致对给定化身执行单个
   [ExecutionStatuses::finish_abort]调用。对于过时的化身可能有多个并发调用
```

## 7. 错误处理 (Error Handling)

### 7.1 并行块执行错误

**English:**
```
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParallelBlockExecutionError {
    /// unrecoverable VM error
    FatalVMError,
    // Incarnation number that is higher than a threshold is observed during parallel execution.
    // This might be indicative of some sort of livelock, or at least some sort of inefficiency
    // that would warrants investigating the root cause. Execution can fallback to sequential.
    IncarnationTooHigh,
}
```

**中文翻译:**
```
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParallelBlockExecutionError {
    /// 不可恢复的VM错误
    FatalVMError,
    // 在并行执行期间观察到高于阈值的化身编号。
    // 这可能表明某种活锁，或至少某种低效率，
    // 值得调查根本原因。执行可以回退到顺序执行。
    IncarnationTooHigh,
}
```

### 7.2 资源组序列化错误

**English:**
```
// This is separate error because we need to match the error variant to provide a specialized
// fallback logic if a resource group serialization error occurs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResourceGroupSerializationError;
```

**中文翻译:**
```
// 这是一个单独的错误，因为我们需要匹配错误变体，
// 以在发生资源组序列化错误时提供专门的回退逻辑。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResourceGroupSerializationError;
```

## 8. 视图管理 (View Management)

### 8.1 读取结果类型

**English:**
```
#[derive(Debug)]
pub(crate) enum ReadResult {
    Value(Option<StateValue>, Option<Arc<MoveTypeLayout>>),
    Metadata(Option<StateValueMetadata>),
    ResourceSize(Option<u64>),
    Exists(bool),
    Uninitialized,
    HaltSpeculativeExecution(String),
}
```

**中文翻译:**
```
#[derive(Debug)]
pub(crate) enum ReadResult {
    Value(Option<StateValue>, Option<Arc<MoveTypeLayout>>),  // 值（可选状态值，可选移动类型布局）
    Metadata(Option<StateValueMetadata>),                    // 元数据（可选状态值元数据）
    ResourceSize(Option<u64>),                               // 资源大小（可选u64）
    Exists(bool),                                            // 存在性（布尔值）
    Uninitialized,                                           // 未初始化
    HaltSpeculativeExecution(String),                        // 停止推测执行（字符串）
}
```

### 8.2 组读取结果

**English:**
```
#[derive(Debug, Eq, PartialEq)]
pub enum GroupReadResult {
    Value(Option<Bytes>, Option<Arc<MoveTypeLayout>>),
    ResourceSize(Option<u64>),
    Exists(bool),
    Uninitialized,
}
```

**中文翻译:**
```
#[derive(Debug, Eq, PartialEq)]
pub enum GroupReadResult {
    Value(Option<Bytes>, Option<Arc<MoveTypeLayout>>),  // 值（可选字节，可选移动类型布局）
    ResourceSize(Option<u64>),                          // 资源大小（可选u64）
    Exists(bool),                                       // 存在性（布尔值）
    Uninitialized,                                      // 未初始化
}
```

## 9. 值交换机制 (Value Exchange Mechanism)

### 9.1 TemporaryValueToIdentifierMapping

**English:**
```
For aggregators V2, values are replaced with identifiers at deserialization time, and are replaced back when the value is serialized
The "lifted" values are cached by the `LatestView` in the aggregators multi-version data structure
Manages delayed field IDs that were touched during value/identifier replacement operations
```

**中文翻译:**
```
对于聚合器V2，值在反序列化时被替换为标识符，在序列化时再替换回来
"提升"的值由`LatestView`在聚合器多版本数据结构中缓存
管理在值/标识符替换操作期间触及的延迟字段ID
```

## 10. 显式同步包装器 (Explicit Sync Wrapper)

### 10.1 ExplicitSyncWrapper 设计理念

**English:**
```
Parallel algorithms often guarantee a sequential use of certain data structures, or parts of the data-structures (like elements of a vector)
The rust compiler can not prove the safety of even slightly complex parallel algorithms
ExplicitSyncWrapper is meant to be used in parallel algorithms where we can prove that there will be no concurrent access to the underlying object (or its elements)
Use with caution - only when the safety can be proven
```

**中文翻译:**
```
并行算法通常保证某些数据结构或数据结构部分（如向量元素）的顺序使用
Rust编译器无法证明即使是稍微复杂的并行算法的安全性
ExplicitSyncWrapper用于我们可以证明不会对底层对象（或其元素）进行并发访问的并行算法
谨慎使用 - 仅在可以证明安全性时使用
```

## 11. 热状态操作累加器 (Hot State Operation Accumulator)

### 11.1 BlockHotStateOpAccumulator 功能

**English:**
```
Keys read but never written to across the entire block are to be made hot (or refreshed `hot_since_version` one is already hot but last refresh is far in the history) as the side effect of the block epilogue
Keep track of all the keys that are written to across the whole block, these keys are made hot (or have a refreshed `hot_since_version`) immediately at the version they got changed
To prevent the block epilogue from being too heavy, there's a limit on promotions per block
Every now and then refresh `hot_since_version` for hot items to prevent them from being evicted
```

**中文翻译:**
```
在整个区块中被读取但从未写入的键将在区块尾声中被设为热状态（或刷新`hot_since_version`，如果已经是热状态但上次刷新时间久远）
跟踪整个区块中所有被写入的键，这些键在被更改的版本立即变为热状态（或刷新`hot_since_version`）
为防止区块尾声过于繁重，对每个区块的提升次数有限制
定期刷新热项目的`hot_since_version`以防止它们被驱逐
```

## 12. 调度器核心设计 (Scheduler Core Design)

### 12.1 Block-STM v1 调度器实现 (Block-STM v1 Scheduler Implementation)

#### 执行状态管理 (Execution Status Management)

**English:**
```
All possible execution statuses of a transaction. In the explanation below,
we abbreviate "transaction" as "txn" and "incarnation" as "inc".
```

**中文翻译:**
```
Block-STM v1调度器通过ExecutionStatus枚举管理交易的生命周期状态：
- Ready(incarnation, ExecutionTaskType): 交易准备就绪，等待执行
- Executing(incarnation, ExecutionTaskType): 交易正在执行中  
- Executed(incarnation): 交易执行完成，等待验证
- Aborting(incarnation): 交易正在中止过程中
- Suspended(incarnation, DependencyCondvar): 交易因依赖而暂停
- Committed(incarnation): 交易已提交
- ExecutionHalted: 执行已停止
```

#### 验证状态管理 (Validation Status Management)

**English:**
```
ValidationStatus manages validation-related wave information:
- max_triggered_wave: Maximum triggered validation wave
- maybe_max_validated_wave: Possible maximum validated wave  
- required_wave: Minimum wave the transaction needs to pass
```

**中文翻译:**
```
ValidationStatus结构体管理验证相关的波次信息：
- max_triggered_wave: 最大触发的验证波次
- maybe_max_validated_wave: 可能的最大已验证波次
- required_wave: 交易需要通过的最小波次

验证波次机制确保交易按正确顺序进行验证，当验证索引减少时会影响所有后续交易。
```

#### 调度器核心结构 (Scheduler Core Structure)

**English:**
```
The index is reduced as necessary when transactions require validation, in particular,
after aborts and executions that write outside of the write set of the same transaction's
previous incarnation. This also creates a new wave of validations, identified by the
monotonically increasing index stored in the first 32 bits.
```

**中文翻译:**
```
Scheduler结构体包含以下关键组件：
- num_txns: 交易总数
- txn_dependency: 交易依赖关系向量
- txn_status: 交易状态（执行状态和验证状态）
- commit_state: 提交状态管理
- execution_idx: 执行索引，指示下一个要执行的交易
- validation_idx: 验证索引，包含交易索引和波次信息
- done_marker: 完成标记，指示所有交易是否可以提交
- has_halted: 停止标记，指示执行是否已停止
- commit_queue: 提交队列
```

#### 依赖管理机制 (Dependency Management Mechanism)

**English:**
```
When a txn depends on another txn, adds it to the dependency list of the other txn.
Returns true if successful, or false, if the dependency got resolved in the meantime.
If true is returned, Scheduler guarantees that later (dep_txn_idx will finish execution)
transaction txn_idx will be resumed, and corresponding execution task created.
```

**中文翻译:**
```
当交易依赖于其他交易时，调度器会：
1. 创建条件变量关联依赖关系
2. 检查依赖交易是否已执行
3. 如果未执行，将当前交易添加到依赖列表并暂停
4. 当依赖交易完成执行时，唤醒所有依赖的交易
```

#### 提交协调机制 (Commit Coordination Mechanism)

**English:**
```
If successful, returns Some(TxnIndex), the index of committed transaction.
```

**中文翻译:**
```
try_commit函数实现交易的提交协调：
1. 获取提交状态锁
2. 检查交易是否已执行
3. 验证波次是否满足要求
4. 更新交易状态为已提交
5. 递增提交索引
```

#### 早期停止机制 (Early Halt Mechanism)

**English:**
```
This function can halt the BlockSTM early, even if there are unfinished tasks.
It will set the done_marker to be true, and resolve all pending dependencies.

Currently, the reasons for halting the scheduler are as follows:
1. There is a module publishing txn that has read/write intersection with any txns
   even during speculative execution.
2. There is a resource group serialization error.
3. There is a txn with VM execution status Abort.
4. There is a txn with VM execution status SkipRest.
5. The committed txns have exceeded the PER_BLOCK_GAS_LIMIT.
6. All transactions have been committed.
```

**中文翻译:**
```
halt函数可以提前停止Block-STM执行，停止原因包括：
1. 模块发布交易与其他交易存在读写冲突
2. 资源组序列化错误
3. 交易VM执行状态为Abort
4. 交易VM执行状态为SkipRest
5. 已提交交易超过区块Gas限制
6. 所有交易已提交
```

#### 调度器包装器 (Scheduler Wrapper)

**English:**
```
The AtomicBool contains a flag that determines whether to skip module reads
when performing validation. BlockSTMv1 uses this as an optimization to
avoid unnecessary work when no modules have been published. BlockSTMv2 has
a different validation logic, and does not require this flag.
```

**中文翻译:**
```
SchedulerWrapper枚举提供统一接口支持v1和v2调度器：
- V1包含Scheduler引用和模块读取验证标志
- V2包含SchedulerV2引用
- 提供版本无关的操作接口，如halt、依赖等待等
```

### 12.2 ArmedLock 机制

**English:**
```
Last bit: 1 -> unlocked; 0 -> locked
Second bit: 1 -> there's work; 0 -> no work
try_lock succeeds when the lock is unlocked and armed (there is work to do)
```

**中文翻译:**
```
最后一位：1 -> 未锁定；0 -> 已锁定
第二位：1 -> 有工作；0 -> 无工作
当锁未锁定且已武装（有工作要做）时，try_lock成功
```

### 12.3 ExecutionStatus 说明

**English:**
```
All possible execution status for each transaction. In the explanation below, we abbreviate transaction as "txn" and incarnation as "inc"
Two types of execution tasks: Execution and Wakeup
Execution is a normal execution task, Wakeup is a task that just wakes up a suspended execution
```

**中文翻译:**
```
每个交易的所有可能执行状态。在下面的解释中，我们将交易缩写为"txn"，将化身缩写为"inc"
两种类型的执行任务：执行和唤醒
执行是正常的执行任务，唤醒是仅唤醒暂停执行的任务
```

## 13. 区块执行器核心 (Block Executor Core)

### 13.1 SharedSyncParams 结构

**English:**
```
Contains shared parameters for synchronized execution including base view, scheduler, versioned cache, global module cache, and various processors
Manages the coordination between different components during parallel execution
```

**中文翻译:**
```
包含同步执行的共享参数，包括基础视图、调度器、版本化缓存、全局模块缓存和各种处理器
管理并行执行期间不同组件之间的协调
```

## 14. 属性测试类型 (Property Test Types)

### 14.1 测试框架设计

**English:**
```
Contains mock storage value with STORAGE_AGGREGATOR_VALUE for testing aggregator functionality
For some resource group tests we ensure that the groups are never empty because they contain a value at RESERVED_TAG (starting from mock storage resolution) that is never deleted
Should not be possible to overflow or underflow, as each delta is at most 100 in the tests
```

**中文翻译:**
```
包含用于测试聚合器功能的模拟存储值STORAGE_AGGREGATOR_VALUE
对于某些资源组测试，我们确保组永远不为空，因为它们在RESERVED_TAG处包含一个值（从模拟存储解析开始），该值永远不会被删除
不应该可能溢出或下溢，因为测试中每个增量最多为100
```

### 14.2 基线评估 (Baseline Evaluation)

**English:**
```
This file implements the baseline evaluation, performed sequentially, the output of which is used to test the results of the block executor
The baseline must be evaluated after the block executor has completed, as the transaction type used for testing tracks the incarnation number, which is used to emulate dynamic behavior
Dynamic behavior means that when a transaction is re-executed, it might read different values and end up with a completely different behavior (be it read set, write set, or executed code)
In the tests, behavior changes based on the incarnation number, and hence it is crucial for the baseline to know the final incarnation number of each transaction of the tested block executor execution
```

**中文翻译:**
```
此文件实现基线评估，按顺序执行，其输出用于测试区块执行器的结果
基线必须在区块执行器完成后进行评估，因为用于测试的交易类型跟踪化身编号，用于模拟动态行为
动态行为意味着当交易重新执行时，它可能读取不同的值并最终产生完全不同的行为（无论是读取集、写入集还是执行的代码）
在测试中，行为根据化身编号变化，因此基线了解测试的区块执行器执行中每个交易的最终化身编号至关重要
```

## 总结

本文档整理了Block-STM源码中的关键多行注释，涵盖了：

1. **交易状态生命周期管理** - 详细的状态转换机制和两阶段中止处理
2. **停滞机制** - 用于管理交易依赖关系的平衡括号概念
3. **SchedulerV2设计** - BlockSTMv2的核心调度器架构
4. **数据读取和验证** - CapturedReads和DataRead的细粒度状态管理
5. **执行日志模块** - 全面的监控和跟踪功能
6. **并发性控制** - 方法调用的并发性规则和约束
7. **错误处理** - 各种错误类型和回退机制
8. **视图管理** - 读取结果和组读取结果的类型定义

这些注释展现了Block-STM作为并行执行引擎的复杂性和精密设计，特别是在处理并发控制、状态管理和错误恢复方面的深度考虑。

### 核心设计原则

1. **预定义顺序（Pre-ordered Block）**: 交易在区块中具有预定义的执行顺序，最终结果必须与顺序执行一致
2. **乐观并发控制（Optimistic Concurrency Control）**: 交易乐观地并行执行，通过验证阶段检测冲突
3. **推测-验证-重做（Speculate-Validate-Redo）**: 核心执行模式，失败时重新执行
4. **多版本内存管理**: 使用MVHashMap维护不同版本的状态数据

## Block-STM v1 架构

### 调度器（Scheduler）

Scheduler v1 是 Block-STM 的核心组件，负责管理交易的执行和验证任务分发。

#### 核心数据结构

```rust
pub struct Scheduler {
    num_txns: TxnIndex,
    txn_dependency: Vec<Mutex<Vec<TxnIndex>>>,
    txn_status: Vec<(RwLock<ExecutionStatus>, RwLock<ValidationStatus>)>,
    commit_state: Vec<Mutex<CommitState>>,
    execution_idx: AtomicU32,
    validation_idx: AtomicU64,
    done_marker: AtomicBool,
    has_halted: AtomicBool,
    queueing_commits_lock: ArmedLock,
    commit_queue: ConcurrentQueue<TxnIndex>,
}
```

#### 执行状态管理

**ExecutionStatus** 跟踪交易执行生命周期：
- `Ready(incarnation, ExecutionTaskType)`: 准备执行
- `Executing(incarnation, ExecutionTaskType)`: 正在执行
- `Suspended(incarnation, DependencyCondvar)`: 因依赖而挂起
- `Executed(incarnation)`: 执行完成
- `Committed(incarnation)`: 已提交
- `Aborting(incarnation)`: 正在中止
- `ExecutionHalted`: 执行已停止

**ValidationStatus** 管理验证波次：
```rust
struct ValidationStatus {
    max_triggered_wave: Wave,
    required_wave: Wave,
    maybe_max_validated_wave: Option<Wave>,
}
```

#### 任务类型

```rust
enum SchedulerTask {
    ExecutionTask(TxnIndex, Incarnation, ExecutionTaskType),
    ValidationTask(TxnIndex, Incarnation, Wave),
    Retry,
    Done,
}
```

### 依赖管理

v1 使用条件变量（DependencyCondvar）实现依赖等待机制：
- 当交易读取未完成交易的写入时，建立依赖关系
- 使用 `wait_for_dependency` 方法注册依赖
- 依赖解决后通过 `resume` 方法唤醒等待的交易

### 执行流程

1. **任务获取**: 工作线程从调度器获取执行或验证任务
2. **乐观执行**: 交易并行执行，记录读写集
3. **依赖处理**: 遇到依赖时挂起，等待依赖解决
4. **验证阶段**: 重新读取验证读写集的一致性
5. **提交处理**: 按顺序提交已验证的交易

## Block-STM v2 架构增强

### SchedulerV2 核心改进

SchedulerV2 引入了多项重要优化，显著提升了并发性能和资源利用率。

#### 新增核心组件

```rust
pub struct SchedulerV2 {
    num_txns: TxnIndex,
    num_workers: u32,
    txn_statuses: ExecutionStatuses,
    aborted_dependencies: Vec<CachePadded<Mutex<AbortedDependencies>>>,
    next_to_commit_idx: CachePadded<AtomicU32>,
    is_done: CachePadded<AtomicBool>,
    queueing_commits_lock: ArmedLock,
    post_commit_queue: ConcurrentQueue<TxnIndex>,
}
```

#### ExecutionQueueManager

负责执行队列管理和调度进度跟踪：

```rust
struct ExecutionQueueManager {
    executed_once_max_idx: CachePadded<AtomicU32>,
    min_not_scheduled_idx: CachePadded<AtomicU32>,
    execution_queue: Mutex<BTreeSet<TxnIndex>>,
}
```

**关键优化指标**：
- `executed_once_max_idx`: 跟踪所有交易至少执行一次的最高索引
- `min_not_scheduled_idx`: 跟踪尚未调度的最小交易索引

### Stall 机制

v2 引入了智能的 stall 传播机制，减少无效重执行：

#### AbortedDependencies

```rust
struct AbortedDependencies {
    stalled_deps: BTreeSet<TxnIndex>,
    not_stalled_deps: BTreeSet<TxnIndex>,
    is_stalled: bool,
}
```

#### Stall 传播算法

1. **Stall 添加**: 当交易 T_i 被 T_j 中止时，T_i 进入 stall 状态
2. **传播机制**: Stall 状态沿依赖图向下游传播
3. **智能调度**: 被 stall 的交易延迟重执行，直到上游依赖稳定
4. **Unstall 处理**: 上游交易完成后，递归解除下游 stall

### 两阶段中止处理

v2 实现了更精细的中止管理：

```rust
struct AbortManager<'a> {
    txn_idx: TxnIndex,
    incarnation: Incarnation,
    invalidated_set: BTreeSet<TxnIndex>,
    scheduler: &'a SchedulerV2,
}
```

**处理流程**：
1. `start_abort`: 标记交易开始中止
2. `finish_abort`: 完成中止并重新调度
3. **依赖记录**: 记录被中止的依赖关系用于 stall 管理

### 执行流程控制

#### 水位线机制

`executed_once_max_idx` 作为执行水位线：
- 确保所有前置交易至少执行一次后才允许重执行
- 避免基于不完整推测状态的重执行
- 提高重执行的成功率

#### 任务优先级

v2 的任务调度优先级：
1. **Post-commit 任务**: 最高优先级，确保提交流程不阻塞
2. **执行任务**: 从执行队列获取，按索引顺序处理
3. **控制任务**: 无可用工作时返回 NextTask 或 Done

## 多版本数据结构（MVHashMap）

### 核心概念

MVHashMap 是 Block-STM 的核心数据结构，支持多版本并发访问：

```rust
struct MVHashMap<K, T, V, D> {
    data: VersionedData<K, V>,
    group_data: VersionedGroupData<K, T, V>,
    delayed_fields: VersionedDelayedFields<D>,
    logger: Option<Arc<dyn BlockSTMLogger>>,
}
```

### 版本化读取

#### 读取类型

```rust
enum ReadKind {
    Value,
    MetadataAndResourceSize,
    Metadata,
    ResourceSize,
    Exists,
}
```

#### 读取结果

```rust
enum MVDataOutput<V> {
    Versioned(Version, Arc<V>),
    Resolved(V),
}

enum MVDataError {
    Dependency(TxnIndex),
    Unresolved(DelayedFieldID),
    DeltaApplicationFailure,
    Uninitialized,
}
```

### 读写集捕获

#### CapturedReads

```rust
struct CapturedReads<T, K, DC, VC, S> {
    data_reads: HashMap<T::Key, DataRead<T::Value>>,
    group_reads: HashMap<T::Key, GroupRead<T>>,
    delayed_field_reads: HashMap<DelayedFieldID, DelayedFieldRead>,
    module_reads: HashMap<K, ModuleRead<DC, VC, S>>,
    delayed_field_speculative_failure: bool,
    non_delayed_field_speculative_failure: bool,
    incorrect_use: bool,
}
```

#### 验证机制

```rust
fn validate_data_reads(
    &self,
    data_map: &VersionedData<T::Key, T::Value>,
    idx_to_validate: TxnIndex,
) -> bool
```

验证过程：
1. 重新读取原读写集中的所有键
2. 比较当前值与执行时读取的值
3. 不一致则验证失败，需要重新执行

## v1 与 v2 核心差异对比

### 架构层面

| 特性 | Block-STM v1 | Block-STM v2 |
|------|-------------|-------------|
| 调度器 | Scheduler | SchedulerV2 + ExecutionQueueManager |
| 依赖管理 | 条件变量等待 | Stall 传播机制 |
| 中止处理 | 单阶段中止 | 两阶段中止 + AbortManager |
| 执行控制 | execution_idx + validation_idx | 水位线 + 执行队列 |
| 任务类型 | 4种基础任务 | 增强任务类型 + Post-commit |

### 性能优化

#### v1 局限性
1. **盲目重执行**: 缺乏上下文信息的重执行可能基于不完整状态
2. **级联中止**: 依赖链中的中止可能引发大量无效重执行
3. **资源竞争**: 简单的原子索引可能导致不必要的竞争

#### v2 改进
1. **智能调度**: 水位线机制确保重执行基于相对完整的推测状态
2. **Stall 传播**: 减少级联中止导致的无效工作
3. **精细化管理**: 分离执行队列和状态管理，减少锁竞争
4. **增强监控**: 详细的性能指标和日志系统

### 并发控制

#### v1 并发模型
- 基于条件变量的依赖等待
- 全局执行和验证索引
- 简单的波次验证机制

#### v2 并发模型
- 基于 stall 状态的智能调度
- 分布式执行队列管理
- 依赖图感知的传播机制
- 两阶段中止减少竞争窗口

## 日志与监控系统

### BlockSTMLogger

v2 引入了全面的日志系统，支持 50+ 种事件类型：

#### 事件分类
1. **区块生命周期**: BlockStart, BlockFinish, BlockCommit
2. **调度器状态**: SchedulerSample, TaskDistribution
3. **MV哈希表操作**: MVRead, MVWrite, EstimateMark
4. **依赖关系**: DependencyBlock, StallPropagation
5. **执行流程**: ExecutionStart, ValidationFinish
6. **中止恢复**: AbortInitiated, IncarnationIncrement
7. **性能指标**: PerformanceMetric, MemoryUsageSnapshot
8. **BlockSTMv2增强**: StallAdd, WaterlineAdvance, AbortStart

#### 采样机制

```rust
const READ_SAMPLE_RATE: u32 = 1000;  // 1/1000 采样率
const WRITE_SAMPLE_RATE: u32 = 100;  // 1/100 采样率
```

### 性能统计

```rust
struct BlockExecutionStats {
    stall_events_count: AtomicU32,
    waterline_advances_count: AtomicU32,
    abort_cycles_count: AtomicU32,
    max_concurrent_executions: AtomicU32,
    total_reexecutions: AtomicU64,
    // ... 更多指标
}
```

## 算法原理深入分析

### 乐观并发控制原理

Block-STM 基于以下两个核心原则实现正确的乐观执行：

1. **READLAST(k)**: 交易 TXk 读取时，获取索引小于 k 的最高交易写入的值
2. **VALIDAFTER**: 验证失败时，只需重新验证索引更高的交易

### 依赖关系建模

在顺序执行中，如果交易 TXj 写入的值被 TXk 读取，则存在依赖关系 TXj → TXk。Block-STM 通过以下机制维护这种依赖：

1. **读时依赖检测**: 读取操作检查是否存在未完成的写入
2. **版本化存储**: MVHashMap 维护每个键的多个版本
3. **依赖传播**: v2 中的 stall 机制沿依赖图传播

### 验证算法

验证过程确保推测执行的正确性：

```rust
fn validate_transaction(txn_idx: TxnIndex, read_set: &ReadSet) -> bool {
    for (key, expected_version) in read_set {
        let current_version = mvhashmap.get_latest_version(key, txn_idx);
        if current_version != expected_version {
            return false;
        }
    }
    true
}
```

## 实际应用与性能

### 性能特征

根据 Aptos 的实测数据，Block-STM 能够实现：
- **160k+ TPS**: 在复杂 Move 交易场景下
- **线性扩展**: 随 CPU 核心数近似线性提升
- **自适应性**: 自动适应不同冲突模式的工作负载

### 适用场景

1. **高并发 DeFi**: 大量并行的代币转账和交易
2. **NFT 市场**: 并行的铸造和交易操作
3. **游戏应用**: 大量玩家的并行状态更新
4. **企业应用**: 高吞吐量的业务逻辑处理

### 局限性

1. **内存开销**: 多版本存储需要额外内存
2. **复杂性**: 实现和调试相对复杂
3. **最坏情况**: 高冲突场景下可能退化为顺序执行

## 总结

Block-STM 代表了区块链并行执行技术的重要突破。从 v1 到 v2 的演进，体现了在保持核心算法正确性的基础上，通过工程优化显著提升实际性能的成功实践。

### 技术创新点

1. **预定义顺序的乐观执行**: 巧妙结合确定性要求与并行性能
2. **多版本内存管理**: 高效的版本化数据结构设计
3. **智能依赖管理**: v2 的 stall 机制显著减少无效工作
4. **自适应调度**: 根据执行状态动态调整调度策略

### 未来发展方向

1. **内存优化**: 进一步减少多版本存储的内存开销
2. **跨区块优化**: 探索跨区块的状态预取和缓存
3. **硬件加速**: 利用专用硬件加速关键路径
4. **更智能的调度**: 基于机器学习的自适应调度算法

Block-STM 的成功证明了在区块链这样的确定性环境中，通过精心设计的并发控制机制，可以在保证正确性的前提下实现显著的性能提升。这为未来高性能区块链系统的设计提供了重要的技术参考。