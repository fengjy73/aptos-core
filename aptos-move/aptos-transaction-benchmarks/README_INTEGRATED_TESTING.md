# Block-STM 历史转账重放综合测试方案

本项目提供了一套完整的 Block-STM 历史转账重放综合测试方案，集成了批量性能测试与细粒度日志收集功能。

## 🎯 项目概述

### 核心功能
- **批量性能测试**: 基于历史交易数据的多规模、多核心配置性能测试
- **细粒度日志收集**: 详细的 Block-STM 执行、并发、性能和读写集合日志
- **自动化分析**: 智能的测试结果分析和摘要报告生成
- **多场景支持**: 基础、详细、压力等多种测试场景

### 技术特点
- 🚀 **高性能**: 支持大规模历史交易数据重放
- 📊 **深度分析**: 提供执行时间、并发冲突、性能指标等多维度分析
- 🔧 **易于使用**: 一键式测试脚本，支持多种配置选项
- 📈 **可视化**: 生成详细的测试报告和性能摘要

## 📁 文件结构

```
aptos-transaction-benchmarks/
├── run_integrated_historical_test.sh    # 主要集成测试脚本
├── quick_start_test.sh                  # 快速开始验证脚本
├── INTEGRATED_TESTING_GUIDE.md          # 详细使用指南
├── README_INTEGRATED_TESTING.md         # 本文件
├── scripts/
│   ├── run_data_window_historical.sh    # 原始批量测试脚本
│   └── cpu_info.sh                      # CPU 信息获取脚本
├── data/                                # 历史交易数据目录
│   ├── ETH_2401_100000.csv
│   ├── USDT_240101_240331_data_100000.csv
│   └── ...
├── result/                              # 测试结果目录
├── block_stm_logs/                      # Block-STM 详细日志目录
└── temp_data/                           # 临时数据目录
```

## 🚀 快速开始

### 1. 环境验证

首先运行快速验证脚本确保环境配置正确：

```bash
# 切换到项目目录
cd /path/to/aptos-core/aptos-move/aptos-transaction-benchmarks

# 运行环境验证
./quick_start_test.sh
```

这个脚本会：
- ✅ 检查必要的依赖项 (cargo, jq, bc)
- ✅ 验证数据文件存在
- ✅ 测试项目编译
- ✅ 运行快速验证测试
- ✅ 提供下一步操作指南

### 2. 运行基础测试

环境验证通过后，运行基础集成测试：

```bash
# 基础测试 (推荐首次使用)
./run_integrated_historical_test.sh

# 清理之前的结果并运行
./run_integrated_historical_test.sh -c
```

### 3. 查看结果

测试完成后查看结果：

```bash
# 查看测试结果文件
ls -la result/integrated_*.log

# 查看摘要报告
cat result/test_summary_*.md

# 查看 Block-STM 详细日志
ls -la block_stm_logs/*.log
```

## 📋 测试场景

### 🔰 Basic 场景
```bash
./run_integrated_historical_test.sh -s basic
```
- **交易数量**: 10, 100, 1000
- **日志级别**: INFO
- **适用于**: 快速性能评估和功能验证
- **执行时间**: ~5-10 分钟

### 🔍 Detailed 场景
```bash
./run_integrated_historical_test.sh -s detailed -l DEBUG
```
- **交易数量**: 100, 1000, 10000
- **日志级别**: DEBUG
- **适用于**: 详细的并发行为分析和冲突模式研究
- **执行时间**: ~15-30 分钟

### 💪 Stress 场景
```bash
./run_integrated_historical_test.sh -s stress
```
- **交易数量**: 1000, 10000, 100000
- **日志级别**: INFO
- **适用于**: 系统压力测试和大规模性能评估
- **执行时间**: ~30-60 分钟

## 📊 输出结果说明

### 测试结果文件

每个测试会生成以下文件：

#### 1. 集成测试日志 (`result/integrated_*.log`)
包含：
- 📋 测试元信息 (场景、交易数量、时间戳)
- 🖥️ 多核心配置测试结果
- 📈 Block-STM 详细日志分析
- 🎯 性能摘要 (TPS、加速比)

#### 2. Block-STM 详细日志 (`block_stm_logs/*.log`)
- `block_stm_execution.log`: 交易执行事件
- `block_stm_concurrency.log`: 并发控制事件
- `block_stm_performance.log`: 性能指标
- `block_stm_readwrite.log`: 读写集合信息
- `block_stm_summary.log`: 高级摘要事件

#### 3. 摘要报告 (`result/test_summary_*.md`)
Markdown 格式的测试摘要，包含关键指标和分析建议。

### 关键性能指标

