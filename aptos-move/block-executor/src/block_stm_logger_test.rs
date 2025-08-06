// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for Block-STM logging functionality

use crate::{
    block_stm_logger::{init_global_logger, LoggingConfig, LogLevel},
    task::{ExecutionStatus, ExecutorTask, TransactionOutput},
    txn_provider::TxnProvider,
};
use aptos_aggregator::resolver::TAggregatorV1View;
use aptos_types::{
    contract_event::ContractEvent,
    error::PanicError,
    state_store::{errors::StateViewError, state_key::StateKey, TStateView},
    transaction::BlockExecutableTransaction,
    vm_status::{StatusCode, VMStatus},
    write_set::WriteOp,
};
use aptos_vm_types::{
    module_write_set::ModuleWrite,
    resolver::ResourceGroupSize,
};
use std::{
    collections::HashMap,
    fs,
    sync::{Arc, LazyLock},
    time::Duration,
};
use tempfile::TempDir;

// Mock transaction type for testing
#[derive(Clone, Debug)]
struct MockTransaction {
    id: u64,
    gas_limit: u64,
}

impl BlockExecutableTransaction for MockTransaction {
    type Key = StateKey;
    type Tag = String;
    type Value = WriteOp;
    type Event = ContractEvent;

    fn user_txn_bytes_len(&self) -> usize {
        100
    }
}

// Mock executor task
struct MockExecutorTask;

impl ExecutorTask for MockExecutorTask {
    type Txn = MockTransaction;
    type Output = MockTransactionOutput;
    type Error = String;

    fn init(
        _environment: &aptos_vm_environment::environment::AptosEnvironment,
        _state_view: &impl aptos_types::state_store::TStateView<Key = <Self::Txn as aptos_types::transaction::BlockExecutableTransaction>::Key>,
    ) -> Self {
        Self
    }

    fn is_transaction_dynamic_change_set_capable(_txn: &Self::Txn) -> bool {
        false
    }

    fn execute_transaction(
        &self,
        _view: &(impl aptos_vm_types::resolver::TExecutorView<
            <Self::Txn as aptos_types::transaction::BlockExecutableTransaction>::Key,
            <Self::Txn as aptos_types::transaction::BlockExecutableTransaction>::Tag,
            move_core_types::value::MoveTypeLayout,
            <Self::Txn as aptos_types::transaction::BlockExecutableTransaction>::Value,
        > + aptos_vm_types::resolver::TResourceGroupView<
            GroupKey = <Self::Txn as aptos_types::transaction::BlockExecutableTransaction>::Key,
            ResourceTag = <Self::Txn as aptos_types::transaction::BlockExecutableTransaction>::Tag,
            Layout = move_core_types::value::MoveTypeLayout,
        > + aptos_vm_types::module_and_script_storage::code_storage::AptosCodeStorage
              + aptos_vm_types::resolver::BlockSynchronizationKillSwitch),
        _txn: &MockTransaction,
        _txn_idx: aptos_mvhashmap::types::TxnIndex,
    ) -> ExecutionStatus<Self::Output, Self::Error> {
        ExecutionStatus::Success(MockTransactionOutput {
            gas_used: 1000,
            write_set: vec![(StateKey::raw(b"key1"), WriteOp::legacy_modification(b"value1".to_vec().into()))],
        })
    }
}

// Mock transaction output
#[derive(Clone, Debug)]
struct MockTransactionOutput {
    gas_used: u64,
    write_set: Vec<(StateKey, WriteOp)>,
}

