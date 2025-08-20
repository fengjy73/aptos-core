// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use aptos_mvhashmap::{types::{TxnIndex, Incarnation}, MVLogger};
use crate::block_stm_logger::{self, BlockSTMLogger};
use std::sync::Arc;

/// Implementation of MVLogger trait that bridges to BlockSTMLogger
pub struct BlockSTMLoggerAdapter {
    inner: Arc<BlockSTMLogger>,
}

impl BlockSTMLoggerAdapter {
    pub fn new(logger: Arc<BlockSTMLogger>) -> Self {
        Self { inner: logger }
    }
}

impl MVLogger for BlockSTMLoggerAdapter {
    fn log_mv_read(
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
        self.inner.log_mv_read(
            txn_id,
            incarnation,
            state_key,
            read_from,
            writer_tx,
            writer_incarnation,
            is_estimate,
            value_size,
        );
    }

    fn log_mv_write(
        &self,
        txn_id: TxnIndex,
        incarnation: Incarnation,
        state_key: &str,
        value_size: usize,
        write_type: &str,
    ) {
        self.inner.log_mv_write(
            txn_id,
            incarnation,
            state_key,
            value_size,
            write_type,
        );
    }
}

/// Get a logger adapter that can be used with MVHashMap
pub fn create_mv_logger_adapter() -> Option<Arc<dyn MVLogger>> {
    block_stm_logger::get_global_logger()
        .map(|logger| Arc::new(BlockSTMLoggerAdapter::new(logger)) as Arc<dyn MVLogger>)
}