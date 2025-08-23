// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Example demonstrating Block-STM logging functionality
//!
//! This example shows how to:
//! 1. Initialize the Block-STM logger with custom configuration
//! 2. Enable logging through environment variables
//! 3. Run a simple parallel execution with logging enabled
//! 4. Analyze the generated log files

use aptos_block_executor::{
    block_stm_logger::{init_global_logger, LoggingConfig, LogLevel},
    executor::BlockExecutor,
    task::{ExecutionStatus, ExecutorTask, TransactionOutput},
    txn_provider::TxnProvider,
};
use aptos_types::{
    block_executor::config::BlockExecutorConfig,
    state_store::TStateView,
    transaction::BlockExecutableTransaction,
};
use std::{
    collections::HashMap,
    env,
    fs,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tempfile::TempDir;

// Simple mock transaction for demonstration
#[derive(Clone, Debug)]
struct DemoTransaction {
    id: u64,
    operations: Vec<String>,
}

impl BlockExecutableTransaction for DemoTransaction {
    type Key = String;
    type Tag = String;
    type Value = String;
    type Event = aptos_types::contract_event::ContractEvent;

    fn user_txn_bytes_len(&self) -> usize {
        self.operations.len() * 10 // Rough estimate
    }
}

// Mock executor for demonstration
struct DemoExecutor;

impl ExecutorTask for DemoExecutor {
    type Txn = DemoTransaction;
    type Output = DemoTransactionOutput;
    type Error = String;

    fn init(_: aptos_vm_types::resolver::ResourceGroupSize) -> Self {
        Self
    }

    fn execute_transaction(
        &self,
        _view: &impl TStateView<Key = String>,
        txn: &DemoTransaction,
        txn_idx: u32,
    ) -> ExecutionStatus<Self::Output, Self::Error> {
        // Simulate some work
        std::thread::sleep(Duration::from_millis(10 + (txn_idx % 5) * 2));
        
        // Simulate occasional aborts for demonstration
        if txn_idx % 7 == 0 && txn_idx > 0 {
            return ExecutionStatus::Abort("Simulated conflict".to_string());
        }
        
        ExecutionStatus::Success(DemoTransactionOutput {
            transaction_id: txn.id,
            gas_used: 1000 + (txn_idx * 100),
            operations_performed: txn.operations.clone(),
            write_set: vec![
                (format!("account_{}", txn.id), format!("balance_{}", txn.id * 100)),
                (format!("state_{}", txn.id), "updated".to_string()),
            ],
        })
    }
}

// Mock transaction output
#[derive(Clone, Debug)]
struct DemoTransactionOutput {
    transaction_id: u64,
    gas_used: u64,
    operations_performed: Vec<String>,
    write_set: Vec<(String, String)>,
}

impl TransactionOutput for DemoTransactionOutput {
    type Txn = DemoTransaction;

    fn resource_write_set(
        &self,
    ) -> Vec<(
        <Self::Txn as BlockExecutableTransaction>::Key,
        Arc<<Self::Txn as BlockExecutableTransaction>::Value>,
        Option<Arc<move_core_types::value::MoveTypeLayout>>,
    )> {
        self.write_set
            .iter()
            .map(|(k, v)| (k.clone(), Arc::new(v.clone()), None))
            .collect()
    }

    fn resource_group_write_set(
        &self,
    ) -> Vec<(
        <Self::Txn as BlockExecutableTransaction>::Key,
        aptos_types::write_set::WriteOp,
        aptos_vm_types::resolver::ResourceGroupSize,
        Vec<(
            <Self::Txn as BlockExecutableTransaction>::Tag,
            aptos_types::write_set::WriteOp,
        )>,
    )> {
        vec![]
    }

    fn aggregator_v1_write_set(
        &self,
    ) -> Vec<(
        <Self::Txn as BlockExecutableTransaction>::Key,
        Arc<<Self::Txn as BlockExecutableTransaction>::Value>,
    )> {
        vec![]
    }

    fn aggregator_v1_delta_set(
        &self,
    ) -> Vec<(
        <Self::Txn as BlockExecutableTransaction>::Key,
        aptos_aggregator::delta_change_set::DeltaOp,
    )> {
        vec![]
    }

    fn delayed_field_change_set(
        &self,
    ) -> aptos_aggregator::delayed_change::DelayedChangeSet<
        move_vm_types::delayed_values::delayed_field_id::DelayedFieldID,
    > {
        aptos_aggregator::delayed_change::DelayedChangeSet::new()
    }

    fn events(&self) -> Vec<aptos_types::contract_event::ContractEvent> {
        vec![]
    }

    fn fee_statement(&self) -> aptos_types::fee_statement::FeeStatement {
        aptos_types::fee_statement::FeeStatement::new(
            self.gas_used,
            0,
            0,
            0,
            0,
        )
    }

    fn get_writes(
        &self,
    ) -> Vec<(
        aptos_types::state_store::state_key::StateKey,
        aptos_types::write_set::WriteOp,
    )> {
        vec![]
    }

    fn get_events(&self) -> Vec<aptos_types::contract_event::ContractEvent> {
        vec![]
    }

    fn get_write_summary(&self) -> aptos_block_executor::types::ReadWriteSummary {
        aptos_block_executor::types::ReadWriteSummary {
            resource_write_set_size: self.write_set.len(),
            ..Default::default()
        }
    }
}

// Mock state view
struct DemoStateView;

impl TStateView for DemoStateView {
    type Key = String;

    fn get_state_value(
        &self,
        _state_key: &aptos_types::state_store::state_key::StateKey,
    ) -> anyhow::Result<Option<aptos_types::state_store::state_value::StateValue>> {
        Ok(None)
    }

    fn get_usage(&self) -> anyhow::Result<aptos_types::state_store::state_storage_usage::StateStorageUsage> {
        Ok(aptos_types::state_store::state_storage_usage::StateStorageUsage::new_untracked())
    }
}

// Mock transaction provider
struct DemoTxnProvider {
    transactions: Vec<DemoTransaction>,
}

impl TxnProvider<DemoTransaction> for DemoTxnProvider {
    fn get_txn(&self, txn_idx: aptos_mvhashmap::types::TxnIndex) -> &DemoTransaction {
        &self.transactions[txn_idx as usize]
    }

    fn num_txns(&self) -> usize {
        self.transactions.len()
    }
}

fn setup_logging_from_env() -> std::io::Result<PathBuf> {
    // Create a temporary directory for logs if not specified
    let log_dir = env::var("BLOCK_STM_LOG_DIR")
        .unwrap_or_else(|_| "./block_stm_logs".to_string());
    
    let log_path = PathBuf::from(&log_dir);
    fs::create_dir_all(&log_path)?;
    
    // Set up logging configuration
    let config = LoggingConfig {
        enabled: true,
        log_dir: log_path.clone(),
        log_level: env::var("BLOCK_STM_LOG_LEVEL")
            .unwrap_or_else(|_| "INFO".to_string())
            .parse()
            .unwrap_or(LogLevel::Info),
        max_file_size: env::var("BLOCK_STM_LOG_MAX_SIZE")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<u64>()
            .unwrap_or(10)
            * 1024 * 1024, // MB to bytes
        buffer_size: 8192,
        async_logging: true,
        include_read_write_details: env::var("BLOCK_STM_LOG_DETAILED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(true),
    };
    
    init_global_logger(config)?;
    println!("Block-STM logging initialized. Logs will be written to: {}", log_path.display());
    
    Ok(log_path)
}

fn create_demo_transactions(count: usize) -> Vec<DemoTransaction> {
    (0..count)
        .map(|i| DemoTransaction {
            id: i as u64,
            operations: vec![
                format!("read_account_{}", i),
                format!("update_balance_{}", i),
                format!("write_state_{}", i),
            ],
        })
        .collect()
}

fn analyze_logs(log_dir: &PathBuf) -> std::io::Result<()> {
    println!("\n=== Log Analysis ===");
    
    let log_files = [
        ("block_stm_execution.log", "Execution Events"),
        ("block_stm_concurrency.log", "Concurrency Events"),
        ("block_stm_readwrite.log", "Read/Write Events"),
        ("block_stm_performance.log", "Performance Metrics"),
        ("block_stm_summary.log", "Summary Events"),
    ];
    
    for (filename, description) in &log_files {
        let log_file = log_dir.join(filename);
        if log_file.exists() {
            let content = fs::read_to_string(&log_file)?;
            let line_count = content.lines().count();
            println!("{}: {} events logged", description, line_count);
            
            // Show first few events as examples
            if line_count > 0 {
                println!("  Sample events:");
                for (i, line) in content.lines().take(3).enumerate() {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                        if let Some(event_type) = event.get("event_type") {
                            println!("    {}: {}", i + 1, event_type);
                        }
                    }
                }
                if line_count > 3 {
                    println!("    ... and {} more events", line_count - 3);
                }
            }
        } else {
            println!("{}: No events logged", description);
        }
    }
    
    Ok(())
}

fn main() -> std::io::Result<()> {
    println!("Block-STM Logging Example");
    println!("=========================");
    
    // Set up logging
    let log_dir = setup_logging_from_env()?;
    
    // Create demo transactions
    let transaction_count = env::var("DEMO_TXN_COUNT")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap_or(20);
    
    println!("Creating {} demo transactions...", transaction_count);
    let transactions = create_demo_transactions(transaction_count);
    
    // Set up execution environment
    let txn_provider = DemoTxnProvider { transactions };
    let state_view = DemoStateView;
    
    // Configure block executor
    let config = BlockExecutorConfig {
        local: aptos_types::block_executor::config::BlockExecutorLocalConfig {
            concurrency_level: num_cpus::get().min(4), // Limit for demo
            allow_fallback: true,
            discard_failed_blocks: false,
        },
        onchain: aptos_types::block_executor::config::BlockExecutorConfigFromOnchain::default_if_missing(),
    };
    
    let thread_pool = Arc::new(
        rayon::ThreadPoolBuilder::new()
            .num_threads(config.local.concurrency_level)
            .build()
            .unwrap(),
    );
    
    println!("Starting parallel execution with {} threads...", config.local.concurrency_level);
    
    // Note: This is a simplified example. In a real implementation, you would need to:
    // 1. Implement proper BlockExecutor integration
    // 2. Handle the complete execution pipeline
    // 3. Set up proper state management
    
    // For demonstration, we'll simulate some logging events
    if let Some(logger) = aptos_block_executor::block_stm_logger::get_global_logger() {
        println!("Simulating transaction execution with logging...");
        
        for i in 0..transaction_count {
            let txn_idx = i as u32;
            
            // Log transaction start
            logger.log_transaction_start(txn_idx, 0);
            
            // Simulate execution time
            std::thread::sleep(Duration::from_millis(5));
            
            // Log transaction finish
            logger.log_transaction_finish(
                txn_idx,
                0,
                if i % 7 == 0 && i > 0 { "Abort" } else { "Success" },
                Duration::from_millis(5 + (i % 3) as u64),
                1000 + (i as u64 * 100),
                3 + (i % 2),
                2,
            );
            
            // Simulate some aborts and dependencies
            if i % 7 == 0 && i > 0 {
                logger.log_transaction_abort(
                    txn_idx,
                    0,
                    "Simulated conflict",
                    1,
                    vec![txn_idx.saturating_sub(1)],
                );
            }
            
            // Log performance metrics occasionally
            if i % 5 == 0 {
                let mut additional_data = HashMap::new();
                additional_data.insert("cache_hits".to_string(), format!("{}", i * 10));
                additional_data.insert("cache_misses".to_string(), format!("{}", i * 2));
                
                logger.log_performance_metric(
                    "execution_time_ms",
                    5.0 + (i as f64 * 0.5),
                    Some(txn_idx),
                    additional_data,
                );
            }
        }
        
        logger.flush();
    }
    
    println!("Execution completed. Analyzing logs...");
    
    // Analyze the generated logs
    analyze_logs(&log_dir)?;
    
    println!("\n=== Usage Instructions ===");
    println!("To analyze logs with jq (if installed):");
    println!("  # Count transaction starts:");
    println!("  jq 'select(.event_type == \"TransactionStart\")' {}/block_stm_execution.log | wc -l", log_dir.display());
    println!("  ");
    println!("  # Show all abort events:");
    println!("  jq 'select(.event_type == \"TransactionAbort\")' {}/block_stm_concurrency.log", log_dir.display());
    println!("  ");
    println!("  # Calculate average execution time:");
    println!("  jq -r 'select(.event_type == \"TransactionFinish\") | .duration_us' {}/block_stm_execution.log | awk '{{sum+=$1; count++}} END {{print \"Average: \" sum/count \" microseconds\"}}'", log_dir.display());
    
    println!("\n=== Environment Variables ===");
    println!("Set these environment variables to customize logging:");
    println!("  BLOCK_STM_LOG_DIR=/path/to/logs");
    println!("  BLOCK_STM_LOG_LEVEL=DEBUG|INFO|WARN|ERROR");
    println!("  BLOCK_STM_LOG_MAX_SIZE=100  # MB");
    println!("  BLOCK_STM_LOG_DETAILED=true|false");
    println!("  DEMO_TXN_COUNT=50");
    
    Ok(())
}