# 文档3：日志数据格式与分析示例 - 详细编写计划

## 创建时间

2025-08-22 14:35

## 文档基本信息

- **目标文档**:`20250822-1430-logging-data-format-and-examples.md`
- **重点**: 日志格式规范、字段解释、实际数据介绍

## 章节结构规划

### 第一章：日志系统概述 

#### 1.1 日志分类与用途

- **日志文件类型**:
  - `block_stm_execution.log` - 执行流程日志
  - `block_stm_concurrency.log` - 并发性能日志
  - `block_stm_performance.log` - 性能指标日志
  - `block_stm_readwrite.log` - 读写集依赖日志
  - `block_stm_summary.log` - 执行汇总日志
- **用途分析**:
  - 性能调优的数据支撑
  - 并发问题的调试工具
  - 系统行为的监控手段
  - 学术研究的数据基础

#### 1.2 环境变量配置说明

详细说明配置变量：

```bash
# 日志级别控制
BLOCK_STM_LOG_LEVEL=DEBUG|INFO|WARN|ERROR

# 输出目录指定
BLOCK_STM_LOG_DIR=./test_logs_corrected

# 文件大小限制
BLOCK_STM_LOG_MAX_SIZE=100  # MB
```

- **配置影响**: 不同配置对日志内容和性能的影响
- **最佳实践**: 不同场景下的推荐配置

#### 1.3 输出格式标准化

- **JSON格式规范**: NDJSON (Newline Delimited JSON) 标准
- **时间戳标准**: ISO 8601格式，微秒精度
- **字段命名规范**: 下划线命名法，英文描述
- **数据类型约定**: 数值类型、字符串编码、布尔值表示

### 第二章：各类日志详细解析

> 注意，此处可能在一个日志中有多种记录，你需要每一个记录都展开解析！！！

#### 2.1 执行流程日志 (block_stm_execution.log)

**主要事件类型**:

- `TransactionStart` - 交易开始执行
- `TransactionFinish` - 交易执行完成
- `ExecutionStateTransition` - 执行状态转换
- `ValidationStart/Finish` - 验证开始/结束

**关键字段解析**:

```json
{
    "timestamp": "2025-08-22T14:30:00.123456Z",
    "event_type": "TransactionStart", 
    "txn_id": 1234,
    "incarnation": 1,
    "worker_id": 2,
    "context": "Initial execution attempt"
}
```

**实际数据示例分析**:
从 `test_logs_corrected/block_stm_execution.log`提取典型片段：

- 单个交易的完整执行周期
- 并发执行的交易间状态交错
- 异常情况的日志表现

#### 2.2 并发性能日志 (block_stm_concurrency.log)

**监控指标**:

- 活跃工作线程数量
- 任务调度频率
- 线程负载均衡状态
- 并发度变化趋势

**数据格式**:

```json
{
    "timestamp": "2025-08-22T14:30:00.123456Z",
    "event_type": "ConcurrencyMetric",
    "active_workers": 4,
    "pending_tasks": 12,
    "completed_tasks": 156,
    "avg_task_duration_us": 1250
}
```

**性能分析要点**:

- 并发效率的量化指标
- 瓶颈识别的关键信号
- 负载不均衡的表现特征

#### 2.3 性能指标日志 (block_stm_performance.log)

**核心性能指标**:

- TPS (Transactions Per Second)
- 平均执行延迟
- Gas使用统计
- 内存使用峰值

**数据结构**:

```json
{
    "timestamp": "2025-08-22T14:30:00.123456Z",
    "metric_name": "transaction_execution_time_us",
    "value": 1245.67,
    "txn_id": 1234,
    "additional_data": {
        "execution_result": "Success",
        "read_set_size": 5,
        "write_set_size": 3
    }
}
```

**趋势分析**:

- 性能随时间的变化模式
- 异常峰值的识别和分析
- 系统负载与性能的关联

#### 2.4 读写集依赖日志 (block_stm_readwrite.log)

**依赖关系记录**:

- 交易间的读写冲突
- 依赖解析过程
- 回滚触发条件

**日志格式**:

```json
{
    "timestamp": "2025-08-22T14:30:00.123456Z", 
    "event_type": "ReadWriteSetChange",
    "txn_id": 1234,
    "incarnation": 1,
    "read_keys": ["0x1::account::balance", "0x2::coin::info"],
    "write_keys": ["0x1::account::balance"],
    "dependency_count": 2
}
```

**分析价值**:

- 交易冲突模式识别
- 热点数据访问分析
- 并行化效果评估

#### 2.5 执行汇总日志 (block_stm_summary.log)

**汇总统计信息**:

- 整体执行统计
- 性能基准数据
- 错误和异常统计

**数据格式**:

```json
{
    "timestamp": "2025-08-22T14:30:00.123456Z",
    "event_type": "BlockExecutionSummary",
    "total_transactions": 100000,
    "successful_executions": 99995,
    "total_duration_ms": 15678,
    "average_tps": 6375.2,
    "concurrency_level": 4,
    "abort_count": 245,
    "stall_count": 12
}
```

### 第三章：字段定义与数据类型

#### 3.1 通用字段定义

**时间相关字段**:

- `timestamp`: ISO 8601格式的精确时间戳
- `duration_us`: 微秒级持续时间
- `start_time`/`end_time`: 事件开始和结束时间

**交易相关字段**:

- `txn_id`: 交易唯一标识符 (u32)
- `incarnation`: 交易执行版本号 (u32)
- `worker_id`: 执行工作线程ID (u32)
- `block_id`: 区块标识符

**状态相关字段**:

- `event_type`: 事件类型字符串
- `execution_result`: 执行结果枚举
- `validation_result`: 验证结果布尔值

#### 3.2 性能指标字段

**执行统计**:

- `execution_count`: 执行次数累计
- `validation_count`: 验证次数累计
- `abort_count`: 中止次数累计
- `stall_count`: 停滞次数累计

**时间统计**:

- `total_execution_time_us`: 总执行时间
- `avg_execution_time_us`: 平均执行时间
- `max_execution_time_us`: 最大执行时间

**资源统计**:

- `gas_used`: Gas消耗量
- `memory_usage_bytes`: 内存使用量
- `read_set_size`/`write_set_size`: 读写集大小

#### 3.3 特殊数据类型说明

**枚举类型**:

- `ExecutionResult`: Success, Abort, SkipRest, SpeculativeAbort
- `TaskKind`: Execute, PostCommitProcessing, NextTask, Done
- `LogLevel`: DEBUG, INFO, WARN, ERROR

**复合类型**:

- `additional_data`: 键值对映射，包含上下文相关信息
- `read_keys`/`write_keys`: 字符串数组，表示访问的存储键

### 第四章 数据分析思路指南

#### 4.1 单文件分析思路

##### 指标设计

##### 图表绘制思路（图表类型、坐标轴设计）

#### 4.2 多文件联合分析思路

##### 指标设计

##### 图表绘制思路（图表类型、坐标轴设计）

#### 4.3 多数据集联合分析思路

对比ETH和USDT在历史回放中的不同表现特征

#### 4.4 扩展思路

##### 可增加重放合约类型

##### 可增加合成负载
