# Block-STM日志记录问题分析与修复汇总

## 已修复的问题

### 1. InvalidationEdge的key字段为null
**问题**: 在`abort_recovery.ndjson`中，InvalidationEdge事件的key字段始终为null。
**原因**: scheduler_v2.rs中log_invalidation_edge调用时硬编码传递了None。
**修复**: 在scheduler_v2.rs:225行，将`None`改为`Some(format!("tx_{}_to_tx_{}", self.owner_txn_idx, invalidated_txn_idx))`，提供有意义的key标识符。

### 2. BlockStart中不必要的字段
**问题**: BlockStart事件包含git_commit和build_profile字段，用户要求移除。
**修复**: 
- 从LogEvent::BlockStart定义中移除git_commit和build_profile字段
- 从ExecutionContext结构体中移除这两个字段
- 修改log_block_start方法参数
- 更新simulator.rs中的ExecutionContext创建代码

## 需要复杂统计分析的问题

### 1. row_tx_mapping.csv中的统计字段为0
**文件**: `test_logs_mv/block_000/row_tx_mapping.csv`
**问题字段**:
- `reexecution_count`: 重新执行次数，当前全部为0
- `stall_count`: 阻塞次数，当前全部为0
- `waterline_at_exec`: 执行时的waterline位置，当前全部为0
- `abort_count`: 中止次数，当前全部为0

**分析**: 这些统计数据需要在交易执行过程中实时收集和累计，涉及多个组件的协作：
- 需要在scheduler中跟踪每个交易的重新执行次数
- 需要在依赖管理中统计阻塞事件
- 需要记录waterline推进时的状态
- 需要统计abort事件

### 2. BlockFinish中的性能指标缺失
**文件**: `test_logs_mv/block_summary.ndjson`
**问题字段**:
- `parallel_tps`: 并行执行TPS，当前为0.0
- `sequential_tps`: 顺序执行TPS，当前为0.0 
- `waterline_advances_count`: waterline推进次数，当前为0
- `abort_cycles_count`: 中止循环次数，当前为0
- `max_concurrent_executions`: 最大并发执行数，当前为0

**分析**: 需要额外的性能监控逻辑：
- TPS计算需要准确的时间测量和交易计数
- 需要独立的顺序执行基准来对比
- 需要监控waterline的推进事件
- 需要分析abort的连锁反应形成的循环
- 需要跟踪同时执行的交易数量峰值

### 3. dependencies.ndjson中的字段缺失
**文件**: `test_logs_mv/dependencies.ndjson`
**问题字段**:
- `owner_incarnation`: 所有者化身号，当前为null
- `state_key`: 状态键，当前为"unknown"
- `depends_on_incarnation`: 依赖的化身号，当前为null
- `propagation_latency_us`: 传播延迟，当前为0
- `stall_chain_id`: 阻塞链ID，当前为null

**分析**: 需要增强依赖跟踪系统：
- 依赖解析时需要记录完整的化身信息
- 需要传递实际的状态键而不是"unknown"
- 需要计算依赖传播的时间延迟
- 需要构建阻塞链的标识系统

### 4. scheduler_states.ndjson中的时间字段缺失
**文件**: `test_logs_mv/scheduler_states.ndjson`  
**问题字段**:
- `total_suspended_time_us`: 总暂停时间，当前为0
- `state_key`: 相关状态键，当前为null
- `suspend_duration_us`: 暂停持续时间，当前为null

**分析**: 当前的调度器状态转换日志缺少这些字段：
- 需要在状态转换时计算累计暂停时间
- 需要关联具体的状态键信息
- 需要测量每次暂停的持续时间

## 架构层面的改进建议

### 1. thread_id改为thread_list
**问题**: BlockFinish事件中thread_id为单一值0，用户要求改为thread_list列表。
**建议**: 
- 收集所有参与执行的线程ID
- 在BlockFinish时记录完整的线程列表
- 有助于分析线程使用模式和负载分布

### 2. 统一的统计数据收集框架
**建议**: 
- 创建专门的MetricsCollector组件
- 在关键执行点插入统计数据收集逻辑
- 建立统计数据的生命周期管理
- 提供统计数据的聚合和查询接口

### 3. 状态键管理改进
**建议**:
- 在MVHashMap操作中传递实际的状态键信息
- 在依赖跟踪时保留状态键上下文
- 建立状态键的标准化格式

## 实现优先级

### 高优先级（可独立修复）
1. dependencies中的state_key从"unknown"改为实际值
2. thread_id改为thread_list
3. 基础的时间字段填充

### 中优先级（需要架构改动）
1. 重新执行次数和阻塞次数统计
2. TPS性能指标计算
3. waterline相关统计

### 低优先级（需要全面重构）
1. 完整的阻塞链分析
2. 高级性能分析指标
3. 复杂的依赖传播分析

## 总结

当前已修复了2个直接可修复的问题。剩余问题主要集中在运行时统计数据收集方面，需要在Block-STM执行引擎的多个层面添加监控逻辑。建议分阶段实施，优先处理可以独立修复的字段，再逐步完善复杂的统计分析功能。