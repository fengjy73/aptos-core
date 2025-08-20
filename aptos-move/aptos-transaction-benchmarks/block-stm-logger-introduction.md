# Block-STM日志系统详细介绍

## 概述

Block-STM日志系统是Aptos区块链核心组件中的综合性能监控和分析工具，专门设计用于记录和分析Block-STM（Software Transactional Memory）乐观并行执行引擎的运行状态。该系统提供了深度的执行洞察，包括交易调度、依赖管理、冲突检测、并发优化等关键方面。

## 架构特点

### 🎯 核心设计原则
- **原子事件驱动**: 记录所有基础事件，派生指标按需计算
- **分类存储**: 8个专门的日志文件类型，便于分析
- **高精度时间戳**: 微秒级精度，支持精确的时间线分析
- **Thread-Safe**: 支持多线程并发写入
- **可配置采样**: 支持读操作采样以控制日志大小

### 📊 日志分类架构
```
Block-STM日志系统
├── block_summary.ndjson      # 区块生命周期和整体统计
├── scheduler_states.ndjson   # 调度器状态转换和任务管理  
├── mvhashmap_ops.ndjson     # 多版本哈希表读写操作
├── execution_flow.ndjson    # 交易执行和验证流程
├── dependencies.ndjson      # 依赖关系和解决过程
├── abort_recovery.ndjson    # 中止和恢复机制
├── detailed_operations.ndjson # 高级特性操作记录
├── system_operations.ndjson # 系统级操作和性能指标
└── block_000/              # 交易映射和元数据
    ├── row_tx_mapping.csv  # CSV到Block-STM的交易映射
    ├── meta.json          # 执行环境和参数信息
    └── code_map.json      # 源码位置映射
```

## 测试命令

### 基本测试命令
```bash
# 进入基准测试目录
cd aptos-move/aptos-transaction-benchmarks

# 基础日志记录测试
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs \
cargo run --release -- replay-erc20 \
--data-path data/ETH_2401_100.csv --concurrency-level 4 --num-runs 1
```

### 高级配置选项
```bash
# 完整环境变量配置
export BLOCK_STM_LOG_LEVEL=DEBUG          # DEBUG|INFO|WARN|ERROR
export BLOCK_STM_LOG_DIR=./test_logs_all  # 日志输出目录
export BLOCK_STM_LOG_MAX_SIZE=100         # 单个日志文件最大大小(MB)
export SAMPLE_PERIOD_MS=50                # 调度器采样周期
export READ_SAMPLE_RATE=0.01              # MV读操作采样率

# 并发级别对比测试
for cores in 1 2 4 8; do
  BLOCK_STM_LOG_DIR=./test_logs_${cores}cores \
  cargo run --release -- replay-erc20 \
    --data-path data/ETH_2401_100.csv --concurrency-level $cores
done

# 性能基准测试
cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_100000.csv \
  --concurrency-level 16 --num-warmups 2 --num-runs 5
```

## 日志文件详解

基于`test_logs_all`目录的实际输出，以下是各类日志文件的详细介绍：

### 1. block_summary.ndjson - 区块生命周期

**用途**: 记录区块执行的开始和结束事件，提供整体执行统计。

**关键字段**:
- `timestamp`: 微秒级时间戳
- `thread_list`: 参与执行的所有线程ID（优化后）
- `committed_count`: 成功提交的交易数量
- `total_duration_us`: 总执行时间

**日志示例**:
```json
{"type":"BlockStart","timestamp":18696,"thread_id":0,"block_id":"block_14878","dataset":"ETH","tx_count":100,"concurrency_level":4,"sample_period_ms":50,"read_sample_rate":0.01,"source_csv":"data/ETH_2401_100.csv"}

{"type":"BlockFinish","timestamp":36132,"thread_list":[3320665455366264189,3673300442962989464,5357406925723651718,12318721104400761032],"block_id":"block_14878","committed_count":100,"total_duration_us":17435}
```

**分析价值**:
- 计算并行TPS: `committed_count * 1,000,000 / total_duration_us`
- 线程利用率分析: 通过thread_list了解实际参与的线程数量
- 数据集特征: 通过dataset和source_csv进行分类分析

### 2. scheduler_states.ndjson - 调度器状态管理

**用途**: 记录调度器的状态转换、任务分配和完成情况，是并发分析的核心数据源。

**关键事件类型**:
- `SchedulerStateTransition`: 交易状态转换
- `TaskPickedV2`: 任务被工作线程获取  
- `TaskFinishedV2`: 任务执行完成

**日志示例**:
```json
{"type":"SchedulerStateTransition","timestamp":19048,"thread_id":3320665455366264189,"transaction_id":0,"incarnation":0,"old_state":"PendingScheduling","new_state":"Executing","trigger_reason":"start_executing"}

{"type":"TaskPickedV2","timestamp":19076,"picked_ts_us":19076,"thread_id":3320665455366264189,"task_kind":"Execute","tx_index":0,"incarnation":0,"is_first_execution":true,"from_queue":"ExecutionQueue"}
```

**分析价值**:
- **并发度分析**: 统计同时处于"Executing"状态的交易数量
- **调度效率**: 计算TaskPicked到TaskFinished的处理时间
- **重试模式**: 通过incarnation变化识别重新执行

### 3. mvhashmap_ops.ndjson - 多版本存储操作

