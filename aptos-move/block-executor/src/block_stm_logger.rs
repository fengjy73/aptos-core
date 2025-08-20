// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Block-STM detailed execution logging module
//!
//! This module provides comprehensive logging capabilities for tracking transaction execution,
//! concurrency control, read/write set changes, and performance metrics in Block-STM.

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

/// Configuration for Block-STM logging
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub log_dir: PathBuf,
    pub log_level: LogLevel,
    pub max_file_size: u64,
    pub buffer_size: usize,
    pub async_logging: bool,
    pub include_read_write_details: bool,
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

/// Log levels for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
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

/// Types of events that can be logged - Block-STM Core Events (31 types)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEvent {
    // === 区块生命周期与采样 ===
    
    // 1. BlockStart（每次区块执行开始）
    BlockStart {
        timestamp: u64,
        thread_id: u64,          // 0
        block_id: String,
        dataset: String,         // "ETH"|"USDT"
        tx_count: u32,
        concurrency_level: u32,
        sample_period_ms: u32,
        read_sample_rate: f64,
        source_csv: String,
        git_commit: String,
        build_profile: String,
    },
    
    // 2. BlockFinish（区块执行完成）
    BlockFinish {
        timestamp: u64,
        thread_id: u64,          // 0
        block_id: String,
        committed_count: u32,
        total_duration_us: u64,
        parallel_tps: f64,
        sequential_tps: f64,
    },
    
    // 3. SchedulerSample（每 SAMPLE_PERIOD_MS 触发）
    SchedulerSample {
        timestamp: u64,
        thread_id: u64,          // 0
        execution_min_index: TxnIndex,      // 已完成最终执行的连续前缀+1
        validation_min_index: TxnIndex,     // 已完成最终验证的连续前缀+1
        executing_tasks: u32,
        validating_tasks: u32,
        active_tasks: u32,
        done_marker: bool,                  // 是否满足commit gate判定
        scheduler_efficiency: f64,          // 调度效率百分比
        avg_task_queue_depth: f64,
        parallelism_utilization: f64,       // 并行度利用率
        contention_hotspots: Vec<String>,   // 竞争热点状态键
    },

    // === 调度/状态机（延续现有风格） ===
    
    // 4. SchedulerStateTransition（核心状态转换）
    SchedulerStateTransition {
        timestamp: u64,
        thread_id: u64,
        transaction_id: Option<TxnIndex>,
        incarnation: Option<Incarnation>,
        old_state: String,       // 使用实际枚举值: 经典Ready|Executing|Suspended|Executed|Committed|Aborting|ExecutionHalted, V2: PendingScheduling|Executing|Executed|Aborted|Committed
        new_state: String,
        trigger_reason: String,  // ExecutionTaskDispatched|ValidationTaskDispatched|DependencyEncountered|DependencyResolved|ValidationFailed|ValidationPassed|AbortRequested|IncarnationIncremented|ExternalHalt
    },

    // 5. TaskPicked（任务被工作线程领取）
    TaskPicked {
        timestamp: u64,
        thread_id: u64,
        kind: String,            // "Execution"|"Validation"  
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        priority: Option<u32>,
    },
    
    // 6. TaskFinished（任务完成）
    TaskFinished {
        timestamp: u64,
        thread_id: u64,
        kind: String,            // "Execution"|"Validation"
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        result: String,          // "Ok"|"Fail"
        processing_time_us: u64,
    },

    // === MVHashMap / 估计版本（Block-STM核心） ===
    
    // 7. MVRead（按 READ_SAMPLE_RATE 抽样）
    MVRead {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        state_key: String,       // 实际的state key或数字ID
        read_from: String,       // "Storage"|"MVCommitted"|"Uncommitted"|"NotFound"
        writer_tx: Option<TxnIndex>,
        writer_incarnation: Option<Incarnation>,
        is_estimate: bool,       // FLAG_ESTIMATE状态
        value_size: Option<usize>,
    },
    
    // 8. MVWrite（MV存储写入）
    MVWrite {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        state_key: String,
        value_size: usize,
        write_type: String,      // "Create", "Modify", "Delete"
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
        state_key: String,
        depends_on_tx: TxnIndex, // 依赖的低序号写者
        depends_on_incarnation: Option<Incarnation>,
    },
    
    // 12. DependencyResolve（解除依赖边）
    DependencyResolve {
        timestamp: u64,
        thread_id: u64,
        depender_tx: TxnIndex,   // 被解除依赖的交易
        state_key: String,
        on_tx: TxnIndex,         // 被依赖的交易
        resolve_cause: String,   // "OnTxCommitted"|"OnTxAborted"|"OnTxExecuted"
        wait_time_us: u64,       // 等待时长
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
    
    // 16. ValidationFinish（验证完成）
    ValidationFinish {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        result: String,          // "Pass"|"Fail"
        val_duration_us: u64,
        validated_reads: usize,
        conflicts_found: usize,
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
    
    // 29. MemoryUsageSnapshot（内存使用统计）
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
    
    // 30. LockContention（锁竞争统计）
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
    
    // 31. StallPropagation（Stall传播分析）
    StallPropagation {
        timestamp: u64,
        thread_id: u64,
        owner_txn: TxnIndex,
        owner_incarnation: Option<Incarnation>,
        affected_txns: Vec<TxnIndex>,
        affected_incarnations: Vec<Incarnation>,
        propagation_type: String, // "Stall"|"Unstall"
        reason: String,
        state_key: String,       // 导致stall的state key
        stall_depth: u32,        // stall传播深度
        affected_count: u32,
        propagation_latency_us: u64, // 传播延迟
        stall_chain_id: Option<String>, // stall链标识
    },

    // 32. TaskSuspend（任务挂起）
    TaskSuspend {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        suspend_reason: String,  // "DependencyEncountered"|"ResourceContention"|"StallPropagation"
        depends_on_tx: Option<TxnIndex>,
        state_key: Option<String>,
        suspend_duration_us: Option<u64>,
    },

    // 33. TaskResume（任务恢复）
    TaskResume {
        timestamp: u64,
        thread_id: u64,
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        resume_reason: String,   // "DependencyResolved"|"StallRemoved"|"Rescheduled"
        resolved_by_tx: Option<TxnIndex>,
        total_suspended_time_us: u64,
    },
}

/// Log file types - Enhanced 8-category classification
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LogFileType {
    BlockSummary,      // BlockStart/Finish, SchedulerSample, MemoryUsageSnapshot
    SchedulerStates,   // SchedulerStateTransition, TaskPicked/Finished, LockContention
    MVHashMapOps,      // MVRead/Write, EstimateMark/Clear
    Dependencies,      // DependencyBlock/Resolve, StallPropagation
    ExecutionFlow,     // ExecutionStart/Finish, ValidationStart/Finish, TransactionOutputDetail
    AbortRecovery,     // AbortInitiated, IncarnationIncrement, RescheduleHigher, ValidationConflict
    DetailedOperations, // AggregatorOperation, DelayedFieldOperation, ResourceGroupOperation, ReadWriteSetChange
    SystemOperations,  // ModuleCacheOperation, EventEmission, PerformanceMetric
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
        }
    }
}