impl TransactionOutput for MockTransactionOutput {
    type Txn = MockTransaction;

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
        <Self::Txn as BlockExecutableTransaction>::Value,
        ResourceGroupSize,
        std::collections::BTreeMap<
            <Self::Txn as BlockExecutableTransaction>::Tag,
            (
                <Self::Txn as BlockExecutableTransaction>::Value,
                Option<std::sync::Arc<move_core_types::value::MoveTypeLayout>>,
            ),
        >,
    )> {
        vec![]
    }

    fn aggregator_v1_write_set(
        &self,
    ) -> std::collections::BTreeMap<<Self::Txn as BlockExecutableTransaction>::Key, <Self::Txn as BlockExecutableTransaction>::Value> {
        std::collections::BTreeMap::new()
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
    ) -> std::collections::BTreeMap<move_vm_types::delayed_values::delayed_field_id::DelayedFieldID, aptos_aggregator::delayed_change::DelayedChange<move_vm_types::delayed_values::delayed_field_id::DelayedFieldID>> {
        std::collections::BTreeMap::new()
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



    fn get_events(&self) -> Vec<(<Self::Txn as BlockExecutableTransaction>::Event, Option<move_core_types::value::MoveTypeLayout>)> {
        vec![]
    }

    fn get_write_summary(
        &self,
    ) -> std::collections::HashSet<crate::types::InputOutputKey<<Self::Txn as BlockExecutableTransaction>::Key, <Self::Txn as BlockExecutableTransaction>::Tag>> {
        std::collections::HashSet::new()
    }

    fn module_write_set(&self) -> Vec<ModuleWrite<<Self::Txn as BlockExecutableTransaction>::Value>> {
        vec![]
    }

    fn reads_needing_delayed_field_exchange(
        &self,
    ) -> Vec<(
        <Self::Txn as BlockExecutableTransaction>::Key,
        aptos_types::state_store::state_value::StateValueMetadata,
        std::sync::Arc<move_core_types::value::MoveTypeLayout>,
    )> {
        vec![]
    }

    fn group_reads_needing_delayed_field_exchange(
        &self,
    ) -> Vec<(<Self::Txn as BlockExecutableTransaction>::Key, aptos_types::state_store::state_value::StateValueMetadata)> {
        vec![]
    }



    fn skip_output() -> Self {
        Self {
            gas_used: 0,
            write_set: vec![],
        }
    }

    fn discard_output(_discard_code: StatusCode) -> Self {
        Self {
            gas_used: 0,
            write_set: vec![],
        }
    }

    fn materialize_agg_v1(
        &self,
        _view: &impl TAggregatorV1View<Identifier = <Self::Txn as BlockExecutableTransaction>::Key>,
    ) {
        // No-op for mock
    }

    fn incorporate_materialized_txn_output(
        &self,
        _aggregator_v1_writes: Vec<(<Self::Txn as BlockExecutableTransaction>::Key, aptos_types::write_set::WriteOp)>,
        _patched_resource_write_set: Vec<(
            <Self::Txn as BlockExecutableTransaction>::Key,
            <Self::Txn as BlockExecutableTransaction>::Value,
        )>,
        _patched_events: Vec<<Self::Txn as BlockExecutableTransaction>::Event>,
    ) -> Result<(), PanicError> {
        Ok(())
    }

    fn set_txn_output_for_non_dynamic_change_set(&self) {
        // No-op for mock
    }

    fn is_retry(&self) -> bool {
        false
    }

    fn has_new_epoch_event(&self) -> bool {
        false
    }

    fn is_success(&self) -> bool {
        true
    }

    fn output_approx_size(&self) -> u64 {
        self.gas_used
    }
}

// Mock state view
struct MockStateView;

impl TStateView for MockStateView {
    type Key = StateKey;

    fn get_state_value(
        &self,
        _state_key: &StateKey,
    ) -> Result<Option<aptos_types::state_store::state_value::StateValue>, StateViewError> {
        Ok(None)
    }

    fn get_usage(&self) -> Result<aptos_types::state_store::state_storage_usage::StateStorageUsage, StateViewError> {
        Ok(aptos_types::state_store::state_storage_usage::StateStorageUsage::new_untracked())
    }
}

// Mock transaction provider
struct MockTxnProvider {
    transactions: Vec<MockTransaction>,
}

impl TxnProvider<MockTransaction> for MockTxnProvider {
    fn get_txn(&self, txn_idx: aptos_mvhashmap::types::TxnIndex) -> &MockTransaction {
        &self.transactions[txn_idx as usize]
    }



    fn num_txns(&self) -> usize {
        self.transactions.len()
    }

    fn get_auxiliary_info(&self, _idx: aptos_mvhashmap::types::TxnIndex) -> &aptos_types::transaction::AuxiliaryInfo {
        // Return a default auxiliary info for testing
        static DEFAULT_AUX_INFO: LazyLock<aptos_types::transaction::AuxiliaryInfo> = LazyLock::new(|| aptos_types::transaction::AuxiliaryInfo::new_empty());
        &*DEFAULT_AUX_INFO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_stm_logger::{BlockSTMLogger, LogEvent};
    use serde_json;

    #[test]
    fn test_logger_initialization() {
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
        
        // Test basic logging
        logger.log_transaction_start(0, 0);
        logger.log_transaction_finish(
            0,
            0,
            "Success",
            Duration::from_millis(100),
            1000,
            5,
            3,
        );
        logger.flush();

        // Check if log files were created
        let execution_log = temp_dir.path().join("block_stm_execution.log");
        assert!(execution_log.exists());
        
        let content = fs::read_to_string(execution_log).unwrap();
        assert!(content.contains("TransactionStart"));
        assert!(content.contains("TransactionFinish"));
    }

    #[test]
    fn test_global_logger_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let config = LoggingConfig {
            enabled: true,
            log_dir: temp_dir.path().to_path_buf(),
            log_level: LogLevel::Info,
            max_file_size: 1024 * 1024,
            buffer_size: 1024,
            async_logging: false,
            include_read_write_details: true,
        };

        init_global_logger(config).unwrap();
        
        // Test that global logger is accessible
        assert!(crate::block_stm_logger::get_global_logger().is_some());
    }

