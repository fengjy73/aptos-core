// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Block-STM detailed execution logging module
//!
//! This module provides comprehensive logging capabilities for tracking transaction execution,
//! concurrency control, read/write set changes, and performance metrics in Block-STM.

use crate::types::{Incarnation, TxnIndex};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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

/// Types of events that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum LogEvent {
    TransactionStart {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
    },
    TransactionFinish {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        execution_result: String,
        duration_us: u64,
        gas_used: u64,
        read_set_size: usize,
        write_set_size: usize,
    },
    TransactionAbort {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        abort_reason: String,
        retry_count: u32,
        dependencies: Vec<TxnIndex>,
    },
    TransactionValidate {
        transaction_id: TxnIndex,
        thread_id: u64,
        timestamp: u64,
        validation_result: bool,
        duration_us: u64,
    },
    DependencyStall {
        transaction_id: TxnIndex,
        thread_id: u64,
        timestamp: u64,
        stalled_by: Vec<TxnIndex>,
    },
    DependencyUnstall {
        transaction_id: TxnIndex,
        thread_id: u64,
        timestamp: u64,
        unstalled_by: TxnIndex,
    },
    ReadWriteConflict {
        transaction_id: TxnIndex,
        conflicting_txn: TxnIndex,
        thread_id: u64,
        timestamp: u64,
        conflict_key: String,
        conflict_type: String, // "read_after_write", "write_after_read", "write_after_write"
    },
    ReadWriteSetChange {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        read_keys: Vec<String>,
        write_keys: Vec<String>,
        resource_reads: usize,
        resource_writes: usize,
        module_reads: usize,
        module_writes: usize,
        delayed_field_reads: usize,
        delayed_field_writes: usize,
    },
    PerformanceMetric {
        timestamp: u64,
        metric_name: String,
        metric_value: f64,
        transaction_id: Option<TxnIndex>,
        thread_id: u64,
        additional_data: HashMap<String, String>,
    },
    // 新增的8种日志事件类型
    MVHashMapRead {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        state_key: String,
        read_result: String, // "Found(version)" 或 "NotFound" 或 "Dependency"
        version: Option<TxnIndex>,
    },
    MVHashMapWrite {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        state_key: String,
        value_size: usize,
        write_type: String, // "Insert" 或 "Update" 或 "Delete"
    },
    SchedulerStateTransition {
        timestamp: u64,
        thread_id: u64,
        old_state: String,
        new_state: String,
        transaction_id: Option<TxnIndex>,
        trigger_reason: String,
    },
    DependencyTracking {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        dependency_type: String, // "Read" 或 "Write" 或 "Validation"
        dependent_txn: TxnIndex,
        state_key: String,
        action: String, // "Add" 或 "Remove" 或 "Check"
    },
    StallPropagation {
        timestamp: u64,
        thread_id: u64,
        owner_txn: TxnIndex,
        affected_txns: Vec<TxnIndex>,
        propagation_type: String, // "Stall" 或 "Unstall"
        reason: String,
    },
    ExecutionStateTransition {
        transaction_id: TxnIndex,
        incarnation: Incarnation,
        thread_id: u64,
        timestamp: u64,
        old_state: String,
        new_state: String,
        transition_reason: String,
        execution_phase: String, // "Execute" 或 "Validate" 或 "Commit"
    },
    BlockExecutionFlow {
        timestamp: u64,
        block_id: u64,
        flow_event: String, // "Start" 或 "End" 或 "Phase"
        phase_name: String,
        transaction_count: usize,
        active_threads: usize,
        additional_info: HashMap<String, String>,
    },
    TransactionMapping {
        timestamp: u64,
        transaction_id: TxnIndex,
        original_index: usize,
        block_id: u64,
        mapping_type: String, // "CSV_to_TxnIndex" 或 "Address_to_Account"
        source_data: String,
        mapped_data: String,
    },
}

/// Log file types
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub enum LogFileType {
    Execution,
    Concurrency,
    ReadWrite,
    Performance,
    Summary,
}

impl LogFileType {
    fn filename(&self) -> &'static str {
        match self {
            LogFileType::Execution => "block_stm_execution.log",
            LogFileType::Concurrency => "block_stm_concurrency.log",
            LogFileType::ReadWrite => "block_stm_readwrite.log",
            LogFileType::Performance => "block_stm_performance.log",
            LogFileType::Summary => "block_stm_summary.log",
        }
    }
}

/// Thread-safe logger for Block-STM events
pub struct BlockSTMLogger {
    config: LoggingConfig,
    writers: Arc<Mutex<HashMap<LogFileType, BufWriter<File>>>>,
    start_time: Instant,
}

