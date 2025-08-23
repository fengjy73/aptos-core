# Block-STM 日志收集器升级方案

## 1. 概述

本文档详细规划了Block-STM日志收集器的升级方案，专注于日志收集功能的技术实现，旨在解决以下核心问题：
1. 交易编号与原始CSV数据无法对应
2. 缺少`suspend`状态日志记录
3. 日志数据结构不完整，缺少关键字段

## 2. 问题分析

### 2.1 交易编号对应问题
- **现状**：当前日志系统记录的交易编号（transaction_id）是Block-STM内部分配的索引
- **问题**：无法直接对应到原始CSV数据中的交易记录
- **影响**：难以分析特定交易的执行行为和性能特征
- **CSV数据结构**：`ETH_2401_100000.csv`包含`from`、`to`、`value`字段，代表以太坊转账交易

### 2.2 suspend状态日志缺失
- **现状**：日志中缺少交易suspend（暂停/等待）状态的记录
- **问题**：无法完整追踪交易的生命周期，特别是依赖等待阶段
- **影响**：无法准确分析交易间的依赖关系和等待时间

### 2.3 日志数据结构不完整
- **现状**：当前日志缺少CSV原始数据字段（from_address、to_address、amount）
- **问题**：无法直接从日志中获取交易的业务语义信息
- **影响**：需要额外的映射文件才能关联日志与原始交易数据

## 3. 核心文件定位

### 3.1 Block-STM执行器核心
- **文件**：`aptos-move/block-executor/src/executor.rs`
- **关键函数**：`BlockExecutor::execute_transactions_parallel`
- **作用**：交易并行执行的主入口，管理执行流程

### 3.2 调度器实现
- **文件**：`aptos-move/block-executor/src/scheduler_v2.rs`
- **关键结构**：`SchedulerV2`、`AbortManager`、`AbortedDependencies`
- **作用**：管理交易调度、依赖追踪和暂停传播

### 3.3 日志收集器
- **文件**：`aptos-move/aptos-transaction-benchmarks/src/block_stm_logger.rs`
- **作用**：记录交易执行状态和读写集信息

### 3.4 测试模拟器
- **文件**：`aptos-move/aptos-transaction-benchmarks/src/simulator.rs`
- **关键函数**：`replay_erc20_historic`、`gen_transaction_for_erc20`
- **作用**：读取CSV数据，生成交易并执行Block-STM

### 3.5 交易生成
- **文件**：`aptos-move/aptos-transaction-benchmarks/src/common_transactions.rs`
- **关键函数**：`peer_to_peer_txn`
- **作用**：生成APT转账交易，调用`aptos_account::fungible_transfer_only`

### 3.6 Mempool和Consensus
- **文件**：`mempool/src/core_mempool/mempool.rs`
- **文件**：`consensus/src/payload_manager/mod.rs`
- **作用**：交易分组和区块形成机制

## 4. 日志收集技术架构

### 4.1 日志数据流
```
CSV数据读取 → 交易元数据提取 → Block-STM执行 → 日志事件触发 → 日志缓冲区 → 文件输出
```

### 4.2 日志收集点分布
```
1. 交易生成阶段：记录CSV索引和原始数据
2. 执行开始阶段：记录执行开始时间和状态
3. 依赖检测阶段：记录suspend/resume事件
4. 读写集操作：记录状态键访问详情
5. 执行完成阶段：记录最终结果和性能指标
```

### 4.3 日志事件类型定义
```rust
pub enum LogEventType {
    TransactionStart,     // 交易开始执行
    TransactionSuspend,   // 交易暂停等待
    TransactionResume,    // 交易恢复执行
    TransactionFinish,    // 交易执行完成
    TransactionAbort,     // 交易执行中止
    StateKeyRead,         // 状态键读取
    StateKeyWrite,        // 状态键写入
    DependencyDetected,   // 检测到依赖
    ConflictDetected,     // 检测到冲突
}
```

## 5. 实施方案

### 5.1 阶段一：增强交易追踪机制

#### 5.1.1 扩展交易元数据收集
**目标文件**：`simulator.rs`

```rust
// 在replay_erc20_historic函数中增强数据收集
struct TransactionMetadata {
    csv_index: usize,           // CSV文件中的行号
    from_address: String,       // 发送方地址
    to_address: String,         // 接收方地址
    amount: u64,               // 转账金额
    timestamp: u64,            // 生成时间戳
    block_stm_id: usize,       // Block-STM内部ID
}

// 建立映射关系
static TRANSACTION_MAPPING: Lazy<RwLock<HashMap<usize, TransactionMetadata>>> = 
    Lazy::new(|| RwLock::new(HashMap::new()));
```

#### 5.1.2 修改交易生成函数
**目标文件**：`common_transactions.rs`

```rust
pub fn peer_to_peer_txn_with_metadata(
    from: &Account,
    to: &Account, 
    amount: u64,
    csv_index: usize,  // 新增参数
) -> SignedTransaction {
    // 记录映射关系
    let metadata = TransactionMetadata {
        csv_index,
        from_address: from.address().to_string(),
        to_address: to.address().to_string(), 
        amount,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
        block_stm_id: 0, // 稍后在执行时填充
    };
    
    // 存储映射关系
    TRANSACTION_MAPPING.write().unwrap().insert(csv_index, metadata);
    
    // 原有逻辑...
}
```

#### 5.1.3 区块分组策略
**目标文件**：`simulator.rs`