/// Thread-safe logger for Block-STM events
pub struct BlockSTMLogger {
    config: LoggingConfig,
    writers: Arc<Mutex<HashMap<LogFileType, BufWriter<File>>>>,
    _start_time: Instant,
}

impl BlockSTMLogger {
    /// Create a new logger with the given configuration
    pub fn new(config: LoggingConfig) -> std::io::Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                writers: Arc::new(Mutex::new(HashMap::new())),
                _start_time: Instant::now(),
            });
        }

        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&config.log_dir)?;

        let mut writers = HashMap::new();
        
        // Initialize all log files
        for file_type in [
            LogFileType::BlockSummary,
            LogFileType::SchedulerStates,
            LogFileType::MVHashMapOps,
            LogFileType::Dependencies,
            LogFileType::ExecutionFlow,
            LogFileType::AbortRecovery,
            LogFileType::DetailedOperations,
            LogFileType::SystemOperations,
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
        })
    }

    /// Create a logger with default configuration
    pub fn with_default_config() -> std::io::Result<Self> {
        Self::new(LoggingConfig::default())
    }


    /// Log an event to the appropriate file
    pub fn log_event(&self, event: LogEvent, level: LogLevel) {
        if !self.config.enabled || level < self.config.log_level {
            return;
        }

        let file_type = match &event {
            // BlockSummary: BlockStart/Finish, SchedulerSample, MemoryUsageSnapshot
            LogEvent::BlockStart { .. }
            | LogEvent::BlockFinish { .. }
            | LogEvent::SchedulerSample { .. }
            | LogEvent::MemoryUsageSnapshot { .. } => LogFileType::BlockSummary,
            
            // SchedulerStates: SchedulerStateTransition, TaskPicked/Finished, TaskSuspend/Resume, LockContention
            LogEvent::SchedulerStateTransition { .. }
            | LogEvent::TaskPicked { .. }
            | LogEvent::TaskFinished { .. }
            | LogEvent::TaskSuspend { .. }
            | LogEvent::TaskResume { .. }
            | LogEvent::LockContention { .. } => LogFileType::SchedulerStates,
            
            // MVHashMapOps: MVRead/Write, EstimateMark/Clear
            LogEvent::MVRead { .. }
            | LogEvent::MVWrite { .. }
            | LogEvent::EstimateMark { .. }
            | LogEvent::EstimateClear { .. } => LogFileType::MVHashMapOps,
            
            // Dependencies: DependencyBlock/Resolve, StallPropagation
            LogEvent::DependencyBlock { .. }
            | LogEvent::DependencyResolve { .. }
            | LogEvent::StallPropagation { .. } => LogFileType::Dependencies,
            
            // ExecutionFlow: ExecutionStart/Finish, ValidationStart/Finish, TransactionOutputDetail
            LogEvent::ExecutionStart { .. }
            | LogEvent::ExecutionFinish { .. }
            | LogEvent::ValidationStart { .. }
            | LogEvent::ValidationFinish { .. }
            | LogEvent::TransactionOutputDetail { .. } => LogFileType::ExecutionFlow,
            
            // AbortRecovery: AbortInitiated, IncarnationIncrement, RescheduleHigher, ValidationConflict
            LogEvent::AbortInitiated { .. }
            | LogEvent::IncarnationIncrement { .. }
            | LogEvent::RescheduleHigher { .. }
            | LogEvent::ValidationConflict { .. } => LogFileType::AbortRecovery,
            
            // DetailedOperations: AggregatorOperation, DelayedFieldOperation, ResourceGroupOperation, ReadWriteSetChange
            LogEvent::AggregatorOperation { .. }
            | LogEvent::DelayedFieldOperation { .. }
            | LogEvent::ResourceGroupOperation { .. }
            | LogEvent::ReadWriteSetChange { .. } => LogFileType::DetailedOperations,
            
            // SystemOperations: ModuleCacheOperation, EventEmission, PerformanceMetric
            LogEvent::ModuleCacheOperation { .. }
            | LogEvent::EventEmission { .. }
            | LogEvent::PerformanceMetric { .. } => LogFileType::SystemOperations,
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

    // === Block lifecycle events ===
    
    /// Log block start event
    pub fn log_block_start(
        &self,
        block_id: &str,
        dataset: &str,
        tx_count: u32,
        concurrency_level: u32,
        sample_period_ms: u32,
        read_sample_rate: f64,
        source_csv: &str,
        git_commit: &str,
        build_profile: &str,
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
            git_commit: git_commit.to_string(),
            build_profile: build_profile.to_string(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Log block finish event
    pub fn log_block_finish(
        &self,
        block_id: &str,
        committed_count: u32,
        total_duration_us: u64,
        parallel_tps: f64,
        sequential_tps: f64,
    ) {
        let event = LogEvent::BlockFinish {
            timestamp: Self::current_timestamp_us(),
            thread_id: 0,
            block_id: block_id.to_string(),
            committed_count,
            total_duration_us,
            parallel_tps,
            sequential_tps,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Log scheduler sample event
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

    // === Scheduler state transitions ===

    /// Log scheduler state transition
    pub fn log_scheduler_state_transition(
        &self,
        old_state: &str,
        new_state: &str,
        transaction_id: Option<TxnIndex>,
        incarnation: Option<Incarnation>,
        trigger_reason: &str,
    ) {
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

    /// Log execution start event
    pub fn log_execution_start(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        execution_phase: &str,
    ) {
        let event = LogEvent::ExecutionStart {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            execution_phase: execution_phase.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log execution finish event
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

    /// Log MV read operation (with sampling)
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
            return;
        }
        
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
    
    /// Log MVWrite event
    pub fn log_mv_write(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        state_key: &str,
        value_size: usize,
        write_type: &str,
    ) {
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

    // === Helper methods ===

    /// Check if read operations should be sampled
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

    /// Get current timestamp in microseconds since start
    pub fn current_timestamp_us() -> u64 {
        use std::sync::OnceLock;
        static START_TIME: OnceLock<std::time::Instant> = OnceLock::new();
        
        let start_time = START_TIME.get_or_init(|| std::time::Instant::now());
        start_time.elapsed().as_micros() as u64
    }

    /// Get current thread ID
    pub fn current_thread_id() -> u64 {
        use std::hash::{Hash, Hasher};
        let thread_id = thread::current().id();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        thread_id.hash(&mut hasher);
        hasher.finish()
    }

    /// Flush all writers
    pub fn flush(&self) {
        if let Ok(mut writers) = self.writers.lock() {
            for writer in writers.values_mut() {
                let _ = writer.flush();
            }
        }
    }

    // === Additional core event methods ===

    /// Log transaction abort event
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

    // === Backward compatibility methods ===
    
    /// Legacy method for transaction start (backward compatibility)
    pub fn log_transaction_start(&self, txn_id: TxnIndex, incarnation: Incarnation) {
        self.log_execution_start(txn_id, incarnation, "Initial");
    }

    /// Legacy method for execution state transition (backward compatibility)  
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

    /// Legacy method for transaction commit
    pub fn log_transaction_commit(&self, txn_id: TxnIndex, incarnation: Incarnation) {
        self.log_execution_finish(txn_id, incarnation, "Committed", 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    }

    /// Legacy method for transaction finish  
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

    /// Legacy method for transaction abort (backward compatibility)
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

    /// Legacy method for stall propagation (backward compatibility)
    pub fn log_stall_propagation(
        &self,
        owner_txn: TxnIndex,
        affected_txns: Vec<TxnIndex>,
        propagation_type: &str,
        reason: &str,
    ) {
        let event = LogEvent::StallPropagation {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            owner_txn,
            owner_incarnation: None,
            affected_txns: affected_txns.clone(),
            affected_incarnations: vec![],
            propagation_type: propagation_type.to_string(),
            reason: reason.to_string(),
            state_key: "unknown".to_string(),
            stall_depth: 1,
            affected_count: affected_txns.len() as u32,
            propagation_latency_us: 0,
            stall_chain_id: None,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Legacy method for task dispatch (backward compatibility)
    pub fn log_task_dispatch(&self, txn_id: TxnIndex, incarnation: Incarnation, _task_type: &str, description: &str) {
        self.log_scheduler_state_transition("IDLE", "DISPATCHING", Some(txn_id), Some(incarnation), description);
    }

    /// Legacy method for validation (backward compatibility)
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
            val_duration_us: 0,
            validated_reads: 0,
            conflicts_found: if validation_result { 0 } else { 1 },
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Legacy method for read/write set change (backward compatibility)
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

    /// Legacy method for performance metric (backward compatibility)
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
            incarnation: None,
            metric_unit: "count".to_string(),
            measurement_context: "general".to_string(),
            additional_data,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Legacy method for dependency stall (backward compatibility)
    pub fn log_dependency_stall(&self, txn_id: TxnIndex, stalled_by: Vec<TxnIndex>) {
        if let Some(owner_tx) = stalled_by.first() {
            let event = LogEvent::DependencyBlock {
                timestamp: Self::current_timestamp_us(),
                thread_id: Self::current_thread_id(),
                transaction_id: txn_id,
                incarnation: 1,
                state_key: "unknown".to_string(),
                depends_on_tx: *owner_tx,
                depends_on_incarnation: None,
            };
            self.log_event(event, LogLevel::Debug);
            
            // Also log as TaskSuspend for suspend analysis
            self.log_task_suspend(
                txn_id,
                1, // Use same incarnation as DependencyBlock
                "StallPropagation",
                Some(*owner_tx),
                None,
            );
        }
    }

    /// Legacy method for dependency unstall (backward compatibility)
    pub fn log_dependency_unstall(&self, txn_id: TxnIndex, unstalled_by: TxnIndex) {
        let event = LogEvent::DependencyResolve {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            depender_tx: txn_id,
            state_key: "unknown".to_string(),
            on_tx: unstalled_by,
            resolve_cause: "OnTxExecuted".to_string(),
            wait_time_us: 0,
        };
        self.log_event(event, LogLevel::Debug);
        
        // Also log as TaskResume for suspend analysis
        self.log_task_resume(
            txn_id,
            1, // Default incarnation for compatibility
            "StallRemoved",
            Some(unstalled_by),
            0, // Duration not tracked in legacy method
        );
    }

    /// Legacy method for scheduler task assignment (backward compatibility)
    pub fn log_scheduler_task_assignment(&self) {
        self.log_performance_metric("scheduler_task_assignment", 1.0, None, HashMap::new());
    }

    /// Log task suspend event
    pub fn log_task_suspend(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        suspend_reason: &str,
        depends_on_tx: Option<TxnIndex>,
        state_key: Option<String>,
    ) {
        let event = LogEvent::TaskSuspend {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            suspend_reason: suspend_reason.to_string(),
            depends_on_tx,
            state_key,
            suspend_duration_us: None, // Will be calculated on resume
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log task resume event
    pub fn log_task_resume(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        resume_reason: &str,
        resolved_by_tx: Option<TxnIndex>,
        total_suspended_time_us: u64,
    ) {
        let event = LogEvent::TaskResume {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            transaction_id: txn_id,
            incarnation,
            resume_reason: resume_reason.to_string(),
            resolved_by_tx,
            total_suspended_time_us,
        };
        self.log_event(event, LogLevel::Debug);
    }
}

// === Global logger instance management ===

use std::sync::OnceLock;
static GLOBAL_LOGGER: OnceLock<Arc<BlockSTMLogger>> = OnceLock::new();

/// Initialize global logger with the given configuration
pub fn init_global_logger(config: LoggingConfig) -> std::io::Result<()> {
    let logger = BlockSTMLogger::new(config)?;
    GLOBAL_LOGGER.set(Arc::new(logger)).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Global logger already initialized")
    })?;
    Ok(())
}

/// Get the global logger instance
pub fn get_global_logger() -> Option<Arc<BlockSTMLogger>> {
    GLOBAL_LOGGER.get().cloned()
}

// === Macros for convenient logging ===

#[macro_export]
macro_rules! log_block_start {
    ($block_id:expr, $dataset:expr, $tx_count:expr, $concurrency_level:expr, $sample_period_ms:expr, $read_sample_rate:expr, $source_csv:expr, $git_commit:expr, $build_profile:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_block_start($block_id, $dataset, $tx_count, $concurrency_level, $sample_period_ms, $read_sample_rate, $source_csv, $git_commit, $build_profile);
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