impl BlockSTMLogger {
    /// Create a new logger with the given configuration
    pub fn new(config: LoggingConfig) -> std::io::Result<Self> {
        if !config.enabled {
            return Ok(Self {
                config,
                writers: Arc::new(Mutex::new(HashMap::new())),
                start_time: Instant::now(),
            });
        }

        // Create log directory if it doesn't exist
        std::fs::create_dir_all(&config.log_dir)?;

        let mut writers = HashMap::new();
        
        // Initialize all log files
        for file_type in [
            LogFileType::Execution,
            LogFileType::Concurrency,
            LogFileType::ReadWrite,
            LogFileType::Performance,
            LogFileType::Summary,
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
            start_time: Instant::now(),
        })
    }

    /// Create a logger with default configuration
    pub fn with_default_config() -> std::io::Result<Self> {
        Self::new(LoggingConfig::default())
    }

    /// Get current timestamp in microseconds since epoch
    fn current_timestamp_us() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }

    /// Get current thread ID
    fn current_thread_id() -> u64 {
        // Use a hash of the thread ID since as_u64().get() is unstable
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        thread::current().id().hash(&mut hasher);
        hasher.finish()
    }

    /// Log an event to the appropriate file
    pub fn log_event(&self, event: LogEvent, level: LogLevel) {
        if !self.config.enabled || level < self.config.log_level {
            return;
        }

        let file_type = match &event {
            LogEvent::TransactionStart { .. }
            | LogEvent::TransactionFinish { .. }
            | LogEvent::ExecutionStateTransition { .. } => LogFileType::Execution,
            LogEvent::TransactionAbort { .. }
            | LogEvent::DependencyStall { .. }
            | LogEvent::DependencyUnstall { .. }
            | LogEvent::ReadWriteConflict { .. }
            | LogEvent::DependencyTracking { .. }
            | LogEvent::StallPropagation { .. }
            | LogEvent::SchedulerStateTransition { .. } => LogFileType::Concurrency,
            LogEvent::ReadWriteSetChange { .. }
            | LogEvent::MVHashMapRead { .. }
            | LogEvent::MVHashMapWrite { .. } => LogFileType::ReadWrite,
            LogEvent::PerformanceMetric { .. } => LogFileType::Performance,
            LogEvent::TransactionValidate { .. }
            | LogEvent::BlockExecutionFlow { .. }
            | LogEvent::TransactionMapping { .. } => LogFileType::Summary,
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

    /// Log transaction start event
    pub fn log_transaction_start(&self, txn_id: TxnIndex, incarnation: Incarnation) {
        let event = LogEvent::TransactionStart {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Log transaction finish event
    pub fn log_transaction_finish(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        execution_result: &str,
        duration: Duration,
        gas_used: u64,
        read_set_size: usize,
        write_set_size: usize,
    ) {
        let event = LogEvent::TransactionFinish {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            execution_result: execution_result.to_string(),
            duration_us: duration.as_micros() as u64,
            gas_used,
            read_set_size,
            write_set_size,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Log transaction abort event
    pub fn log_transaction_abort(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        abort_reason: &str,
        retry_count: u32,
        dependencies: Vec<TxnIndex>,
    ) {
        let event = LogEvent::TransactionAbort {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            abort_reason: abort_reason.to_string(),
            retry_count,
            dependencies,
        };
        self.log_event(event, LogLevel::Warn);
    }

    /// Log validation event
    pub fn log_validation(
        &self,
        txn_id: TxnIndex,
        validation_result: bool,
        duration: Duration,
    ) {
        let event = LogEvent::TransactionValidate {
            transaction_id: txn_id,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            validation_result,
            duration_us: duration.as_micros() as u64,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Log dependency stall event
    pub fn log_dependency_stall(&self, txn_id: TxnIndex, stalled_by: Vec<TxnIndex>) {
        let event = LogEvent::DependencyStall {
            transaction_id: txn_id,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            stalled_by,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log dependency unstall event
    pub fn log_dependency_unstall(&self, txn_id: TxnIndex, unstalled_by: TxnIndex) {
        let event = LogEvent::DependencyUnstall {
            transaction_id: txn_id,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            unstalled_by,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log read/write conflict
    pub fn log_readwrite_conflict(
        &self,
        txn_id: TxnIndex,
        conflicting_txn: TxnIndex,
        conflict_key: &str,
        conflict_type: &str,
    ) {
        let event = LogEvent::ReadWriteConflict {
            transaction_id: txn_id,
            conflicting_txn,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            conflict_key: conflict_key.to_string(),
            conflict_type: conflict_type.to_string(),
        };
        self.log_event(event, LogLevel::Warn);
    }

    /// Log read/write set changes
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
        if !self.config.include_read_write_details {
            return;
        }

        let event = LogEvent::ReadWriteSetChange {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            read_keys,
            write_keys,
            resource_reads,
            resource_writes,
            module_reads,
            module_writes,
            delayed_field_reads,
            delayed_field_writes,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log performance metric
    pub fn log_performance_metric(
        &self,
        metric_name: &str,
        metric_value: f64,
        txn_id: Option<TxnIndex>,
        additional_data: HashMap<String, String>,
    ) {
        let event = LogEvent::PerformanceMetric {
            timestamp: Self::current_timestamp_us(),
            metric_name: metric_name.to_string(),
            metric_value,
            transaction_id: txn_id,
            thread_id: Self::current_thread_id(),
            additional_data,
        };
        self.log_event(event, LogLevel::Info);
    }

    // 新增的8个日志记录方法
    
    /// Log MVHashMap read operation
    pub fn log_mvhashmap_read(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        state_key: &str,
        read_result: &str,
        version: Option<TxnIndex>,
    ) {
        let event = LogEvent::MVHashMapRead {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            state_key: state_key.to_string(),
            read_result: read_result.to_string(),
            version,
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log MVHashMap write operation
    pub fn log_mvhashmap_write(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        state_key: &str,
        value_size: usize,
        write_type: &str,
    ) {
        let event = LogEvent::MVHashMapWrite {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            state_key: state_key.to_string(),
            value_size,
            write_type: write_type.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log scheduler task assignment
    pub fn log_scheduler_task_assignment(&self) {
        let event = LogEvent::SchedulerStateTransition {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            old_state: "idle".to_string(),
            new_state: "task_assigned".to_string(),
            transaction_id: None,
            trigger_reason: "task_assignment".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log transaction commit
    pub fn log_transaction_commit(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
    ) {
        let event = LogEvent::ExecutionStateTransition {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            old_state: "executing".to_string(),
            new_state: "committed".to_string(),
            transition_reason: "commit_complete".to_string(),
            execution_phase: "Commit".to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log scheduler state transition
    pub fn log_scheduler_state_transition(
        &self,
        old_state: &str,
        new_state: &str,
        txn_id: Option<TxnIndex>,
        trigger_reason: &str,
    ) {
        let event = LogEvent::SchedulerStateTransition {
            timestamp: Self::current_timestamp_us(),
            thread_id: Self::current_thread_id(),
            old_state: old_state.to_string(),
            new_state: new_state.to_string(),
            transaction_id: txn_id,
            trigger_reason: trigger_reason.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log dependency tracking
    pub fn log_dependency_tracking(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        dependency_type: &str,
        dependent_txn: TxnIndex,
        state_key: &str,
        action: &str,
    ) {
        let event = LogEvent::DependencyTracking {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            dependency_type: dependency_type.to_string(),
            dependent_txn,
            state_key: state_key.to_string(),
            action: action.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log stall propagation
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
            affected_txns,
            propagation_type: propagation_type.to_string(),
            reason: reason.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log execution state transition
    pub fn log_execution_state_transition(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        old_state: &str,
        new_state: &str,
        transition_reason: &str,
        execution_phase: &str,
    ) {
        let event = LogEvent::ExecutionStateTransition {
            transaction_id: txn_id,
            incarnation,
            thread_id: Self::current_thread_id(),
            timestamp: Self::current_timestamp_us(),
            old_state: old_state.to_string(),
            new_state: new_state.to_string(),
            transition_reason: transition_reason.to_string(),
            execution_phase: execution_phase.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Log block execution flow
    pub fn log_block_execution_flow(
        &self,
        block_id: u64,
        flow_event: &str,
        phase_name: &str,
        transaction_count: usize,
        active_threads: usize,
        additional_info: HashMap<String, String>,
    ) {
        let event = LogEvent::BlockExecutionFlow {
            timestamp: Self::current_timestamp_us(),
            block_id,
            flow_event: flow_event.to_string(),
            phase_name: phase_name.to_string(),
            transaction_count,
            active_threads,
            additional_info,
        };
        self.log_event(event, LogLevel::Info);
    }

    /// Log transaction mapping
    pub fn log_transaction_mapping(
        &self,
        txn_id: TxnIndex,
        original_index: usize,
        block_id: u64,
        mapping_type: &str,
        source_data: &str,
        mapped_data: &str,
    ) {
        let event = LogEvent::TransactionMapping {
            timestamp: Self::current_timestamp_us(),
            transaction_id: txn_id,
            original_index,
            block_id,
            mapping_type: mapping_type.to_string(),
            source_data: source_data.to_string(),
            mapped_data: mapped_data.to_string(),
        };
        self.log_event(event, LogLevel::Debug);
    }

    /// Flush all buffers
    pub fn flush(&self) {
        if let Ok(mut writers) = self.writers.lock() {
            for writer in writers.values_mut() {
                let _ = writer.flush();
            }
        }
    }
}

impl Drop for BlockSTMLogger {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Global logger instance
static GLOBAL_LOGGER: std::sync::OnceLock<BlockSTMLogger> = std::sync::OnceLock::new();

/// Initialize the global logger
pub fn init_global_logger(config: LoggingConfig) -> std::io::Result<()> {
    let logger = BlockSTMLogger::new(config)?;
    GLOBAL_LOGGER
        .set(logger)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::AlreadyExists, "Logger already initialized"))?;
    Ok(())
}

/// Get the global logger instance
pub fn get_global_logger() -> Option<&'static BlockSTMLogger> {
    GLOBAL_LOGGER.get()
}

/// Convenience macros for logging
#[macro_export]
macro_rules! log_transaction_start {
    ($txn_id:expr, $incarnation:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_transaction_start($txn_id, $incarnation);
        }
    };
}

#[macro_export]
macro_rules! log_transaction_finish {
    ($txn_id:expr, $incarnation:expr, $result:expr, $duration:expr, $gas:expr, $read_size:expr, $write_size:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_transaction_finish($txn_id, $incarnation, $result, $duration, $gas, $read_size, $write_size);
        }
    };
}

#[macro_export]
macro_rules! log_transaction_abort {
    ($txn_id:expr, $incarnation:expr, $reason:expr, $retry:expr, $deps:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_transaction_abort($txn_id, $incarnation, $reason, $retry, $deps);
        }
    };
}

// 新增的8个日志宏定义

#[macro_export]
macro_rules! log_mvhashmap_read {
    ($txn_id:expr, $incarnation:expr, $state_key:expr, $read_result:expr, $version:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_mvhashmap_read($txn_id, $incarnation, $state_key, $read_result, $version);
        }
    };
}

#[macro_export]
macro_rules! log_mvhashmap_write {
    ($txn_id:expr, $incarnation:expr, $state_key:expr, $value_size:expr, $write_type:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_mvhashmap_write($txn_id, $incarnation, $state_key, $value_size, $write_type);
        }
    };
}

#[macro_export]
macro_rules! log_scheduler_state_transition {
    ($old_state:expr, $new_state:expr, $txn_id:expr, $trigger_reason:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_scheduler_state_transition($old_state, $new_state, $txn_id, $trigger_reason);
        }
    };
}

#[macro_export]
macro_rules! log_dependency_tracking {
    ($txn_id:expr, $incarnation:expr, $dependency_type:expr, $dependent_txn:expr, $state_key:expr, $action:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_dependency_tracking($txn_id, $incarnation, $dependency_type, $dependent_txn, $state_key, $action);
        }
    };
}

#[macro_export]
macro_rules! log_stall_propagation {
    ($owner_txn:expr, $affected_txns:expr, $propagation_type:expr, $reason:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_stall_propagation($owner_txn, $affected_txns, $propagation_type, $reason);
        }
    };
}

#[macro_export]
macro_rules! log_execution_state_transition {
    ($txn_id:expr, $incarnation:expr, $old_state:expr, $new_state:expr, $transition_reason:expr, $execution_phase:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_execution_state_transition($txn_id, $incarnation, $old_state, $new_state, $transition_reason, $execution_phase);
        }
    };
}

#[macro_export]
macro_rules! log_block_execution_flow {
    ($block_id:expr, $flow_event:expr, $phase_name:expr, $transaction_count:expr, $active_threads:expr, $additional_info:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_block_execution_flow($block_id, $flow_event, $phase_name, $transaction_count, $active_threads, $additional_info);
        }
    };
}

#[macro_export]
macro_rules! log_transaction_mapping {
    ($txn_id:expr, $original_index:expr, $block_id:expr, $mapping_type:expr, $source_data:expr, $mapped_data:expr) => {
        if let Some(logger) = $crate::block_stm_logger::get_global_logger() {
            logger.log_transaction_mapping($txn_id, $original_index, $block_id, $mapping_type, $source_data, $mapped_data);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_logger_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = LoggingConfig {
            enabled: true,
            log_dir: temp_dir.path().to_path_buf(),
            log_level: LogLevel::Debug,
            max_file_size: 1024 * 1024,
            buffer_size: 1024,
            async_logging: false,
            include_read_write_details: true,
        };

        let logger = BlockSTMLogger::new(config).unwrap();
        
        // Test logging
        logger.log_transaction_start(1, 0);
        logger.flush();

        // Check if log file was created
        let log_file = temp_dir.path().join("block_stm_execution.log");
        assert!(log_file.exists());
        
        let content = fs::read_to_string(log_file).unwrap();
        assert!(content.contains("TransactionStart"));
    }

    #[test]
    fn test_log_levels() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_log_level_parsing() {
        assert_eq!("DEBUG".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("WARN".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("ERROR".parse::<LogLevel>().unwrap(), LogLevel::Error);
    }
}