```rust
// 实现智能分组，考虑地址重复度
fn create_transaction_blocks(
    transactions: Vec<(usize, String, String, u64)>
) -> Vec<Vec<(usize, String, String, u64)>> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();
    let mut used_addresses = HashSet::new();
    
    for (idx, from, to, amount) in transactions {
        // 检查地址冲突
        if used_addresses.contains(&from) || used_addresses.contains(&to) {
            // 开始新区块
            if !current_block.is_empty() {
                blocks.push(current_block);
                current_block = Vec::new();
                used_addresses.clear();
            }
        }
        
        current_block.push((idx, from.clone(), to.clone(), amount));
        used_addresses.insert(from);
        used_addresses.insert(to);
        
        // 区块大小限制
        if current_block.len() >= MAX_BLOCK_SIZE {
            blocks.push(current_block);
            current_block = Vec::new();
            used_addresses.clear();
        }
    }
    
    if !current_block.is_empty() {
        blocks.push(current_block);
    }
    
    blocks
}
```

### 5.2 阶段二：完善suspend状态日志收集

#### 5.2.1 扩展日志宏定义
**目标文件**：`block_stm_logger.rs`

```rust
// 新增suspend相关日志宏
macro_rules! log_transaction_suspend {
    ($txn_id:expr, $dependency_txn:expr, $state_key:expr) => {
        if ENABLE_LOGGING.load(Ordering::Relaxed) {
            let entry = LogEntry {
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros(),
                transaction_id: $txn_id,
                event_type: "suspend".to_string(),
                details: format!(
                    "waiting_for_txn:{},state_key:{}", 
                    $dependency_txn, 
                    $state_key
                ),
                csv_index: get_csv_index($txn_id),  // 新增CSV索引
                from_address: get_from_address($txn_id),  // 新增发送方
                to_address: get_to_address($txn_id),      // 新增接收方
            };
            LOG_BUFFER.lock().unwrap().push(entry);
        }
    };
}

macro_rules! log_transaction_resume {
    ($txn_id:expr, $dependency_txn:expr) => {
        if ENABLE_LOGGING.load(Ordering::Relaxed) {
            let entry = LogEntry {
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros(),
                transaction_id: $txn_id,
                event_type: "resume".to_string(),
                details: format!("dependency_resolved:{}", $dependency_txn),
                csv_index: get_csv_index($txn_id),
                from_address: get_from_address($txn_id),
                to_address: get_to_address($txn_id),
            };
            LOG_BUFFER.lock().unwrap().push(entry);
        }
    };
}
```

#### 5.2.2 在调度器中插入日志点
**目标文件**：`scheduler_v2.rs`

```rust
// 在AbortManager::start_abort中添加
impl AbortManager {
    pub(crate) fn start_abort(&self, txn_idx: TxnIndex, incarnation: Incarnation) {
        // 记录暂停开始
        log_transaction_suspend!(txn_idx, incarnation, "abort_start");
        
        // 原有逻辑...
    }
}

// 在AbortedDependencies中添加
impl AbortedDependencies {
    pub(crate) fn mark_dependency(&mut self, txn_idx: TxnIndex, dep_txn_idx: TxnIndex) {
        // 记录依赖等待
        log_transaction_suspend!(txn_idx, dep_txn_idx, "dependency_wait");
        
        // 原有逻辑...
    }
    
    pub(crate) fn resolve_dependency(&mut self, txn_idx: TxnIndex) {
        // 记录依赖解决
        log_transaction_resume!(txn_idx, "dependency_resolved");
        
        // 原有逻辑...
    }
}
```

#### 5.2.3 在执行器中插入日志点
**目标文件**：`executor.rs`

```rust
// 在交易执行循环中添加
loop {
    match scheduler.next_task() {
        Some(SchedulerTask::ExecutionTask(txn_idx, incarnation)) => {
            // 检查是否需要等待
            if let Some(dep_txn) = check_dependencies(txn_idx) {
                log_transaction_suspend!(txn_idx, dep_txn, "execution_wait");
                // 等待逻辑...
                log_transaction_resume!(txn_idx, dep_txn);
            }
            
            // 执行交易...
        }
        // 其他任务类型...
    }
}
```

### 5.3 阶段三：集成Mempool和Consensus模拟

#### 5.3.1 扩展测试框架
**目标文件**：`simulator.rs`

```rust
// 新增完整的区块链模拟
pub fn simulate_full_blockchain_execution(
    csv_file: &str,
    block_size: usize,
    enable_mempool: bool,
) -> Result<()> {
    // 1. 读取CSV数据
    let transactions = read_csv_transactions(csv_file)?;
    
    // 2. 模拟Mempool排序
    let sorted_transactions = if enable_mempool {
        simulate_mempool_ordering(transactions)
    } else {
        transactions
    };
    
    // 3. 分组成区块
    let blocks = create_transaction_blocks(sorted_transactions);
    
    // 4. 逐区块执行
    for (block_idx, block_transactions) in blocks.iter().enumerate() {
        log_block_start!(block_idx, block_transactions.len());
        
        // 执行Block-STM
        let result = execute_block_stm(block_transactions)?;
        
        log_block_finish!(block_idx, result.execution_time, result.conflict_count);
    }
    
    Ok(())
}

// 模拟Mempool排序逻辑
fn simulate_mempool_ordering(
    transactions: Vec<(usize, String, String, u64)>
) -> Vec<(usize, String, String, u64)> {
    // 按gas价格排序（这里简化为按金额排序）
    let mut sorted = transactions;
    sorted.sort_by(|a, b| b.3.cmp(&a.3));  // 按value降序
    sorted
}
```

