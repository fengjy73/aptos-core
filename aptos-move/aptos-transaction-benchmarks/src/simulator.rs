// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

// 标准库导入
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use std::fs::File;
use std::io::{BufRead, BufReader};

// 第三方库导入
use serde::{Deserialize, Serialize};
use proptest::{prelude::Strategy, strategy::ValueTree, test_runner::TestRunner};
use rand::Rng;
use num_cpus;

// Aptos核心模块导入
use aptos_vm::{VMBlockExecutor, aptos_vm::AptosVMBlockExecutor};
use aptos_vm_logging::disable_speculative_logging;
use aptos_block_executor::block_stm_logger::{init_global_logger, LoggingConfig, BlockSTMLogger};
use aptos_block_executor::txn_provider::default::DefaultTxnProvider;
use aptos_language_e2e_tests::{
    account_universe::{AccountPickStyle, AccountUniverse, AccountUniverseGen}, 
    common_transactions::{
        airdrop_initialize_txn,
        airdrop_transfer_txn,
        ballot_initialize_txn,
        ballot_vote_txn,
        million_pixel_initialize_txn,
        million_pixel_occupy_txn,
        kitty_initialize_txn,
        kitty_mint_txn,
        kitty_breed_txn,
        empty_empty_txn,
        peer_to_peer_txn
    }, 
    executor::FakeExecutor
};
use aptos_types::{
    transaction::{
        signature_verified_transaction::{
            into_signature_verified_block, SignatureVerifiedTransaction,
        }, ExecutionStatus, SignedTransaction, Transaction, TransactionOutput, TransactionStatus
    },
};

/// 打印TPS性能对比宏
macro_rules! print_tps {
    ($par_tps:expr,$seq_tps:expr) => {
        println!("并行执行 TPS: {}", $par_tps);
        println!("顺序执行 TPS: {}", $seq_tps);
        println!("相对顺序执行加速 {:.2}x", $par_tps as f64 / $seq_tps as f64);
    };
}

/// 交易数据结构 - 存储从CSV文件解析的交易信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionData {
    pub from_address: String,      // 发送方地址
    pub to_address: String,        // 接收方地址
    pub amount: u64,               // 转账金额
    pub transaction_hash: String,  // 交易哈希
    pub block_number: u64,         // 区块号
    pub transaction_index: usize,  // 在区块中的交易索引
}

/// CSV交易记录结构 - 直接从CSV文件读取的原始交易记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvTransactionRecord {
    pub csv_index: usize,         // CSV文件中的行索引
    pub sender_address: String,   // 发送方地址
    pub receiver_address: String, // 接收方地址
    pub amount: u64,              // 转账金额
    pub timestamp: u64,           // 时间戳
    pub transaction_hash: String, // 交易哈希
}

/// 详细执行指标结构 - 包含Block-STM执行过程中的各项性能指标
#[derive(Debug, Clone)]
pub struct DetailedExecutionMetrics {
    pub execution_count: u64,      // 执行次数（包括重执行）
    pub validation_count: u64,     // 验证次数
    pub abort_count: u64,          // 中止次数
    pub stall_count: u64,          // 停滞次数
    pub avg_stall_time_us: f64,    // 平均停滞时间（微秒）
    pub total_stall_time_us: f64,  // 总停滞时间（微秒）
    pub execution_time_ms: u128,   // 执行时间（毫秒）
    pub tps: usize,                // 每秒交易数
}

/// Block-STM基准测试模拟器
/// 
/// 提供完整的Block-STM并行执行基准测试功能，包括：
/// - 账户管理和交易生成
/// - CSV历史数据重放
/// - 详细的性能指标收集
/// - 综合日志记录系统
pub struct Simulator{
    account_universe: AccountUniverse,        // 账户宇宙，管理测试账户
    executor: FakeExecutor,                   // 假执行器，用于状态管理
    _logger: Option<Arc<BlockSTMLogger>>,     // Block-STM日志记录器（保留用于兼容性）
    log_enabled: bool,                        // 是否启用日志记录
    csv_data: Option<Vec<TransactionData>>,   // 从CSV加载的交易数据
    current_block_id: u64,                    // 当前区块ID
    log_output_dir: Option<String>,           // 日志输出目录
    concurrency_level: u32,                   // 并发执行级别
}

impl Simulator {
    /// 创建新的模拟器实例（基本版本）
    /// 
    /// 参数:
    /// - num_accounts: 创建的账户数量
    /// - concurrency_level: 并发执行级别
    pub fn with_account_nums(
        num_accounts: usize,
        concurrency_level: u32,
    ) -> Self {
        let mut runner = TestRunner::default();
        let balance = 500_000 * 1_000_000 * 5 as u64;
        let universe_strategy = AccountUniverseGen::strategy(num_accounts, balance..(balance + 1), AccountPickStyle::Unlimited);

        let universe_gen = universe_strategy
            .new_tree(&mut runner)
            .expect("creating a new value should succeed")
            .current();
        let executor = FakeExecutor::from_head_genesis();
        
        // Use FakeExecutor's state_store to ensure proper VM initialization with gas schedule
        let universe = universe_gen.setup_gas_cost_stability(executor.state_store());

        Self {
            account_universe: universe,
            executor,
            _logger: None,
            log_enabled: false,
            csv_data: None,
            current_block_id: 0,
            log_output_dir: None,
            concurrency_level,
        }
    }

    /// 创建新的模拟器实例（支持日志功能）
    /// 
    /// 参数:
    /// - num_accounts: 创建的账户数量
    /// - enable_logging: 是否启用日志记录
    /// - log_output_dir: 日志输出目录
    /// - concurrency_level: 并发执行级别
    pub fn new_with_logging(
        num_accounts: usize,
        enable_logging: bool,
        log_output_dir: Option<String>,
        concurrency_level: u32,
    ) -> Self {
        let mut runner = TestRunner::default();
        let balance = 500_000 * 1_000_000 * 5 as u64;
        let universe_strategy = AccountUniverseGen::strategy(
            num_accounts, 
            balance..(balance + 1), 
            AccountPickStyle::Unlimited
        );

        let universe_gen = universe_strategy
            .new_tree(&mut runner)
            .expect("creating a new value should succeed")
            .current();
        let executor = FakeExecutor::from_head_genesis();
        
        let universe = universe_gen.setup_gas_cost_stability(executor.state_store());

        Self {
            account_universe: universe,
            executor,
            _logger: None,
            log_enabled: enable_logging,
            csv_data: None,
            current_block_id: 0,
            log_output_dir,
            concurrency_level,
        }
    }

    /// 初始化Block-STM日志系统
    /// 
    /// 设置全局日志记录器，配置日志级别、输出目录等参数
    pub fn setup_logging_environment(&mut self) -> Result<(), String> {
        if !self.log_enabled {
            return Ok(());
        }

        let config = if let Some(ref log_dir) = self.log_output_dir {
            LoggingConfig {
                enabled: true,
                log_dir: std::path::PathBuf::from(log_dir),
                log_level: aptos_block_executor::block_stm_logger::LogLevel::Debug,
                max_file_size: 100 * 1024 * 1024, // 100MB
                buffer_size: 10000,
                async_logging: true,
                include_read_write_details: true,
            }
        } else {
            LoggingConfig::default()
        };
        
        init_global_logger(config)
            .map_err(|e| format!("Failed to initialize logger: {:?}", e))?;
        
        println!("Block-STM logger initialized with output dir: {:?}", 
                self.log_output_dir);
        Ok(())
    }

