// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Block-STM详细执行日志模块
//!
//! 本模块提供全面的Block-STM并行执行日志记录功能，包括：
//! - 交易执行状态追踪
//! - 并发控制监控
//! - 读写集变化记录
//! - 性能指标收集
//! - 依赖关系分析
//! - 停滞和中止事件记录

use aptos_mvhashmap::types::{Incarnation, TxnIndex};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

/// Block-STM日志系统配置结构
/// 
/// 用于配置日志记录的各项参数，包括输出目录、日志级别、文件大小限制等
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub enabled: bool,                      // 是否启用日志记录
    pub log_dir: PathBuf,                   // 日志文件输出目录
    pub log_level: LogLevel,                // 日志级别过滤
    pub max_file_size: u64,                 // 单个日志文件最大大小（字节）
    pub buffer_size: usize,                 // 缓冲区大小
    pub async_logging: bool,                // 是否启用异步日志（当前未使用）
    pub include_read_write_details: bool,   // 是否包含读写详情（当前未使用）
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("BLOCK_STM_LOG_LEVEL").is_ok(),
            log_dir: std::env::var("BLOCK_STM_LOG_DIR")
                .unwrap_or_else(|_| "./logs".to_string())
                .into(),
            log_level: std::env::var("BLOCK_STM_LOG_LEVEL")
                .unwrap_or_else(|_| "INFO".to_string())
                .parse()
                .unwrap_or(LogLevel::Info),
            max_file_size: std::env::var("BLOCK_STM_LOG_MAX_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse::<u64>()
                .unwrap_or(100)
                * 1024
                * 1024, // MB to bytes
            buffer_size: 8192,
            async_logging: true,
            include_read_write_details: true,
        }
    }
}

/// 日志级别枚举
/// 
/// 用于过滤不同重要性的日志事件，数值越大级别越高
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Debug = 0,  // 调试级别 - 详细的调试信息
    Info = 1,   // 信息级别 - 一般的信息记录
    Warn = 2,   // 警告级别 - 警告信息
    Error = 3,  // 错误级别 - 错误信息
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DEBUG" => Ok(LogLevel::Debug),
            "INFO" => Ok(LogLevel::Info),
            "WARN" => Ok(LogLevel::Warn),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

/// 执行上下文信息
/// 
/// 记录当前执行环境的基本信息，用于日志记录和分析
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub dataset: String,            // 数据集类型（ETH、USDT等）
    pub source_csv: String,         // CSV数据源文件路径
    pub sample_period_ms: u32,      // 采样周期（毫秒）
    pub read_sample_rate: f64,      // 读操作采样率（0.0-1.0）
}

/// 单个交易执行统计信息
/// 
/// 记录每个交易在Block-STM执行过程中的详细统计数据
#[derive(Debug, Default, Clone)]
pub struct TransactionStats {
    pub task_kind: String,                          // 任务类型（Execute、Validate等）
    pub first_execution: bool,                      // 是否为首次执行
    pub execution_time_us: u64,                     // 执行时间（微秒）
    pub reexecution_count: u32,                     // 重执行次数
    pub stall_count: u32,                           // 停滞次数
    pub waterline_at_exec: u32,                     // 执行时的水位线位置
    pub abort_count: u32,                           // 中止次数
    pub start_time: Option<std::time::Instant>,     // 执行开始时间
}

/// BlockSTMv2增强版执行统计指标
/// 
/// 实时收集Block-STM执行过程中的各项性能和并发指标，
/// 用于分析系统性能和优化执行策略
#[derive(Debug, Default)]
pub struct BlockExecutionStats {
    pub stall_events_count: std::sync::atomic::AtomicU32,                       // 停滞事件总计数
    pub waterline_advances_count: std::sync::atomic::AtomicU32,                 // 水位线推进次数
    pub abort_cycles_count: std::sync::atomic::AtomicU32,                       // 中止周期计数
    pub max_concurrent_executions: std::sync::atomic::AtomicU32,                // 最大并发执行数
    pub total_reexecutions: std::sync::atomic::AtomicU64,                       // 总重执行次数
    pub total_transactions: std::sync::atomic::AtomicU64,                       // 总交易数量
    pub commit_marker_transitions: std::sync::atomic::AtomicU32,                // 提交标记转换次数
    pub post_commit_tasks_count: std::sync::atomic::AtomicU32,                  // 后提交任务数量
    pub task_distribution_map: std::sync::Mutex<std::collections::HashMap<String, u32>>, // 任务类型分布统计
    pub concurrent_execution_samples: std::sync::Mutex<Vec<u32>>,               // 并发执行采样数据
    pub start_time: std::sync::Mutex<Option<std::time::Instant>>,               // 区块执行开始时间
    pub transaction_stats: std::sync::Mutex<std::collections::HashMap<u32, TransactionStats>>, // 单个交易统计映射
    pub participating_threads: std::sync::Mutex<std::collections::HashSet<u64>>, // 参与执行的线程ID集合
    // 停滞时间追踪
    pub total_stall_time_ns: std::sync::atomic::AtomicU64,                      // 总停滞时间（纳秒）
    pub stall_start_times: std::sync::Mutex<std::collections::HashMap<(u32, u32), std::time::Instant>>, // (交易ID, 化身) -> 停滞开始时间
}

/// 分离式Stall统计结构
/// 
/// 将Transaction-Level Stall和System-Level Stall分离统计，
/// 提供更精确的性能分析数据
#[derive(Debug, Default)]
pub struct SeparatedStallStatistics {
    // Transaction-Level Stall统计
    pub transaction_stalls: std::sync::atomic::AtomicU32,                           // 交易级stall计数
    pub transaction_stall_duration_total_us: std::sync::atomic::AtomicU64,          // 交易级stall总时长(微秒)
    pub transaction_stall_count_by_txn: std::sync::Mutex<std::collections::HashMap<u32, u32>>, // 按交易ID统计stall次数
    
    // System-Level Stall统计  
    pub system_stalls: std::sync::atomic::AtomicU32,                                // 系统级stall计数
    pub system_stall_duration_total_us: std::sync::atomic::AtomicU64,               // 系统级stall总时长(微秒)
    pub system_stall_count_by_worker: std::sync::Mutex<std::collections::HashMap<u32, u32>>, // 按工作线程统计stall次数
    
    // 综合统计
    pub total_stall_events: std::sync::atomic::AtomicU32,                           // 总stall事件数
    pub stall_statistics_start_time: std::sync::Mutex<Option<std::time::Instant>>,  // 统计开始时间
    
    // System stall状态追踪
    pub system_stall_start_times: std::sync::Mutex<std::collections::HashMap<u32, std::time::Instant>>, // worker_id -> stall开始时间
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            dataset: "UNKNOWN".to_string(),
            source_csv: "unknown".to_string(),
            sample_period_ms: std::env::var("SAMPLE_PERIOD_MS")
                .unwrap_or_else(|_| "50".to_string())
                .parse::<u32>()
                .unwrap_or(50),
            read_sample_rate: std::env::var("READ_SAMPLE_RATE")
                .unwrap_or_else(|_| "0.01".to_string())
                .parse::<f64>()
                .unwrap_or(0.01),
        }
    }
}