#### 5.3.2 增强日志输出
**目标文件**：`block_stm_logger.rs`

```rust
// 扩展日志条目结构
#[derive(Debug, Clone, Serialize)]
struct EnhancedLogEntry {
    timestamp: u128,
    transaction_id: usize,
    csv_index: Option<usize>,        // CSV文件行号
    from_address: Option<String>,    // 发送方地址
    to_address: Option<String>,      // 接收方地址
    amount: Option<u64>,            // 转账金额
    event_type: String,             // 事件类型
    details: String,                // 详细信息
    block_id: Option<usize>,        // 区块ID
    state_keys: Vec<String>,        // 涉及的状态键
    read_set_size: usize,           // 读集大小
    write_set_size: usize,          // 写集大小
    execution_time_us: Option<u64>, // 执行时间（微秒）
    retry_count: usize,             // 重试次数
}

// 输出增强的CSV格式
pub fn export_enhanced_csv(filename: &str) -> Result<()> {
    let mut writer = csv::Writer::from_path(filename)?;
    
    // 写入表头
    writer.write_record(&[
        "timestamp", "transaction_id", "csv_index", "from_address", "to_address",
        "amount", "event_type", "details", "block_id", "state_keys", 
        "read_set_size", "write_set_size", "execution_time_us", "retry_count"
    ])?;
    
    // 写入数据
    for entry in LOG_BUFFER.lock().unwrap().iter() {
        writer.write_record(&[
            entry.timestamp.to_string(),
            entry.transaction_id.to_string(),
            entry.csv_index.map_or(String::new(), |i| i.to_string()),
            entry.from_address.clone().unwrap_or_default(),
            entry.to_address.clone().unwrap_or_default(),
            entry.amount.map_or(String::new(), |a| a.to_string()),
            entry.event_type.clone(),
            entry.details.clone(),
            entry.block_id.map_or(String::new(), |b| b.to_string()),
            entry.state_keys.join(";"),
            entry.read_set_size.to_string(),
            entry.write_set_size.to_string(),
            entry.execution_time_us.map_or(String::new(), |t| t.to_string()),
            entry.retry_count.to_string(),
        ])?;
    }
    
    writer.flush()?;
    Ok(())
}
```

## 6. 实施优先级

### 6.1 高优先级（立即实施）
1. **交易元数据收集**：建立CSV索引与Block-STM ID的映射关系
2. **基础suspend日志**：在调度器中添加关键的暂停/恢复日志点
3. **状态键追踪**：记录每个交易涉及的具体状态键

### 6.2 中优先级（短期实施）
1. **完整suspend流程**：覆盖所有暂停场景的日志记录
2. **读写集详细信息**：记录读写集的具体内容和大小
3. **性能指标收集**：添加执行时间、重试次数等指标

### 6.3 低优先级（长期优化）
1. **Mempool模拟**：集成交易排序和分组逻辑
2. **区块级别分析**：支持多区块的连续执行分析
3. **可视化工具**：开发日志分析和可视化工具

## 7. 预期效果

### 7.1 问题解决
- ✅ **交易对应**：每条日志都能追溯到CSV文件中的原始交易
- ✅ **完整生命周期**：包含start、suspend、resume、finish的完整状态转换
- ✅ **读写集映射**：清晰展示CSV交易与状态键的对应关系

### 7.2 日志数据完整性
- ✅ **完整的交易生命周期**：从start到finish的所有状态转换
- ✅ **详细的状态访问记录**：每次读写操作的具体状态键
- ✅ **精确的时间戳**：微秒级别的事件时间记录
- ✅ **丰富的上下文信息**：CSV索引、地址、金额等业务数据

### 7.3 日志输出格式
```csv
timestamp,transaction_id,csv_index,from_address,to_address,amount,event_type,details,state_keys,thread_id,incarnation
1640995200000000,0,1,0xabc...,0xdef...,100,start,"execution_start","0x123...;0x456...",1,0
1640995200001000,0,1,0xabc...,0xdef...,100,suspend,"waiting_for_txn:5","0x123...",1,0
1640995200002000,0,1,0xabc...,0xdef...,100,resume,"dependency_resolved:5","0x123...",1,0
1640995200003000,0,1,0xabc...,0xdef...,100,finish,"execution_success","0x123...;0x456...",1,0
```

## 8. 风险评估与缓解

### 8.1 性能风险
- **风险**：增加日志记录可能影响执行性能
- **缓解**：使用异步日志缓冲区，批量写入文件

### 8.2 内存风险
- **风险**：大量交易的元数据可能消耗过多内存
- **缓解**：实现LRU缓存机制，定期清理旧数据

### 8.3 兼容性风险
- **风险**：修改可能影响现有测试和基准测试
- **缓解**：通过配置开关控制新功能，保持向后兼容

### 8.4 数据一致性风险
- **风险**：并发环境下日志记录可能出现竞态条件
- **缓解**：使用线程安全的数据结构和原子操作

## 9. Block-STM日志收集的源码级实现方案

### 9.1 基于源码分析的日志收集点定位

#### 9.1.1 MVHashMap核心日志收集点
基于 `aptos-move/mvhashmap/src/versioned_data.rs` 的分析，在以下关键位置添加日志收集：

