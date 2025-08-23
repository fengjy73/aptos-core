# Block-STM Logger 基础功能文档

## 概述

Block-STM Logger 是一个综合性的日志记录系统，用于详细跟踪和分析 Aptos Block-STM 并行执行引擎的运行状态。该系统能够记录交易执行、并发控制、读写依赖、性能指标等关键信息，为性能分析和调试提供强大支持。

## 测试命令

### 基本命令格式
```bash
BLOCK_STM_LOG_LEVEL=<级别> BLOCK_STM_LOG_DIR=<目录> cargo run --release -- replay-erc20 --data-path <数据文件> --concurrency-level <并发数> --num-runs <运行次数>
```

### 环境变量配置

| 环境变量 | 说明 | 可选值 | 默认值 |
|---------|------|--------|--------|
| `BLOCK_STM_LOG_LEVEL` | 日志级别 | DEBUG, INFO, WARN, ERROR | INFO |
| `BLOCK_STM_LOG_DIR` | 日志输出目录 | 任何有效路径 | ./block_stm_logs |
| `BLOCK_STM_LOG_MAX_SIZE` | 单个日志文件最大大小(MB) | 正整数 | 100 |

### 常用测试命令示例

#### 1. 并行执行测试（推荐）
```bash
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_parallel cargo run --release -- replay-erc20 --data-path data/ETH_2401_100.csv --concurrency-level 4 --num-runs 1
```

#### 2. 串行执行测试
```bash
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_sequential cargo run --release -- replay-erc20 --data-path data/ETH_2401_100.csv --concurrency-level 1 --num-runs 1
```

#### 3. 大数据集测试
```bash
BLOCK_STM_LOG_LEVEL=INFO BLOCK_STM_LOG_DIR=./test_logs_large cargo run --release -- replay-erc20 --data-path data/ETH_2401_100000.csv --concurrency-level 8 --num-runs 3
```

#### 4. 性能对比测试
```bash
# 并行测试
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_par cargo run --release -- replay-erc20 --data-path data/ETH_2401_100.csv --concurrency-level 4 --num-runs 1

# 串行测试  
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_seq cargo run --release -- replay-erc20 --data-path data/ETH_2401_100.csv --concurrency-level 1 --num-runs 1
```

### 命令参数说明

| 参数 | 说明 | 示例 |
|------|------|------|
| `--data-path` | CSV历史数据文件路径 | `data/ETH_2401_100.csv` |
| `--concurrency-level` | 并发级别（1=串行，>1=并行） | `4` |
| `--num-runs` | 基准测试运行次数 | `1` |
| `--skip-parallel` | 跳过并行执行（可选） | - |
| `--skip-sequential` | 跳过串行执行（可选） | - |

## 测试流程

### 1. 数据加载阶段
- 读取CSV历史交易数据
- 解析交易信息（from, to, value等）
- 生成交易映射关系
- 创建签名验证的交易对象

### 2. 日志系统初始化
- 根据环境变量配置日志级别和输出目录
- 创建分类日志文件
- 生成元数据和映射文件

### 3. 执行阶段
- **并行执行**（concurrency-level > 1）：使用Block-STM并行执行引擎
- **串行执行**（concurrency-level = 1）：使用标准顺序执行
- 实时记录执行状态、性能指标和依赖关系

### 4. 结果输出
- 生成JSON格式的详细日志
- 输出性能统计信息
- 保存交易映射和元数据

## 已实现的基本功能

### 1. 全方位日志记录

#### 交易执行日志 (`execution_flow.ndjson`)
```json
{
  "timestamp": "2025-08-09T05:09:58.485123Z",
  "event_type": "TransactionStart",
  "block_id": 0,
  "transaction_id": 5,
  "incarnation": 0,
  "thread_id": 2,
  "worker_id": 2,
  "execution_mode": "Parallel"
}
```

**记录字段**：
- `timestamp`: 精确时间戳
- `event_type`: 事件类型（TransactionStart/Finish/Abort等）
- `block_id`: 区块ID
- `transaction_id`: 交易ID
- `incarnation`: 交易转世次数
- `thread_id`: 执行线程ID
- `worker_id`: 工作线程ID
- `execution_mode`: 执行模式（Parallel/Sequential）

