# Block-STM 详细跟踪日志记录方案

## 1. 需要记录的关键字段

基于乐观并发控制理论和Block-STM特点，我们需要跟踪以下信息：

### 1.1 事务执行状态字段

- **transaction_id**: 事务索引 (TxnIndex)
- **incarnation**: 事务化身编号 (Incarnation)
- **thread_id**: 执行线程ID
- **timestamp**: 时间戳 (微秒精度)
- **execution_status**: 执行状态 (PendingScheduling, Executing, Executed, Aborted)
- **execution_result**: 执行结果 (Success, Abort, SkipRest, SpeculativeExecutionAbortError)
- **execution_duration_us**: 执行耗时 (微秒)
- **validation_duration_us**: 验证耗时 (微秒)

### 1.2 并发控制字段

- **stall_status**: 暂停状态 (stalled/not_stalled)
- **abort_reason**: 中止原因 (dependency_conflict, validation_failure, speculative_error)
- **dependency_txns**: 依赖的事务列表
- **invalidated_by**: 被哪个事务无效化
- **retry_count**: 重试次数

### 1.3 读写集字段

- **read_set_size**: 读集大小
- **write_set_size**: 写集大小
- **read_keys**: 读取的键列表 (截断显示)
- **write_keys**: 写入的键列表 (截断显示)
- **resource_reads**: 资源读取数量
- **resource_writes**: 资源写入数量
- **module_reads**: 模块读取数量
- **module_writes**: 模块写入数量
- **delayed_field_reads**: 延迟字段读取数量
- **delayed_field_writes**: 延迟字段写入数量

### 1.4 性能指标字段

- **gas_used**: 消耗的Gas
- **output_size**: 输出大小
- **memory_usage**: 内存使用量
- **cache_hits**: 缓存命中次数
- **cache_misses**: 缓存未命中次数

## 2. 日志记录位置规划

### 2.1 核心记录点

#### 在 `scheduler_v2.rs` 中:

- `start_executing()`: 记录事务开始执行
- `finish_execution()`: 记录事务执行完成
- `start_abort()`: 记录事务开始中止
- `finish_abort()`: 记录事务中止完成
- `propagate()`: 记录暂停/恢复传播

#### 在 `executor.rs` 中:

- `execute()`: 记录详细执行过程
- `validate()`: 记录验证过程
- `update_transaction_on_abort()`: 记录中止时的状态更新

#### 在 `task.rs` 中:

- `execute_transaction()`: 记录任务级别的执行

### 2.2 日志文件结构

```
logs/
├── block_stm_execution.log      # 主执行日志
├── block_stm_concurrency.log    # 并发控制日志
├── block_stm_readwrite.log      # 读写集变化日志
├── block_stm_performance.log    # 性能指标日志
└── block_stm_summary.log        # 汇总统计日志
```

## 3. 日志格式设计

### 3.1 JSON格式日志

每条日志记录采用JSON格式，便于解析和分析：

```json
{
  "timestamp": "2024-01-15T10:30:45.123456Z",
  "event_type": "transaction_execution",
  "transaction_id": 42,
  "incarnation": 1,
  "thread_id": 3,
  "execution_status": "Executing",
  "read_set_size": 5,
  "write_set_size": 2,
  "dependencies": [38, 40],
  "duration_us": 1250,
  "gas_used": 1000,
  "details": {
    "read_keys": ["0x1::account::Account", "0x1::coin::CoinStore"],
    "write_keys": ["0x1::account::Account"]
  }
}
```

### 3.2 事件类型定义

- `transaction_start`: 事务开始执行
- `transaction_finish`: 事务执行完成
- `transaction_abort`: 事务中止
- `transaction_validate`: 事务验证
- `dependency_stall`: 依赖暂停
- `dependency_unstall`: 依赖恢复
- `readwrite_conflict`: 读写冲突
- `performance_metric`: 性能指标

## 4. 实现方案

### 4.1 日志记录器结构

创建 `BlockSTMLogger` 结构体，包含：

- 多个日志文件句柄
- 线程安全的写入机制
- 配置选项 (日志级别、文件大小限制等)
- 性能优化 (批量写入、异步IO)

### 4.2 集成点

在现有代码中添加日志记录调用：

- 最小化性能影响
- 使用条件编译 (feature flags)
- 异步日志写入
- 内存缓冲区

### 4.3 配置管理

通过环境变量或配置文件控制：

- `BLOCK_STM_LOG_LEVEL`: 日志级别 (DEBUG, INFO, WARN, ERROR)
- `BLOCK_STM_LOG_DIR`: 日志目录
- `BLOCK_STM_LOG_MAX_SIZE`: 单个日志文件最大大小
- `BLOCK_STM_LOG_ROTATION`: 日志轮转策略

## 5. 测试方案

### 5.1 单元测试

- 测试日志记录器的基本功能
- 验证JSON格式的正确性
- 测试并发写入的安全性

### 5.2 集成测试

- 使用现有的Block-STM测试用例
- 验证日志记录不影响执行正确性
- 测试性能影响在可接受范围内

### 5.3 性能测试

- 对比启用/禁用日志的性能差异
- 测试不同日志级别的性能影响
- 验证内存使用情况

### 5.4 功能测试

- 运行包含冲突事务的测试场景
- 验证日志能正确记录中止和重试
- 测试复杂依赖关系的记录

## 6. 运行测试方式

### 6.1 环境准备

```bash
# 设置日志配置
export BLOCK_STM_LOG_LEVEL=DEBUG
export BLOCK_STM_LOG_DIR=./logs
export BLOCK_STM_LOG_MAX_SIZE=100MB

# 创建日志目录
mkdir -p logs
```

### 6.2 编译和运行

```bash
# 启用日志功能编译
cargo build --release --features block_stm_logging

# 运行测试
cargo test --features block_stm_logging

# 运行基准测试
cargo run --release --features block_stm_logging -- replay-erc20 --data-path test_data.csv --concurrency-level 4
```

### 6.3 日志分析

```bash
# 分析执行日志
jq '.event_type' logs/block_stm_execution.log | sort | uniq -c

# 统计中止次数
jq 'select(.event_type == "transaction_abort")' logs/block_stm_concurrency.log | wc -l

# 分析性能指标
jq 'select(.event_type == "performance_metric") | .duration_us' logs/block_stm_performance.log | awk '{sum+=$1; count++} END {print "Average:", sum/count}'
```

## 7. 预期收益

### 7.1 调试能力提升

- 精确定位并发冲突原因
- 追踪事务执行路径
- 分析性能瓶颈

### 7.2 性能优化

- 识别热点事务
- 优化调度策略
- 减少不必要的中止

### 7.3 系统监控

- 实时监控执行状态
- 预警异常情况
- 生成性能报告

## 8. 实施计划

### 阶段1: 基础框架 (1-2周)

- 实现 `BlockSTMLogger` 结构体
- 添加基本的日志记录功能
- 集成到核心执行路径

### 阶段2: 详细记录 (2-3周)

- 添加读写集跟踪
- 实现并发控制日志
- 优化性能影响

### 阶段3: 测试和优化 (1-2周)

- 完善测试用例
- 性能调优
- 文档完善

### 阶段4: 分析工具 (1周)

- 开发日志分析脚本
- 创建可视化工具
- 编写使用指南