```rust
// 在 VersionedData::read() 方法中添加日志
impl<V> VersionedData<V> {
    pub fn read(&self, txn_idx: TxnIndex, incarnation: Incarnation) -> ReadResult<V> {
        let read_start = Instant::now();
        
        // 原有读取逻辑
        let result = match self.versioned_map.get(&txn_idx) {
            Some(versioned_value) => {
                // 检查incarnation匹配
                if versioned_value.incarnation <= incarnation {
                    ReadResult::Value(versioned_value.value.clone())
                } else {
                    ReadResult::Unresolved(versioned_value.incarnation)
                }
            },
            None => ReadResult::None,
        };
        
        // 新增：记录读取操作
        if let Some(logger) = &self.logger {
            logger.log_state_key_read(StateKeyReadEvent {
                txn_idx,
                state_key: self.key.clone(),
                read_duration: read_start.elapsed(),
                result_type: match &result {
                    ReadResult::Value(_) => "Value",
                    ReadResult::Unresolved(_) => "Unresolved",
                    ReadResult::None => "None",
                }.to_string(),
                version_count: self.versioned_map.len(),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
            });
        }
        
        result
    }
    
    pub fn write(&mut self, txn_idx: TxnIndex, incarnation: Incarnation, value: V) {
        let write_start = Instant::now();
        let pre_version_count = self.versioned_map.len();
        
        // 原有写入逻辑
        self.versioned_map.insert(txn_idx, VersionedValue {
            value,
            incarnation,
        });
        
        // 新增：记录写入操作
        if let Some(logger) = &self.logger {
            logger.log_state_key_write(StateKeyWriteEvent {
                txn_idx,
                incarnation,
                state_key: self.key.clone(),
                write_duration: write_start.elapsed(),
                pre_version_count,
                post_version_count: self.versioned_map.len(),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
            });
        }
    }
}
```

#### 9.1.2 日志事件结构定义
基于 `aptos-move/aptos-transaction-benchmarks/src/block_stm_logger.rs` 的分析，首先定义完整的日志事件结构：

```rust
// 在 block_stm_logger.rs 中定义日志事件结构
#[derive(Debug, Clone, Serialize)]
pub enum LogEvent {
    // 交易生命周期事件
    TransactionStart {
        txn_idx: TxnIndex,
        csv_index: usize,
        from_address: String,
        to_address: String,
        amount: u64,
        timestamp: u64,
    },
    TransactionSuspend {
        txn_idx: TxnIndex,
        dependency_txn: TxnIndex,
        state_key: String,
        csv_index: usize,
        timestamp: u64,
    },
    TransactionResume {
        txn_idx: TxnIndex,
        dependency_txn: TxnIndex,
        csv_index: usize,
        timestamp: u64,
    },
    TransactionFinish {
        txn_idx: TxnIndex,
        status: String,
        execution_duration_us: u64,
        csv_index: usize,
        timestamp: u64,
    },
    
    // 状态访问事件
    StateKeyRead {
        txn_idx: TxnIndex,
        state_key: String,
        version: Option<TxnIndex>,
        csv_index: usize,
        timestamp: u64,
    },
    StateKeyWrite {
        txn_idx: TxnIndex,
        state_key: String,
        csv_index: usize,
        timestamp: u64,
    },
    
    // 依赖和冲突事件
    DependencyDetected {
        txn_idx: TxnIndex,
        dependency_txn: TxnIndex,
        state_key: String,
        csv_index: usize,
        timestamp: u64,
    },
    ConflictDetected {
        txn_idx: TxnIndex,
        conflict_txn: TxnIndex,
        state_key: String,
        csv_index: usize,
        timestamp: u64,
    },
}

// 日志记录器实现
impl BlockSTMLogger {
    // 记录交易开始
    pub fn log_transaction_start(&self, txn_idx: TxnIndex, csv_index: usize, 
                               from: String, to: String, amount: u64) {
        if !self.enabled() {
            return;
        }
        
        let event = LogEvent::TransactionStart {
            txn_idx,
            csv_index,
            from_address: from,
            to_address: to,
            amount,
            timestamp: self.current_timestamp_micros(),
        };
        
        self.record_event(event);
    }
    
    // 其他日志记录方法...
}
```

#### 9.1.3 SchedulerV2核心状态转换日志
基于 `execution/block-executor/src/scheduler.rs` 的分析，在关键调度点添加日志：