/// Block-STM核心日志事件类型枚举（31种类型）
/// 
/// 定义了Block-STM执行过程中所有可能记录的事件类型，
/// 涵盖区块生命周期、调度状态、依赖关系、执行验证等各个方面
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEvent {
    // === 区块生命周期与采样事件 ===
    
    // 1. BlockStart - 区块执行开始事件（每次区块执行时触发）
    BlockStart {
        timestamp: u64,             // 事件时间戳（微秒）
        thread_id: u64,             // 线程ID（主线程为0）
        block_id: String,           // 区块标识符
        dataset: String,            // 数据集类型："ETH"|"USDT"
        tx_count: u32,              // 区块内交易数量
        concurrency_level: u32,     // 并发执行级别
        sample_period_ms: u32,      // 采样周期（毫秒）
        read_sample_rate: f64,      // 读操作采样率
        source_csv: String,         // 源CSV文件路径
    },
    
    // 2. BlockFinish - 区块执行完成事件（精简版，统计字段通过原子事件计算）
    BlockFinish {
        timestamp: u64,             // 区块完成时间戳
        thread_list: Vec<u64>,      // 参与执行的所有线程ID列表
        block_id: String,           // 区块标识符
        committed_count: u32,       // 实际提交的交易数量
        total_duration_us: u64,     // 总执行时间（微秒）
        // 注意：部分统计字段已从 BlockFinish 中移除，现在通过日志文件计算：
        // - 并行/顺序 TPS、停滞次数、水位线推进、中止周期等指标
        // - 最大并发数、平均重执行次数、调度效率等统计
        // - 任务分布、提交标记转换、后提交任务数量等
    },
    
    // 3. SchedulerSample - 调度器采样事件（每SAMPLE_PERIOD_MS触发一次）
    SchedulerSample {
        timestamp: u64,                     // 采样时间戳
        thread_id: u64,                     // 线程ID（主线程为0）
        execution_min_index: TxnIndex,     // 已完成最终执行的连续前缀+1
        validation_min_index: TxnIndex,    // 已完成最终验证的连续前缀+1
        executing_tasks: u32,               // 正在执行的任务数
        validating_tasks: u32,              // 正在验证的任务数
        active_tasks: u32,                  // 活跃任务总数
        done_marker: bool,                  // 是否满足提交门限判定
        scheduler_efficiency: f64,          // 调度效率百分比
        avg_task_queue_depth: f64,          // 平均任务队列深度
        parallelism_utilization: f64,       // 并行度利用率
        contention_hotspots: Vec<String>,   // 竞争热点状态键列表
    },

    // === 调度器状态转换事件 ===
    
    // 4. SchedulerStateTransition - 调度器核心状态转换事件
    SchedulerStateTransition {
        timestamp: u64,                     // 状态转换时间戳
        thread_id: u64,                     // 线程ID
        transaction_id: Option<TxnIndex>,   // 交易索引（可选）
        incarnation: Option<Incarnation>,   // 交易化身（可选）
        old_state: String,                  // 原状态（Ready/Executing/Suspended/Executed/Committed等）
        new_state: String,                  // 新状态
        trigger_reason: String,             // 触发原因（任务分发/依赖遇到/验证失败等）
    },

    // 5. TaskPicked - 任务被工作线程领取事件
    TaskPicked {
        timestamp: u64,             // 任务领取时间戳
        thread_id: u64,             // 工作线程ID
        kind: String,               // 任务类型："Execution"|"Validation"
        transaction_id: TxnIndex,   // 交易索引
        incarnation: Incarnation,   // 交易化身
        priority: Option<u32>,      // 任务优先级（可选）
    },
    
    // 6. TaskFinished - 任务完成事件
    TaskFinished {
        timestamp: u64,             // 任务完成时间戳
        thread_id: u64,             // 工作线程ID
        kind: String,               // 任务类型："Execution"|"Validation"
        transaction_id: TxnIndex,   // 交易索引
        incarnation: Incarnation,   // 交易化身
        result: String,             // 执行结果："Ok"|"Fail"
        processing_time_us: u64,    // 处理时间（微秒）
    },

    // === MVHashMap多版本哈希表操作事件 ===
    
    // 7. MVRead - 多版本读操作事件（按READ_SAMPLE_RATE采样）
    MVRead {
        timestamp: u64,                         // 读操作时间戳
        thread_id: u64,                         // 执行线程ID
        transaction_id: TxnIndex,               // 读取交易索引
        incarnation: Incarnation,               // 读取交易化身
        state_key: String,                      // 状态键标识符
        read_from: String,                      // 读取源："Storage"|"MVCommitted"|"Uncommitted"|"NotFound"
        writer_tx: Option<TxnIndex>,           // 写入者交易索引
        writer_incarnation: Option<Incarnation>, // 写入者交易化身
        is_estimate: bool,                      // 是否为估计版本（FLAG_ESTIMATE）
        value_size: Option<usize>,              // 值大小（字节）
    },
    
    // 8. MVWrite - 多版本写操作事件
    MVWrite {
        timestamp: u64,             // 写操作时间戳
        thread_id: u64,             // 执行线程ID
        transaction_id: TxnIndex,   // 写入交易索引
        incarnation: Incarnation,   // 写入交易化身
        state_key: String,          // 状态键标识符
        value_size: usize,          // 写入值大小（字节）
        write_type: String,         // 写入类型："Create"|"Modify"|"Delete"
    },
    
    // 9. EstimateMark（标记推测版本）
    EstimateMark {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        state_keys_count: usize, // 本次标记的key数量
        resource_keys_count: usize,
        module_keys_count: usize,
    },
    
    // 10. EstimateClear（清理推测版本）
    EstimateClear {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        cleared_keys_count: usize,
        cleared_resources_count: usize,
        cleared_modules_count: usize,
    },

    // === 依赖（stall 传播）——记录链路创建与解除 ===
    
    // 11. DependencyBlock（创建依赖边）
    DependencyBlock {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        depends_on_tx: TxnIndex, // 依赖的低序号写者
        depends_on_incarnation: Option<Incarnation>,
    },
    
    // 12. DependencyResolve（解除依赖边 - 精简版）
    DependencyResolve {
        timestamp: u64,
        thread_id: u64,
        depender_tx: TxnIndex,   // 被解除依赖的交易
        on_tx: TxnIndex,         // 被依赖的交易
        resolve_cause: String,   // "OnTxCommitted"|"OnTxAborted"|"OnTxExecuted"
        // 删除冗余字段：
        // - wait_time_us: 可通过DependencyResolve.timestamp - DependencyBlock.timestamp计算
        // - state_key: 可从transaction_id重建为"dependency_resolve_tx_{}_by_tx_{}"
    },

    // === 执行与验证（incarnation管理） ===
    
    // 13. ExecutionStart（进入VM执行前）
    ExecutionStart {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        execution_phase: String, // "Initial"|"Retry"
    },
    
    // 14. ExecutionFinish（VM执行完成后）
    ExecutionFinish {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        result: String,          // "Success"|"Suspended"|"Abort"|"SkipRest"|"SpeculativeExecutionAbortError"|"DelayedFieldsCodeInvariantError"
        exec_duration_us: u64,
        gas_used: u64,           // 从TransactionOutput获取
        read_set_size: usize,
        write_set_size: usize,
        resource_reads: usize,
        resource_writes: usize,
        module_reads: usize,
        module_writes: usize,
        delayed_field_reads: usize,
        delayed_field_writes: usize,
    },
    
    // 15. ValidationStart（验证开始）
    ValidationStart {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        is_revalidation: bool,   // 由失败触发的"全体高序再验证"为true
        validation_wave: u32,    // 验证轮次
    },
    
    // 16. ValidationFinish（验证完成 - 精简版）
    ValidationFinish {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        result: String,          // "Pass"|"Fail"
        // 删除冗余字段（可计算）：
        // - val_duration_us: ValidationFinish.timestamp - ValidationStart.timestamp
        // - validated_reads: 统计验证期间的MVRead事件
        // - conflicts_found: 统计相关ValidationConflict事件数量
    },
    
    // 17. ValidationConflict（每个冲突键一条，可选）
    ValidationConflict {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        state_key: String,
        conflict_type: String,   // "read_after_write"|"write_after_read"|"write_after_write"
        conflicting_txn: TxnIndex,
        conflicting_incarnation: Option<Incarnation>,
        expected_version: Option<String>, // "{tx,inc}"格式或null
        observed_version: Option<String>,
        violator_tx: Option<TxnIndex>,
        conflict_location: String, // 冲突检测的代码位置
    },
    
    // 18. RescheduleHigher（"再验证波"指标关键）
    RescheduleHigher {
        timestamp: u64,
        thread_id: u64,
        trigger_tx: TxnIndex,    // 触发重新验证的交易
        affected_count: u32,     // 受影响的交易数量
        reschedule_reason: String,
    },

    // === 中止/重试（incarnation+1） ===
    
    // 19. AbortInitiated（中止开始）
    AbortInitiated {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        abort_reason: String,
        retry_count: u32,
        dependencies: Vec<TxnIndex>, // 导致中止的依赖交易
    },
    
    // 20. IncarnationIncrement（incarnation递增）
    IncarnationIncrement {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        old_incarnation: Incarnation,
        new_incarnation: Incarnation,
        increment_reason: String, // "ValidationFailed"|"Dependency"|"ManualAbort"
    },

    // === 事务输出详情（基于TransactionOutput trait的完整字段） ===
    
    // 21. TransactionOutputDetail（完整的输出信息）
    TransactionOutputDetail {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        gas_used: u64,
        fee_statement: String,   // FeeStatement的JSON序列化
        output_approx_size: u64,
        resource_write_count: usize,
        module_write_count: usize,
        aggregator_v1_write_count: usize,
        aggregator_v1_delta_count: usize,
        delayed_field_change_count: usize,
        event_count: usize,
        resource_group_write_count: usize,
        reads_needing_delayed_field_exchange_count: usize,
        group_reads_needing_delayed_field_exchange_count: usize,
        has_new_epoch_event: bool,
        is_retry: bool,
        is_success: bool,
        execution_status: String, // "Success"|"Abort"|"SkipRest"|"SpeculativeExecutionAbortError"|"DelayedFieldsCodeInvariantError"
        write_summary_hash: String, // 写入摘要的哈希值
    },

    // === 高级特性详细记录（所有源码可获得的字段） ===
    
    // 22. AggregatorOperation（聚合器操作详情）
    AggregatorOperation {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        aggregator_key: String,
        operation_type: String,  // "Delta"|"Write"|"Read"|"Materialize"
        delta_value: Option<i128>, // DeltaOp的值
        base_value: Option<u128>,
        resolved_value: Option<u128>,
        aggregator_limit: Option<u128>,
        delta_history: String,   // DeltaHistory的JSON序列化
        application_result: String, // "Success"|"Overflow"|"Underflow"|"ApplicationFailure"
    },
    
    // 23. DelayedFieldOperation（延迟字段操作）
    DelayedFieldOperation {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        delayed_field_id: String,
        operation_type: String,  // "Read"|"Change"|"Exchange"|"Materialize"
        field_value: Option<String>,
        change_type: String,     // "Create"|"Update"|"Delete"
        needs_exchange: bool,
        exchange_result: String,
    },
    
    // 24. ResourceGroupOperation（资源组操作）
    ResourceGroupOperation {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        group_key: String,
        operation_type: String,  // "Read"|"Write"|"MetadataUpdate"|"SizeCheck"
        group_size: Option<u64>,
        tag_count: usize,
        affected_tags: Vec<String>,
        metadata_changed: bool,
        size_limit_exceeded: bool,
    },
    
    // 25. ModuleCacheOperation（模块缓存操作）
    ModuleCacheOperation {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        module_id: String,
        operation_type: String,  // "Read"|"Write"|"Cache"|"Evict"
        module_size: Option<usize>,
        cache_hit: bool,
        compilation_required: bool,
        cache_utilization: f64,
    },
    
    // 26. EventEmission（事件发射详情）
    EventEmission {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        event_type: String,
        event_key: String,
        event_sequence_number: u64,
        event_data_size: usize,
        event_has_layout: bool,
    },
    
    // 27. ReadWriteSetChange（读写集变化追踪）
    ReadWriteSetChange {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        read_keys: Vec<String>,
        write_keys: Vec<String>,
        resource_reads: usize,
        resource_writes: usize,
        module_reads: usize,
        module_writes: usize,
        delayed_field_reads: usize,
        delayed_field_writes: usize,
        aggregator_v1_reads: usize,
        aggregator_v1_writes: usize,
        resource_group_reads: usize,
        resource_group_writes: usize,
        read_set_delta: i32,     // 相对上次incarnation的变化
        write_set_delta: i32,
        change_trigger: String,  // "execution"|"validation"|"abort_recovery"
    },

    // === 系统监控和性能指标 ===
    
    // 28. PerformanceMetric（通用性能指标）
    PerformanceMetric {
        timestamp: u64,
        thread_id: u64,
        metric_name: String,
        metric_value: f64,
        transaction_id: Option<TxnIndex>,
        incarnation: Option<Incarnation>,
        metric_unit: String,     // "microseconds"|"bytes"|"count"|"percentage"
        measurement_context: String, // "execution"|"validation"|"commit"|"block_summary"
        additional_data: HashMap<String, String>,
    },
    
    // 29. SchedulerMetric（调度器级别的性能指标，无transaction_id）
    SchedulerMetric {
        timestamp: u64,
        thread_id: u64,
        metric_name: String,        // "scheduler_task_assignment"|"scheduler_commit_start"|"scheduler_next_task"
        metric_value: f64,
        metric_unit: String,        // "count"|"microseconds"
        measurement_context: String, // "scheduler"|"commit"|"task_dispatch"
        additional_data: HashMap<String, String>,
    },
    
    // 31. MemoryUsageSnapshot（内存使用统计）
    MemoryUsageSnapshot {
        timestamp: u64,
        thread_id: u64,
        mvhashmap_size: usize,
        mvhashmap_key_count: usize,
        total_base_value_size: u64,
        estimate_entries_count: usize,
        dependency_edges_count: usize,
        scheduler_memory_usage: usize,
        executor_memory_usage: usize,
        thread_local_cache_size: usize,
    },
    
    // 32. LockContention（锁竞争统计）
    LockContention {
        timestamp: u64,
        thread_id: u64,
        lock_type: String,       // "MVHashMapLock"|"SchedulerLock"|"DependencyCondvar"|"ArmedLock"
        lock_identifier: String,
        contention_type: String, // "Acquire"|"Release"|"Wait"|"Timeout"
        wait_time_us: u64,
        holder_thread_id: Option<u64>,
        queue_depth: u32,
    },
    
    // 33. StallPropagation（Stall传播分析 - 精简版）
    StallPropagation {
        timestamp: u64,
        thread_id: u64,
        owner_txn: TxnIndex,
        owner_incarnation: Option<Incarnation>,
        affected_txns: Vec<TxnIndex>,
        affected_incarnations: Vec<Incarnation>,
        propagation_type: String, // "Stall"|"Unstall"
        reason: String,
        // 删除冗余字段（可计算）：
        // - stall_depth: 通过分析依赖链计算
        // - affected_count: affected_txns.len()
        // - state_key: 可从owner_txn重建为"stall_propagation_tx_{}"
        // - propagation_latency_us: 时间戳差值计算
        // - stall_chain_id: 通过依赖图分析生成
    },

    // 32. TaskSuspend（任务挂起 - 精简版）
    TaskSuspend {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        suspend_reason: String,  // "DependencyEncountered"|"ResourceContention"|"StallPropagation"
        depends_on_tx: Option<TxnIndex>,
        // 删除冗余字段：
        // - state_key: 对于StallPropagation类型的挂起不是必需的
        // - suspend_duration_us: 通过TaskResume.timestamp - TaskSuspend.timestamp计算
    },

    // 33. TaskResume（任务恢复 - 精简版）
    TaskResume {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        resume_reason: String,   // "DependencyResolved"|"StallRemoved"|"Rescheduled"
        resolved_by_tx: Option<TxnIndex>,
        // 删除冗余字段：
        // - total_suspended_time_us: 通过累计所有TaskResume.timestamp - TaskSuspend.timestamp计算
    },

    // === BlockSTMv2 Enhancement Events (34-50) ===

    // 34. StallAdd（V2 stall管理 - 添加stall）
    StallAdd {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,                 // 触发stall的上游交易
        stall_count_after: u32,          // 该交易当前stall计数
        first_stall: bool,               // 是否是从0到1的首次stall
    },

    // 35. StallRemove（V2 stall管理 - 移除stall）
    StallRemove {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,                 // 触发unstall的上游交易
        stall_count_after: u32,          // 该交易当前stall计数
        became_unstalled: bool,          // 是否从1到0变为unstall
    },

    // 36. StallPropagateTick（V2 stall批量传播）
    StallPropagateTick {
        timestamp: u64,
        thread_id: u64,
        source_tx: TxnIndex,             // 传播起点交易
        affected_range: Vec<TxnIndex>,   // 受影响的交易范围
        propagated_count: u32,           // 本轮传播的交易数
        propagation_type: String,        // "Add"|"Remove"
    },

    // 37. WaterlineAdvance（V2 水位线推进）
    WaterlineAdvance {
        timestamp: u64,
        thread_id: u64,
        executed_once_max_idx_after: TxnIndex,  // 推进后的水位线
        advanced_by_tx: TxnIndex,               // 推进水位线的交易
        previous_waterline: TxnIndex,           // 之前的水位线
    },

    // 38. AbortStart（V2 两段式abort - 启动阶段）
    AbortStart {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,                 // 触发abort的交易
        result: String,                  // "Started"|"AlreadyAborted"
    },

    // 39. AbortFinish（V2 两段式abort - 完成阶段）
    AbortFinish {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,                 // 触发abort的交易
        result: String,                  // "EnqueuedForReexec"|"NoReexec"
        new_incarnation: Option<Incarnation>,  // 新的incarnation号（如果重新入队）
    },

    // 40. InvalidationEdge（V2 依赖失效边）
    InvalidationEdge {
        timestamp: u64,
        thread_id: u64,
        by_tx: TxnIndex,                 // 低序号写者
        to_tx: TxnIndex,                 // 被无效化的高序号交易
        to_incarnation: Option<Incarnation>,  // 被无效化的incarnation
        key: Option<String>,             // 冲突的状态键
    },

    // 41. CommitMarkerTransition（V2 提交状态转换）
    CommitMarkerTransition {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        from_marker: String,             // "NotCommitted"|"CommitStarted"|"Committed"
        to_marker: String,
    },

    // 42. PostCommitStart（V2 并行后处理开始）
    PostCommitStart {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        hook_kind: String,               // 钩子类型（聚合器/延迟字段等）
    },

    // 43. PostCommitFinish（V2 并行后处理完成）
    PostCommitFinish {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        hook_kind: String,
        duration_us: u64,               // 后处理持续时间
    },

    // 44. ExecutionPhaseTransition（扩展的执行阶段追踪）
    ExecutionPhaseTransition {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        from_phase: String,             // "Initial"|"Retry"|"Deferred"
        to_phase: String,
        is_reexecution: bool,           // 是否是重执行
        first_reexecution_deferred: bool,  // 首次重执行是否被延迟
        executed_once_watermark: TxnIndex, // 当时的水位线
        defer_reason: Option<String>,    // 延迟原因："Waterline"|"Stalled"|"QueueBackoff"
    },

    // 45. TaskPickedV2（V2扩展的任务选择）
    TaskPickedV2 {
        timestamp: u64,
        picked_ts_us: u64,              // 微秒时间戳（便于精确计时）
        thread_id: u64,
        task_kind: String,              // "Execute"|"PostCommit"
        tx_index: TxnIndex,             // 与transaction_id同值（向后兼容）
        incarnation: Incarnation,
        is_first_execution: bool,       // 是否首次执行
        from_queue: String,             // "ExecutionQueue"|"PostCommitQueue"
    },

    // 46. TaskFinishedV2（V2扩展的任务完成）
    TaskFinishedV2 {
        timestamp: u64,
        finished_ts_us: u64,           // 微秒时间戳
        thread_id: u64,
        task_kind: String,             // "Execute"|"PostCommit"
        tx_index: TxnIndex,
        incarnation: Incarnation,
        result: String,                // "Success"|"Aborted"|"Suspended"
        processing_time_us: u64,       // 处理时间
        is_reexecution: bool,
        first_reexecution_deferred: bool,
        executed_once_watermark: TxnIndex,
        defer_reason: Option<String>,
    },

    // 47. DependencyBlocked（数据层依赖阻塞）
    DependencyBlocked {
        timestamp: u64,
        thread_id: u64,
        dependent_tx: TxnIndex,
        on_tx: TxnIndex,                // 所依赖的上游交易
        reason: String,                 // "EstimateRead"|"InFlightWrite"
        state_key: Option<String>,
    },

    // 48. DependencyUnblocked（数据层依赖解除）
    DependencyUnblocked {
        timestamp: u64,
        thread_id: u64,
        dependent_tx: TxnIndex,
        on_tx: TxnIndex,
        reason: String,                 // "Committed"|"Aborted"|"Executed"
        wait_time_us: u64,
    },

    // 49. MVReadV2（扩展的MV读操作）
    MVReadV2 {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        key: String,
        source: String,                  // "Committed"|"EstimateOf(tx)"|"Storage"|"NotFound"
        result: String,                  // "Ok"|"Dependency"
        writer_tx: Option<TxnIndex>,
        writer_incarnation: Option<Incarnation>,
        value_size: Option<usize>,
    },

    // === 分离式Stall统计事件 (50-52) ===
    
    // 50. SystemStallStart（系统级stall开始）
    SystemStallStart {
        timestamp: u64,
        thread_id: u64,
        worker_id: u32,
        total_workers: u32,
        reason: String,                     // "NextTask_no_available_work"
        scheduler_state: String,            // "ALL_TRANSACTIONS_WAITING"
        system_stall_count_before: u32,
        system_stall_count_after: u32,
        stall_transition: String,           // "ACTIVE_TO_STALLED"
    },

    // 51. SystemStallEnd（系统级stall结束）
    SystemStallEnd {
        timestamp: u64,
        thread_id: u64,
        worker_id: u32,
        total_workers: u32,
        reason: String,                     // "NewTask_available"
        scheduler_state: String,            // "TASK_DISPATCHED"
        system_stall_count_before: u32,
        system_stall_count_after: u32,
        stall_transition: String,           // "STALLED_TO_ACTIVE"
        duration_us: u64,                   // 系统stall持续时间
    },

    // 52. TransactionStallAdd（重命名的交易级stall添加）
    TransactionStallAdd {
        timestamp: u64,
        thread_id: u64,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        transaction_stall_count_before: u32,
        transaction_stall_count_after: u32,
        first_stall: bool,
        stall_transition: String,           // "UNSTALLED_TO_STALLED"
    },

    // 53. TransactionStallRemove（重命名的交易级stall移除）
    TransactionStallRemove {
        timestamp: u64,
        thread_id: u64,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        transaction_stall_count_before: u32,
        transaction_stall_count_after: u32,
        became_unstalled: bool,
        stall_transition: String,           // "STALLED_TO_UNSTALLED"
    },
}

