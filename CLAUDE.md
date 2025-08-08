# CLAUDE.md

本文件为Claude Code (claude.ai/code)在此代码仓库中工作提供指导。

## 重要提示：日志文件处理

**每次查看.log文件时，默认只读取前20行，除非特别说明需要完整读取文件。**
这是因为日志文件通常很大，只读取开头部分即可了解基本结构和内容。

## 仓库概述

Aptos Core是基于Rust和Move语言构建的第一层区块链实现。这是一个大规模项目，实现了包含共识、执行、存储和网络层的完整区块链系统。

## 常用开发命令

### 构建和测试
```bash
# 构建整个项目
cargo build --release

# 运行所有测试
cargo test

# 运行特定crate的测试
cargo test -p aptos-framework
cargo test -p aptos-block-executor

# 专门运行Move框架测试
cd aptos-move/framework && cargo test

# 跳过Move证明器测试（更快）
cargo test -- --skip prover

# 使用环境变量运行特定Move测试
TEST_FILTER="bulletproofs" cargo test -- aptos_stdlib --skip prover
REPORT_STATS=1 cargo test -- aptos_stdlib --skip prover

# 构建CLI发布版
cargo build --release -p aptos

# 运行代码检查
cargo clippy --all-targets --all-features
```

### 运行节点和服务
```bash
# 运行本地验证器节点
cargo run --bin aptos-node -- -f aptos-node/src/aptos_node.rs

# 运行CLI
cargo run --bin aptos

# 构建和运行交易基准测试
cd aptos-move/aptos-transaction-benchmarks
cargo run --bin aptos-transaction-benchmarks
```

### Move开发
```bash
# 测试Move代码
cd aptos-move/framework
cargo test -- move_framework

# 生成Move文档
aptos move document --help

# 编译Move包
aptos move compile
```

## 架构概述

### 核心组件

**1. Move执行引擎**
- `aptos-move/aptos-vm/` - 执行Move字节码的主要VM实现
- `aptos-move/block-executor/` - 使用Block-STM算法的并行交易执行
- `aptos-move/mvhashmap/` - 用于并行执行状态管理的多版本哈希表
- `aptos-move/framework/` - 核心Move模块 (stdlib, aptos-framework, aptos-token)

**2. 并行执行系统 (Block-STM)** 🔥核心创新
- **位置**: `aptos-move/block-executor/` 和 `aptos-move/mvhashmap/`
- **核心架构**:
  - **调度器 (Scheduler)**: `scheduler.rs` - 经典调度器实现，使用ArmedLock机制
  - **调度器V2 (SchedulerV2)**: `scheduler_v2.rs` - 新一代调度器，支持更精细的任务管理
  - **执行器 (Executor)**: `executor.rs` - 主要的Block-STM执行器，协调并行执行流程
  - **多版本哈希表**: `mvhashmap` crate - 支持并发读写的版本化数据结构，提供状态版本管理
  - **日志系统**: `block_stm_logger.rs` - 综合性能和并发执行日志记录

- **核心执行流程** (CSV历史数据重放链条):
  1. **CSV解析阶段**: `simulator.rs:187-250` - 读取历史交易CSV，解析为TransactionData结构
  2. **交易映射**: `simulator.rs:227-247` - 将CSV索引映射到Block-STM交易ID，建立追踪关系  
  3. **区块构造**: 通过AptosVMBlockExecutor将交易数据转换为Aptos原生交易格式
  4. **并行调度**: SchedulerV2分配交易到工作线程，管理执行状态转换
  5. **乐观执行**: 多线程并行执行交易，MVHashMap维护版本化读写集
  6. **依赖验证**: 检测读写冲突，触发必要的中止和重执行
  7. **顺序提交**: 按原始顺序提交已验证交易，确保最终状态一致性

- **关键特性**:
  - **乐观并行执行**: 投机性并行执行交易，检测到冲突时重新执行
  - **依赖跟踪**: 跟踪读写依赖关系，支持智能中止和重调度  
  - **线程安全**: 使用ArmedLock和原子操作保证并发安全
  - **动态降级**: 并行化无效时自动回退到顺序执行
  - **状态版本化**: MVHashMap为每个交易维护独立的状态版本视图
  - **智能中止管理**: AbortManager跟踪依赖关系，最小化不必要的重执行