```rust
// 在 SchedulerV2::next_task() 方法中添加日志
impl SchedulerV2 {
    pub fn next_task(&self, worker_idx: usize) -> Option<SchedulerTask> {
        let dispatch_start = Instant::now();
        
        // 原有任务分发逻辑
        let task = self.try_get_next_task(worker_idx);
        
        // 新增：记录任务分发
        if let Some(ref task) = task {
            if let Some(logger) = &self.logger {
                logger.log_task_dispatch(TaskDispatchEvent {
                    worker_idx,
                    task_type: match task {
                        SchedulerTask::ExecutionTask(txn_idx, incarnation) => 
                            format!("Execution({}, {})", txn_idx, incarnation),
                        SchedulerTask::ValidationTask(txn_idx, incarnation) => 
                            format!("Validation({}, {})", txn_idx, incarnation),
                    },
                    dispatch_duration: dispatch_start.elapsed(),
                    executed_once_max_idx: self.executed_once_max_idx.load(Ordering::Acquire),
                    min_not_scheduled_idx: self.min_not_scheduled_idx.load(Ordering::Acquire),
                    timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
                });
            }
        }
        
        task
    }
    
    pub fn finish_execution(&self, txn_idx: TxnIndex, incarnation: Incarnation, 
                           execution_result: ExecutionStatus) {
        let finish_start = Instant::now();
        let old_status = self.execution_statuses.get_status(txn_idx);
        
        // 原有完成逻辑
        self.execution_statuses.set_status(txn_idx, execution_result.clone());
        
        // 新增：记录执行完成
        if let Some(logger) = &self.logger {
            logger.log_execution_finish(ExecutionFinishEvent {
                txn_idx,
                incarnation,
                old_status: format!("{:?}", old_status),
                new_status: format!("{:?}", execution_result),
                execution_result: match execution_result {
                    ExecutionStatus::Success(_) => "Success",
                    ExecutionStatus::Abort(_) => "Abort",
                    ExecutionStatus::Retry => "Retry",
                }.to_string(),
                finish_duration: finish_start.elapsed(),
                timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
            });
        }
    }
}
```

#### 9.1.4 AbortManager依赖失效传播日志
 基于 `execution/block-executor/src/scheduler.rs` 中的 `AbortManager`，添加依赖失效日志：
 
 ```rust
 // 在 AbortManager::invalidate() 方法中添加日志
 impl<'a> AbortManager<'a> {
     pub fn invalidate(&mut self, txn_idx: TxnIndex, incarnation: Incarnation) {
         let invalidation_start = Instant::now();
         let mut affected_transactions = Vec::new();
         
         // 原有失效逻辑
         for higher_txn_idx in (txn_idx + 1)..self.num_txns {
             if self.execution_statuses.get_incarnation(higher_txn_idx) > incarnation {
                 self.execution_statuses.set_status(higher_txn_idx, ExecutionStatus::Retry);
                 affected_transactions.push(higher_txn_idx);
             }
         }
         
         // 新增：记录依赖失效传播
         if let Some(logger) = &self.logger {
             logger.log_dependency_invalidation(DependencyInvalidationEvent {
                 source_txn: txn_idx,
                 source_incarnation: incarnation,
                 affected_transactions: affected_transactions.clone(),
                 cascade_size: affected_transactions.len(),
                 invalidation_duration: invalidation_start.elapsed(),
                 timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
             });
         }
     }
 }
 ```
 
 #### 9.1.5 ExecutionStatus状态机完整追踪
 基于 `execution/block-executor/src/scheduler.rs` 中的 `ExecutionStatuses`，添加状态转换日志：
 
 ```rust
 // 在 ExecutionStatuses::set_status() 方法中添加日志
 impl ExecutionStatuses {
     pub fn set_status(&self, txn_idx: TxnIndex, status: ExecutionStatus) {
         let transition_start = Instant::now();
         
         // 获取旧状态和incarnation
         let old_status = self.statuses[txn_idx].load(Ordering::Acquire);
         let old_incarnation = self.incarnations[txn_idx].load(Ordering::Acquire);
         
         // 原有状态设置逻辑
         self.statuses[txn_idx].store(status.clone(), Ordering::Release);
         
         // 更新incarnation（如果需要）
         let new_incarnation = match status {
             ExecutionStatus::Retry => {
                 let new_inc = old_incarnation + 1;
                 self.incarnations[txn_idx].store(new_inc, Ordering::Release);
                 new_inc
             },
             _ => old_incarnation,
         };
         
         // 新增：记录状态转换
         if let Some(logger) = &self.logger {
             logger.log_status_transition(StatusTransitionEvent {
                 txn_idx,
                 old_status: format!("{:?}", ExecutionStatus::from_u32(old_status)),
                 new_status: format!("{:?}", status),
                 old_incarnation,
                 new_incarnation,
                 transition_duration: transition_start.elapsed(),
                 timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
             });
         }
     }
     
     pub fn get_incarnation(&self, txn_idx: TxnIndex) -> Incarnation {
         self.incarnations[txn_idx].load(Ordering::Acquire)
     }
 }
 ```
 
 #### 9.1.6 Simulator执行阶段日志
 基于 `execution/block-executor/src/executor.rs` 中的 `BlockExecutor`，添加区块执行日志：
 
 ```rust
 // 在 BlockExecutor::execute_block() 方法中添加日志
 impl<T: Transaction, E: ExecutorTask<T>, S: TStateView<Key = T::Key>> BlockExecutor<T, E, S> {
     pub fn execute_block(&self, transactions: Vec<T>, state_view: &S) -> Result<Vec<T::Output>, E::Error> {
         let block_start = Instant::now();
         let block_id = format!("block_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
         
         // 新增：记录区块执行开始
         if let Some(logger) = &self.logger {
             logger.log_block_execution_start(BlockExecutionStartEvent {
                 block_id: block_id.clone(),
                 transaction_count: transactions.len(),
                 block_timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                 timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
             });
         }
         
         // 原有区块执行逻辑
         let mut successful_transactions = 0;
         let mut failed_transactions = 0;
         
         let results = self.execute_transactions_parallel(transactions, state_view);
         
         // 统计执行结果
         for result in &results {
             match result {
                 Ok(_) => successful_transactions += 1,
                 Err(_) => failed_transactions += 1,
             }
         }
         
         // 新增：记录区块执行结束
         if let Some(logger) = &self.logger {
             logger.log_block_execution_end(BlockExecutionEndEvent {
                 block_id,
                 total_duration: block_start.elapsed(),
                 successful_transactions,
                 failed_transactions,
                 timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_micros() as u64,
             });
         }
         
         results
     }
 }
 ```