#### 调度器状态日志 (`scheduler_states.ndjson`)
```json
{
  "timestamp": "2025-08-09T05:09:58.488956Z",
  "event_type": "StateTransition",
  "transaction_id": 12,
  "incarnation": 0,
  "from_state": "Ready",
  "to_state": "Executing",
  "thread_id": 1,
  "validation_wave": 0
}
```

**记录字段**：
- `from_state/to_state`: 状态转换（Ready→Executing→Executed等）
- `validation_wave`: 验证波次
- `dependency_info`: 依赖信息
- `scheduler_decision`: 调度器决策

#### 依赖关系日志 (`dependencies.ndjson`)
```json
{
  "timestamp": "2025-08-09T05:09:58.490123Z",
  "event_type": "DependencyDetected",
  "dependent_txn": 15,
  "dependency_txn": 8,
  "dependency_type": "ReadAfterWrite",
  "resource_key": "0x1234...abcd",
  "resolution_action": "Abort"
}
```

**记录字段**：
- `dependent_txn/dependency_txn`: 依赖和被依赖的交易ID
- `dependency_type`: 依赖类型（ReadAfterWrite/WriteAfterRead等）
- `resource_key`: 冲突的资源键
- `resolution_action`: 解决方案（Abort/Suspend/Retry）

#### MVHashMap操作日志 (`mvhashmap_ops.ndjson`)
```json
{
  "timestamp": "2025-08-09T05:09:58.487654Z",
  "event_type": "MVHashMapWrite",
  "transaction_id": 7,
  "incarnation": 0,
  "key": "0x1::aptos_coin::AptosCoin<0x1234...>",
  "operation": "Write",
  "value_size": 128,
  "version": 1
}
```

**记录字段**：
- `key`: 访问的状态键
- `operation`: 操作类型（Read/Write/Delete）
- `value_size`: 数据大小
- `version`: 版本号
- `read_set/write_set`: 读写集信息

#### 系统操作日志 (`system_operations.ndjson`)
```json
{
  "timestamp": "2025-08-09T05:09:58.492440Z",
  "event_type": "BlockExecutionComplete",
  "block_id": 0,
  "total_transactions": 100,
  "successful_transactions": 100,
  "aborted_transactions": 6,
  "execution_time_ms": 12,
  "tps": 3333
}
```

**记录字段**：
- `total_transactions`: 总交易数
- `successful_transactions`: 成功交易数
- `aborted_transactions`: 中止交易数
- `execution_time_ms`: 执行时间（毫秒）
- `tps`: 每秒交易处理量

#### 中止恢复日志 (`abort_recovery.ndjson`)
```json
{
  "timestamp": "2025-08-09T05:09:58.489321Z",
  "event_type": "AbortRecovery",
  "transaction_id": 23,
  "incarnation": 1,
  "abort_reason": "ReadWriteConflict",
  "conflicting_txn": 18,
  "retry_count": 1,
  "recovery_action": "Reexecute"
}
```

**记录字段**：
- `abort_reason`: 中止原因
- `conflicting_txn`: 冲突交易
- `retry_count`: 重试次数
- `recovery_action`: 恢复动作

### 2. 区块级汇总信息 (`block_summary.ndjson`)
```json
{
  "block_id": 0,
  "total_transactions": 100,
  "execution_summary": {
    "total_executions": 106,
    "total_validations": 100,
    "total_aborts": 6,
    "total_suspends": 0,
    "avg_suspend_time_us": 0.0,
    "suspend_time_total_us": 0.0
  },
  "performance_metrics": {
    "parallel_tps": 3333,
    "sequential_tps": 0,
    "parallel_time_ms": 12,
    "sequential_time_ms": 0,
    "speedup_ratio": 0.0
  },
  "concurrency_stats": {
    "concurrency_level": 4,
    "thread_utilization": 0.85,
    "lock_contention_ratio": 0.12
  }
}
```

### 3. 交易映射和元数据