/// 日志文件类型枚举 - 增强的8类分类
/// 
/// 将Block-STM的所有日志事件按功能区域分为8个类别，
/// 便于分文件存储和后续分析处理
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LogFileType {
    BlockSummary,       // 区块级别统计：BlockStart/Finish、SchedulerSample、MemoryUsageSnapshot
    SchedulerStates,    // 调度器状态：SchedulerStateTransition、TaskPicked/Finished、LockContention
    MVHashMapOps,       // MV哈希表操作：MVRead/Write、EstimateMark/Clear
    Dependencies,       // 依赖关系：DependencyBlock/Resolve、StallPropagation
    ExecutionFlow,      // 执行流程：ExecutionStart/Finish、ValidationStart/Finish、TransactionOutputDetail
    AbortRecovery,      // 中止恢复：AbortInitiated、IncarnationIncrement、RescheduleHigher、ValidationConflict
    DetailedOperations, // 详细操作：AggregatorOperation、DelayedFieldOperation、ResourceGroupOperation、ReadWriteSetChange
    SystemOperations,   // 系统操作：ModuleCacheOperation、EventEmission、PerformanceMetric
    StallEvents,        // 停滞事件：记录每个stall/unstall事件的详细信息，包含持续时间（兼容现有）
    TransactionStalls,  // 交易级stall：TransactionStallAdd/Remove事件
    SystemStalls,       // 系统级stall：SystemStallStart/End事件
}

impl LogFileType {
    fn filename(&self) -> &'static str {
        match self {
            LogFileType::BlockSummary => "block_summary.ndjson",
            LogFileType::SchedulerStates => "scheduler_states.ndjson",
            LogFileType::MVHashMapOps => "mvhashmap_ops.ndjson",
            LogFileType::Dependencies => "dependencies.ndjson",
            LogFileType::ExecutionFlow => "execution_flow.ndjson",
            LogFileType::AbortRecovery => "abort_recovery.ndjson",
            LogFileType::DetailedOperations => "detailed_operations.ndjson",
            LogFileType::SystemOperations => "system_operations.ndjson",
            LogFileType::StallEvents => "stall_events.ndjson",
            LogFileType::TransactionStalls => "transaction_stalls.ndjson",
            LogFileType::SystemStalls => "system_stalls.ndjson",
        }
    }
}

/// Block-STM事件线程安全日志记录器
/// 
/// 主要的Block-STM日志系统类，负责：
/// - 管理多个日志文件的写入器
/// - 实时收集执行统计数据
/// - 提供线程安全的日志记录接口
/// - 处理不同类型的Block-STM事件
pub struct BlockSTMLogger {
    config: LoggingConfig,                                       // 日志系统配置
    writers: Arc<Mutex<HashMap<LogFileType, BufWriter<File>>>>, // 各类型日志文件写入器
    _start_time: Instant,                                        // 日志系统启动时间（备用）
    execution_context: Arc<Mutex<ExecutionContext>>,            // 执行上下文信息
    execution_stats: BlockExecutionStats,                       // 实时执行统计数据
    separated_stall_stats: SeparatedStallStatistics,            // 分离式stall统计数据
}