### 9.2 日志事件结构体定义

基于上述日志收集点，需要定义相应的事件结构体：

```rust
// 在 block_stm_logger.rs 中新增事件结构体
#[derive(Debug, Clone)]
pub struct StateKeyReadEvent {
    pub txn_idx: TxnIndex,
    pub state_key: String,
    pub read_duration: Duration,
    pub result_type: String,
    pub version_count: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct StateKeyWriteEvent {
    pub txn_idx: TxnIndex,
    pub incarnation: Incarnation,
    pub state_key: String,
    pub write_duration: Duration,
    pub pre_version_count: usize,
    pub post_version_count: usize,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct TaskDispatchEvent {
    pub worker_idx: usize,
    pub task_type: String,
    pub dispatch_duration: Duration,
    pub executed_once_max_idx: TxnIndex,
    pub min_not_scheduled_idx: TxnIndex,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct ExecutionFinishEvent {
    pub txn_idx: TxnIndex,
    pub incarnation: Incarnation,
    pub old_status: String,
    pub new_status: String,
    pub execution_result: String,
    pub finish_duration: Duration,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct StatusTransitionEvent {
    pub txn_idx: TxnIndex,
    pub old_status: String,
    pub new_status: String,
    pub old_incarnation: Incarnation,
    pub new_incarnation: Incarnation,
    pub transition_duration: Duration,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct DependencyInvalidationEvent {
    pub source_txn: TxnIndex,
    pub source_incarnation: Incarnation,
    pub affected_transactions: Vec<TxnIndex>,
    pub cascade_size: usize,
    pub invalidation_duration: Duration,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct BlockExecutionStartEvent {
    pub block_id: String,
    pub transaction_count: usize,
    pub block_timestamp: u64,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct BlockExecutionEndEvent {
    pub block_id: String,
    pub total_duration: Duration,
    pub successful_transactions: usize,
    pub failed_transactions: usize,
    pub timestamp: u64,
}
```

### 9.3 扩展BlockSTMLogger配置

基于现有的 `BlockSTMLoggerConfig`，新增配置选项：

```rust
// 在 block_stm_logger.rs 中扩展配置
#[derive(Debug, Clone)]
pub struct BlockSTMLoggerConfig {
    // 现有配置字段
    pub enabled: bool,
    pub log_level: LogLevel,
    pub output_file: Option<String>,
    pub buffer_size: usize,
    pub max_file_size: usize,
    
    // 新增：细粒度日志控制
    pub log_mvhashmap_operations: bool,
    pub log_scheduler_events: bool,
    pub log_status_transitions: bool,
    pub log_dependency_invalidations: bool,
    pub log_block_execution: bool,
    
    // 新增：性能优化配置
    pub async_logging: bool,
    pub sampling_rate: f64, // 0.0-1.0，用于采样日志
    pub batch_size: usize,  // 批量写入大小
    pub flush_interval_ms: u64, // 刷新间隔
}

impl Default for BlockSTMLoggerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_level: LogLevel::Info,
            output_file: None,
            buffer_size: 8192,
            max_file_size: 100 * 1024 * 1024, // 100MB
            
            // 默认启用所有日志类型
            log_mvhashmap_operations: true,
            log_scheduler_events: true,
            log_status_transitions: true,
            log_dependency_invalidations: true,
            log_block_execution: true,
            
            // 性能优化默认配置
            async_logging: true,
            sampling_rate: 1.0, // 默认记录所有事件
            batch_size: 100,
            flush_interval_ms: 1000,
        }
    }
}
```

### 9.4 扩展BlockSTMLogger实现

基于现有的 `BlockSTMLogger`，新增日志记录方法：

