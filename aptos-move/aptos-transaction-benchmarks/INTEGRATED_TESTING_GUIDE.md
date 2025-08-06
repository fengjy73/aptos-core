# Block-STM 历史转账重放综合测试指南

本指南介绍如何使用 `run_integrated_historical_test.sh` 脚本进行 Block-STM 的历史转账重放综合测试。该脚本集成了批量性能测试与细粒度日志收集功能。

## 概述

`run_integrated_historical_test.sh` 脚本结合了以下功能：
- **批量性能测试**: 基于 `run_data_window_historical.sh` 的多交易数量、多核心配置测试
- **细粒度日志收集**: 集成 Block-STM 的详细日志功能，包括执行、并发、性能和读写集合日志
- **自动化分析**: 提供测试结果的自动分析和摘要报告生成

## 前置要求

### 系统依赖
- **Rust 和 Cargo**: 用于编译和运行 Block-STM 代码
- **jq**: 用于高级 JSON 日志分析（可选，但推荐）
- **bc**: 用于计算加速比
- **标准 Unix 工具**: grep, awk, sort, head, tail 等

### 安装依赖

**macOS**:
```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装其他工具
brew install jq bc
```

**Ubuntu/Debian**:
```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装其他工具
sudo apt-get update
sudo apt-get install jq bc
```

### 数据准备
确保在 `./data` 目录下有历史交易数据的 CSV 文件。脚本会自动处理该目录下的所有 `.csv` 文件。

## 使用方法

### 基本用法

```bash
# 切换到 aptos-transaction-benchmarks 目录
cd /path/to/aptos-core/aptos-move/aptos-transaction-benchmarks

# 运行基本测试
./run_integrated_historical_test.sh
```

### 命令行选项

```bash
./run_integrated_historical_test.sh [OPTIONS]
```

**选项说明**:
- `-s, --scenario SCENARIO`: 测试场景 (basic|detailed|stress，默认: basic)
- `-l, --log-level LEVEL`: 日志级别 (DEBUG|INFO|WARN|ERROR，默认: INFO)
- `-d, --data-dir DIR`: 数据目录 (默认: ./data)
- `-r, --result-dir DIR`: 结果目录 (默认: ./result)
- `-c, --clean`: 运行前清理现有日志和结果
- `-a, --analyze-only`: 仅分析现有结果，不运行测试
- `-h, --help`: 显示帮助信息

### 测试场景

#### 1. Basic 场景
```bash
./run_integrated_historical_test.sh -s basic
```
- **交易数量**: 10, 100, 1000
- **日志级别**: INFO
- **适用于**: 快速性能评估和基本功能验证

#### 2. Detailed 场景
```bash
./run_integrated_historical_test.sh -s detailed -l DEBUG
```
- **交易数量**: 100, 1000, 10000
- **日志级别**: DEBUG（推荐）
- **适用于**: 详细的并发行为分析和冲突模式研究

#### 3. Stress 场景
```bash
./run_integrated_historical_test.sh -s stress -l INFO
```
- **交易数量**: 1000, 10000, 100000
- **日志级别**: INFO
- **适用于**: 系统压力测试和大规模性能评估

### 高级用法示例

```bash
# 清理之前的结果并运行详细测试
./run_integrated_historical_test.sh -c -s detailed -l DEBUG

# 使用自定义目录
./run_integrated_historical_test.sh -d /custom/data -r /custom/results

# 仅分析现有结果
./run_integrated_historical_test.sh -a
```

## 输出结果

### 目录结构

测试完成后，会生成以下目录结构：

```
aptos-transaction-benchmarks/
├── result/                              # 测试结果目录
│   ├── integrated_basic_ETH_2401_10.log      # 集成测试日志
│   ├── integrated_basic_ETH_2401_100.log
│   ├── integrated_basic_ETH_2401_1000.log
│   └── test_summary_YYYYMMDD_HHMMSS.md       # 摘要报告
├── block_stm_logs/                      # Block-STM 详细日志
│   ├── block_stm_execution.log              # 执行事件日志
│   ├── block_stm_concurrency.log            # 并发事件日志
│   ├── block_stm_performance.log            # 性能指标日志
│   ├── block_stm_readwrite.log              # 读写集合日志
│   └── block_stm_summary.log                # 摘要事件日志
└── temp_data/                           # 临时采样数据（测试后清理）
```

### 结果文件内容

#### 集成测试日志 (integrated_*.log)
每个日志文件包含：
1. **测试元信息**: 场景、交易数量、日志级别、时间戳
2. **多核心配置测试结果**: 不同 CPU 核心数下的性能数据
3. **Block-STM 详细日志分析**: 执行统计、并发统计、性能指标
4. **性能摘要**: TPS、加速比等关键指标

#### Block-STM 日志文件
- **block_stm_execution.log**: 交易执行事件（开始、完成、执行时间）
- **block_stm_concurrency.log**: 并发控制事件（中止、重试、冲突）
- **block_stm_performance.log**: 性能指标（吞吐量、延迟、资源使用）
- **block_stm_readwrite.log**: 读写集合信息（访问模式、冲突检测）
- **block_stm_summary.log**: 高级摘要事件

#### 摘要报告 (test_summary_*.md)
Markdown 格式的测试摘要，包含：
- 测试概览
- 关键性能指标
- 分析建议

