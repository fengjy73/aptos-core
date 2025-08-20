# How to Run with Block-STM Logger

本文档提供了使用Block-STM Logger运行Aptos交易基准测试的完整指南。

## 概述

Block-STM Logger是一个综合性的日志记录系统，用于分析Aptos区块链的并行执行性能。它提供详细的执行追踪、并发分析、性能指标和依赖关系记录。

## 环境配置

### 必需的环境变量

在运行基准测试之前，需要设置以下环境变量：

```bash
# 日志级别配置 (DEBUG/INFO/WARN/ERROR)
export BLOCK_STM_LOG_LEVEL=DEBUG

# 日志输出目录 (每次运行建议使用不同的目录名)
export BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S)

# 日志文件最大大小 (MB)
export BLOCK_STM_LOG_MAX_SIZE=100

# 可选：启用详细统计报告
export REPORT_STATS=1

# 可选：Rust日志级别
export RUST_LOG=debug
```

### 快速设置脚本

创建一个设置脚本 `setup_logging.sh`：

```bash
#!/bin/bash
export BLOCK_STM_LOG_LEVEL=DEBUG
export BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S)
export BLOCK_STM_LOG_MAX_SIZE=100
export REPORT_STATS=1
export RUST_LOG=debug

echo "Block-STM Logger environment configured:"
echo "  Log Level: $BLOCK_STM_LOG_LEVEL"
echo "  Log Directory: $BLOCK_STM_LOG_DIR"
echo "  Max Log Size: ${BLOCK_STM_LOG_MAX_SIZE}MB"
```

使用方式：
```bash
source setup_logging.sh
```

## 构建项目

确保项目已正确构建：

```bash
# 进入基准测试目录
cd aptos-move/aptos-transaction-benchmarks

# 构建发布版本
cargo build --release

# 或者构建整个项目的发布版本
cd ../../
cargo build --release -p aptos-transaction-benchmarks
cd aptos-move/aptos-transaction-benchmarks
```

## 运行基准测试

### 1. ERC20历史数据重放测试（推荐）

这是最常用的基准测试，使用真实的以太坊历史交易数据：

```bash
# 基本ERC20重放测试
cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_100000.csv \
  --concurrency-level 4 \
  --num-runs 1

# 或使用预构建的二进制文件
./target/release/aptos-transaction-benchmarks replay-erc20 \
  --data-path data/ETH_2401_100000.csv \
  --concurrency-level 4 \
  --num-runs 1
```

### 2. 不同并发级别测试

测试不同的并发级别以分析性能扩展性：

```bash
# 测试不同的核心数
for cores in 2 4 8 16; do
  echo "Testing with $cores cores..."
  BLOCK_STM_LOG_DIR=./test_logs_${cores}cores \
  cargo run --release -- replay-erc20 \
    --data-path data/ETH_2401_100000.csv \
    --concurrency-level $cores \
    --num-runs 3
  sleep 5  # 短暂休息，避免系统过载
done
```

### 3. 不同数据集测试

测试不同大小的数据集：

```bash
# 小数据集 (1000 transactions)
BLOCK_STM_LOG_DIR=./test_logs_small \
cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_1000.csv \
  --concurrency-level 4

# 中等数据集 (10000 transactions)
BLOCK_STM_LOG_DIR=./test_logs_medium \
cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_10000.csv \
  --concurrency-level 4

# 大数据集 (100000 transactions)
BLOCK_STM_LOG_DIR=./test_logs_large \
cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_100000.csv \
  --concurrency-level 4
```

### 4. 其他基准测试类型

```bash
# 空投测试
cargo run --release -- run-airdrop \
  --num-accounts 1000 \
  --concurrency-level 4

# 投票测试
cargo run --release -- run-voting \
  --num-voters 500 \
  --concurrency-level 4

# 代币交换测试
cargo run --release -- run-token-swap \
  --num-swaps 1000 \
  --concurrency-level 4
```

## 日志文件说明

Block-STM Logger会在指定目录中生成以下日志文件：

### 核心日志文件

1. **`block_stm_execution.log`** - 执行流程日志
   - 交易开始/结束事件
   - 状态转换记录
   - 执行阶段追踪

2. **`block_stm_concurrency.log`** - 并发性能日志
   - 线程调度信息
   - 任务分配记录
   - 并发冲突分析

3. **`block_stm_performance.log`** - 性能指标日志
   - TPS (Transactions Per Second)
   - Gas使用统计
   - 执行时间分析

4. **`block_stm_readwrite.log`** - 读写依赖日志
   - 读写集记录
   - 依赖关系分析
   - 冲突检测结果

5. **`block_stm_summary.log`** - 汇总报告
   - 整体执行统计
   - 性能汇总数据
   - 关键指标摘要

### 可选的详细日志文件

当使用更详细的日志配置时，还可能生成：

- **`abort_recovery.ndjson`** - 中止和恢复事件
- **`dependencies.ndjson`** - 详细依赖关系
- **`detailed_operations.ndjson`** - 详细操作记录
- **`execution_flow.ndjson`** - 执行流程详情
- **`mvhashmap_ops.ndjson`** - MVHashMap操作
- **`scheduler_states.ndjson`** - 调度器状态变化
- **`system_operations.ndjson`** - 系统级操作