#### 行交易映射 (`row_tx_mapping.csv`)
```csv
csv_row,transaction_id,from_address,to_address,value,tx_hash
0,0,account_1234,account_5678,100,0xabcd1234...
1,1,account_2345,account_6789,200,0xbcde2345...
```

#### 代码映射 (`code_map.json`)
```json
{
  "transaction_mappings": {
    "0": {
      "csv_row": 0,
      "transaction_id": 0,
      "from_account": "account_1234",
      "to_account": "account_5678",
      "value": "100"
    }
  },
  "account_mappings": {
    "account_1234": {
      "original_address": "1234",
      "aptos_address": "0x1234000000000000000000000000000000000000000000000000000000000000"
    }
  }
}
```

#### 元数据 (`meta.json`)
```json
{
  "block_info": {
    "block_id": 0,
    "timestamp": "2025-08-09T05:09:53.456Z",
    "transaction_count": 100,
    "data_source": "data/ETH_2401_100.csv"
  },
  "execution_config": {
    "concurrency_level": 4,
    "execution_mode": "Parallel",
    "log_level": "DEBUG"
  },
  "system_info": {
    "hostname": "MacBook-Pro",
    "os": "Darwin",
    "cpu_cores": 8,
    "rust_version": "1.70.0"
  }
}
```

## 输出文件结构

```
test_logs_parallel/
├── block_000/
│   ├── row_tx_mapping.csv      # CSV行到交易ID的映射
│   ├── code_map.json           # 完整的代码映射关系
│   └── meta.json               # 区块和系统元数据
├── execution_flow.ndjson       # 交易执行流程日志
├── scheduler_states.ndjson     # 调度器状态转换日志
├── dependencies.ndjson         # 依赖关系检测日志
├── mvhashmap_ops.ndjson       # MVHashMap操作日志
├── system_operations.ndjson    # 系统级操作日志
├── abort_recovery.ndjson       # 中止恢复处理日志
└── block_summary.ndjson        # 区块级性能汇总
```

## 性能指标和统计

### 1. 执行统计
- **execution_total**: 总执行次数（包括重执行）
- **validation_total**: 总验证次数
- **abort**: 中止次数
- **suspend**: 挂起次数
- **avg_suspend_time**: 平均挂起时间
- **suspend_time_total**: 总挂起时间

### 2. 性能指标

- **TPS**: 每秒事务处理数
- **并行加速比**: 并行vs串行性能比较
- **线程利用率**: 工作线程使用效率
- **锁争用比例**: 资源争用统计

### 3. 并发分析
- **依赖冲突率**: 读写依赖冲突的比例
- **中止恢复时间**: 中止后重新执行的时间开销
- **调度效率**: 调度器任务分配效率

## 使用建议

### 1. 日志级别选择
- **DEBUG**: 最详细，用于深度分析（文件较大）
- **INFO**: 平衡详细程度和性能（推荐）
- **WARN**: 只记录问题和警告
- **ERROR**: 只记录错误信息

### 2. 数据集选择
- **小数据集**（100-1000交易）：快速测试和验证
- **中数据集**（10000-100000交易）：性能分析
- **大数据集**（100000+交易）：压力测试

### 3. 并发配置
- **低并发**（2-4线程）：详细分析依赖关系
- **中并发**（4-8线程）：平衡性能和分析复杂度
- **高并发**（8-16线程）：极限性能测试

## 故障排除

### 常见问题
1. **日志目录权限问题**: 确保目录可写
2. **磁盘空间不足**: 监控日志文件大小
3. **内存不足**: 调整日志级别或数据集大小
4. **编译警告**: 已修复所有警告，确保clean编译

### 调试技巧
1. 使用小数据集快速验证功能
2. 对比并行和串行执行结果
3. 分析依赖日志找出性能瓶颈
4. 监控中止恢复模式找出问题交易

---

**注意**: 该日志系统已经过全面测试，能够在不影响Block-STM核心性能的前提下提供详细的执行分析数据。所有功能都已验证正常工作，无编译警告，可用于生产环境的性能分析和调试。