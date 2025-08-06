# Block-STM 日志记录使用指南

本指南详细介绍如何使用Block-STM的日志记录功能来监控、调试和优化并行执行性能。

## 目录

1. [快速开始](#快速开始)
2. [配置选项](#配置选项)
3. [日志文件结构](#日志文件结构)
4. [使用示例](#使用示例)
5. [日志分析](#日志分析)
6. [性能优化](#性能优化)
7. [故障排除](#故障排除)
8. [最佳实践](#最佳实践)

## 快速开始

### 1. 启用日志记录

通过环境变量启用Block-STM日志记录：

```bash
# 基本配置
export BLOCK_STM_LOG_DIR="./logs"
export BLOCK_STM_LOG_LEVEL="INFO"
export BLOCK_STM_LOG_DETAILED="true"

# 运行你的应用
cargo run --example block_stm_logging_example
```

### 2. 使用演示脚本

我们提供了一个完整的演示脚本：

```bash
# 进入block-executor目录
cd aptos-move/block-executor

# 运行演示
./run_logging_demo.sh

# 或者使用自定义参数
./run_logging_demo.sh -d /tmp/block_stm_logs -l DEBUG -t 100
```

## 配置选项

### 环境变量配置

| 环境变量 | 默认值 | 描述 |
|---------|--------|------|
| `BLOCK_STM_LOG_DIR` | `./block_stm_logs` | 日志文件存储目录 |
| `BLOCK_STM_LOG_LEVEL` | `INFO` | 日志级别 (DEBUG/INFO/WARN/ERROR) |
| `BLOCK_STM_LOG_MAX_SIZE` | `10` | 单个日志文件最大大小(MB) |
| `BLOCK_STM_LOG_DETAILED` | `true` | 是否记录详细的读写集信息 |

### 代码配置

```rust
use aptos_block_executor::block_stm_logger::{init_global_logger, LoggingConfig, LogLevel};

let config = LoggingConfig {
    enabled: true,
    log_dir: PathBuf::from("./logs"),
    log_level: LogLevel::Info,
    max_file_size: 10 * 1024 * 1024, // 10MB
    buffer_size: 8192,
    async_logging: true,
    include_read_write_details: true,
};

init_global_logger(config)?;
```

## 日志文件结构

Block-STM日志记录系统会创建以下日志文件：

### 1. `block_stm_execution.log`
记录事务执行的生命周期事件：
- 事务开始执行
- 事务执行完成
- 执行结果和性能指标

```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "event_type": "TransactionStart",
  "txn_idx": 42,
  "incarnation": 0,
  "thread_id": 3
}
```

### 2. `block_stm_concurrency.log`
记录并发控制相关事件：
- 事务中止
- 依赖关系
- 冲突检测

```json
{
  "timestamp": "2024-01-15T10:30:45.456Z",
  "event_type": "TransactionAbort",
  "txn_idx": 42,
  "incarnation": 1,
  "reason": "Read-write conflict",
  "retry_count": 2,
  "dependencies": [40, 41]
}
```

### 3. `block_stm_readwrite.log`
记录读写集变化：
- 读写集大小
- 访问的键值
- 资源类型统计

### 4. `block_stm_performance.log`
记录性能指标：
- 执行时间
- 内存使用
- 缓存命中率

### 5. `block_stm_summary.log`
记录汇总信息：
- 块级别统计
- 整体性能指标

## 使用示例

### 示例1：基本监控

```bash
# 启用基本日志记录
export BLOCK_STM_LOG_DIR="./monitoring_logs"
export BLOCK_STM_LOG_LEVEL="INFO"

# 运行你的区块链应用
cargo run --bin your_blockchain_app

# 实时监控执行日志
tail -f ./monitoring_logs/block_stm_execution.log
```

### 示例2：性能调试

```bash
# 启用详细调试日志
export BLOCK_STM_LOG_LEVEL="DEBUG"
export BLOCK_STM_LOG_DETAILED="true"

# 运行测试
cargo test --test parallel_execution_test

# 分析性能瓶颈
jq '.duration_us' ./block_stm_logs/block_stm_execution.log | \
  awk '{sum+=$1; count++} END {print "Average execution time: " sum/count " μs"}'
```

### 示例3：冲突分析

```bash
# 分析事务冲突模式
jq 'select(.event_type == "TransactionAbort") | .reason' \
  ./block_stm_logs/block_stm_concurrency.log | \
  sort | uniq -c | sort -nr
```

## 日志分析

### 使用jq进行分析

#### 1. 基本统计

```bash
# 统计事务执行结果
jq -r 'select(.event_type == "TransactionFinish") | .result' \
  block_stm_execution.log | sort | uniq -c

# 计算平均执行时间
jq -r 'select(.event_type == "TransactionFinish") | .duration_us' \
  block_stm_execution.log | \
  awk '{sum+=$1; count++} END {print "Average: " sum/count " μs"}'

# 统计中止原因
jq -r 'select(.event_type == "TransactionAbort") | .reason' \
  block_stm_concurrency.log | sort | uniq -c
```

#### 2. 性能分析

```bash
# 找出执行时间最长的事务
jq -r 'select(.event_type == "TransactionFinish") | 
  "\(.txn_idx): \(.duration_us) μs"' \
  block_stm_execution.log | sort -k2 -nr | head -10

# 分析重试模式
jq 'select(.event_type == "TransactionAbort") | 
  {txn_idx, retry_count}' \
  block_stm_concurrency.log | \
  jq -s 'group_by(.txn_idx) | 
    map({txn_idx: .[0].txn_idx, max_retries: map(.retry_count) | max})'
```

#### 3. 并发分析

```bash
# 分析线程利用率
jq -r 'select(.event_type == "TransactionStart") | .thread_id' \
  block_stm_execution.log | sort | uniq -c

# 分析依赖关系
jq 'select(.event_type == "TransactionAbort" and .dependencies != null) | 
  {txn_idx, dependencies}' \
  block_stm_concurrency.log
```

### 使用Python进行高级分析

```python
import json
import pandas as pd
import matplotlib.pyplot as plt

# 读取执行日志
execution_events = []
with open('block_stm_execution.log', 'r') as f:
    for line in f:
        execution_events.append(json.loads(line))

df = pd.DataFrame(execution_events)

# 分析执行时间分布
finish_events = df[df['event_type'] == 'TransactionFinish']
plt.hist(finish_events['duration_us'], bins=50)
plt.xlabel('Execution Time (μs)')
plt.ylabel('Frequency')
plt.title('Transaction Execution Time Distribution')
plt.show()

# 分析成功率
success_rate = len(finish_events[finish_events['result'] == 'Success']) / len(finish_events)
print(f"Transaction Success Rate: {success_rate:.2%}")
```

## 性能优化

### 1. 基于日志的优化策略

#### 识别热点事务
```bash
# 找出经常中止的事务
jq 'select(.event_type == "TransactionAbort") | .txn_idx' \
  block_stm_concurrency.log | sort | uniq -c | sort -nr | head -10
```

#### 分析依赖模式
```bash
# 分析依赖链长度
jq 'select(.event_type == "TransactionAbort" and .dependencies != null) | 
  .dependencies | length' \
  block_stm_concurrency.log | \
  awk '{sum+=$1; count++} END {print "Average dependency count: " sum/count}'
```

### 2. 配置调优

基于日志分析结果调整配置：

```rust
// 如果发现频繁的内存分配
BlockExecutorConfig {
    local: BlockExecutorLocalConfig {
        concurrency_level: 8, // 根据CPU核心数调整
        allow_fallback: true,
        discard_failed_blocks: false,
    },
    // 其他配置...
}
```

## 故障排除

### 常见问题

#### 1. 日志文件未生成

**问题**：设置了环境变量但没有生成日志文件

**解决方案**：
```bash
# 检查目录权限
ls -la $BLOCK_STM_LOG_DIR

# 确保目录存在
mkdir -p $BLOCK_STM_LOG_DIR

# 检查环境变量
echo $BLOCK_STM_LOG_DIR
echo $BLOCK_STM_LOG_LEVEL
```

#### 2. 日志文件过大

**问题**：日志文件增长过快

**解决方案**：
```bash
# 减小最大文件大小
export BLOCK_STM_LOG_MAX_SIZE="5"  # 5MB

# 提高日志级别
export BLOCK_STM_LOG_LEVEL="WARN"

# 禁用详细信息
export BLOCK_STM_LOG_DETAILED="false"
```

#### 3. 性能影响

**问题**：日志记录影响执行性能

**解决方案**：
```bash
# 启用异步日志记录（默认启用）
# 增加缓冲区大小
# 在代码中设置更大的buffer_size

# 或者在生产环境中禁用详细日志
export BLOCK_STM_LOG_LEVEL="ERROR"
export BLOCK_STM_LOG_DETAILED="false"
```

### 调试技巧

#### 1. 实时监控

```bash
# 实时查看执行事件
tail -f block_stm_execution.log | jq 'select(.event_type == "TransactionFinish")'

# 监控中止事件
tail -f block_stm_concurrency.log | jq 'select(.event_type == "TransactionAbort")'
```

#### 2. 过滤特定事务

```bash
# 跟踪特定事务的完整生命周期
TXN_ID=42
jq "select(.txn_idx == $TXN_ID)" block_stm_*.log | sort
```

## 最佳实践

### 1. 生产环境配置

```bash
# 生产环境推荐配置
export BLOCK_STM_LOG_LEVEL="WARN"          # 只记录警告和错误
export BLOCK_STM_LOG_DETAILED="false"      # 禁用详细信息
export BLOCK_STM_LOG_MAX_SIZE="50"         # 较大的文件大小
```

### 2. 开发环境配置

```bash
# 开发环境推荐配置
export BLOCK_STM_LOG_LEVEL="DEBUG"         # 详细调试信息
export BLOCK_STM_LOG_DETAILED="true"       # 启用详细信息
export BLOCK_STM_LOG_MAX_SIZE="10"         # 较小的文件大小便于分析
```

### 3. 测试环境配置

```bash
# 测试环境推荐配置
export BLOCK_STM_LOG_LEVEL="INFO"          # 平衡的信息量
export BLOCK_STM_LOG_DETAILED="true"       # 启用详细信息用于分析
export BLOCK_STM_LOG_MAX_SIZE="20"         # 中等文件大小
```

### 4. 日志轮转

建议使用logrotate或类似工具管理日志文件：

```bash
# /etc/logrotate.d/block-stm
/path/to/block_stm_logs/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
}
```

### 5. 监控脚本

创建监控脚本定期检查日志：

```bash
#!/bin/bash
# monitor_block_stm.sh

LOG_DIR="./block_stm_logs"
ALERT_THRESHOLD=10  # 中止率阈值（%）

# 计算最近1小时的中止率
recent_aborts=$(jq 'select(.event_type == "TransactionAbort" and 
  (.timestamp | fromdateiso8601) > (now - 3600))' \
  $LOG_DIR/block_stm_concurrency.log | wc -l)

recent_finishes=$(jq 'select(.event_type == "TransactionFinish" and 
  (.timestamp | fromdateiso8601) > (now - 3600))' \
  $LOG_DIR/block_stm_execution.log | wc -l)

if [ $recent_finishes -gt 0 ]; then
    abort_rate=$((recent_aborts * 100 / recent_finishes))
    if [ $abort_rate -gt $ALERT_THRESHOLD ]; then
        echo "ALERT: High abort rate detected: ${abort_rate}%"
    fi
fi
```

## 总结

Block-STM日志记录功能提供了强大的监控和调试能力。通过合理配置和分析日志，你可以：

1. **监控系统健康状况**：实时跟踪事务执行状态
2. **识别性能瓶颈**：分析执行时间和资源使用
3. **优化并发策略**：理解冲突模式和依赖关系
4. **调试问题**：快速定位和解决执行问题

记住在生产环境中适当调整日志级别以平衡监控需求和性能影响。

## 相关文件

- `block_stm_logger.rs` - 日志记录核心实现
- `block_stm_logging_example.rs` - 使用示例
- `run_logging_demo.sh` - 演示脚本
- `block_stm_logger_test.rs` - 测试用例
- `block_stm_logging_plan.md` - 详细设计文档