| 指标 | 说明 | 重要性 |
|------|------|--------|
| **TPS** | 每秒处理交易数 | 🔥 衡量系统吞吐量 |
| **加速比** | 并行相对单核的性能提升 | 🔥 衡量并行效率 |
| **中止率** | 交易中止次数/总交易数 | 🔥 衡量冲突程度 |
| **平均执行时间** | 单个交易平均处理时间 | ⭐ 衡量响应性能 |
| **冲突模式** | 读写冲突的分布情况 | ⭐ 指导优化方向 |

## 🔧 高级用法

### 自定义配置

```bash
# 使用自定义数据和结果目录
./run_integrated_historical_test.sh -d /custom/data -r /custom/results

# 详细测试 + DEBUG 日志
./run_integrated_historical_test.sh -s detailed -l DEBUG

# 仅分析现有结果
./run_integrated_historical_test.sh -a
```

### 手动日志分析

```bash
# 使用 jq 分析执行日志
jq 'select(.event_type == "TransactionStart")' block_stm_logs/block_stm_execution.log | wc -l

# 计算平均执行时间
jq -r 'select(.event_type == "TransactionFinish") | .duration_us' block_stm_logs/block_stm_execution.log | awk '{sum+=$1; count++} END {print "Average: " sum/count " microseconds"}'

# 分析中止原因
jq -r 'select(.event_type == "TransactionAbort") | .reason' block_stm_logs/block_stm_concurrency.log | sort | uniq -c
```

## 🛠️ 故障排除

### 常见问题及解决方案

#### 1. 编译失败
```bash
# 清理并重新构建
cargo clean
cargo build --release
```

#### 2. 数据文件问题
```bash
# 检查数据文件格式
head -5 data/your_file.csv

# 确保文件权限正确
chmod 644 data/*.csv
```

#### 3. 权限问题
```bash
# 确保脚本可执行
chmod +x *.sh

# 确保目录可写
chmod 755 result block_stm_logs
```

#### 4. 依赖缺失
```bash
# macOS
brew install jq bc

# Ubuntu/Debian
sudo apt-get install jq bc
```

### 调试模式

```bash
# 启用详细日志
./run_integrated_historical_test.sh -s basic -l DEBUG

# 查看详细错误信息
tail -f result/integrated_*.log
```

## 📈 性能优化建议

### 基于测试结果的优化方向

1. **并发控制优化**
   - 分析中止率和冲突模式
   - 调整调度策略和冲突检测算法

2. **内存管理优化**
   - 基于读写集合大小优化内存分配
   - 减少不必要的内存拷贝

3. **负载均衡优化**
   - 根据核心利用率调整工作分配
   - 优化任务调度算法

4. **系统配置优化**
   - 调整 CPU 亲和性设置
   - 优化内存和缓存配置

## 🔗 相关资源

### 文档链接
- 📖 [详细使用指南](INTEGRATED_TESTING_GUIDE.md)
- 📋 [综合测试规划](../block-executor/COMPREHENSIVE_TESTING_PLAN.md)
- 📝 [Block-STM 日志指南](../block-executor/BLOCK_STM_LOGGING_GUIDE.md)

### 脚本文件
- 🚀 [主测试脚本](run_integrated_historical_test.sh)
- ⚡ [快速验证脚本](quick_start_test.sh)
- 📊 [原始批量测试](scripts/run_data_window_historical.sh)
- 🔍 [日志演示脚本](../block-executor/run_logging_demo.sh)

## 🤝 贡献指南

### 扩展测试场景

1. 在 `run_integrated_historical_test.sh` 中添加新场景
2. 定义相应的交易数量和配置
3. 更新文档说明

### 添加新的分析功能

1. 扩展 `analyze_block_stm_logs` 函数
2. 添加新的 jq 查询语句
3. 更新摘要报告格式

### 集成其他工具

1. 添加性能分析器集成
2. 支持可视化工具
3. 扩展报告格式

## 📞 支持与反馈

如果您在使用过程中遇到问题或有改进建议，请：

1. 📖 首先查阅 [详细使用指南](INTEGRATED_TESTING_GUIDE.md)
2. 🔍 检查 [故障排除](#🛠️-故障排除) 部分
3. 🚀 运行 [快速验证脚本](quick_start_test.sh) 检查环境
4. 📝 提交问题报告时请包含详细的错误日志

## 📄 许可证

本项目遵循 Apache 2.0 许可证。详情请参阅项目根目录的 LICENSE 文件。

---

**🎉 开始您的 Block-STM 性能测试之旅！**

```bash
# 一键开始
./quick_start_test.sh && ./run_integrated_historical_test.sh
```