**用途**: 记录Block-STM核心的多版本哈希表操作，包括读写和估计标记。

**关键操作类型**:
- `MVWrite`: 写入操作
- `MVRead`: 读取操作（按采样率记录）

**日志示例**:
```json
{"type":"MVWrite","timestamp":19091,"thread_id":3320665455366264189,"transaction_id":0,"incarnation":0,"state_key":"state_key_0","value_size":128,"write_type":"Modify"}

{"type":"MVWrite","timestamp":20210,"thread_id":12318721104400761032,"transaction_id":1,"incarnation":0,"state_key":"state_key_1","value_size":16,"write_type":"Modify"}
```

**分析价值**:
- **热点状态识别**: 统计state_key的访问频率
- **写入模式分析**: Create/Modify/Delete操作分布
- **数据局部性**: 分析同一线程的连续操作模式

### 4. execution_flow.ndjson - 执行和验证流程

**用途**: 记录交易在Move VM中的详细执行过程，包括Gas使用、读写集统计等。

**关键事件类型**:
- `ExecutionStart/Finish`: VM执行生命周期
- `ValidationStart/Finish`: 读写集验证过程

**日志示例**:
```json
{"type":"ExecutionStart","timestamp":19126,"thread_id":3673300442962989464,"transaction_id":2,"incarnation":0,"execution_phase":"Initial"}

{"type":"ExecutionFinish","timestamp":20294,"thread_id":5357406925723651718,"transaction_id":3,"incarnation":0,"result":"Success","exec_duration_us":1058,"gas_used":7,"read_set_size":25,"write_set_size":1,"resource_reads":25,"resource_writes":1,"module_reads":0,"module_writes":0,"delayed_field_reads":0,"delayed_field_writes":1}
```

**分析价值**:
- **性能瓶颈**: 通过exec_duration_us识别慢交易
- **Gas效率**: 分析gas_used与操作复杂度的关系
- **读写模式**: resource_reads/writes等字段揭示交易特征

### 5. dependencies.ndjson - 依赖关系管理

**用途**: 记录交易间的依赖关系建立和解决过程，是乐观并行执行分析的关键。

**关键事件类型**:
- `DependencyBlock`: 依赖关系建立
- `DependencyResolve`: 依赖解决

**日志示例**:
```json
{"type":"DependencyResolve","timestamp":20298,"thread_id":5357406925723651718,"depender_tx":4,"state_key":"dependency_resolve_tx_4_by_tx_3","on_tx":3,"resolve_cause":"OnTxExecuted"}

{"type":"DependencyResolve","timestamp":20317,"thread_id":5357406925723651718,"depender_tx":5,"state_key":"dependency_resolve_tx_5_by_tx_3","on_tx":3,"resolve_cause":"OnTxExecuted"}
```

**分析价值**:
- **依赖链分析**: 构建交易依赖图谱
- **等待时间计算**: `DependencyResolve.timestamp - DependencyBlock.timestamp`
- **热点检测**: 识别引起多个依赖的"热点"交易

### 6. abort_recovery.ndjson - 中止和恢复机制

**用途**: 记录交易中止、重试和依赖失效的详细过程。

**关键事件类型**:
- `InvalidationEdge`: 依赖失效边
- `AbortStart/Finish`: 两阶段中止过程
- `AbortInitiated`: 中止启动事件

**日志示例**:
```json
{"type":"InvalidationEdge","timestamp":20251,"thread_id":3320665455366264189,"by_tx":0,"to_tx":1,"to_incarnation":0,"key":"tx_0_to_tx_1"}

{"type":"AbortStart","timestamp":20272,"thread_id":3320665455366264189,"transaction_id":1,"incarnation":0,"by_tx":1,"result":"Started"}

{"type":"AbortInitiated","timestamp":20574,"thread_id":3320665455366264189,"transaction_id":1,"incarnation":0,"abort_reason":"Dependency invalidation","retry_count":0,"dependencies":[1,0]}
```

**分析价值**:
- **冲突模式**: 分析导致中止的原因模式
- **重试效率**: 通过retry_count评估重试成本
- **失效传播**: 跟踪InvalidationEdge的连锁反应

### 7. 交易映射和元数据

#### row_tx_mapping.csv - 交易映射表
**用途**: 建立CSV原始数据与Block-STM交易索引的对应关系。

**字段说明**（精简后17个字段）:
```csv
block_id,dataset,source_csv,row_number,tx_index,from_raw,to_raw,value_raw,from_norm,to_norm,from_account_id,to_account_id,key_from_id,key_to_id,included,skip_reason,row_hash
```

**示例数据**:
```csv
block_000,ETH,data/ETH_2401_100.csv,1,1,0,5327,1,0,5327,1,2,1,2,true,,9bb908ae07358285c488df8e3c1dcdfff3897dd6559640d98036be81ad3fe1a2
```

#### meta.json - 执行环境元数据
**用途**: 记录执行环境、参数配置和系统信息。

**关键信息**:
```json
{
  "block_id": "block_000",
  "dataset": "ETH",
  "tx_count": 100,
  "concurrency_level": 4,
  "read_sample_rate": 0.01,
  "environment": {
    "rust_version": "rustc 1.86.0",
    "host_os": "macos",
    "cpu_count": 16
  }
}
```