impl BlockSTMLogger {
    /// 创建新的Block-STM日忕记录器
    /// 
    /// 参数:
    /// - config: 日志系统配置
    /// 
    /// 返回:
    /// - 日志记录器实例或错误
    pub fn new(config: LoggingConfig) -> std::io::Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                writers: Arc::new(Mutex::new(HashMap::new())),
                _start_time: Instant::now(),
                execution_context: Arc::new(Mutex::new(ExecutionContext::default())),
                execution_stats: BlockExecutionStats::default(),
                separated_stall_stats: SeparatedStallStatistics::default(),
            });
        }

        // 创建日志目录（如果不存在）
        std::fs::create_dir_all(&config.log_dir)?;

        let mut writers = HashMap::new();
        
        // 初始化所有类型的日志文件
        for file_type in [
            LogFileType::BlockSummary,
            LogFileType::SchedulerStates,
            LogFileType::MVHashMapOps,
            LogFileType::Dependencies,
            LogFileType::ExecutionFlow,
            LogFileType::AbortRecovery,
            LogFileType::DetailedOperations,
            LogFileType::SystemOperations,
            LogFileType::StallEvents,
        ] {
            let file_path = config.log_dir.join(file_type.filename());
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(file_path)?;
            let writer = BufWriter::with_capacity(config.buffer_size, file);
            writers.insert(file_type, writer);
        }

        Ok(Self {
            config,
            writers: Arc::new(Mutex::new(writers)),
            _start_time: Instant::now(),
            execution_context: Arc::new(Mutex::new(ExecutionContext::default())),
            execution_stats: BlockExecutionStats::default(),
            separated_stall_stats: SeparatedStallStatistics::default(),
        })
    }

    /// 使用默认配置创建日志记录器
    pub fn with_default_config() -> std::io::Result<Self> {
        Self::new(LoggingConfig::default())
    }

    /// 设置执行上下文信息
    /// 
    /// 用于记录当前执行环境的基本信息
    pub fn set_execution_context(&self, context: ExecutionContext) {
        if let Ok(mut ctx) = self.execution_context.lock() {
            *ctx = context;
        }
    }

    /// 更新执行上下文的特定字段
    pub fn update_execution_context(&self, dataset: Option<String>, source_csv: Option<String>) {
        if let Ok(mut ctx) = self.execution_context.lock() {
            if let Some(dataset) = dataset {
                ctx.dataset = dataset;
            }
            if let Some(source_csv) = source_csv {
                ctx.source_csv = source_csv;
            }
        }
    }

    // === BlockSTMv2统计数据收集方法 ===
    
    /// 标记区块执行开始时间（用于计时）
    pub fn mark_block_execution_start(&self) {
        if let Ok(mut start_time) = self.execution_stats.start_time.lock() {
            *start_time = Some(std::time::Instant::now());
        }
    }

    /// 递增停滞事件计数器
    pub fn increment_stall_events(&self) {
        self.execution_stats.stall_events_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 获取当前停滞事件总数
    pub fn get_stall_events_count(&self) -> u32 {
        self.execution_stats.stall_events_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 记录真实依赖等待的停滞时间
    pub fn record_stall_time(&self, duration_ns: u64) {
        self.execution_stats.total_stall_time_ns.fetch_add(duration_ns, std::sync::atomic::Ordering::Relaxed);
    }

    /// 获取总停滞时间（微秒，精确到小数）
    pub fn get_total_stall_time_us(&self) -> f64 {
        let total_ns = self.execution_stats.total_stall_time_ns.load(std::sync::atomic::Ordering::Relaxed);
        total_ns as f64 / 1000.0
    }

    /// 为新的基准测试运行重置停滞统计数据
    pub fn reset_stall_statistics(&self) {
        self.execution_stats.stall_events_count.store(0, std::sync::atomic::Ordering::Relaxed);
        self.execution_stats.total_stall_time_ns.store(0, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut stall_times) = self.execution_stats.stall_start_times.lock() {
            stall_times.clear();
        }
        
        // 重置分离式stall统计
        self.reset_separated_stall_statistics();
    }

    // === 分离式Stall统计方法 ===

    /// 重置分离式stall统计数据
    pub fn reset_separated_stall_statistics(&self) {
        self.separated_stall_stats.transaction_stalls.store(0, std::sync::atomic::Ordering::Relaxed);
        self.separated_stall_stats.system_stalls.store(0, std::sync::atomic::Ordering::Relaxed);
        self.separated_stall_stats.total_stall_events.store(0, std::sync::atomic::Ordering::Relaxed);
        self.separated_stall_stats.transaction_stall_duration_total_us.store(0, std::sync::atomic::Ordering::Relaxed);
        self.separated_stall_stats.system_stall_duration_total_us.store(0, std::sync::atomic::Ordering::Relaxed);
        if let Ok(mut count_map) = self.separated_stall_stats.transaction_stall_count_by_txn.lock() {
            count_map.clear();
        }
        if let Ok(mut count_map) = self.separated_stall_stats.system_stall_count_by_worker.lock() {
            count_map.clear();
        }
        
        if let Ok(mut start_time) = self.separated_stall_stats.stall_statistics_start_time.lock() {
            *start_time = Some(std::time::Instant::now());
        }
        if let Ok(mut system_stall_times) = self.separated_stall_stats.system_stall_start_times.lock() {
            system_stall_times.clear();
        }
    }

    /// Transaction Stall API

    /// 递增交易级stall计数并记录
    pub fn increment_transaction_stalls(&self) -> u32 {
        let count = self.separated_stall_stats.transaction_stalls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        self.separated_stall_stats.total_stall_events.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        count
    }

    /// 记录交易级stall持续时间
    pub fn record_transaction_stall_duration(&self, duration_us: u64) {
        self.separated_stall_stats.transaction_stall_duration_total_us.fetch_add(duration_us, std::sync::atomic::Ordering::Relaxed);
    }

    /// 记录特定交易的stall次数
    pub fn increment_transaction_stall_by_txn(&self, txn_id: u32) {
        if let Ok(mut count_map) = self.separated_stall_stats.transaction_stall_count_by_txn.lock() {
            *count_map.entry(txn_id).or_insert(0) += 1;
        }
    }

    /// System Stall API

    /// 递增系统级stall计数并记录
    pub fn increment_system_stalls(&self) -> u32 {
        let count = self.separated_stall_stats.system_stalls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        self.separated_stall_stats.total_stall_events.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        count
    }

    /// 记录系统级stall持续时间
    pub fn record_system_stall_duration(&self, duration_us: u64) {
        self.separated_stall_stats.system_stall_duration_total_us.fetch_add(duration_us, std::sync::atomic::Ordering::Relaxed);
    }

    /// 记录特定工作线程的stall次数
    pub fn increment_system_stall_by_worker(&self, worker_id: u32) {
        if let Ok(mut count_map) = self.separated_stall_stats.system_stall_count_by_worker.lock() {
            *count_map.entry(worker_id).or_insert(0) += 1;
        }
    }

    /// 开始记录系统级stall时间
    pub fn start_system_stall(&self, worker_id: u32) {
        if let Ok(mut start_times) = self.separated_stall_stats.system_stall_start_times.lock() {
            start_times.insert(worker_id, std::time::Instant::now());
        }
    }

    /// 结束记录系统级stall时间并计算持续时间
    pub fn end_system_stall(&self, worker_id: u32) -> Option<u64> {
        if let Ok(mut start_times) = self.separated_stall_stats.system_stall_start_times.lock() {
            if let Some(start_time) = start_times.remove(&worker_id) {
                let duration_us = start_time.elapsed().as_micros() as u64;
                self.record_system_stall_duration(duration_us);
                return Some(duration_us);
            }
        }
        None
    }

    /// 统计查询API

    /// 获取交易级stall计数
    pub fn get_transaction_stall_count(&self) -> u32 {
        self.separated_stall_stats.transaction_stalls.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取系统级stall计数
    pub fn get_system_stall_count(&self) -> u32 {
        self.separated_stall_stats.system_stalls.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取总stall计数（新的分离统计）
    pub fn get_total_stall_count_separated(&self) -> u32 {
        self.separated_stall_stats.total_stall_events.load(std::sync::atomic::Ordering::Relaxed)
    }


    /// 获取分离式stall统计摘要
    pub fn get_separated_stall_statistics_summary(&self) -> serde_json::Value {
        let transaction_stalls = self.get_transaction_stall_count();
        let system_stalls = self.get_system_stall_count();
        let total_stalls = transaction_stalls + system_stalls;
        
        let transaction_duration_total = self.separated_stall_stats.transaction_stall_duration_total_us.load(std::sync::atomic::Ordering::Relaxed);
        let system_duration_total = self.separated_stall_stats.system_stall_duration_total_us.load(std::sync::atomic::Ordering::Relaxed);
        
        let transaction_avg_duration = if transaction_stalls > 0 {
            transaction_duration_total as f64 / transaction_stalls as f64
        } else {
            0.0
        };
        
        let system_avg_duration = if system_stalls > 0 {
            system_duration_total as f64 / system_stalls as f64
        } else {
            0.0
        };
        
        let overall_avg_duration = if total_stalls > 0 {
            (transaction_duration_total + system_duration_total) as f64 / total_stalls as f64
        } else {
            0.0
        };
        
        let transaction_percentage = if total_stalls > 0 {
            (transaction_stalls as f64 / total_stalls as f64) * 100.0
        } else {
            0.0
        };
        
        let system_percentage = if total_stalls > 0 {
            (system_stalls as f64 / total_stalls as f64) * 100.0
        } else {
            0.0
        };
        
        serde_json::json!({
            "transaction_stalls": transaction_stalls,
            "system_stalls": system_stalls,
            "total_stalls": total_stalls,
            "transaction_stall_avg_duration_us": transaction_avg_duration,
            "system_stall_avg_duration_us": system_avg_duration,
            "overall_stall_avg_duration_us": overall_avg_duration,
            "transaction_stall_percentage": transaction_percentage,
            "system_stall_percentage": system_percentage,
            "transaction_stall_duration_total_us": transaction_duration_total,
            "system_stall_duration_total_us": system_duration_total
        })
    }

    /// 递增水位线推进计数器
    pub fn increment_waterline_advances(&self) {
        self.execution_stats.waterline_advances_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 递增中止周期计数器
    pub fn increment_abort_cycles(&self) {
        self.execution_stats.abort_cycles_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 更新最大并发执行数
    pub fn update_max_concurrent_executions(&self, current_concurrent: u32) {
        use std::sync::atomic::Ordering;
        let mut current_max = self.execution_stats.max_concurrent_executions.load(Ordering::Relaxed);
        while current_concurrent > current_max {
            match self.execution_stats.max_concurrent_executions.compare_exchange_weak(
                current_max, current_concurrent, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }
    }

    /// 记录一次交易执行（用于平均重执行次数计算）
    pub fn record_transaction_execution(&self, is_reexecution: bool) {
        self.execution_stats.total_transactions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if is_reexecution {
            self.execution_stats.total_reexecutions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// 递增提交标记转换计数器
    pub fn increment_commit_marker_transitions(&self) {
        self.execution_stats.commit_marker_transitions.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 递增后提交任务计数器
    pub fn increment_post_commit_tasks(&self) {
        self.execution_stats.post_commit_tasks_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 记录任务类型用于分布分析
    pub fn record_task_type(&self, task_type: &str) {
        if let Ok(mut task_map) = self.execution_stats.task_distribution_map.lock() {
            *task_map.entry(task_type.to_string()).or_insert(0) += 1;
        }
    }

    /// 采样并发执行数量用于调度器效率计算
    pub fn sample_concurrent_executions(&self, concurrent_count: u32) {
        if let Ok(mut samples) = self.execution_stats.concurrent_execution_samples.lock() {
            samples.push(concurrent_count);
            // Keep only the last 1000 samples to avoid unbounded growth
            if samples.len() > 1000 {
                samples.truncate(500); // Keep the most recent 500
            }
        }
    }

    /// 记录参与执行的线程ID
    pub fn record_participating_thread(&self, thread_id: u64) {
        if let Ok(mut threads) = self.execution_stats.participating_threads.lock() {
            threads.insert(thread_id);
        }
    }

    /// 获取所有参与执行的线程ID列表
    pub fn get_participating_threads(&self) -> Vec<u64> {
        if let Ok(threads) = self.execution_stats.participating_threads.lock() {
            let mut thread_list: Vec<u64> = threads.iter().cloned().collect();
            thread_list.sort();
            thread_list
        } else {
            vec![0] // 退回到单线程模式
        }
    }

    // === 单个交易统计追踪 ===

    /// 开始跟踪交易的执行统计
    /// 
    /// 为指定交易初始化执行统计信息，记录开始时间和执行状态
    pub fn start_transaction_execution(&self, txn_idx: u32, incarnation: u32) {
        if let Ok(mut stats_map) = self.execution_stats.transaction_stats.lock() {
            let stats = stats_map.entry(txn_idx).or_insert_with(TransactionStats::default);
            stats.start_time = Some(std::time::Instant::now());
            stats.first_execution = incarnation == 1;
            if incarnation > 1 {
                stats.reexecution_count += 1;
            }
        }
    }

    /// 完成交易执行统计跟踪
    /// 
    /// 计算并记录交易的总执行时间
    pub fn finish_transaction_execution(&self, txn_idx: u32, _incarnation: u32) {
        if let Ok(mut stats_map) = self.execution_stats.transaction_stats.lock() {
            if let Some(stats) = stats_map.get_mut(&txn_idx) {
                if let Some(start_time) = stats.start_time {
                    stats.execution_time_us = start_time.elapsed().as_micros() as u64;
                }
                stats.task_kind = "Execute".to_string();
            }
        }
    }

    /// 记录交易的停滞事件
    /// 
    /// 递增指定交易的停滞计数器
    pub fn record_transaction_stall(&self, txn_idx: u32) {
        if let Ok(mut stats_map) = self.execution_stats.transaction_stats.lock() {
            if let Some(stats) = stats_map.get_mut(&txn_idx) {
                stats.stall_count += 1;
            }
        }
    }

    /// 记录交易的中止事件
    /// 
    /// 递增指定交易的中止计数器
    pub fn record_transaction_abort(&self, txn_idx: u32) {
        if let Ok(mut stats_map) = self.execution_stats.transaction_stats.lock() {
            if let Some(stats) = stats_map.get_mut(&txn_idx) {
                stats.abort_count += 1;
            }
        }
    }

    /// 更新交易执行时的水位线位置
    /// 
    /// 记录交易执行时的系统水位线状态
    pub fn update_transaction_waterline(&self, txn_idx: u32, waterline: u32) {
        if let Ok(mut stats_map) = self.execution_stats.transaction_stats.lock() {
            if let Some(stats) = stats_map.get_mut(&txn_idx) {
                stats.waterline_at_exec = waterline;
            }
        }
    }

    /// 获取交易统计信息用于CSV更新
    /// 
    /// 返回指定交易的详细统计数据副本
    pub fn get_transaction_stats(&self, txn_idx: u32) -> Option<TransactionStats> {
        if let Ok(stats_map) = self.execution_stats.transaction_stats.lock() {
            stats_map.get(&txn_idx).cloned()
        } else {
            None
        }
    }

    /// 导出所有交易统计信息用于CSV更新
    /// 
    /// 返回所有交易统计数据的完整副本
    pub fn export_transaction_stats(&self) -> std::collections::HashMap<u32, TransactionStats> {
        if let Ok(stats_map) = self.execution_stats.transaction_stats.lock() {
            stats_map.clone()
        } else {
            std::collections::HashMap::new()
        }
    }

    /// 将事件记录到相应的日志文件
    /// 
    /// 根据事件类型自动选择正确的日志文件进行记录
    pub fn log_event(&self, event: LogEvent, level: LogLevel) {
        if !self.config.enabled || level < self.config.log_level {
            return;
        }

        let file_type = match &event {
            // 区块汇总: BlockStart/Finish、SchedulerSample、MemoryUsageSnapshot
            LogEvent::BlockStart { .. }
            | LogEvent::BlockFinish { .. }
            | LogEvent::SchedulerSample { .. }
            | LogEvent::MemoryUsageSnapshot { .. } => LogFileType::BlockSummary,
            
            // 调度器状态: SchedulerStateTransition、TaskPicked/Finished、TaskSuspend/Resume、LockContention、StallAdd/Remove、StallPropagateTick、WaterlineAdvance、TaskPickedV2/TaskFinishedV2
            LogEvent::SchedulerStateTransition { .. }
            | LogEvent::TaskPicked { .. }
            | LogEvent::TaskFinished { .. }
            | LogEvent::TaskSuspend { .. }
            | LogEvent::TaskResume { .. }
            | LogEvent::LockContention { .. }
            | LogEvent::StallAdd { .. }
            | LogEvent::StallRemove { .. }
            | LogEvent::StallPropagateTick { .. }
            | LogEvent::WaterlineAdvance { .. }
            | LogEvent::TaskPickedV2 { .. }
            | LogEvent::TaskFinishedV2 { .. } => LogFileType::SchedulerStates,
            
            // MV哈希表操作: MVRead/Write、EstimateMark/Clear、MVReadV2
            LogEvent::MVRead { .. }
            | LogEvent::MVWrite { .. }
            | LogEvent::EstimateMark { .. }
            | LogEvent::EstimateClear { .. }
            | LogEvent::MVReadV2 { .. } => LogFileType::MVHashMapOps,
            
            // 依赖关系: DependencyBlock/Resolve、StallPropagation、DependencyBlocked/Unblocked
            LogEvent::DependencyBlock { .. }
            | LogEvent::DependencyResolve { .. }
            | LogEvent::StallPropagation { .. }
            | LogEvent::DependencyBlocked { .. }
            | LogEvent::DependencyUnblocked { .. } => LogFileType::Dependencies,
            
            // 执行流程: ExecutionStart/Finish、ValidationStart/Finish、TransactionOutputDetail、ExecutionPhaseTransition
            LogEvent::ExecutionStart { .. }
            | LogEvent::ExecutionFinish { .. }
            | LogEvent::ValidationStart { .. }
            | LogEvent::ValidationFinish { .. }
            | LogEvent::TransactionOutputDetail { .. }
            | LogEvent::ExecutionPhaseTransition { .. } => LogFileType::ExecutionFlow,
            
            // 中止恢复: AbortInitiated、IncarnationIncrement、RescheduleHigher、ValidationConflict、AbortStart/Finish、InvalidationEdge
            LogEvent::AbortInitiated { .. }
            | LogEvent::IncarnationIncrement { .. }
            | LogEvent::RescheduleHigher { .. }
            | LogEvent::ValidationConflict { .. }
            | LogEvent::AbortStart { .. }
            | LogEvent::AbortFinish { .. }
            | LogEvent::InvalidationEdge { .. } => LogFileType::AbortRecovery,
            
            // 详细操作: AggregatorOperation、DelayedFieldOperation、ResourceGroupOperation、ReadWriteSetChange
            LogEvent::AggregatorOperation { .. }
            | LogEvent::DelayedFieldOperation { .. }
            | LogEvent::ResourceGroupOperation { .. }
            | LogEvent::ReadWriteSetChange { .. } => LogFileType::DetailedOperations,
            
            // 系统操作: ModuleCacheOperation、EventEmission、PerformanceMetric、SchedulerMetric、CommitMarkerTransition、PostCommitStart/Finish
            LogEvent::ModuleCacheOperation { .. }
            | LogEvent::EventEmission { .. }
            | LogEvent::PerformanceMetric { .. }
            | LogEvent::SchedulerMetric { .. }
            | LogEvent::CommitMarkerTransition { .. }
            | LogEvent::PostCommitStart { .. }
            | LogEvent::PostCommitFinish { .. } => LogFileType::SystemOperations,
            
            // 分离式stall事件: TransactionStallAdd/Remove、SystemStallStart/End
            LogEvent::TransactionStallAdd { .. }
            | LogEvent::TransactionStallRemove { .. } => LogFileType::TransactionStalls,
            
            LogEvent::SystemStallStart { .. }
            | LogEvent::SystemStallEnd { .. } => LogFileType::SystemStalls,
        };

        if let Ok(mut writers) = self.writers.lock() {
            if let Some(writer) = writers.get_mut(&file_type) {
                if let Ok(json_str) = serde_json::to_string(&event) {
                    let _ = writeln!(writer, "{}", json_str);
                    let _ = writer.flush();
                }
            }
        }
    }

    // === 区块生命周期事件日志方法 ===
    
    /// 记录区块开始事件
    /// 
    /// 记录区块执行开始的关键信息，包括数据集、交易数量、并发级别等
    pub fn log_block_start(
        &self,
        block_id: &str,
        dataset: &str,
        tx_count: u32,
        concurrency_level: u32,
        sample_period_ms: u32,
        read_sample_rate: f64,
        source_csv: &str,
    ) {
        let event = LogEvent::BlockStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: 0,
            block_id: block_id.to_string(),
            dataset: dataset.to_string(),
            tx_count,
            concurrency_level,
            sample_period_ms,
            read_sample_rate,
            source_csv: source_csv.to_string(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 使用存储的执行上下文记录区块开始事件
    /// 
    /// 从存储的执行上下文中获取数据集和采样参数信息
    pub fn log_block_start_with_context(
        &self,
        block_id: &str,
        tx_count: u32,
        concurrency_level: u32,
    ) {
        // 标记区块执行开始时间用于计时
        self.mark_block_execution_start();

        let context = if let Ok(ctx) = self.execution_context.lock() {
            ctx.clone()
        } else {
            ExecutionContext::default()
        };

        let event = LogEvent::BlockStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: 0,
            block_id: block_id.to_string(),
            dataset: context.dataset,
            tx_count,
            concurrency_level,
            sample_period_ms: context.sample_period_ms,
            read_sample_rate: context.read_sample_rate,
            source_csv: context.source_csv,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录区块完成事件（精简版）
    /// 
    /// 记录区块执行完成的基本信息，复杂统计数据通过其他日志文件计算
    pub fn log_block_finish(
        &self,
        block_id: &str,
        committed_count: u32,
        total_duration_us: u64,
    ) {
        let event = LogEvent::BlockFinish {
            timestamp: Self::current_timestamp_us(),
            thread_list: self.get_participating_threads(),
            block_id: block_id.to_string(),
            committed_count,
            total_duration_us,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录区块完成事件（旧版本，向后兼容）
    /// 
    /// 保留旧版本接口以保持向后兼容性，自动计算实际执行时间
    pub fn log_block_finish_legacy(
        &self,
        block_id: &str,
        committed_count: u32,
        total_duration_us: u64,
        _parallel_tps: f64,
        _sequential_tps: f64,
    ) {
        // 不再需要原子操作 - 统计字段已移除

        // 如果未提供则计算实际执行时间
        let actual_duration_us = if total_duration_us == 0 {
            if let Ok(start_time_guard) = self.execution_stats.start_time.lock() {
                if let Some(start_time) = *start_time_guard {
                    start_time.elapsed().as_micros() as u64
                } else {
                    0
                }
            } else {
                total_duration_us
            }
        } else {
            total_duration_us
        };

        // 统计字段现已删除 - 这些指标将通过原子日志事件计算：
        // - stall_events_count: 统计scheduler_states.ndjson中StallAdd事件
        // - waterline_advances_count: 统计scheduler_states.ndjson中WaterlineAdvance事件  
        // - abort_cycles_count: 统计abort_recovery.ndjson中AbortStart事件
        // - max_concurrent_executions: 分析TaskPicked/TaskFinished时间重叠
        // - avg_reexecution_per_tx: 统计IncarnationIncrement事件平均值
        // - scheduler_efficiency: 基于TaskFinished success/total比率计算
        // - task_distribution: 统计TaskPicked事件的任务类型分布

        self.log_block_finish(
            block_id, 
            committed_count, 
            actual_duration_us
        );
    }

    /// 记录调度器采样事件
    /// 
    /// 定期采样调度器状态，记录执行进度、任务分布和性能指标
    pub fn log_scheduler_sample(
        &self,
        execution_min_index: TxnIndex,
        validation_min_index: TxnIndex,
        executing_tasks: u32,
        validating_tasks: u32,
        active_tasks: u32,
        done_marker: bool,
        scheduler_efficiency: f64,
        avg_task_queue_depth: f64,
        parallelism_utilization: f64,
        contention_hotspots: Vec<String>,
    ) {
        let event = LogEvent::SchedulerSample {
            timestamp: Self::current_timestamp_us(),
            thread_id: 0,
            execution_min_index,
            validation_min_index,
            executing_tasks,
            validating_tasks,
            active_tasks,
            done_marker,
            scheduler_efficiency,
            avg_task_queue_depth,
            parallelism_utilization,
            contention_hotspots,
        };
        self.log_event(event, LogLevel::Debug);
    }

    // === 调度器状态转换日志方法 ===

    /// 记录调度器状态转换
    /// 
    /// 记录交易在调度器中的状态变化，包括触发原因和相关上下文
    pub fn log_scheduler_state_transition(
        &self,
        old_state: &str,
        new_state: &str,
        transaction_id: Option<TxnIndex>,
        incarnation: Option<Incarnation>,
        trigger_reason: &str,
    ) {
        self.record_participating_thread(Self::current_thread_id());
        let event = LogEvent::SchedulerStateTransition {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id,
            incarnation,
            old_state: old_state.to_string(),
            new_state: new_state.to_string(),
            trigger_reason: trigger_reason.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录执行开始事件
    /// 
    /// 记录交易进入VM执行阶段的开始时间和执行阶段
    pub fn log_execution_start(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        execution_phase: &str,
    ) {
        self.record_participating_thread(Self::current_thread_id());
        let event = LogEvent::ExecutionStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            execution_phase: execution_phase.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录执行完成事件
    /// 
    /// 记录交易VM执行完成的详细信息，包括结果、执行时间、Gas使用量等
    pub fn log_execution_finish(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        result: &str,
        exec_duration_us: u64,
        gas_used: u64,
        read_set_size: usize,
        write_set_size: usize,
        resource_reads: usize,
        resource_writes: usize,
        module_reads: usize,
        module_writes: usize,
        delayed_field_reads: usize,
        delayed_field_writes: usize,
    ) {
        self.record_participating_thread(Self::current_thread_id());
        let event = LogEvent::ExecutionFinish {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            result: result.to_string(),
            exec_duration_us,
            gas_used,
            read_set_size,
            write_set_size,
            resource_reads,
            resource_writes,
            module_reads,
            module_writes,
            delayed_field_reads,
            delayed_field_writes,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录MV读操作（含采样）
    /// 
    /// 按配置的采样率记录多版本哈希表读操作，用于分析读写模式
    pub fn log_mv_read(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        state_key: &str,
        read_from: &str,
        writer_tx: Option<TxnIndex>,
        writer_incarnation: Option<Incarnation>,
        is_estimate: bool,
        value_size: Option<usize>,
    ) {
        if !self.should_sample_read() {
            return; // 按采样率过滤
        }
        
        self.record_participating_thread(Self::current_thread_id());
        let event = LogEvent::MVRead {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            state_key: state_key.to_string(),
            read_from: read_from.to_string(),
            writer_tx,
            writer_incarnation,
            is_estimate,
            value_size,
        };
        self.log_event(event, LogLevel::Debug);
    }
    
    /// 记录MV写操作事件
    /// 
    /// 记录多版本哈希表写操作，包括写入类型和数据大小
    pub fn log_mv_write(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        state_key: &str,
        value_size: usize,
        write_type: &str,
    ) {
        self.record_participating_thread(Self::current_thread_id());
        let event = LogEvent::MVWrite {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            state_key: state_key.to_string(),
            value_size,
            write_type: write_type.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    // === 辅助工具方法 ===

    /// 检查读操作是否应该被采样
    /// 
    /// 根据环境变量READ_SAMPLE_RATE决定是否记录当前读操作
    pub fn should_sample_read(&self) -> bool {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        
        let sample_rate = std::env::var("READ_SAMPLE_RATE")
            .unwrap_or_else(|_| "0.01".to_string())
            .parse::<f64>()
            .unwrap_or(0.01);
            
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        (count as f64 * sample_rate) % 1.0 < sample_rate
    }

    /// 获取当前时间戳（相对于程序启动时间的微秒数）
    /// 
    /// 使用全局的起始时间作为基准，计算相对时间戳
    pub fn current_timestamp_us() -> u64 {
        use std::sync::OnceLock;
        static START_TIME: OnceLock<std::time::Instant> = OnceLock::new();
        
        let start_time = START_TIME.get_or_init(|| std::time::Instant::now());
        start_time.elapsed().as_micros() as u64
    }

    /// 获取当前线程ID
    /// 
    /// 将系统线程ID转换为哈希值用于日志记录
    pub fn current_thread_id() -> u64 {
        use std::hash::{Hash, Hasher};
        let thread_id = thread::current().id();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        thread_id.hash(&mut hasher);
        hasher.finish()
    }

    /// 刷新所有写入器
    /// 
    /// 强制将缓存中的数据写入磁盘文件
    pub fn flush(&self) {
        if let Ok(mut writers) = self.writers.lock() {
            for writer in writers.values_mut() {
                let _ = writer.flush();
            }
        }
    }

    // === 附加核心事件日志方法 ===

    /// 记录交易中止事件
    /// 
    /// 记录交易中止的详细信息，包括中止原因和依赖关系
    pub fn log_abort_initiated(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        abort_reason: &str,
        retry_count: u32,
        dependencies: Vec<TxnIndex>,
    ) {
        let event = LogEvent::AbortInitiated {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            abort_reason: abort_reason.to_string(),
            retry_count,
            dependencies,
        };
        self.log_event(event, LogLevel::Info);
    }

    // === 向后兼容方法 ===
    
    /// 交易开始的遗留方法（向后兼容）
    pub fn log_transaction_start(&self, txn_id: TxnIndex, incarnation: Incarnation) {
        self.log_execution_start(txn_id, incarnation, "Initial");
    }

    /// 执行状态转换的遗留方法（向后兼容）  
    pub fn log_execution_state_transition(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        old_state: &str,
        new_state: &str,
        transition_reason: &str,
        _execution_phase: &str,
    ) {
        self.log_scheduler_state_transition(old_state, new_state, Some(txn_id), Some(incarnation), transition_reason);
    }

    /// 交易提交的遗留方法（向后兼容）
    pub fn log_transaction_commit(&self, txn_id: TxnIndex, incarnation: Incarnation) {
        self.log_execution_finish(txn_id, incarnation, "Committed", 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    }

    /// 交易完成的遗留方法（向后兼容）  
    pub fn log_transaction_finish(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        execution_result: &str,
        duration: std::time::Duration,
        gas_used: u64,
        read_set_size: usize,
        write_set_size: usize,
    ) {
        let duration_us = duration.as_micros() as u64;
        self.log_execution_finish(txn_id, incarnation, execution_result, duration_us, gas_used, read_set_size, write_set_size, 0, 0, 0, 0, 0, 0);
    }

    /// 交易中止的遗留方法（向后兼容）
    pub fn log_transaction_abort(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        abort_reason: &str,
        retry_count: u32,
        dependencies: Vec<TxnIndex>,
    ) {
        self.log_abort_initiated(txn_id, incarnation, abort_reason, retry_count, dependencies);
    }

    /// 停滞传播的遗留方法（向后兼容）
    pub fn log_stall_propagation(
        &self,
        owner_txn: TxnIndex,
        owner_incarnation: Incarnation,
        affected_txns: Vec<TxnIndex>,
        affected_incarnations: Vec<Incarnation>,
        propagation_type: &str,
        reason: &str,
    ) {
        let event = LogEvent::StallPropagation {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            owner_txn,
            owner_incarnation: Some(owner_incarnation),
            affected_txns: affected_txns.clone(),
            affected_incarnations,
            propagation_type: propagation_type.to_string(),
            reason: reason.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 任务分发的遗留方法（向后兼容）
    pub fn log_task_dispatch(&self, txn_id: TxnIndex, incarnation: Incarnation, _task_type: &str, description: &str) {
        self.log_scheduler_state_transition("IDLE", "DISPATCHING", Some(txn_id), Some(incarnation), description);
    }

    /// 验证的遗留方法（向后兼容）
    pub fn log_transaction_validate(
        &self, 
        txn_id: TxnIndex, 
        incarnation: Incarnation,
        validation_result: bool,
        _description: &str
    ) {
        let event = LogEvent::ValidationFinish {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            result: if validation_result { "Pass" } else { "Fail" }.to_string(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录详细的读写统计到专门的日志文件
    /// 
    /// 为执行阶段记录详细的读写操作统计数据
    pub fn log_readwrite_detailed_stats(
        &self,
        tx_index: TxnIndex,
        incarnation: Incarnation,
        resource_reads: usize,
        resource_writes: usize,
        module_reads: usize,
        module_writes: usize,
        delayed_field_reads: usize,
        delayed_field_writes: usize,
        aggregator_v1_reads: usize,
        aggregator_v1_writes: usize,
        _read_summary_size: usize,
        _write_summary_size: usize,
    ) {
        if !self.config.enabled {
            return;
        }

        let event = LogEvent::ReadWriteSetChange {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: tx_index,
            incarnation,
            read_keys: vec![], // 详细统计的简化版本
            write_keys: vec![], // 详细统计的简化版本
            resource_reads,
            resource_writes,
            module_reads,
            module_writes,
            delayed_field_reads,
            delayed_field_writes,
            aggregator_v1_reads,
            aggregator_v1_writes,
            resource_group_reads: 0,
            resource_group_writes: 0,
            read_set_delta: 0, // 详细统计中未实现增量计算
            write_set_delta: 0, // 详细统计中未实现增量计算
            change_trigger: "execution".to_string(), // 在执行阶段调用
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 读写集变化的遗留方法（向后兼容）
    pub fn log_readwrite_set_change(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        read_keys: Vec<String>,
        write_keys: Vec<String>,
        resource_reads: usize,
        resource_writes: usize,
        module_reads: usize,
        module_writes: usize,
        delayed_field_reads: usize,
        delayed_field_writes: usize,
    ) {
        let event = LogEvent::ReadWriteSetChange {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            read_keys,
            write_keys,
            resource_reads,
            resource_writes,
            module_reads,
            module_writes,
            delayed_field_reads,
            delayed_field_writes,
            aggregator_v1_reads: 0,
            aggregator_v1_writes: 0,
            resource_group_reads: 0,
            resource_group_writes: 0,
            read_set_delta: 0,
            write_set_delta: 0,
            change_trigger: "execution".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 性能指标的遗留方法（向后兼容）
    pub fn log_performance_metric(
        &self,
        metric_name: &str,
        metric_value: f64,
        txn_id: Option<TxnIndex>,
        additional_data: HashMap<String, String>,
    ) {
        let event = LogEvent::PerformanceMetric {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            metric_name: metric_name.to_string(),
            metric_value,
            transaction_id: txn_id,
            incarnation: Some(0),
            metric_unit: "count".to_string(),
            measurement_context: "general".to_string(),
            additional_data,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 依赖停滞的遗留方法（向后兼容）
    pub fn log_dependency_stall(&self, txn_id: TxnIndex, txn_incarnation: Incarnation, stalled_by: Vec<TxnIndex>, depends_on_incarnation: Incarnation) {
        if let Some(owner_tx) = stalled_by.first() {
            let event = LogEvent::DependencyBlock {
                timestamp: Self::current_timestamp_us(),
                thread_id: Self::current_thread_id(),
                transaction_id: txn_id,
                incarnation: txn_incarnation,
                depends_on_tx: *owner_tx,
                depends_on_incarnation: Some(depends_on_incarnation),
            };
            self.log_event(event, LogLevel::Debug);
            
            // 同时记录为TaskSuspend用于挂起分析
            self.log_task_suspend(
                txn_id,
                txn_incarnation,
                "StallPropagation",
                Some(*owner_tx),
            );
        }
    }

    /// 依赖解除停滞的遗留方法（向后兼容）
    pub fn log_dependency_unstall(&self, txn_id: TxnIndex, unstalled_by: TxnIndex) {
        let event = LogEvent::DependencyResolve {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            depender_tx: txn_id,
            on_tx: unstalled_by,
            resolve_cause: "OnTxExecuted".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
        
        // 同时记录为TaskResume用于挂起分析
        self.log_task_resume(
            txn_id,
            1, // 兼容性的默认incarnation值
            "StallRemoved",
            Some(unstalled_by),
        );
    }

    /// 调度器任务分配日志方法（不包含transaction_id字段）
    pub fn log_scheduler_task_assignment(&self) {
        let event = LogEvent::SchedulerMetric {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            metric_name: "scheduler_task_assignment".to_string(),
            metric_value: 1.0,
            metric_unit: "count".to_string(),
            measurement_context: "scheduler".to_string(),
            additional_data: HashMap::new(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录任务挂起事件
    /// 
    /// 记录交易任务被挂起的原因和相关信息
    pub fn log_task_suspend(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        suspend_reason: &str,
        depends_on_tx: Option<TxnIndex>,
    ) {
        let event = LogEvent::TaskSuspend {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            suspend_reason: suspend_reason.to_string(),
            depends_on_tx,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录任务恢复事件
    /// 
    /// 记录交易任务恢复执行的原因和相关信息
    pub fn log_task_resume(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        resume_reason: &str,
        resolved_by_tx: Option<TxnIndex>,
    ) {
        let event = LogEvent::TaskResume {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            resume_reason: resume_reason.to_string(),
            resolved_by_tx,
        };
        self.log_event(event, LogLevel::Debug);
    }

    // === BlockSTMv2增强功能方法 ===

    /// 记录停滞添加事件（BlockSTMv2）
    /// 
    /// 记录交易被加入停滞状态，包括停滞计数和是否首次停滞
    pub fn log_stall_add(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        stall_count_after: u32,
        first_stall: bool,
    ) {
        // 使用新的分离式统计API
        self.log_transaction_stall_add(txn_id, incarnation, by_tx, first_stall);
        
        // 为了兼容性，也保持原有的计数和日志
        self.increment_stall_events();
        self.log_detailed_stall_event(
            "StallAdd",
            txn_id,
            incarnation,
            by_tx,
            stall_count_after,
            first_stall,
            false, // became_unstalled只用于StallRemove
        );
        
        // 记录停滞开始时间用于持续时间计算
        // 仅在first_stall为true时记录开始时间（交易变为停滞状态）
        if first_stall {
            if let Ok(mut stall_times) = self.execution_stats.stall_start_times.lock() {
                stall_times.insert((txn_id, incarnation), std::time::Instant::now());
            }
        }
        
        let event = LogEvent::StallAdd {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            by_tx,
            stall_count_after,
            first_stall,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录停滞移除事件（BlockSTMv2）
    /// 
    /// 记录交易从停滞状态中移除，计算停滞持续时间
    pub fn log_stall_remove(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        stall_count_after: u32,
        became_unstalled: bool,
    ) {
        // 使用新的分离式统计API
        self.log_transaction_stall_remove(txn_id, incarnation, by_tx, became_unstalled);
        
        // 为了兼容性，也保持原有的日志
        self.log_detailed_stall_event(
            "StallRemove",
            txn_id,
            incarnation,
            by_tx,
            stall_count_after,
            false, // first_stall只用于StallAdd
            became_unstalled,
        );
        // 仅在became_unstalled为true时计算停滞持续时间（交易变为非停滞状态）
        if became_unstalled {
            if let Ok(mut stall_times) = self.execution_stats.stall_start_times.lock() {
                // 首先尝试精确匹配 (txn_id, incarnation)
                let mut start_time_opt = stall_times.remove(&(txn_id, incarnation));
                let mut matched_incarnation = incarnation;
                
                // 如果未找到，寻找该交易的任意incarnation
                if start_time_opt.is_none() {
                    // 为该txn_id寻找任意incarnation的停滞开始时间
                    let mut key_to_remove = None;
                    for &(tid, inc) in stall_times.keys() {
                        if tid == txn_id {
                            key_to_remove = Some((tid, inc));
                            matched_incarnation = inc;
                            break;
                        }
                    }
                    if let Some(key) = key_to_remove {
                        start_time_opt = stall_times.remove(&key);
                    }
                }
                
                if let Some(start_time) = start_time_opt {
                    let end_time = std::time::Instant::now();
                    let stall_duration_ns = start_time.elapsed().as_nanos() as u64;
                    self.record_stall_time(stall_duration_ns);
                    let stall_duration_us = stall_duration_ns as f64 / 1000.0;
                    
                    // 记录停滞持续时间到专用文件 - 使用匹配的incarnation
                    let match_type = if matched_incarnation == incarnation { "exact" } else { "fallback" };
                    self.log_stall_duration(
                        txn_id, 
                        matched_incarnation, 
                        start_time, 
                        end_time, 
                        stall_duration_us,
                        Some(incarnation), // 原始请求的incarnation
                        match_type
                    );
                } else {
                    // 记录找不到匹配开始时间的情况
                    self.log_stall_duration(
                        txn_id,
                        incarnation,
                        std::time::Instant::now(), // 占位时间
                        std::time::Instant::now(),
                        0.0, // 无法计算持续时间
                        Some(incarnation),
                        "notfound"
                    );
                }
            }
        }
        
        let event = LogEvent::StallRemove {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            by_tx,
            stall_count_after,
            became_unstalled,
        };
        self.log_event(event, LogLevel::Debug);
    }

    // === 分离式Stall日志方法 ===

    /// 记录交易级stall添加事件（新的分离式API）
    pub fn log_transaction_stall_add(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        first_stall: bool,
    ) {
        // 使用分离式统计
        let transaction_stall_count_before = self.get_transaction_stall_count();
        let transaction_stall_count_after = self.increment_transaction_stalls();
        self.increment_transaction_stall_by_txn(txn_id);
        
        // 记录事件到新的分离式日志文件
        let event = LogEvent::TransactionStallAdd {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            txn_id,
            incarnation,
            by_tx,
            transaction_stall_count_before,
            transaction_stall_count_after,
            first_stall,
            stall_transition: "UNSTALLED_TO_STALLED".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
        
        // 记录停滞开始时间用于持续时间计算
        if first_stall {
            if let Ok(mut stall_times) = self.execution_stats.stall_start_times.lock() {
                stall_times.insert((txn_id, incarnation), std::time::Instant::now());
            }
        }
    }

    /// 记录交易级stall移除事件（新的分离式API）
    pub fn log_transaction_stall_remove(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        became_unstalled: bool,
    ) {
        // 使用分离式统计
        let transaction_stall_count_before = self.get_transaction_stall_count();
        let transaction_stall_count_after = transaction_stall_count_before.saturating_sub(1);
        
        // 记录事件到新的分离式日志文件
        let event = LogEvent::TransactionStallRemove {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            txn_id,
            incarnation,
            by_tx,
            transaction_stall_count_before,
            transaction_stall_count_after,
            became_unstalled,
            stall_transition: "STALLED_TO_UNSTALLED".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
        
        // 计算并记录持续时间
        if became_unstalled {
            if let Ok(mut stall_times) = self.execution_stats.stall_start_times.lock() {
                if let Some(start_time) = stall_times.remove(&(txn_id, incarnation)) {
                    let duration_us = start_time.elapsed().as_micros() as u64;
                    self.record_transaction_stall_duration(duration_us);
                }
            }
        }
    }

    /// 记录系统级stall开始事件
    pub fn log_system_stall_start(
        &self,
        worker_id: u32,
        total_workers: u32,
        reason: &str,
    ) {
        // 使用分离式统计
        let system_stall_count_before = self.get_system_stall_count();
        let system_stall_count_after = self.increment_system_stalls();
        self.increment_system_stall_by_worker(worker_id);
        
        // 记录开始时间
        self.start_system_stall(worker_id);
        
        // 记录事件到新的分离式日志文件
        let event = LogEvent::SystemStallStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            worker_id,
            total_workers,
            reason: reason.to_string(),
            scheduler_state: "ALL_TRANSACTIONS_WAITING".to_string(),
            system_stall_count_before,
            system_stall_count_after,
            stall_transition: "ACTIVE_TO_STALLED".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录系统级stall结束事件
    pub fn log_system_stall_end(
        &self,
        worker_id: u32,
        total_workers: u32,
        reason: &str,
    ) {
        // 使用分离式统计并计算持续时间
        let system_stall_count_before = self.get_system_stall_count();
        let system_stall_count_after = system_stall_count_before.saturating_sub(1);
        let duration_us = self.end_system_stall(worker_id).unwrap_or(0);
        
        // 记录事件到新的分离式日志文件
        let event = LogEvent::SystemStallEnd {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            worker_id,
            total_workers,
            reason: reason.to_string(),
            scheduler_state: "TASK_DISPATCHED".to_string(),
            system_stall_count_before,
            system_stall_count_after,
            stall_transition: "STALLED_TO_ACTIVE".to_string(),
            duration_us,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录详细的停滞事件信息
    /// 
    /// 为Block-STM v2的两阶段stall策略记录每个stall/unstall事件的详细信息
    pub fn log_detailed_stall_event(
        &self,
        event_type: &str,           // "StallAdd" or "StallRemove"
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,           // 导致stall/unstall的上游交易
        stall_count_after: u32,    // 操作后的stall计数
        first_stall: bool,         // 是否为首次stall（只用于StallAdd）
        became_unstalled: bool,    // 是否变为unstalled（只用于StallRemove）
    ) {
        // 计算当前系统时间戳
        let current_system_time = std::time::SystemTime::now();
        let duration_since_epoch = current_system_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let timestamp_us = duration_since_epoch.as_micros() as u64;

        // 创建基础记录
        let mut stall_event_record = serde_json::json!({
            "event_type": event_type,
            "timestamp_us": timestamp_us,
            "thread_id": Self::current_thread_id(),
            "txn_id": txn_id,
            "incarnation": incarnation,
            "by_tx": by_tx,
            "stall_count_before": if event_type == "StallAdd" { 
                stall_count_after.saturating_sub(1) 
            } else { 
                stall_count_after + 1 
            },
            "stall_count_after": stall_count_after,
        });

        // 添加事件特定字段，避免null值
        if event_type == "StallAdd" {
            stall_event_record["first_stall"] = serde_json::Value::Bool(first_stall);
            stall_event_record["stall_transition"] = serde_json::Value::String(
                if first_stall { "UNSTALLED_TO_STALLED".to_string() } 
                else { "STALLED_TO_MORE_STALLED".to_string() }
            );
        } else if event_type == "StallRemove" {
            stall_event_record["became_unstalled"] = serde_json::Value::Bool(became_unstalled);
            stall_event_record["stall_transition"] = serde_json::Value::String(
                if became_unstalled { "STALLED_TO_UNSTALLED".to_string() } 
                else { "MORE_STALLED_TO_STALLED".to_string() }
            );
        }

        // 直接写入到 stall_events.ndjson 文件
        if let Ok(mut writers) = self.writers.lock() {
            if let Some(writer) = writers.get_mut(&LogFileType::StallEvents) {
                if let Ok(json_str) = serde_json::to_string(&stall_event_record) {
                    let _ = writeln!(writer, "{}", json_str);
                    let _ = writer.flush();
                }
            }
        }
    }

    /// 记录精确时间戳的停滞持续时间
    /// 
    /// 将停滞持续时间信息记录到stall_events.ndjson文件，作为停滞期间结束事件
    pub fn log_stall_duration(
        &self,
        txn_id: TxnIndex,
        matched_incarnation: Incarnation,  // 实际匹配到的incarnation
        start_time: std::time::Instant,
        end_time: std::time::Instant,
        duration_us: f64,
        original_incarnation: Option<Incarnation>, // 原始请求的incarnation
        match_type: &str,                 // 匹配类型："exact" | "fallback" | "notfound"
    ) {
        // 基于当前系统时间计算精确的时间戳
        let current_system_time = std::time::SystemTime::now();
        let duration_since_epoch = current_system_time
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let current_timestamp_us = duration_since_epoch.as_micros() as u64;
        
        // 计算开始和结束时间戳
        let elapsed_from_start = end_time.duration_since(start_time).as_micros() as u64;
        let end_timestamp_us = current_timestamp_us;
        let start_timestamp_us = end_timestamp_us - elapsed_from_start;

        let stall_duration_record = serde_json::json!({
            "event_type": "StallDuration",
            "timestamp_us": end_timestamp_us,
            "thread_id": Self::current_thread_id(),
            "txn_id": txn_id,
            "matched_incarnation": matched_incarnation,
            "original_incarnation": original_incarnation,
            "match_type": match_type,
            "start_timestamp_us": start_timestamp_us,
            "end_timestamp_us": end_timestamp_us,
            "duration_us": duration_us,
            "duration_ns": (duration_us * 1000.0) as u64,
            "stall_period_type": "FULL_STALL_PERIOD", // 标识这是完整的stall period记录
            "incarnation_match_success": match_type != "notfound"
        });

        // 写入到 stall_events.ndjson 文件，与其他 stall 事件合并
        if let Ok(mut writers) = self.writers.lock() {
            if let Some(writer) = writers.get_mut(&LogFileType::StallEvents) {
                if let Ok(json_str) = serde_json::to_string(&stall_duration_record) {
                    let _ = writeln!(writer, "{}", json_str);
                    let _ = writer.flush();
                }
            }
        }
    }

    /// 记录停滞传播tick事件（BlockSTMv2）
    /// 
    /// 记录停滞状态在交易间的批量传播过程
    pub fn log_stall_propagate_tick(
        &self,
        source_tx: TxnIndex,
        affected_range: Vec<TxnIndex>,
        propagated_count: u32,
        propagation_type: &str,
    ) {
        let event = LogEvent::StallPropagateTick {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            source_tx,
            affected_range,
            propagated_count,
            propagation_type: propagation_type.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录水位线推进事件（BlockSTMv2）
    /// 
    /// 记录执行水位线的推进情况，用于跟踪执行进度
    pub fn log_waterline_advance(
        &self,
        executed_once_max_idx_after: TxnIndex,
        advanced_by_tx: TxnIndex,
        previous_waterline: TxnIndex,
    ) {
        let event = LogEvent::WaterlineAdvance {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            executed_once_max_idx_after,
            advanced_by_tx,
            previous_waterline,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录中止开始事件（BlockSTMv2两阶段中止）
    /// 
    /// 记录交易中止过程的开始阶段
    pub fn log_abort_start(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        result: &str,
    ) {
        let event = LogEvent::AbortStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            by_tx,
            result: result.to_string(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录中止完成事件（BlockSTMv2两阶段中止）
    /// 
    /// 记录交易中止过程的完成阶段和后续操作
    pub fn log_abort_finish(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        by_tx: TxnIndex,
        result: &str,
        new_incarnation: Option<Incarnation>,
    ) {
        let event = LogEvent::AbortFinish {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            by_tx,
            result: result.to_string(),
            new_incarnation,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录无效化边事件（BlockSTMv2 AbortManager）
    /// 
    /// 记录交易间的无效化依赖关系，用于中止管理
    pub fn log_invalidation_edge(
        &self,
        by_tx: TxnIndex,
        to_tx: TxnIndex,
        to_incarnation: Option<Incarnation>,
        key: Option<String>,
    ) {
        let event = LogEvent::InvalidationEdge {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            by_tx,
            to_tx,
            to_incarnation,
            key,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录提交标记转换事件（BlockSTMv2）
    /// 
    /// 记录交易提交状态的转换过程
    pub fn log_commit_marker_transition(
        &self,
        txn_id: TxnIndex,
        from_marker: &str,
        to_marker: &str,
    ) {
        let event = LogEvent::CommitMarkerTransition {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            from_marker: from_marker.to_string(),
            to_marker: to_marker.to_string(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录后提交开始事件（BlockSTMv2并行后处理）
    /// 
    /// 记录交易提交后的并行后处理开始
    pub fn log_post_commit_start(
        &self,
        txn_id: TxnIndex,
        hook_kind: &str,
    ) {
        let event = LogEvent::PostCommitStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            hook_kind: hook_kind.to_string(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录后提交完成事件（BlockSTMv2并行后处理）
    /// 
    /// 记录交易提交后的并行后处理完成
    pub fn log_post_commit_finish(
        &self,
        txn_id: TxnIndex,
        hook_kind: &str,
        duration_us: u64,
    ) {
        let event = LogEvent::PostCommitFinish {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            hook_kind: hook_kind.to_string(),
            duration_us,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// 记录任务选取V2事件（BlockSTMv2增强任务跟踪）
    /// 
    /// 记录增强版本的任务选取事件，包含更多的跟踪信息
    pub fn log_task_picked_v2(
        &self,
        task_kind: &str,
        tx_index: TxnIndex,
        incarnation: Incarnation,
        is_first_execution: bool,
        from_queue: &str,
    ) {
        let timestamp = Self::current_timestamp_us();
        let event = LogEvent::TaskPickedV2 {
            timestamp,
            picked_ts_us: timestamp,
            thread_id: Self::current_thread_id(),
            task_kind: task_kind.to_string(),
            tx_index,
            incarnation,
            is_first_execution,
            from_queue: from_queue.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// 记录任务完成V2事件（BlockSTMv2增强任务跟踪）
    /// 
    /// 记录增强版本的任务完成事件，包含详细的执行信息
    pub fn log_task_finished_v2(
        &self,
        task_kind: &str,
        tx_index: TxnIndex,
        incarnation: Incarnation,
        result: &str,
        processing_time_us: u64,
        is_reexecution: bool,
        first_reexecution_deferred: bool,
        executed_once_watermark: TxnIndex,
        defer_reason: Option<String>,
    ) {
        let timestamp = Self::current_timestamp_us();
        let event = LogEvent::TaskFinishedV2 {
            timestamp,
            finished_ts_us: timestamp,
            thread_id: Self::current_thread_id(),
            task_kind: task_kind.to_string(),
            tx_index,
            incarnation,
            result: result.to_string(),
            processing_time_us,
            is_reexecution,
            first_reexecution_deferred,
            executed_once_watermark,
            defer_reason,
        };
        self.log_event(event, LogLevel::Debug);
    }
}

// === 全局日志记录器实例管理 ===

use std::sync::OnceLock;
static GLOBAL_LOGGER: OnceLock<Arc<BlockSTMLogger>> = OnceLock::new();

/// 使用给定配置初始化全局日志记录器
/// 
/// 创建并设置全局日志记录器实例，供整个程序使用
pub fn init_global_logger(config: LoggingConfig) -> std::io::Result<()> {
    let logger = BlockSTMLogger::new(config)?;
    GLOBAL_LOGGER.set(Arc::new(logger)).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "全局日志记录器已初始化")
    })?;
    Ok(())
}

/// 获取全局日志记录器实例
/// 
/// 返回已初始化的全局日志记录器，如果未初始化则返回None
pub fn get_global_logger() -> Option<Arc<BlockSTMLogger>> {
    GLOBAL_LOGGER.get().cloned()
}

// === 日志记录便利宏 ===

#[macro_export]
macro_rules! log_block_start {
    ($block_id:expr, $dataset:expr, $tx_count:expr, $concurrency_level:expr, $sample_period_ms:expr, $read_sample_rate:expr, $source_csv:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_block_start($block_id, $dataset, $tx_count, $concurrency_level, $sample_period_ms, $read_sample_rate, $source_csv);
        }
    };
}

#[macro_export]
macro_rules! log_scheduler_state_transition {
    ($old_state:expr, $new_state:expr, $txn_id:expr, $incarnation:expr, $trigger_reason:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_scheduler_state_transition($old_state, $new_state, $txn_id, $incarnation, $trigger_reason);
        }
    };
}

#[macro_export]
macro_rules! log_execution_finish {
    ($txn_id:expr, $incarnation:expr, $result:expr, $exec_duration_us:expr, $gas_used:expr, $read_set_size:expr, $write_set_size:expr, $resource_reads:expr, $resource_writes:expr, $module_reads:expr, $module_writes:expr, $delayed_field_reads:expr, $delayed_field_writes:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_execution_finish($txn_id, $incarnation, $result, $exec_duration_us, $gas_used, $read_set_size, $write_set_size, $resource_reads, $resource_writes, $module_reads, $module_writes, $delayed_field_reads, $delayed_field_writes);
        }
    };
}