## 日志分析

### 自动分析

脚本会自动分析 Block-STM 日志并在结果文件中包含：

**执行统计**:
- 交易开始/完成数量
- 平均执行时间
- 执行结果分布

**并发统计**:
- 交易中止次数
- 中止原因分布
- 冲突模式分析

**性能指标**:
- 各类性能事件计数
- 指标类型分布

### 手动分析命令

脚本完成后会提供手动分析命令：

```bash
# 查看所有结果文件
ls -la ./result/integrated_*.log

# 分析特定结果
grep -A 10 '性能摘要' ./result/integrated_*.log

# 查看 Block-STM 日志
ls -la ./block_stm_logs/*.log

# 使用 jq 进行高级分析
jq 'select(.event_type == "TransactionStart")' ./block_stm_logs/block_stm_execution.log | wc -l

# 计算平均执行时间
jq -r 'select(.event_type == "TransactionFinish") | .duration_us' ./block_stm_logs/block_stm_execution.log | awk '{sum+=$1; count++} END {print "Average: " sum/count " microseconds"}'

# 分析中止原因
jq -r 'select(.event_type == "TransactionAbort") | .reason' ./block_stm_logs/block_stm_concurrency.log | sort | uniq -c
```

## 性能分析指南

### 关键指标

1. **TPS (Transactions Per Second)**: 衡量系统吞吐量
2. **加速比**: 并行执行相对于单核的性能提升
3. **中止率**: 交易中止次数 / 总交易数
4. **平均执行时间**: 单个交易的平均处理时间

### 分析维度

1. **扩展性分析**: 比较不同核心数下的性能表现
2. **负载分析**: 比较不同交易数量下的系统行为
3. **冲突分析**: 分析读写冲突模式和中止原因
4. **资源利用**: 分析 CPU、内存等资源使用情况

### 优化建议

基于测试结果，可以从以下方面进行优化：

1. **并发控制策略**: 根据中止率和冲突模式调整
2. **调度算法**: 基于执行时间分布优化任务调度
3. **内存管理**: 根据读写集合大小优化内存分配
4. **负载均衡**: 基于核心利用率调整工作分配

## 故障排除

### 常见问题

1. **编译失败**
   ```bash
   # 确保在正确目录
   cd aptos-transaction-benchmarks
   
   # 清理并重新构建
   cargo clean
   cargo build --release
   ```

2. **数据文件不存在**
   ```bash
   # 检查数据目录
   ls -la ./data/*.csv
   
   # 确保 CSV 文件格式正确
   head -5 ./data/your_file.csv
   ```

3. **权限问题**
   ```bash
   # 确保脚本有执行权限
   chmod +x ./run_integrated_historical_test.sh
   
   # 确保目录可写
   chmod 755 ./result ./block_stm_logs
   ```

4. **依赖缺失**
   ```bash
   # 检查依赖
   which cargo jq bc
   
   # 安装缺失的依赖
   # macOS: brew install jq bc
   # Ubuntu: sudo apt-get install jq bc
   ```

### 调试模式

如果遇到问题，可以启用详细日志：

```bash
# 使用 DEBUG 级别日志
./run_integrated_historical_test.sh -s basic -l DEBUG

# 查看详细的 Block-STM 日志
cat ./block_stm_logs/block_stm_execution.log
```

## 最佳实践

1. **测试前准备**:
   - 确保系统资源充足
   - 关闭不必要的后台程序
   - 使用 `-c` 选项清理之前的结果

2. **场景选择**:
   - 开发阶段使用 `basic` 场景快速验证
   - 性能调优使用 `detailed` 场景深入分析
   - 发布前使用 `stress` 场景压力测试

3. **结果分析**:
   - 重点关注加速比和中止率
   - 对比不同交易数量下的性能表现
   - 分析冲突模式以指导优化

4. **持续监控**:
   - 定期运行测试以监控性能回归
   - 保存历史测试结果用于趋势分析
   - 建立性能基准和告警机制

## 扩展和定制

### 添加新的测试场景

在脚本中的 `test_scenarios` 数组中添加新场景：

```bash
test_scenarios=("basic" "detailed" "stress" "custom")
```

然后在 `run_test_scenario` 函数中添加对应的配置。

### 自定义日志分析

可以扩展 `analyze_block_stm_logs` 函数以添加特定的分析逻辑：

```bash
# 添加自定义分析
if [ -f "$custom_log" ]; then
    echo "自定义分析:" >> "$log_filename"
    # 添加分析代码
fi
```

### 集成其他工具

可以在脚本中集成其他分析工具，如性能分析器、可视化工具等。

## 相关文档

- [COMPREHENSIVE_TESTING_PLAN.md](../block-executor/COMPREHENSIVE_TESTING_PLAN.md): 综合测试规划
- [BLOCK_STM_LOGGING_GUIDE.md](../block-executor/BLOCK_STM_LOGGING_GUIDE.md): Block-STM 日志指南
- [run_logging_demo.sh](../block-executor/run_logging_demo.sh): 日志演示脚本
- [run_data_window_historical.sh](./scripts/run_data_window_historical.sh): 原始批量测试脚本

## 联系和支持

如有问题或建议，请参考相关文档或联系开发团队。