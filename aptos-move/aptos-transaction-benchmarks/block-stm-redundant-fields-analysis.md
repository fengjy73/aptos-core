# Block-STM日志系统冗余字段分析与优化

## 核心原则

基于Block-STM乐观并行执行的核心特征，许多统计指标可以通过联合多个日志文件的原子事件计算得出，无需在单个文件中重复存储。

## 可删除的冗余字段分类

### 1. BlockFinish事件中的派生统计字段

**文件**: `block_summary.ndjson`

#### 可删除字段:
```json
{
  "parallel_tps": 0.0,                    // 可计算
  "sequential_tps": 0.0,                  // 可计算  
  "stall_events_count": 0,                // 可计算
  "waterline_advances_count": 0,          // 可计算
  "abort_cycles_count": 0,                // 可计算
  "max_concurrent_executions": 0,         // 可计算
  "avg_reexecution_per_tx": 0.0,         // 可计算
  "scheduler_efficiency": 0.0             // 可计算
}
```

#### 计算方法:
- **parallel_tps**: `交易数量 / (BlockFinish.timestamp - BlockStart.timestamp) * 1,000,000`
- **stall_events_count**: 统计`scheduler_states.ndjson`中`StallAdd`事件数量
- **waterline_advances_count**: 统计`scheduler_states.ndjson`中`WaterlineAdvance`事件数量
- **abort_cycles_count**: 统计`abort_recovery.ndjson`中`AbortStart`事件数量
- **max_concurrent_executions**: 分析`scheduler_states.ndjson`中`TaskPicked`与`TaskFinished`事件的时间重叠
- **avg_reexecution_per_tx**: 统计`abort_recovery.ndjson`中`IncarnationIncrement`事件，计算平均值
- **scheduler_efficiency**: `成功执行数 / 总尝试执行数`，基于TaskFinished事件

### 2. 交易映射表中的运行时统计

**文件**: `block_000/row_tx_mapping.csv`

#### 可删除字段:
```csv
execution_time_us,        # 可从execution_flow.ndjson计算
reexecution_count,        # 可从scheduler_states.ndjson计算  
stall_count,              # 可从scheduler_states.ndjson计算
waterline_at_exec,        # 可从scheduler_states.ndjson计算
abort_count               # 可从abort_recovery.ndjson计算
```

#### 计算方法:
- **execution_time_us**: `ExecutionFinish.timestamp - ExecutionStart.timestamp`（按tx_index匹配）
- **reexecution_count**: 统计每个transaction_id的`IncarnationIncrement`事件数量
- **stall_count**: 统计每个transaction_id的`StallAdd`事件数量
- **waterline_at_exec**: 查找执行时刻最近的`WaterlineAdvance.executed_once_max_idx_after`
- **abort_count**: 统计每个transaction_id的`AbortInitiated`事件数量

### 3. 依赖事件中的时间计算字段

**文件**: `dependencies.ndjson`

#### 可删除字段:
```json
{
  "wait_time_us": 0,                      // 可计算
  "propagation_latency_us": 0,            // 可计算
  "stall_depth": 1,                       // 可计算
  "affected_count": 1                     // 可计算
}
```

#### 计算方法:
- **wait_time_us**: `DependencyResolve.timestamp - DependencyBlock.timestamp`（按state_key匹配）
- **propagation_latency_us**: 分析`StallPropagation`事件链的时间差
- **stall_depth**: 分析依赖链深度，统计`DependencyBlock`事件的嵌套层级
- **affected_count**: 计算`StallPropagation.affected_txns`数组长度

### 4. 调度器状态中的持续时间字段

**文件**: `scheduler_states.ndjson`

#### 可删除字段:
```json
{
  "total_suspended_time_us": 0,           // 可计算
  "suspend_duration_us": null,            // 可计算
  "processing_time_us": 0                 // 可计算
}
```

#### 计算方法:
- **total_suspended_time_us**: `TaskResume.timestamp - TaskSuspend.timestamp`的累计和
- **suspend_duration_us**: 每次`TaskResume.timestamp - TaskSuspend.timestamp`
- **processing_time_us**: `TaskFinished.timestamp - TaskPicked.timestamp`

### 5. 执行流程中的聚合统计

**文件**: `execution_flow.ndjson`

#### 可删除字段:
```json
{
  "val_duration_us": 0,                   // 可计算
  "validated_reads": 0,                   // 可计算（需要MV日志支持）
  "conflicts_found": 0                    // 可计算（需要abort日志支持）
}
```

#### 计算方法:
- **val_duration_us**: `ValidationFinish.timestamp - ValidationStart.timestamp`
- **validated_reads**: 统计验证期间的`MVRead`事件（需要验证上下文标识）
- **conflicts_found**: 统计相关的`ValidationConflict`事件数量

## Block-STM特定优化策略

### 1. Incarnation-Based计算
Block-STM的incarnation机制让我们可以通过incarnation号变化准确计算重试次数：
```
reexecution_count = max(incarnation) - 1  // 每个transaction_id
```

### 2. 依赖链分析
利用Block-STM的依赖跟踪特性，通过`DependencyBlock`和`DependencyResolve`事件构建依赖图：
```
stall_chain_depth = DFS深度搜索依赖图
wait_time_total = Σ(resolve_time - block_time)
```

### 3. 并发度实时计算
通过`TaskPicked`和`TaskFinished`事件的时间窗口分析：
```
concurrent_count(t) = count(TaskPicked.timestamp ≤ t < TaskFinished.timestamp)
max_concurrent_executions = max(concurrent_count(t)) for all t
```

### 4. 水位线推进效率
Block-STM水位线机制的效率指标：
```
waterline_efficiency = 实际推进次数 / 理论最小推进次数
```

## 推荐的最小化日志架构

### 保留的核心原子事件
1. **时间戳事件**: BlockStart, ExecutionStart/Finish, TaskPicked/Finished
2. **状态转换事件**: SchedulerStateTransition, IncarnationIncrement
3. **依赖关系事件**: DependencyBlock/Resolve（时间戳+关系）
4. **MV操作事件**: MVRead/Write（采样）
5. **abort事件**: AbortStart/Finish, InvalidationEdge

### 删除的派生字段
- 所有可通过时间差计算的duration字段
- 所有可通过事件计数得出的count字段  
- 所有可通过状态分析得出的统计字段

## 实施优势

### 1. 存储效率
- **日志文件大小减少**: 估计减少30-40%的存储空间
- **写入性能提升**: 减少实时计算负担，只记录原子事件

### 2. 数据一致性
- **单一事实源**: 避免同一指标在多处记录导致的不一致
- **计算透明性**: 所有派生指标的计算过程可追溯和验证

### 3. 分析灵活性
- **定制化指标**: 可根据分析需求计算不同的派生指标
- **历史回溯**: 可重新计算历史数据的新指标

### 4. Block-STM特化
- **乐观执行分析**: 更好地分析推测执行的成功率和效率
- **依赖链优化**: 深度分析依赖传播路径和优化点
- **并发瓶颈识别**: 精确识别并发执行的瓶颈点

## 实施建议

1. **分阶段迁移**: 先保留冗余字段，验证计算正确性后再删除
2. **计算工具开发**: 创建专门的日志分析工具进行派生指标计算
3. **性能验证**: 确保实时计算不会显著影响执行性能
4. **向后兼容**: 提供计算接口，保持现有分析工具的兼容性

通过这种优化，Block-STM日志系统将更加精简高效，同时保持完整的分析能力。