    #[test]
    fn test_log_event_serialization() {
        let event = LogEvent::TransactionStart {
            transaction_id: 5,
            incarnation: 2,
            thread_id: 12345,
            timestamp: 1234567890,
        };

        let json_str = serde_json::to_string(&event).unwrap();
        assert!(json_str.contains("TransactionStart"));
        assert!(json_str.contains("transaction_id"));
        assert!(json_str.contains("5"));
        
        // Test deserialization
        let deserialized: LogEvent = serde_json::from_str(&json_str).unwrap();
        match deserialized {
            LogEvent::TransactionStart { transaction_id, incarnation, .. } => {
                assert_eq!(transaction_id, 5);
                assert_eq!(incarnation, 2);
            },
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_log_levels() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test with INFO level - should not log DEBUG events
        let config = LoggingConfig {
            enabled: true,
            log_dir: temp_dir.path().to_path_buf(),
            log_level: LogLevel::Info,
            max_file_size: 1024 * 1024,
            buffer_size: 1024,
            async_logging: false,
            include_read_write_details: true,
        };

        let logger = BlockSTMLogger::new(config).unwrap();
        
        // This should be logged (INFO level)
        logger.log_transaction_start(0, 0);
        
        // This should not be logged (DEBUG level)
        logger.log_dependency_stall(0, vec![1, 2]);
        
        logger.flush();

        let execution_log = temp_dir.path().join("block_stm_execution.log");
        let concurrency_log = temp_dir.path().join("block_stm_concurrency.log");
        
        // Execution log should have content (INFO level event)
        if execution_log.exists() {
            let content = fs::read_to_string(execution_log).unwrap();
            assert!(content.contains("TransactionStart"));
        }
        
        // Concurrency log should be empty or not exist (DEBUG level event)
        if concurrency_log.exists() {
            let content = fs::read_to_string(concurrency_log).unwrap();
            assert!(content.is_empty() || !content.contains("DependencyStall"));
        }
    }

    #[test]
    fn test_performance_metrics_logging() {
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
        
        let mut additional_data = HashMap::new();
        additional_data.insert("cache_hits".to_string(), "150".to_string());
        additional_data.insert("cache_misses".to_string(), "25".to_string());
        
        logger.log_performance_metric(
            "execution_time_ms",
            125.5,
            Some(3),
            additional_data,
        );
        logger.flush();

        let performance_log = temp_dir.path().join("block_stm_performance.log");
        assert!(performance_log.exists());
        
        let content = fs::read_to_string(performance_log).unwrap();
        assert!(content.contains("PerformanceMetric"));
        assert!(content.contains("execution_time_ms"));
        assert!(content.contains("125.5"));
        assert!(content.contains("cache_hits"));
    }

    #[test]
    fn test_read_write_set_logging() {
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
        
        logger.log_readwrite_set_change(
            5,
            1,
            vec!["read_key1".to_string(), "read_key2".to_string()],
            vec!["write_key1".to_string()],
            2, // resource_reads
            1, // resource_writes
            0, // module_reads
            0, // module_writes
            0, // delayed_field_reads
            0, // delayed_field_writes
        );
        logger.flush();

        let readwrite_log = temp_dir.path().join("block_stm_readwrite.log");
        assert!(readwrite_log.exists());
        
        let content = fs::read_to_string(readwrite_log).unwrap();
        assert!(content.contains("ReadWriteSetChange"));
        assert!(content.contains("read_key1"));
        assert!(content.contains("write_key1"));
    }

    #[test]
    fn test_disabled_logging() {
        let temp_dir = TempDir::new().unwrap();
        let config = LoggingConfig {
            enabled: false, // Disabled
            log_dir: temp_dir.path().to_path_buf(),
            log_level: LogLevel::Debug,
            max_file_size: 1024 * 1024,
            buffer_size: 1024,
            async_logging: false,
            include_read_write_details: true,
        };

        let logger = BlockSTMLogger::new(config).unwrap();
        
        // Try to log something
        logger.log_transaction_start(0, 0);
        logger.flush();

        // No log files should be created when logging is disabled
        let execution_log = temp_dir.path().join("block_stm_execution.log");
        assert!(!execution_log.exists());
    }
}