```rust
// 在 BlockSTMLogger 实现中新增方法
impl BlockSTMLogger {
    // 现有方法保持不变...
    
    // 新增：MVHashMap操作日志
    pub fn log_state_key_read(&self, event: StateKeyReadEvent) {
        if !self.config.enabled || !self.config.log_mvhashmap_operations {
            return;
        }
        
        if self.should_sample() {
            let log_event = LogEvent::StateKeyRead {
                txn_idx: event.txn_idx,
                state_key: event.state_key,
                version: None, // 根据实际读取结果设置
                csv_index: 0,  // 需要从上下文获取
                timestamp: event.timestamp,
            };
            self.record_event(log_event);
        }
    }
    
    pub fn log_state_key_write(&self, event: StateKeyWriteEvent) {
        if !self.config.enabled || !self.config.log_mvhashmap_operations {
            return;
        }
        
        if self.should_sample() {
            let log_event = LogEvent::StateKeyWrite {
                txn_idx: event.txn_idx,
                state_key: event.state_key,
                csv_index: 0,  // 需要从上下文获取
                timestamp: event.timestamp,
            };
            self.record_event(log_event);
        }
    }
    
    // 新增：调度器事件日志
    pub fn log_task_dispatch(&self, event: TaskDispatchEvent) {
        if !self.config.enabled || !self.config.log_scheduler_events {
            return;
        }
        
        if self.should_sample() {
            // 将TaskDispatchEvent转换为现有的LogEvent格式
            // 这里需要根据实际的LogEvent定义进行适配
            self.log_performance_metric(
                "task_dispatch",
                event.dispatch_duration.as_micros() as u64,
                Some(json!({
                    "worker_idx": event.worker_idx,
                    "task_type": event.task_type,
                    "executed_once_max_idx": event.executed_once_max_idx,
                    "min_not_scheduled_idx": event.min_not_scheduled_idx
                }))
            );
        }
    }
    
    // 新增：执行完成日志
    pub fn log_execution_finish(&self, event: ExecutionFinishEvent) {
        if !self.config.enabled || !self.config.log_scheduler_events {
            return;
        }
        
        if self.should_sample() {
            self.log_performance_metric(
                "execution_finish",
                event.finish_duration.as_micros() as u64,
                Some(json!({
                    "txn_idx": event.txn_idx,
                    "incarnation": event.incarnation,
                    "old_status": event.old_status,
                    "new_status": event.new_status,
                    "execution_result": event.execution_result
                }))
            );
        }
    }
    
    // 新增：状态转换日志
    pub fn log_status_transition(&self, event: StatusTransitionEvent) {
        if !self.config.enabled || !self.config.log_status_transitions {
            return;
        }
        
        if self.should_sample() {
            self.log_performance_metric(
                "status_transition",
                event.transition_duration.as_micros() as u64,
                Some(json!({
                    "txn_idx": event.txn_idx,
                    "old_status": event.old_status,
                    "new_status": event.new_status,
                    "old_incarnation": event.old_incarnation,
                    "new_incarnation": event.new_incarnation
                }))
            );
        }
    }
    
    // 新增：依赖失效日志
    pub fn log_dependency_invalidation(&self, event: DependencyInvalidationEvent) {
        if !self.config.enabled || !self.config.log_dependency_invalidations {
            return;
        }
        
        if self.should_sample() {
            self.log_performance_metric(
                "dependency_invalidation",
                event.invalidation_duration.as_micros() as u64,
                Some(json!({
                    "source_txn": event.source_txn,
                    "source_incarnation": event.source_incarnation,
                    "affected_transactions": event.affected_transactions,
                    "cascade_size": event.cascade_size
                }))
            );
        }
    }
    
    // 新增：区块执行日志
    pub fn log_block_execution_start(&self, event: BlockExecutionStartEvent) {
        if !self.config.enabled || !self.config.log_block_execution {
            return;
        }
        
        self.log_performance_metric(
            "block_execution_start",
            0,
            Some(json!({
                "block_id": event.block_id,
                "transaction_count": event.transaction_count,
                "block_timestamp": event.block_timestamp
            }))
        );
    }
    
    pub fn log_block_execution_end(&self, event: BlockExecutionEndEvent) {
        if !self.config.enabled || !self.config.log_block_execution {
            return;
        }
        
        self.log_performance_metric(
            "block_execution_end",
            event.total_duration.as_micros() as u64,
            Some(json!({
                "block_id": event.block_id,
                "successful_transactions": event.successful_transactions,
                "failed_transactions": event.failed_transactions
            }))
        );
    }
    
    // 新增：采样控制
    fn should_sample(&self) -> bool {
        if self.config.sampling_rate >= 1.0 {
            true
        } else {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            rng.gen::<f64>() < self.config.sampling_rate
        }
    }
}
```

### 9.5 日志集成策略

#### 9.5.1 依赖注入方式

为了在现有代码中集成日志收集器，采用依赖注入的方式：

```rust
// 在相关结构体中添加logger字段
pub struct VersionedData<V> {
    versioned_map: BTreeMap<TxnIndex, VersionedValue<V>>,
    key: String,
    logger: Option<Arc<BlockSTMLogger>>, // 新增
}

pub struct SchedulerV2 {
    execution_statuses: Arc<ExecutionStatuses>,
    // 其他现有字段...
    logger: Option<Arc<BlockSTMLogger>>, // 新增
}

pub struct ExecutionStatuses {
    statuses: Vec<AtomicU32>,
    incarnations: Vec<AtomicU32>,
    logger: Option<Arc<BlockSTMLogger>>, // 新增
}

pub struct BlockExecutor<T, E, S> {
    // 现有字段...
    logger: Option<Arc<BlockSTMLogger>>, // 新增
}
```

#### 9.5.2 构造函数修改

修改相关结构体的构造函数以支持logger注入：

```rust
impl<V> VersionedData<V> {
    pub fn new(key: String, logger: Option<Arc<BlockSTMLogger>>) -> Self {
        Self {
            versioned_map: BTreeMap::new(),
            key,
            logger,
        }
    }
}

impl SchedulerV2 {
    pub fn new(
        num_txns: usize,
        logger: Option<Arc<BlockSTMLogger>>
    ) -> Self {
        Self {
            execution_statuses: Arc::new(ExecutionStatuses::new(num_txns, logger.clone())),
            // 其他字段初始化...
            logger,
        }
    }
}

impl ExecutionStatuses {
    pub fn new(num_txns: usize, logger: Option<Arc<BlockSTMLogger>>) -> Self {
        Self {
            statuses: (0..num_txns).map(|_| AtomicU32::new(0)).collect(),
            incarnations: (0..num_txns).map(|_| AtomicU32::new(0)).collect(),
            logger,
        }
    }
}
```

#### 9.5.3 配置传递机制