- **核心文件**:
  - `executor.rs:82-100` - SharedSyncParams和BlockExecutor主结构定义
  - `scheduler_v2.rs:25-100` - BlockSTMv2调度器详细架构注释和任务管理
  - `mvhashmap/lib.rs:42-100` - 多版本数据结构和模块缓存实现
  - `block_stm_logger.rs:20-100` - 全方位日志配置和事件类型定义

**3. 共识层**
- `consensus/` - 基于Jolteon协议的AptosBFT共识实现
- 使用3链提交规则，具有BFT安全保证
- 组件：RoundManager, SafetyRules, BlockStore, RoundState
- 直接集成执行层处理非确定性执行

**4. 存储层**
- `storage/` - 支持多种存储后端的AptosDB实现
- 使用认证数据结构（Jellyfish Merkle Trees）进行状态管理
- `storage/aptosdb/` - 主要数据库实现
- `storage/backup/` - 备份和恢复功能
- 支持RocksDB和实验性存储后端

**5. 执行管道**
- `execution/` - 交易执行协调
- `execution/executor/` - 协调VM和存储的主执行器
- 管理execute_block和commit_block操作
- 使用稀疏Merkle树进行状态表示

**6. 网络和API**
- `network/` - P2P网络实现
- `api/` - 对外客户端交互的REST API服务器
- `mempool/` - 交易池管理
- `state-sync/` - 节点间状态同步

### 基准测试系统 📊重点关注

**位置**: `aptos-move/aptos-transaction-benchmarks/`

**核心组件**:
- **主程序**: `src/main.rs:24-42` - CLI接口，定义ReplayERC20HistoricOpt等基准测试命令
- **模拟器框架**: `src/simulator.rs:84-100` - Simulator结构，集成BlockSTMLogger和CSV数据处理
- **基准运行器**: `src/benchmark_runner.rs` - 基准测试执行引擎
- **测量工具**: `src/measurement.rs` - 性能指标收集和分析  
- **交易状态**: `src/transaction_bench_state.rs:42-50` - TransactionBenchState，管理分片执行器

**支持的基准测试类型**:
- **ERC20重放测试**: 使用真实以太坊历史数据进行性能测试，完整实现CSV数据加载链条
- **空投测试**: 批量转账性能测试
- **投票测试**: 投票合约并发性能测试  
- **NFT相关**: Kitty繁殖、Million Pixel等复杂交互测试

**已实现的模拟器功能**:
- **CSV数据处理**: `load_csv_data()` 和 `load_transactions_from_csv()` 实现完整的历史数据加载
- **交易映射追踪**: `process_transaction_mapping()` 建立CSV索引到Block-STM ID的对应关系
- **日志集成**: 集成BlockSTMLogger，支持环境变量配置的详细日志记录
- **性能指标**: DetailedExecutionMetrics提供执行次数、验证次数、中止计数等详细指标

**Block-STM日志系统**:
- **日志配置**: 支持BLOCK_STM_LOG_LEVEL/LOG_DIR/LOG_MAX_SIZE环境变量配置
- **事件类型**: LogEvent枚举定义TransactionStart/Finish、StateTransition、Abort等事件
- **日志文件**:
  - `block_stm_execution.log` - JSON格式的状态转换和执行事件日志
  - `block_stm_concurrency.log` - 线程并发和任务调度分析
  - `block_stm_performance.log` - TPS、Gas使用、执行时间等性能指标
  - `block_stm_readwrite.log` - 读写集依赖关系和冲突检测记录
  - `block_stm_summary.log` - 整体执行统计和性能汇总报告

**数据文件**:
- `data/` - 包含ETH和USDT历史交易数据用于重放测试
- `result/` - 基准测试结果输出
- `scripts/` - 各种自动化测试脚本

### 关键架构模式

**状态管理**: 系统使用版本化方法，每个交易创建新的状态版本。存储层维护Merkle树进行认证和高效状态查询。

**并行执行**: Block-STM通过跟踪读写依赖关系和乐观执行验证实现并行交易执行。这是与顺序区块链执行的关键差异化功能。

**模块化设计**: 共识（排序）、执行（Move VM）和存储（AptosDB）的清晰分离允许独立优化和测试。

**Move集成**: 与Move语言和VM的深度集成，包括自定义gas计量、原生函数和框架模块。