    /// 设置执行上下文
    /// 
    /// 为日志系统设置数据集信息和采样参数
    /// 参数:
    /// - data_path: CSV数据文件路径
    pub fn setup_execution_context(&self, data_path: &str) {
        use aptos_block_executor::block_stm_logger::{get_global_logger, ExecutionContext};
        
        if let Some(logger) = get_global_logger() {
            // 从CSV路径推断dataset
            let dataset = if data_path.contains("ETH") {
                "ETH"
            } else if data_path.contains("USDT") {
                "USDT"  
            } else {
                "UNKNOWN"
            }.to_string();

            let context = ExecutionContext {
                dataset,
                source_csv: data_path.to_string(),
                sample_period_ms: std::env::var("SAMPLE_PERIOD_MS")
                    .unwrap_or_else(|_| "50".to_string())
                    .parse::<u32>()
                    .unwrap_or(50),
                read_sample_rate: std::env::var("READ_SAMPLE_RATE")
                    .unwrap_or_else(|_| "0.01".to_string())
                    .parse::<f64>()
                    .unwrap_or(0.01),
            };

            logger.set_execution_context(context);
            println!("Execution context set: dataset={}, source_csv={}", 
                    if data_path.contains("ETH") { "ETH" } else if data_path.contains("USDT") { "USDT" } else { "UNKNOWN" }, 
                    data_path);
        }
    }

    /// 加载CSV数据并生成交易图
    /// 
    /// 从指定的CSV文件中读取历史交易数据，解析为交易图结构
    /// 同时生成详细的映射文件用于日志分析
    /// 
    /// 参数:
    /// - csv_path: CSV文件路径
    /// 
    /// 返回:
    /// - Ok(Vec<(usize, usize)>): 交易图，每个元组表示(from_account, to_account)
    /// - Err: 加载失败错误
    pub fn load_csv_data(&mut self, csv_path: &str) -> Result<Vec<(usize, usize)>, Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::{BufRead, BufReader};
        use std::collections::HashMap;
        
        println!("Loading CSV data from: {}", csv_path);
        
        let file = File::open(csv_path)?;
        let reader = BufReader::new(file);
        let mut transaction_graph = Vec::new();
        let mut csv_data = Vec::new();
        let mut account_id_map = HashMap::new();
        let mut next_account_id = 1u64;
        