## 日志分析

### 查看日志内容

```bash
# 查看日志文件列表
ls -la $BLOCK_STM_LOG_DIR/

# 查看执行日志的前20行
head -20 $BLOCK_STM_LOG_DIR/block_stm_execution.log

# 查看性能汇总
head -20 $BLOCK_STM_LOG_DIR/block_stm_summary.log

# 查看并发分析
head -20 $BLOCK_STM_LOG_DIR/block_stm_concurrency.log

# 实时监控日志 (在另一个终端运行)
tail -f $BLOCK_STM_LOG_DIR/block_stm_execution.log
```

### 日志格式

所有日志文件都使用JSON格式，便于后续分析：

```json
{
  "timestamp": "2024-08-20T15:30:45.123Z",
  "level": "DEBUG",
  "event_type": "TransactionStart",
  "transaction_index": 42,
  "incarnation": 0,
  "thread_id": 3
}
```

## 性能调优建议

### 1. 系统资源配置

```bash
# 设置合适的栈大小
export RUST_MIN_STACK=4297152

# 优化内存使用
export RUST_LOG=aptos_transaction_benchmarks=debug,aptos_block_executor=debug
```

### 2. 并发级别选择

- **CPU密集型任务**: 设置为CPU核心数
- **I/O密集型任务**: 设置为CPU核心数的1.5-2倍
- **测试环境**: 从较小的值开始（如2或4）

### 3. 数据集选择

- **开发测试**: 使用小数据集（1000-10000 transactions）
- **性能测试**: 使用中等数据集（10000-50000 transactions）
- **压力测试**: 使用大数据集（100000+ transactions）

## 故障排除

### 常见问题

1. **内存不足错误**
   ```bash
   # 增加栈大小
   export RUST_MIN_STACK=8294304
   ```

2. **日志文件过大**
   ```bash
   # 减少日志级别
   export BLOCK_STM_LOG_LEVEL=INFO
   # 或增加最大文件大小
   export BLOCK_STM_LOG_MAX_SIZE=200
   ```

3. **编译错误**
   ```bash
   # 清理并重新构建
   cargo clean
   cargo build --release
   ```

4. **权限错误**
   ```bash
   # 确保日志目录有写权限
   chmod 755 ./test_logs_*
   ```

### 调试技巧

1. **逐步增加数据规模**：从小数据集开始测试
2. **监控系统资源**：使用 `htop` 或 `top` 监控CPU和内存使用
3. **检查日志完整性**：确保所有预期的日志文件都已生成
4. **比较不同配置**：使用不同的并发级别和数据集进行对比

## 示例脚本

### 完整的测试脚本

```bash
#!/bin/bash

# 设置日志环境
export BLOCK_STM_LOG_LEVEL=DEBUG
export BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S)
export BLOCK_STM_LOG_MAX_SIZE=100
export REPORT_STATS=1
export RUST_LOG=debug
export RUST_MIN_STACK=4297152

echo "Starting Block-STM Logger benchmark test..."
echo "Log directory: $BLOCK_STM_LOG_DIR"

# 创建日志目录
mkdir -p $BLOCK_STM_LOG_DIR

# 运行基准测试
cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_100000.csv \
  --concurrency-level 4 \
  --num-runs 1 \
  --num-warmups 0

# 检查结果
echo "Test completed. Log files:"
ls -la $BLOCK_STM_LOG_DIR/

echo "Summary (first 20 lines):"
head -20 $BLOCK_STM_LOG_DIR/block_stm_summary.log || echo "No summary file found"

echo "Performance metrics (first 10 lines):"
head -10 $BLOCK_STM_LOG_DIR/block_stm_performance.log || echo "No performance file found"
```

保存为 `run_benchmark_test.sh` 并使用：
```bash
chmod +x run_benchmark_test.sh
./run_benchmark_test.sh
```

## 高级用法

### 自定义日志配置

可以通过修改 `block_stm_logger.rs` 中的配置来自定义日志行为：

```rust
// 自定义日志事件类型
pub enum CustomLogEvent {
    CustomMetric { value: u64, description: String },
    CustomState { state: String, transition: String },
}

// 在代码中记录自定义事件
if let Some(logger) = get_global_logger() {
    logger.log_custom_event(CustomLogEvent::CustomMetric {
        value: execution_time,
        description: "Custom execution time".to_string(),
    });
}
```

### 集成到CI/CD流程

```yaml
# GitHub Actions 示例
- name: Run Block-STM Logger Benchmark
  run: |
    export BLOCK_STM_LOG_LEVEL=INFO
    export BLOCK_STM_LOG_DIR=./ci_test_logs
    export BLOCK_STM_LOG_MAX_SIZE=50
    cd aptos-move/aptos-transaction-benchmarks
    cargo run --release -- replay-erc20 --data-path data/ETH_2401_1000.csv --concurrency-level 2

- name: Archive logs
  uses: actions/upload-artifact@v3
  with:
    name: block-stm-logs
    path: aptos-move/aptos-transaction-benchmarks/ci_test_logs/
```

通过本指南，您应该能够成功运行Block-STM Logger并获得详细的并行执行分析数据。如有其他问题，请参考项目文档或提交问题报告。