## 开发工作流

### 处理Move代码
- 框架代码位于 `aptos-move/framework/`
- 使用 `cargo test -p aptos-framework` 进行框架测试
- Move证明器测试可能较慢 - 使用 `--skip prover` 跳过
- 环境变量如 `TEST_FILTER` 有助于运行特定测试集

### 性能测试
- 交易基准测试位于 `aptos-move/aptos-transaction-benchmarks/`
- 使用 `REPORT_STATS=1` 获取详细的gas和时间测量
- `scripts/` 目录中提供各种基准测试脚本

### Block-STM专项测试

#### 标准ERC20重放测试命令
```bash
# 进入基准测试目录
cd aptos-move/aptos-transaction-benchmarks

# 基本ERC20历史数据重放测试
cargo run --release -- replay-erc20 --data-path data/ETH_2401_100000.csv --concurrency-level 4

# 带详细日志的完整重放测试（推荐）
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_new cargo run --release -- replay-erc20 \
  --data-path data/ETH_2401_100000.csv --concurrency-level 4 --num-warmups 0 --num-runs 1

# 多线程性能对比测试
for cores in 2 4 8 16; do
  BLOCK_STM_LOG_DIR=./test_logs_${cores}cores cargo run --release -- replay-erc20 \
    --data-path data/ETH_2401_100000.csv --concurrency-level $cores --num-runs 3
done

# 运行传统脚本方式
./scripts/run_eth_historical.sh
```

#### 日志分析和数据查看
```bash
# 查看Block-STM执行日志（只读前20行）
head -20 test_logs_stage7/block_stm_execution.log

# 查看并发性能日志
head -20 test_logs_stage7/block_stm_concurrency.log

# 查看读写依赖日志  
head -20 test_logs_stage7/block_stm_readwrite.log

# 查看性能汇总
head -20 test_logs_stage7/block_stm_summary.log

# 检查日志目录结构
ls -la test_logs_*/
```

#### 环境变量配置
```bash
# 必要的日志环境变量
export BLOCK_STM_LOG_LEVEL=DEBUG        # DEBUG/INFO/WARN/ERROR
export BLOCK_STM_LOG_DIR=./test_logs_new # 每次执行更改目录名避免覆盖
export BLOCK_STM_LOG_MAX_SIZE=100       # 日志文件最大大小(MB)

# 可选的性能环境变量  
export REPORT_STATS=1                   # 启用详细统计报告
export RUST_LOG=debug                   # Rust日志级别
```

### 配置
- 节点配置基于YAML，具有广泛选项
- 关键配置部分：execution, storage, consensus, network
- 默认值通常有效 - 仅在必要时覆盖

### 测试策略
- crate级别的单元测试
- 专用测试crate中的集成测试
- 框架功能的Move测试
- 性能验证的基准测试
- 使用proptest的基于属性的测试

## 重要文件位置

- **主节点二进制**: `aptos-node/src/main.rs`
- **CLI实现**: `crates/aptos/`
- **Move VM**: `aptos-move/aptos-vm/src/aptos_vm.rs`
- **Block执行器**: `aptos-move/block-executor/src/executor.rs`
- **Block-STM调度器**: `aptos-move/block-executor/src/scheduler_v2.rs`
- **多版本哈希表**: `aptos-move/mvhashmap/src/lib.rs`
- **基准测试模拟器**: `aptos-move/aptos-transaction-benchmarks/src/simulator.rs`
- **共识**: `consensus/src/consensus_provider.rs`
- **存储**: `storage/aptosdb/src/lib.rs`
- **框架模块**: `aptos-move/framework/aptos-framework/sources/`

## 常见问题

- **开发构建中的栈溢出**: 设置 `RUST_MIN_STACK=4297152` 或使用 `cargo test --release`
- **Move证明器超时**: 使用 `--skip prover` 标志进行更快测试
- **大型编译时间**: 考虑使用 `cargo check` 进行语法验证
- **并行测试不稳定**: 某些测试可能需要顺序执行
- **日志文件过大**: 默认只读取前20行，使用head命令查看日志开头

此仓库需要大量系统资源进行完整编译和测试。模块化架构允许单独处理各个组件，但理解共识、执行和存储之间的交互对有效开发至关重要。