        // 解析CSV文件（跳过标题行）
        for (line_num, line) in reader.lines().enumerate() {
            if line_num == 0 { continue; } // 跳过标题行
            let line = line?;
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                if let (Ok(from), Ok(to)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                    transaction_graph.push((from, to));
                    
                    let from_raw = from.to_string();
                    let to_raw = to.to_string();
                    let _value_raw = parts.get(2).unwrap_or(&"1").to_string();
                    
                    // 规范化地址（小写、去空格）
                    let from_norm = from_raw.trim().to_lowercase();
                    let to_norm = to_raw.trim().to_lowercase();
                    
                    // 分配account_id
                    let _from_account_id = *account_id_map.entry(from_norm.clone()).or_insert_with(|| {
                        let id = next_account_id;
                        next_account_id += 1;
                        id
                    });
                    let _to_account_id = *account_id_map.entry(to_norm.clone()).or_insert_with(|| {
                        let id = next_account_id;
                        next_account_id += 1;
                        id
                    });
                    
                    // 存储CSV数据用于映射
                    let transaction_data = TransactionData {
                        from_address: format!("account_{}", from),
                        to_address: format!("account_{}", to),
                        amount: parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(1),
                        transaction_hash: format!("txn_{}_{}_to_{}", line_num, from, to),
                        block_number: self.current_block_id,
                        transaction_index: line_num - 1,
                    };
                    csv_data.push(transaction_data);
                }
            }
        }
        
        // 生成row_tx_mapping.csv文件
        if self.log_enabled {
            self.generate_row_tx_mapping(csv_path, &csv_data, &account_id_map)?;
        }
        
        self.csv_data = Some(csv_data);
        println!("Loaded {} transactions from CSV", transaction_graph.len());
        Ok(transaction_graph)
    }
    
    /// 生成交易映射文件
    /// 
    /// 创建 row_tx_mapping.csv 文件，记录CSV行号与Block-STM交易索引的对应关系
    fn generate_row_tx_mapping(
        &self, 
        csv_path: &str, 
        csv_data: &[TransactionData],
        account_id_map: &HashMap<String, u64>
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;
        use sha2::{Sha256, Digest};
        
        let default_log_dir = "./logs".to_string();
        let log_dir = self.log_output_dir.as_ref().unwrap_or(&default_log_dir);
        let block_dir = format!("{}/block_{:03}", log_dir, self.current_block_id);
        std::fs::create_dir_all(&block_dir)?;
        
        let mapping_path = format!("{}/row_tx_mapping.csv", block_dir);
        let mut mapping_file = File::create(&mapping_path)?;
        
        // 从文件路径推断数据集类型
        let dataset = if csv_path.contains("ETH") {
            "ETH"
        } else if csv_path.contains("USDT") {
            "USDT"
        } else {
            "UNKNOWN"
        };
        
        // 写入CSV头部（精简版 - 删除可计算字段）
        writeln!(mapping_file, "block_id,dataset,source_csv,row_number,tx_index,from_raw,to_raw,value_raw,from_norm,to_norm,from_account_id,to_account_id,key_from_id,key_to_id,included,skip_reason,row_hash")?;
        
        // 为每个交易写入映射记录
        for (idx, transaction_data) in csv_data.iter().enumerate() {
            let from_raw = transaction_data.from_address.replace("account_", "");
            let to_raw = transaction_data.to_address.replace("account_", "");
            let value_raw = transaction_data.amount.to_string();
            
            // 规范化地址
            let from_norm = from_raw.trim().to_lowercase();
            let to_norm = to_raw.trim().to_lowercase();
            
            // 获取account_id
            let from_account_id = account_id_map.get(&from_norm).copied().unwrap_or(0);
            let to_account_id = account_id_map.get(&to_norm).copied().unwrap_or(0);
            
            // FungibleStore键ID（假设与account_id相同）
            let key_from_id = from_account_id;
            let key_to_id = to_account_id;
            
            // 计算行数据哈希值
            let mut hasher = Sha256::new();
            hasher.update(format!("{}|{}|{}", from_raw, to_raw, value_raw));
            let row_hash = format!("{:x}", hasher.finalize());
            
            writeln!(mapping_file, 
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                format!("block_{:03}", self.current_block_id), // block_id
                dataset, // dataset
                csv_path, // source_csv
                transaction_data.transaction_index + 1, // row_number (1开始)
                idx + 1, // tx_index (1开始)
                from_raw, // from_raw
                to_raw, // to_raw
                value_raw, // value_raw
                from_norm, // from_norm
                to_norm, // to_norm
                from_account_id, // from_account_id
                to_account_id, // to_account_id
                key_from_id, // key_from_id
                key_to_id, // key_to_id
                "true", // included
                "", // skip_reason
                row_hash // row_hash
                // 注意：部分统计字段已从 CSV 中移除，现在通过日志文件计算：
                // - task_kind: 从 TaskPickedV2 事件统计
                // - first_execution: incarnation == 1
                // - execution_time_us: ExecutionFinish.timestamp - ExecutionStart.timestamp
                // - reexecution_count: max(incarnation) - 1
                // - stall_count: 从 StallAdd 事件统计
                // - waterline_at_exec: 从 WaterlineAdvance 事件查找
                // - abort_count: 从 AbortInitiated 事件统计
            )?;
        }
        
        println!("Generated row_tx_mapping.csv with {} entries at {}", csv_data.len(), mapping_path);
        
        // Generate additional files in the block directory
        self.generate_code_map(&block_dir)?;
        self.generate_meta_json(&block_dir, csv_path, csv_data.len())?;
        
        Ok(())
    }


    /// 生成代码映射文件
    /// 
    /// 创建 code_map.json 文件，记录关键代码位置和架构信息
    fn generate_code_map(&self, block_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;
        use serde_json::json;

        let code_map_path = format!("{}/code_map.json", block_dir);
        let mut code_map_file = File::create(&code_map_path)?;

        let code_map = json!({
            "parallel_execute_entry": "aptos-move/block-executor/src/executor.rs:450",
            "exec_task_loop": "aptos-move/block-executor/src/executor.rs:520",
            "val_task_loop": "aptos-move/block-executor/src/executor.rs:580",
            "suspend_branch": "aptos-move/block-executor/src/executor.rs:600",
            "validate_fail_branch": "aptos-move/block-executor/src/scheduler_v2.rs:250",
            "mv_store_read": "aptos-move/mvhashmap/src/versioned_data.rs:300",
            "mv_store_write": "aptos-move/mvhashmap/src/versioned_data.rs:400",
            "mark_estimate_sites": "aptos-move/mvhashmap/src/versioned_data.rs:720",
            "clear_estimate_on_fail": "aptos-move/mvhashmap/src/versioned_data.rs:800",
            "exec_index_atom": "aptos-move/block-executor/src/scheduler_v2.rs:64",
            "val_index_atom": "aptos-move/block-executor/src/scheduler_v2.rs:68",
            "commit_gate_check": "aptos-move/block-executor/src/scheduler_v2.rs:55",
            "scheduler_state_enum": "aptos-move/block-executor/src/scheduler_status.rs:50",
            "dependency_condvar": "aptos-move/block-executor/src/scheduler.rs:63",
            "estimate_flag_const": "aptos-move/mvhashmap/src/versioned_data.rs:31",
            "transaction_output_trait": "aptos-move/block-executor/src/task.rs:100",
            "execution_status_enum": "aptos-move/block-executor/src/task.rs:35",
            "notes": "FLAG_ESTIMATE=true表示推测版本；SchedulerV2使用SchedulingStatus枚举；MVDataError::Dependency触发依赖等待；incarnation从1开始递增；TransactionOutput trait提供所有输出详情字段"
        });

        writeln!(code_map_file, "{}", serde_json::to_string_pretty(&code_map)?)?;
        println!("Generated code_map.json at {}", code_map_path);
        Ok(())
    }

    /// 生成元数据文件
    /// 
    /// 创建 meta.json 文件，记录运行参数和环境信息
    fn generate_meta_json(
        &self,
        block_dir: &str,
        csv_path: &str,
        tx_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::fs::File;
        use std::io::Write;
        use serde_json::json;

        let meta_path = format!("{}/meta.json", block_dir);
        let mut meta_file = File::create(&meta_path)?;

        let dataset = if csv_path.contains("ETH") {
            "ETH"
        } else if csv_path.contains("USDT") {
            "USDT"
        } else {
            "UNKNOWN"
        };

        let git_commit = std::env::var("GIT_COMMIT")
            .or_else(|_| {
                std::process::Command::new("git")
                    .args(&["rev-parse", "--short", "HEAD"])
                    .output()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .map_err(|_| std::env::VarError::NotPresent)
            })
            .unwrap_or_else(|_| "unknown".to_string());

        let build_profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };

        // 获取当前 Rust 版本信息
        let rust_version = std::env::var("RUSTC_VERSION")
            .or_else(|_| {
                std::process::Command::new("rustc")
                    .args(&["--version"])
                    .output()
                    .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .map_err(|_| std::env::VarError::NotPresent)
            })
            .unwrap_or_else(|_| "unknown".to_string());

        let meta = json!({
            "block_id": format!("block_{:03}", self.current_block_id),
            "dataset": dataset,
            "source_csv": csv_path,
            "tx_count": tx_count,
            "concurrency_level": self.concurrency_level, // 使用实际传入的并发级别
            "sample_period_ms": std::env::var("SAMPLE_PERIOD_MS")
                .unwrap_or_else(|_| "50".to_string())
                .parse::<u32>()
                .unwrap_or(50),
            "read_sample_rate": std::env::var("READ_SAMPLE_RATE")
                .unwrap_or_else(|_| "0.01".to_string())
                .parse::<f64>()
                .unwrap_or(0.01),
            "git_commit": git_commit,
            "build_profile": build_profile,
            "log_output_dir": self.log_output_dir.as_ref().unwrap_or(&"./logs".to_string()),
            "timestamp": SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            "address_mapping_summary": {
                "unique_accounts": tx_count * 2, // 估计值，每个交易有from和to
                "fungible_store_keys": tx_count * 2,
                "normalization_applied": true
            },
            "environment": {
                "rust_version": rust_version,
                "aptos_core_version": env!("CARGO_PKG_VERSION"),
                "host_os": std::env::consts::OS,
                "host_arch": std::env::consts::ARCH,
                "cpu_count": num_cpus::get()
            }
        });

        writeln!(meta_file, "{}", serde_json::to_string_pretty(&meta)?)?;
        println!("Generated meta.json at {}", meta_path);
        Ok(())
    }

    /// 处理交易映射关系
    /// 
    /// 记录 CSV 数据与 Block-STM 交易索引的对应关系
    pub fn process_transaction_mapping(&self, transactions: &[SignatureVerifiedTransaction]) {
        if !self.log_enabled || self.csv_data.is_none() {
            return;
        }
        
        if let Some(ref csv_data) = self.csv_data {
            for (block_stm_index, csv_record) in csv_data.iter().enumerate() {
                if block_stm_index < transactions.len() {
                     // 记录交易映射关系
                     if let Some(logger) = aptos_block_executor::block_stm_logger::get_global_logger() {
                         logger.log_performance_metric(
                             "transaction_mapping",
                             block_stm_index as f64,
                             Some(block_stm_index as u32),
                             std::collections::HashMap::from([
                                 ("csv_index".to_string(), csv_record.transaction_index.to_string()),
                                 ("block_id".to_string(), self.current_block_id.to_string()),
                                 ("mapping_type".to_string(), "CSV_to_BlockSTM".to_string()),
                                 ("transaction_hash".to_string(), csv_record.transaction_hash.clone()),
                                 ("block_stm_index".to_string(), block_stm_index.to_string()),
                             ])
                         );
                     }
                 }
            }
        }
    }

    /// 从 CSV 文件加载交易数据
    /// 
    /// 读取 CSV 文件并转换为 Aptos 交易格式
    /// 
    /// 参数:
    /// - csv_file_path: CSV 文件路径
    /// - max_transactions: 最大交易数量限制
    /// 
    /// 返回:
    /// - 签名验证交易列表和 CSV 记录列表
    pub fn load_transactions_from_csv(
        &mut self,
        csv_file_path: &str,
        max_transactions: Option<usize>,
    ) -> Result<(Vec<SignatureVerifiedTransaction>, Vec<CsvTransactionRecord>), String> {
        let file = File::open(csv_file_path)
            .map_err(|e| format!("Failed to open CSV file: {}", e))?;
        let reader = BufReader::new(file);
        
        let mut csv_records = Vec::new();
        let mut transaction_graph = Vec::new();
        
        for (line_idx, line) in reader.lines().enumerate() {
            if line_idx == 0 { continue; } // 跳过标题行
            
            if let Some(max) = max_transactions {
                if line_idx > max {
                    break;
                }
            }
            
            let line = line.map_err(|e| format!("Failed to read line {}: {}", line_idx, e))?;
            let parts: Vec<&str> = line.split(',').collect();
            
            if parts.len() < 3 {
                continue; // 跳过格式不正确的行
            }
            
            // 解析发送方和接收方账户索引
            let sender_idx = parts[0].trim().parse::<usize>()
                .map_err(|e| format!("Invalid sender index at line {}: {}", line_idx, e))?;
            let receiver_idx = parts[1].trim().parse::<usize>()
                .map_err(|e| format!("Invalid receiver index at line {}: {}", line_idx, e))?;
            
            // 确保有足够的账户
            if sender_idx >= self.account_universe.num_accounts() || 
               receiver_idx >= self.account_universe.num_accounts() {
                return Err(format!("Account index out of range at line {}. Sender: {}, Receiver: {}, Available accounts: {}", 
                                 line_idx, sender_idx, receiver_idx, self.account_universe.num_accounts()));
            }
            
            transaction_graph.push((sender_idx, receiver_idx));
            
            // 创建CSV记录
            let sender_addr = format!("account_{}", sender_idx);
            let receiver_addr = format!("account_{}", receiver_idx);
            let amount = parts.get(2).unwrap_or(&"1000000").trim().parse::<u64>().unwrap_or(1000000);
            let timestamp = parts.get(3).unwrap_or(&"0").trim().parse::<u64>().unwrap_or(0);
            
            csv_records.push(CsvTransactionRecord {
                csv_index: line_idx - 1, // 减1因为跳过了标题行
                sender_address: sender_addr,
                receiver_address: receiver_addr,
                amount,
                timestamp,
                transaction_hash: format!("csv_txn_{}", line_idx - 1),
            });
        }
        
        println!("Loaded {} transaction records from CSV", csv_records.len());
        
        // 生成交易
        let transactions = self.gen_transaction_for_erc20(transaction_graph);
        
        Ok((transactions, csv_records))
    }

    /// 记录详细交易映射信息
    /// 
    /// 为每个交易记录详细的映射关系信息
    fn log_detailed_transaction_mappings(
        &self,
        transactions: &[SignatureVerifiedTransaction],
        csv_records: &[CsvTransactionRecord],
    ) {
        if !self.log_enabled {
            return;
        }
        
        for (block_stm_index, csv_record) in csv_records.iter().enumerate() {
            if block_stm_index < transactions.len() {
                // 记录详细交易映射
                if let Some(logger) = aptos_block_executor::block_stm_logger::get_global_logger() {
                    logger.log_performance_metric(
                        "detailed_transaction_mapping",
                        block_stm_index as f64,
                        Some(block_stm_index as u32),
                        std::collections::HashMap::from([
                            ("csv_index".to_string(), csv_record.csv_index.to_string()),
                            ("block_id".to_string(), self.current_block_id.to_string()),
                            ("transaction_hash".to_string(), csv_record.transaction_hash.clone()),
                            ("sender_address".to_string(), csv_record.sender_address.clone()),
                            ("receiver_address".to_string(), csv_record.receiver_address.clone()),
                            ("amount".to_string(), csv_record.amount.to_string()),
                        ])
                    );
                }
            }
        }
    }

    /// 为 ERC20 类型交易生成 Aptos 交易
    /// 
    /// 根据交易图结构生成对应的 Aptos 点对点转账交易
    /// 
    /// 参数:
    /// - transaction_graph: 交易图，每个元组表示 (from_account, to_account)
    /// 
    /// 返回:
    /// - 签名验证交易列表
    pub fn gen_transaction_for_erc20(&mut self, transaction_graph: Vec<(usize, usize)>) -> Vec<SignatureVerifiedTransaction> {
        let mut seq_map: HashMap<usize, usize> = HashMap::new(); // 账户序列号映射
        let mut signed_transactions = Vec::new();
        
        // 遍历交易图，为每个交易生成签名交易
        for tuple in &transaction_graph {
            // 获取发送方和接收方账户
            let sender = self.account_universe.account(tuple.0);
            let receiver = self.account_universe.account(tuple.1);
            // 更新发送方账户的序列号
            let entry = seq_map.entry(tuple.0);
            match entry {
                std::collections::hash_map::Entry::Occupied(mut occupied) => {
                    *occupied.get_mut() += 1;
                }
                std::collections::hash_map::Entry::Vacant(vacant) => {
                    vacant.insert(sender.sequence_number() as usize);
                }
            };
            // 生成点对点转账交易
            let txn = peer_to_peer_txn(
                sender.account(), 
                receiver.account(), 
                seq_map[&tuple.0] as u64, 
                1,   // 转账金额
                100  // Gas 限制
            );
            signed_transactions.push(txn);   
        }
        // 转换为签名验证交易格式
        let transactions: Vec<Transaction> = signed_transactions
            .into_iter()
            .map(Transaction::UserTransaction)
            .collect();
        into_signature_verified_block(transactions)       
    }

    fn execute_and_apply_transactions(
        &mut self,
        signed_transactions:Vec<SignedTransaction>,
    ){
        let signature_verified_transactions = into_signature_verified_block(
            signed_transactions
            .into_iter()
            .map(|txn| {
                Transaction::UserTransaction(txn)
            })
            .collect()
        );
        let txn_provider = DefaultTxnProvider::new_without_info(signature_verified_transactions);
        let block_executor = AptosVMBlockExecutor::new();
        let output = block_executor.execute_block(
            &txn_provider,
            self.executor.state_store(),
            aptos_types::block_executor::config::BlockExecutorConfigFromOnchain::new_no_block_limit(),
            aptos_types::block_executor::transaction_slice_metadata::TransactionSliceMetadata::unknown(),
        )
        .expect("VM should not fail to start")
        .into_transaction_outputs_forced();
        output.iter().for_each(|txn_output| {
            assert_eq!(
                txn_output.status(),
                &TransactionStatus::Keep(ExecutionStatus::Success)
            );
            self.executor.apply_write_set(txn_output.write_set());
        });
    }

    /// 执行顺序基准测试
    /// 
    /// 使用单线程顺序执行交易，用于与并行执行性能对比
    /// 
    /// 参数:
    /// - transactions: 待执行的交易列表
    /// - maybe_block_gas_limit: 区块 Gas 限制（可选）
    /// 
    /// 返回:
    /// - (交易输出列表, TPS)
    fn execute_benchmark_sequential(
        &self,
        transactions: &[SignatureVerifiedTransaction],
        maybe_block_gas_limit: Option<u64>,
    ) -> (Vec<TransactionOutput>, usize) {
        use aptos_block_executor::counters;
        use aptos_types::block_executor::config::{BlockExecutorConfig, BlockExecutorLocalConfig, BlockExecutorModuleCacheLocalConfig};
        
        let block_size = transactions.len();
        
        // 执行前重置计数器
        let execution_before = counters::TASK_EXECUTE_SECONDS.get_sample_count();
        let validation_before = counters::TASK_VALIDATE_SECONDS.get_sample_count();
        let abort_before = counters::SPECULATIVE_ABORT_COUNT.get();
        
        let timer = Instant::now();
        let txn_provider = DefaultTxnProvider::new_without_info(transactions.to_vec());
        let block_executor = AptosVMBlockExecutor::new();
        
        let config = BlockExecutorConfig {
            local: BlockExecutorLocalConfig {
                blockstm_v2: true,
                concurrency_level: 1,
                allow_fallback: true,
                discard_failed_blocks: false,
                module_cache_config: BlockExecutorModuleCacheLocalConfig::default(),
            },
            onchain: aptos_types::block_executor::config::BlockExecutorConfigFromOnchain::new_maybe_block_limit(maybe_block_gas_limit),
        };
        
        let output = block_executor.execute_block_with_config(
            &txn_provider,
            self.executor.state_store(),
            config,
            aptos_types::block_executor::transaction_slice_metadata::TransactionSliceMetadata::unknown(),
        )
        .expect("VM should not fail to start")
        .into_transaction_outputs_forced();
        let exec_time = timer.elapsed().as_millis();
        
        // 计算此次执行的计数器增量
        let execution_total = counters::TASK_EXECUTE_SECONDS.get_sample_count() - execution_before;
        let validation_total = counters::TASK_VALIDATE_SECONDS.get_sample_count() - validation_before;
        let abort = counters::SPECULATIVE_ABORT_COUNT.get() - abort_before;
        
        // 注意：顺序执行没有停滞机制
        println!("执行次数:{}, 验证次数:{}, 中止次数:{}", 
            execution_total, validation_total, abort);
        (output, block_size * 1000 / exec_time as usize)
    }

    /// 执行并行基准测试
    /// 
    /// 使用 Block-STM 并行执行交易，收集详细性能指标
    /// 
    /// 参数:
    /// - transactions: 待执行的交易列表
    /// - concurrency_level_per_shard: 并发执行级别
    /// - maybe_block_gas_limit: 区块 Gas 限制（可选）
    /// 
    /// 返回:
    /// - (交易输出列表, TPS)
    fn execute_benchmark_parallel(
        &self,
        transactions: &[SignatureVerifiedTransaction],
        concurrency_level_per_shard: usize,
        maybe_block_gas_limit: Option<u64>,
    ) -> (Vec<TransactionOutput>, usize) {
        use aptos_block_executor::counters;
        use aptos_types::block_executor::config::{BlockExecutorConfig, BlockExecutorLocalConfig, BlockExecutorModuleCacheLocalConfig};
        
        let block_size = transactions.len();
        
        
        // 记录区块执行开始
        if self.log_enabled {
            println!("Block-STM parallel execution start: block {}, {} transactions, concurrency {}", 
                    self.current_block_id, block_size, concurrency_level_per_shard);
        }
        
        // Reset counters before execution
        let execution_before = counters::TASK_EXECUTE_SECONDS.get_sample_count();
        let validation_before = counters::TASK_VALIDATE_SECONDS.get_sample_count();
        let abort_before = counters::SPECULATIVE_ABORT_COUNT.get();
        
        // Reset BlockSTM logger stall statistics
        if let Some(logger) = aptos_block_executor::block_stm_logger::get_global_logger() {
            logger.reset_stall_statistics();
        }
        
        let timer = Instant::now();
        let txn_provider = DefaultTxnProvider::new_without_info(transactions.to_vec());
        let block_executor = AptosVMBlockExecutor::new();
        
        let config = BlockExecutorConfig {
            local: BlockExecutorLocalConfig {
                blockstm_v2: true,
                concurrency_level: concurrency_level_per_shard,
                allow_fallback: true,
                discard_failed_blocks: false,
                module_cache_config: BlockExecutorModuleCacheLocalConfig::default(),
            },
            onchain: aptos_types::block_executor::config::BlockExecutorConfigFromOnchain::new_maybe_block_limit(maybe_block_gas_limit),
        };
        
        let output = block_executor.execute_block_with_config(
            &txn_provider,
            self.executor.state_store(),
            config,
            aptos_types::block_executor::transaction_slice_metadata::TransactionSliceMetadata::unknown(),
        )
        .expect("VM should not fail to start")
        .into_transaction_outputs_forced();
        let exec_time = timer.elapsed().as_millis();
        
        // Calculate deltas for this execution
        let execution_total = counters::TASK_EXECUTE_SECONDS.get_sample_count() - execution_before;
        let validation_total = counters::TASK_VALIDATE_SECONDS.get_sample_count() - validation_before;
        let abort = counters::SPECULATIVE_ABORT_COUNT.get() - abort_before;
        
        // Get transaction stall statistics from BlockSTMLogger (only real stalls)
        let (transaction_stalls, stall_time_total, avg_stall_time) = if let Some(logger) = 
            aptos_block_executor::block_stm_logger::get_global_logger() {
            let tx_stall_count = logger.get_transaction_stall_count() as u64;
            
            // Get real total stall time in microseconds and convert to seconds
            let total_time_us = logger.get_total_stall_time_us();
            let total_time = total_time_us / 1_000_000.0; // Convert microseconds to seconds
            let avg_time = if tx_stall_count > 0 { total_time / tx_stall_count as f64 } else { 0.0 };
            
            (tx_stall_count, total_time, avg_time)
        } else {
            // If logger not available, report zero stall statistics
            (0u64, 0.0f64, 0.0f64)
        };
        
        let tps = block_size * 1000 / exec_time as usize;
        
        // 记录区块执行结束
        if self.log_enabled {
            let successful_txns = output.iter().filter(|o| matches!(o.status(), TransactionStatus::Keep(ExecutionStatus::Success))).count();
            let aborted_txns = output.len() - successful_txns;
            
            println!("Block-STM parallel execution end: block {}, {} transactions, successful: {}, failed: {}", 
                    self.current_block_id, block_size, successful_txns, aborted_txns);
        }
        
        // 打印执行统计信息（只显示真正的交易stall）
        println!("执行次数:{}, 验证次数:{}, 中止次数:{}, 停滞次数:{}, 平均停滞时间:{:.2} us, 总停滞时间:{:.2} us", 
            execution_total,
            validation_total,
            abort,
            transaction_stalls,    // 只显示交易级停滞次数（真正的stall）
            avg_stall_time * 1000000.0,
            stall_time_total * 1000000.0
        );
        (output, tps)
    }

    /// 执行并行基准测试（返回详细指标）
    /// 
    /// 使用 Block-STM 并行执行交易，返回详细的性能指标结构
    /// 
    /// 参数:
    /// - transactions: 待执行的交易列表
    /// - concurrency_level_per_shard: 并发执行级别
    /// - maybe_block_gas_limit: 区块 Gas 限制（可选）
    /// 
    /// 返回:
    /// - (交易输出列表, 详细执行指标)
    fn execute_benchmark_parallel_with_metrics(
        &self,
        transactions: &[SignatureVerifiedTransaction],
        concurrency_level_per_shard: usize,
        maybe_block_gas_limit: Option<u64>,
    ) -> (Vec<TransactionOutput>, DetailedExecutionMetrics) {
        use aptos_block_executor::counters;
        use aptos_types::block_executor::config::{BlockExecutorConfig, BlockExecutorLocalConfig, BlockExecutorModuleCacheLocalConfig};
        
        let block_size = transactions.len();
        
        // 记录区块执行开始
        if self.log_enabled {
            println!("Block-STM parallel execution with metrics start: block {}, {} transactions, concurrency {}", 
                    self.current_block_id, block_size, concurrency_level_per_shard);
        }
        
        // Reset counters before execution
        let execution_before = counters::TASK_EXECUTE_SECONDS.get_sample_count();
        let validation_before = counters::TASK_VALIDATE_SECONDS.get_sample_count();
        let abort_before = counters::SPECULATIVE_ABORT_COUNT.get();
        
        let timer = Instant::now();
        let txn_provider = DefaultTxnProvider::new_without_info(transactions.to_vec());
        let block_executor = AptosVMBlockExecutor::new();
        
        let config = BlockExecutorConfig {
            local: BlockExecutorLocalConfig {
                blockstm_v2: true,
                concurrency_level: concurrency_level_per_shard,
                allow_fallback: true,
                discard_failed_blocks: false,
                module_cache_config: BlockExecutorModuleCacheLocalConfig::default(),
            },
            onchain: aptos_types::block_executor::config::BlockExecutorConfigFromOnchain::new_maybe_block_limit(maybe_block_gas_limit),
        };
        
        let output = block_executor.execute_block_with_config(
            &txn_provider,
            self.executor.state_store(),
            config,
            aptos_types::block_executor::transaction_slice_metadata::TransactionSliceMetadata::unknown(),
        )
        .expect("VM should not fail to start")
        .into_transaction_outputs_forced();
        let exec_time = timer.elapsed().as_millis();
        
        // Calculate deltas for this execution
        let execution_total = counters::TASK_EXECUTE_SECONDS.get_sample_count() - execution_before;
        let validation_total = counters::TASK_VALIDATE_SECONDS.get_sample_count() - validation_before;
        let abort = counters::SPECULATIVE_ABORT_COUNT.get() - abort_before;
        
        // Get transaction stall statistics from BlockSTMLogger (only real stalls)
        let (transaction_stalls, stall_time_total, avg_stall_time) = if let Some(logger) = 
            aptos_block_executor::block_stm_logger::get_global_logger() {
            let tx_stall_count = logger.get_transaction_stall_count() as u64;
            
            // Get real total stall time in microseconds and convert to seconds
            let total_time_us = logger.get_total_stall_time_us();
            let total_time = total_time_us / 1_000_000.0; // Convert microseconds to seconds
            let avg_time = if tx_stall_count > 0 { total_time / tx_stall_count as f64 } else { 0.0 };
            
            (tx_stall_count, total_time, avg_time)
        } else {
            // If logger not available, report zero stall statistics
            (0u64, 0.0f64, 0.0f64)
        };
        
        let tps = block_size * 1000 / exec_time as usize;
        
        // 记录区块执行结束
        if self.log_enabled {
            let successful_txns = output.iter().filter(|o| matches!(o.status(), TransactionStatus::Keep(ExecutionStatus::Success))).count();
            let aborted_txns = output.len() - successful_txns;
            
            println!("Block-STM parallel execution with metrics end: block {}, TPS: {}, successful: {}, aborted: {}", 
                    self.current_block_id, tps, successful_txns, aborted_txns);
        }
        
        let metrics = DetailedExecutionMetrics {
            execution_count: execution_total,
            validation_count: validation_total,
            abort_count: abort,
            stall_count: transaction_stalls,
            avg_stall_time_us: avg_stall_time * 1000000.0,
            total_stall_time_us: stall_time_total * 1000000.0,
            execution_time_ms: exec_time,
            tps,
        };
        
        // 注意：详细的停滞分析可在生成的日志文件中查看
        if self.log_enabled {
            println!("执行次数:{}, 验证次数:{}, 中止次数:{} （详细指标在日志和返回值中）", 
                execution_total, validation_total, abort);
        } else {
            println!("执行次数:{}, 验证次数:{}, 中止次数:{}, 停滞次数:{}, 平均停滞时间:{:.2} us, 总停滞时间:{:.2} us", 
                execution_total,
                validation_total,
                abort,
                transaction_stalls,    // 只显示交易级停滞次数（真正的stall）
                avg_stall_time * 1000000.0,
                stall_time_total * 1000000.0
            );
        }
        
        (output, metrics)
    }

    /// 执行 Block-STM 基准测试
    /// 
    /// 执行并行和/或顺序基准测试，返回 TPS 对比结果
    /// 
    /// 参数:
    /// - transactions: 待执行的交易列表
    /// - run_par: 是否运行并行测试
    /// - run_seq: 是否运行顺序测试
    /// - concurrency_level_per_shard: 并发执行级别
    /// - maybe_block_gas_limit: 区块 Gas 限制（可选）
    /// 
    /// 返回:
    /// - (并行 TPS, 顺序 TPS)
    pub fn execute_blockstm_benchmark(
        &mut self,
        transactions: Vec<SignatureVerifiedTransaction>,
        run_par: bool,
        run_seq: bool,
        concurrency_level_per_shard: usize,
        maybe_block_gas_limit: Option<u64>,
    ) -> (usize, usize) {
        // 执行 Block-STM 基准测试
        let (output, par_tps) = if run_par {
            if concurrency_level_per_shard == 1 {
                // 单核使用顺序执行路径
                println!("并行执行开始...");
                let (output, tps) = self.execute_benchmark_sequential(&transactions, maybe_block_gas_limit);
                println!("并行执行完成，TPS = {}", tps);
                (output, tps)
            } else {
                // 多核使用并行执行路径
                println!("并行执行开始...");
                let (output, tps) = self.execute_benchmark_parallel(
                    &transactions, 
                    concurrency_level_per_shard,
                    maybe_block_gas_limit
                );
                println!("并行执行完成，TPS = {}", tps);
                (output, tps)
            }
        } else {
            (vec![], 0)
        };
        output.iter().for_each(|txn_output| {
            assert_eq!(
                txn_output.status(),
                &TransactionStatus::Keep(ExecutionStatus::Success)
            );
        });
        let (output, seq_tps) = if run_seq {
            println!("顺序执行开始...");
            let (output, tps) = self.execute_benchmark_sequential(&transactions, maybe_block_gas_limit);
            println!("顺序执行完成，TPS = {}", tps);
            (output, tps)
        } else {
            (vec![], 0)
        };
        output.iter().for_each(|txn_output| {
            assert_eq!(
                txn_output.status(),
                &TransactionStatus::Keep(ExecutionStatus::Success)
            );
        });
        (par_tps, seq_tps)
    }

    /// 运行空投基准测试
    /// 
    /// 模拟空投场景，包括初始化和批量转账
    pub fn run_airdrop(&mut self, transaction_nums: usize) {
        //initialize accounts
        let mut initialized_transactions = Vec::new();
        {
            for i in 0..self.account_universe.num_accounts(){
                let sender = self.account_universe.account_mut(i as usize);
                let txn = airdrop_initialize_txn(
                    sender.account(), 
                    1_000_000_000, 
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                initialized_transactions.push(txn);
            }
        }
        self.execute_and_apply_transactions(initialized_transactions);
        
        //execute airdrop transfer
        println!("execute transfer for airdrop...");
        let mut transfer_transactions = Vec::new();
        let acc_nums = self.account_universe.num_accounts();
        {
            let mut rng = rand::thread_rng();
            for _i in 0..transaction_nums{
                let random_number: usize = rng.gen_range(0, acc_nums);
                let mut receivers = Vec::new();
                for _j in 0..5{
                    let random_number = rng.gen_range(0, acc_nums);
                    let &receiver = self.account_universe.account_mut(random_number as usize).account().address();
                    receivers.push(receiver);
                }
                let sender = self.account_universe.account_mut(random_number);
                let txn = airdrop_transfer_txn(
                    sender.account(),
                    receivers,
                    sender.sequence_number(),
                    100,
                );
                sender.increase_account_sequence_number();
                transfer_transactions.push(txn);
            }
        }
        let (par_tps, seq_tps) = self.execute_blockstm_benchmark(
            into_signature_verified_block(
                transfer_transactions
                    .into_iter()
                    .map(|txn|{
                        Transaction::UserTransaction(txn)
                    })
                    .collect()
            ), 
            true, 
            true, 
            num_cpus::get(), 
            None
        );
        print_tps!(par_tps,seq_tps);
    }

    /// 运行投票基准测试
    /// 
    /// 模拟投票场景，包括初始化和批量投票
    pub fn run_ballot(&mut self, transaction_num: usize) {
        let acc_nums = self.account_universe.num_accounts();
        let mut initialized_transactions = Vec::new();
        {
            let (forums,_accounts) = self.account_universe.accounts_mut().split_at_mut(1);
            //initialize transaction for ballot
            let proposals:Vec<Vec<u8>> = vec!["aa".to_string(),"bb".to_string(),"cc".to_string()].into_iter().map(|s| s.into_bytes()).collect();
            let txn = ballot_initialize_txn(
                forums[0].account(), 
                proposals, 
                forums[0].sequence_number(), 
                100
            );
            forums[0].increase_account_sequence_number();
            initialized_transactions.push(txn);
        }
        self.execute_and_apply_transactions(initialized_transactions);
        
        //vote
        println!("execute vote for ballot...");
        let mut vote_transactions = Vec::new();
        {
            let mut rng = rand::thread_rng();
            let (forums,accounts) = self.account_universe.accounts_mut().split_at_mut(1);
            for _i in 0..transaction_num{
                let proposal_index = rng.gen_range(0, 3);
                let sender_index = rng.gen_range(0, acc_nums-1);
                let sender = &mut accounts[sender_index];
                let txn = ballot_vote_txn(
                    sender.account(),
                    forums[0].account().address(), 
                    proposal_index, 
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                vote_transactions.push(txn);
            }
        }
        let (par_tps, seq_tps) = self.execute_blockstm_benchmark(
            into_signature_verified_block(
                vote_transactions
                        .into_iter()
                        .map(|txn|{
                            Transaction::UserTransaction(txn)
                        })
                        .collect()
            ), 
            true, 
            true, 
            num_cpus::get(), 
            None
        );
        print_tps!(par_tps,seq_tps);
    }

    /// 运行分片投票基准测试
    /// 
    /// 模拟多分片投票场景
    pub fn run_ballot_sharding(&mut self, transaction_num: usize, shard_num: usize) {
        let acc_nums = self.account_universe.num_accounts();
        let mut initialized_transactions = Vec::new();
        {
            let (forums,_accounts) = self.account_universe.accounts_mut().split_at_mut(shard_num);
            //initialize transaction for ballot
            for i in 0..shard_num{
                let proposals:Vec<Vec<u8>> = vec!["aa".to_string(),"bb".to_string(),"cc".to_string()].into_iter().map(|s| s.into_bytes()).collect();
                let txn = ballot_initialize_txn(
                    forums[i].account(), 
                    proposals, 
                    forums[i].sequence_number(), 
                    100
                );
                forums[i].increase_account_sequence_number();
                initialized_transactions.push(txn);
            }
        }
        self.execute_and_apply_transactions(initialized_transactions);
        
        //vote
        println!("execute vote for ballot...");
        let mut vote_transactions = Vec::new();
        {
            let mut rng = rand::thread_rng();
            let (forums,accounts) = self.account_universe.accounts_mut().split_at_mut(shard_num);
            for _i in 0..transaction_num{
                let proposal_index = rng.gen_range(0, 3);
                let sender_index = rng.gen_range(0, acc_nums-shard_num);
                let sender = &mut accounts[sender_index];
                let forums_index = sender.account().address().to_vec()[31] % shard_num as u8;
                let txn = ballot_vote_txn(
                    sender.account(),
                    forums[forums_index as usize].account().address(), 
                    proposal_index, 
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                vote_transactions.push(txn);
            }
        }
        let (par_tps, seq_tps) = self.execute_blockstm_benchmark(
            into_signature_verified_block(
                vote_transactions
                        .into_iter()
                        .map(|txn|{
                            Transaction::UserTransaction(txn)
                        })
                        .collect()
            ), 
            true, 
            true, 
            num_cpus::get(), 
            None
        );
        print_tps!(par_tps,seq_tps);
    }

    pub fn run_mp(&mut self, transaction_num:usize){
        
        let mut initialized_transactions = Vec::new();
        {
            let (globalstore,_accounts) = self.account_universe.accounts_mut().split_at_mut(1);
            //initialize transaction for mp
            let txn = million_pixel_initialize_txn(
                globalstore[0].account(), 
                globalstore[0].sequence_number(), 
                100
            );
            globalstore[0].increase_account_sequence_number();
            initialized_transactions.push(txn);
        }
        self.execute_and_apply_transactions(initialized_transactions);
        
        //occupy
        println!("execute occupy for million_pixel...");
        let mut occupy_transactions = Vec::new();
        {
            let mut rng = rand::thread_rng();
            let (globalstore,accounts) = self.account_universe.accounts_mut().split_at_mut(1);
            let acc_nums = accounts.len();
            for _i in 0..transaction_num{    
                let x = rng.gen_range(0, 100);
                let y  = rng.gen_range(0, 100);
                let sender_index = rng.gen_range(0, acc_nums);
                let sender = &mut accounts[sender_index];
                let txn = million_pixel_occupy_txn(
                    sender.account(),
                    globalstore[0].account().address(), 
                    x, 
                    y,
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                occupy_transactions.push(txn);
            }
        }
        let (par_tps, seq_tps) = self.execute_blockstm_benchmark(
            into_signature_verified_block(
                occupy_transactions
                        .into_iter()
                        .map(|txn|{
                            Transaction::UserTransaction(txn)
                        })
                        .collect()
            ), 
            true, 
            true, 
            num_cpus::get(), 
            None
        );
        print_tps!(par_tps,seq_tps);
    }

    pub fn run_kitty(&mut self, transaction_num:usize){
        let mut initialized_transactions = Vec::new();
        {
            let (globalstore,accounts) = self.account_universe.accounts_mut().split_at_mut(1);
            let acc_nums = accounts.len();
            //initialize transaction for kitty
            let txn = kitty_initialize_txn(
                globalstore[0].account(), 
                globalstore[0].sequence_number(), 
                100
            );
            globalstore[0].increase_account_sequence_number();
            initialized_transactions.push(txn);
            let mut rng = rand::thread_rng();
            for i in 0..acc_nums{
                let sender = &mut accounts[i];
                                let txn = kitty_mint_txn(
                    sender.account(), 
                    globalstore[0].account().address(), 
                    rng.gen_range(0, (1<<32)-1), 
                    true, 
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                initialized_transactions.push(txn);
            }

            for i in 0..acc_nums{
                let sender = &mut accounts[i];
                let txn = kitty_mint_txn(
                    sender.account(), 
                    globalstore[0].account().address(), 
                    rng.gen_range(0, (1<<32)-1), 
                    false, 
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                initialized_transactions.push(txn);
            }
            
        }
        self.execute_and_apply_transactions(initialized_transactions);
        
        //breed
        println!("execute breed for kitty...");
        let mut breed_transactions = Vec::new();
        {
            let mut rng = rand::thread_rng();
            let (globalstore,accounts) = self.account_universe.accounts_mut().split_at_mut(1);
            let acc_nums = accounts.len();
            for _i in 0..transaction_num{
                let m = rng.gen_range(0, acc_nums) as u64;
                let s  = rng.gen_range(acc_nums, acc_nums*2) as u64;
                let sender_index = rng.gen_range(0, acc_nums);
                let sender = &mut accounts[sender_index];
                let txn = kitty_breed_txn(
                    sender.account(),
                    globalstore[0].account().address(), 
                    m,
                    s,
                    false,
                    sender.sequence_number(), 
                    100
                );
                sender.increase_account_sequence_number();
                breed_transactions.push(txn);
            }
        }
        let (par_tps, seq_tps) = self.execute_blockstm_benchmark(
            into_signature_verified_block(
                breed_transactions
                        .into_iter()
                        .map(|txn|{
                            Transaction::UserTransaction(txn)
                        })
                        .collect()
            ), 
            true, 
            true, 
            num_cpus::get(), 
            None
        );
        print_tps!(par_tps,seq_tps);

    }


    pub fn run_empty(&mut self,transaction_nums:usize){
        //execute airdrop transfer
        println!("execute empty...");
        let mut empty_transactions = Vec::new();
        let mut rng = rand::thread_rng();
        let acc_nums = self.account_universe.num_accounts();
        for _i in 0..transaction_nums{
            let random_number: usize = rng.gen_range(0, acc_nums);
            let sender = self.account_universe.account_mut(random_number);
            let txn = empty_empty_txn(sender.account(), sender.sequence_number(),100);
            sender.increase_account_sequence_number();
            empty_transactions.push(txn);
        }
        let (par_tps, seq_tps) = self.execute_blockstm_benchmark(
            into_signature_verified_block(
                empty_transactions
                    .into_iter()
                    .map(|txn|{
                        Transaction::UserTransaction(txn)
                    })
                    .collect()
            ), 
            true, 
            true, 
            num_cpus::get(), 
            None
        );
        print_tps!(par_tps,seq_tps);
    }

    /// 重放 ERC20 历史数据基准测试
    /// 
    /// 使用真实的历史交易数据进行 Block-STM 性能测试
    /// 
    /// 参数:
    /// - data_path: CSV 历史数据文件路径
    /// - skip_parallel: 是否跳过并行测试
    /// - skip_sequential: 是否跳过顺序测试
    /// - num_runs: 运行次数
    /// - maybe_block_gas_limit: 区块 Gas 限制
    /// - concurrency_level: 并发执行级别
    pub fn replay_erc20_historic(
        &mut self,
        data_path: String,
        skip_parallel: bool,
        skip_sequential: bool,
        num_runs: usize,
        maybe_block_gas_limit: Option<u64>,
        concurrency_level: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        disable_speculative_logging();
        
        // 初始化日志环境
        if let Err(e) = self.setup_logging_environment() {
            eprintln!("日志环境设置失败: {}", e);
            eprintln!("继续执行（不记录日志）...");
        }
        
        // 设置执行上下文
        self.setup_execution_context(&data_path);
        
        println!("Reading ERC20 historic data from: {}", data_path);
        
        // 使用新的CSV数据加载方法
        let transaction_graph = self.load_csv_data(&data_path)?;
        
        // Generate transactions
        let transactions = self.gen_transaction_for_erc20(transaction_graph);
        println!("Generated {} signature verified transactions", transactions.len());
        
        // 处理交易映射（如果启用了日志）
        self.process_transaction_mapping(&transactions);
        
        // 记录基准测试开始
        if self.log_enabled {
            println!("Block-STM logging enabled for block {}, {} transactions, concurrency level {}", 
                    self.current_block_id, transactions.len(), concurrency_level);
        }
        
        // Run benchmarks
        for i in 0..num_runs {
            println!("Benchmark run {}/{}", i + 1, num_runs);
            let (_par_tps, _seq_tps) = self.execute_blockstm_benchmark(
                transactions.clone(),
                skip_parallel,  // run_par: directly use skip_parallel (already negated in main.rs)
                skip_sequential, // run_seq: directly use skip_sequential (already negated in main.rs)
                concurrency_level,
                maybe_block_gas_limit,
            );
            // TPS results are already printed inside execute_blockstm_benchmark
        }
        
        // 记录基准测试结束
        if self.log_enabled {
            println!("Block-STM ERC20 historic replay end: block {}, {} transactions, concurrency {}", 
                    self.current_block_id, transactions.len(), concurrency_level);
            
            // 注意：统计信息现在通过日志文件分析获取
        }
        
        Ok(())
    }

    /// 完整的 CSV 回放测试（含详细日志）
    /// 
    /// 执行完整的 CSV 数据回放，记录详细的性能指标和日志
    /// 
    /// 参数:
    /// - data_path: CSV 数据文件路径
    /// - concurrency_level: 并发执行级别
    /// - num_runs: 运行次数
    /// 
    /// 返回:
    /// - 每次运行的详细执行指标列表
    pub fn replay_with_full_logging(
        &mut self,
        data_path: &str,
        concurrency_level: usize,
        num_runs: usize,
    ) -> Result<Vec<DetailedExecutionMetrics>, Box<dyn std::error::Error>> {
        // 强制启用日志记录
        self.log_enabled = true;
        
        // 设置日志环境
         let _ = self.setup_logging_environment();
         
         // 记录回放开始
         println!("Block-STM CSV replay start: block {}, concurrency {}, data_path: {}", 
                 self.current_block_id, concurrency_level, data_path);
         
         // 加载和验证CSV数据
         let _ = self.load_csv_data(data_path);
         let (transactions, csv_records) = self.load_transactions_from_csv(data_path, None)?;
         
         // 如果没有CSV记录，生成交易
         let transactions = if transactions.is_empty() {
             let transaction_graph = self.load_csv_data(data_path)?;
             self.gen_transaction_for_erc20(transaction_graph)
         } else {
             transactions
         };
        
        // 记录CSV数据统计
        println!("Block {} DataLoaded CSV with {} records", 
            self.current_block_id, csv_records.len());
        
        // 记录详细的交易映射
        self.log_detailed_transaction_mappings(&transactions, &csv_records);
        
        let mut metrics_results = Vec::new();
        
        // 基准测试运行
        for run_idx in 0..num_runs {
            println!("Block {} Start Benchmark_Run_{} with {} transactions at concurrency {}", 
                self.current_block_id, run_idx, transactions.len(), concurrency_level);
            
            let (outputs, metrics) = self.execute_benchmark_parallel_with_metrics(
                &transactions,
                concurrency_level,
                None,
            );
            
            // 验证执行结果
            let successful_count = outputs.iter()
                .filter(|o| matches!(o.status(), TransactionStatus::Keep(ExecutionStatus::Success)))
                .count();
            let failed_count = outputs.len() - successful_count;
            
            if let Some(logger) = aptos_block_executor::block_stm_logger::get_global_logger() {
                logger.log_performance_metric(
                    "execution_results",
                    successful_count as f64,
                    None,
                    std::collections::HashMap::from([
                        ("successful_count".to_string(), successful_count.to_string()),
                        ("failed_count".to_string(), failed_count.to_string()),
                        ("abort_count".to_string(), metrics.abort_count.to_string()),
                        ("stall_count".to_string(), metrics.stall_count.to_string())
                    ])
                );
            }
            
            println!("Block {} End: Benchmark_Run_{}, transactions: {}, concurrency: {}, tps: {}, execution_time_ms: {}",
                self.current_block_id,
                run_idx,
                transactions.len(),
                concurrency_level,
                metrics.tps,
                metrics.execution_time_ms
            );
            
            metrics_results.push(metrics);
        }
        
        // 记录回放结束
        let avg_tps = metrics_results.iter().map(|m| m.tps).sum::<usize>() / metrics_results.len();
        println!("Block {} End CSV_Replay with {} transactions, avg TPS: {}", 
            self.current_block_id, transactions.len(), avg_tps);
        
        Ok(metrics_results)
    }

    /// 执行 CSV 回放基准测试
    /// 
    /// 执行单次 CSV 数据回放基准测试，返回详细指标
    /// 
    /// 参数:
    /// - data_path: CSV 数据文件路径
    /// - concurrency_level: 并发执行级别
    /// 
    /// 返回:
    /// - (详细执行指标列表, CSV 交易记录列表)
    pub fn execute_csv_replay_benchmark(
        &mut self,
        data_path: &str,
        concurrency_level: usize,
    ) -> Result<(Vec<DetailedExecutionMetrics>, Vec<CsvTransactionRecord>), Box<dyn std::error::Error>> {
        // 强制启用日志记录
        self.log_enabled = true;
        
        // 设置日志环境
         let _ = self.setup_logging_environment();
         
         // 加载CSV数据和交易记录
         let _ = self.load_csv_data(data_path);
         let (transactions, csv_records) = self.load_transactions_from_csv(data_path, None)?;
         
         // 如果没有CSV记录，生成交易
         let transactions = if transactions.is_empty() {
             let transaction_graph = self.load_csv_data(data_path)?;
             self.gen_transaction_for_erc20(transaction_graph)
         } else {
             transactions
         };
        
        // 记录详细的交易映射
        self.log_detailed_transaction_mappings(&transactions, &csv_records);
        
        // 执行基准测试
        let (outputs, metrics) = self.execute_benchmark_parallel_with_metrics(
            &transactions,
            concurrency_level,
            None,
        );
        
        // 验证所有交易都成功执行
        for (i, output) in outputs.iter().enumerate() {
            if !matches!(output.status(), TransactionStatus::Keep(ExecutionStatus::Success)) {
                return Err(format!("Transaction {} failed: {:?}", i, output.status()).into());
            }
        }
        
        Ok((vec![metrics], csv_records))
    }
}