在 Block-STM 执行器的入口点配置日志收集器：

```rust
// 在 block_executor.rs 或相应的入口文件中
pub fn create_block_executor_with_logging(
    config: BlockSTMLoggerConfig
) -> BlockExecutor<...> {
    let logger = if config.enabled {
        Some(Arc::new(BlockSTMLogger::new(config)))
    } else {
        None
    };
    
    BlockExecutor::new(
        // 其他参数...
        logger
    )
}
```

### 9.6 实施优先级和阶段规划

#### 阶段一：核心日志基础设施（优先级：高）
1. **扩展LogEvent枚举**：添加新的事件类型定义
2. **扩展BlockSTMLoggerConfig**：添加细粒度控制选项
3. **扩展BlockSTMLogger实现**：添加新的日志记录方法
4. **定义事件结构体**：创建所有必要的事件数据结构

#### 阶段二：MVHashMap日志集成（优先级：高）
1. **修改VersionedData结构**：添加logger字段
2. **集成read操作日志**：在读取操作中添加日志记录
3. **集成write操作日志**：在写入操作中添加日志记录
4. **测试MVHashMap日志功能**：验证日志记录的正确性

#### 阶段三：调度器日志集成（优先级：高）
1. **修改SchedulerV2结构**：添加logger字段
2. **集成任务分发日志**：在next_task方法中添加日志
3. **集成执行完成日志**：在finish_execution方法中添加日志
4. **测试调度器日志功能**：验证调度事件的记录

#### 阶段四：状态管理日志集成（优先级：中）
1. **修改ExecutionStatuses结构**：添加logger字段
2. **集成状态转换日志**：在set_status方法中添加日志
3. **集成依赖失效日志**：在AbortManager中添加日志
4. **测试状态管理日志功能**：验证状态变化的记录

#### 阶段五：区块执行日志集成（优先级：中）
1. **修改BlockExecutor结构**：添加logger字段
2. **集成区块执行开始/结束日志**：在execute_block方法中添加日志
3. **集成性能统计**：记录整体执行性能指标
4. **测试区块执行日志功能**：验证区块级别的日志记录

#### 阶段六：性能优化和测试（优先级：低）
1. **实现异步日志**：优化日志写入性能
2. **实现采样机制**：在高负载下减少日志开销
3. **实现批量写入**：提高日志I/O效率
4. **全面性能测试**：确保日志收集不影响Block-STM性能

### 9.7 验证和测试策略

#### 9.7.1 单元测试
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mvhashmap_read_logging() {
        let config = BlockSTMLoggerConfig::default();
        let logger = Arc::new(BlockSTMLogger::new(config));
        let versioned_data = VersionedData::new("test_key".to_string(), Some(logger.clone()));
        
        // 执行读取操作
        let result = versioned_data.read(0, 0);
        
        // 验证日志是否正确记录
        // 这里需要实现日志验证逻辑
    }
    
    #[test]
    fn test_scheduler_task_dispatch_logging() {
        let config = BlockSTMLoggerConfig::default();
        let logger = Arc::new(BlockSTMLogger::new(config));
        let scheduler = SchedulerV2::new(10, Some(logger.clone()));
        
        // 执行任务分发
        let task = scheduler.next_task(0);
        
        // 验证任务分发日志
        // 这里需要实现日志验证逻辑
    }
}
```

#### 9.7.2 集成测试
 ```rust
 #[test]
 fn test_full_block_execution_logging() {
     let config = BlockSTMLoggerConfig {
         enabled: true,
         log_mvhashmap_operations: true,
         log_scheduler_events: true,
         log_status_transitions: true,
         log_dependency_invalidations: true,
         log_block_execution: true,
         ..Default::default()
     };
     
     let executor = create_block_executor_with_logging(config);
     
     // 执行一个完整的区块
     let transactions = create_test_transactions();
     let result = executor.execute_block(transactions, &test_state_view());
     
     // 验证所有类型的日志都被正确记录
     // 检查日志文件内容的完整性和正确性
 }
 ```

## 10. 总结

本升级方案基于对 Aptos Block-STM 源码的深入分析，提出了全面的日志收集增强策略。通过在 MVHashMap、SchedulerV2、ExecutionStatuses、AbortManager 和 BlockExecutor 等核心组件中集成详细的日志收集点，该方案将实现：

### 10.1 核心价值
1. **全面的数据收集**：覆盖 Block-STM 执行过程中的所有关键事件和状态变化
2. **源码级集成**：直接在核心算法实现中添加日志收集，确保数据的准确性和完整性
3. **可配置的日志控制**：支持细粒度的日志开关和采样控制，平衡数据收集和性能影响
4. **结构化的事件定义**：标准化的日志事件格式，便于后续的数据分析和处理

### 10.2 技术特点
1. **非侵入式设计**：通过依赖注入方式集成，最小化对现有代码的影响
2. **高性能实现**：支持异步日志、采样机制和批量写入，确保日志收集不影响系统性能
3. **分阶段实施**：明确的优先级和实施计划，支持渐进式部署和测试
4. **完整的测试策略**：包含单元测试和集成测试，确保日志功能的可靠性

### 10.3 预期效果
通过实施本升级方案，将建立起 Block-STM 的全面数据收集基础设施，为后续的性能分析、算法优化、故障诊断和系统监控提供强有力的数据支撑，推动 Aptos 区块链并行执行引擎的持续改进和优化。