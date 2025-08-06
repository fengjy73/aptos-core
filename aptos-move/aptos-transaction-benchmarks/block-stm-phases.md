# Block-STM 并行执行阶段详细分析

## 概述

本文档详细分析Block-STM并行执行引擎在处理ERC20历史数据重放时的完整调用链和执行阶段。重点关注核心测试命令的执行流程，深入解析每个阶段的源码实现、状态变化和日志记录点。

### 核心测试命令

```bash
cd /Users/bethestar/Downloads/Crystality/BCParallelConcurrencyEvaluation/aptos-core/aptos-move/aptos-transaction-benchmarks && 
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_new cargo run --release -- replay-erc20 --data-path data/ETH_2401_100000.csv --concurrency-level 4 --num-warmups 0 --num-runs 1
```

## 第一阶段：命令行解析与初始化

### 1.1 程序入口点

**源码位置：** `src/main.rs:300-346`

```rust
fn main() {
    aptos_logger::Logger::new().init();
    START_TIME.set(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    );
    aptos_node_resource_metrics::register_node_metrics_collector(None);
    let _mp = MetricsPusher::start_for_local_run("block-stm-benchmark");
    let args = Args::parse();

    match args.command {
        BenchmarkCommand::ReplayERC20(opt) => {
            if let Err(e) = replay_erc20_historic(opt) {
                eprintln!("Error in replay_erc20_historic: {}", e);
            }
        },
        // ... 其他命令分支
    }
}
```

**执行状态：**
- 初始化Aptos日志系统
- 设置程序启动时间戳
- 注册节点资源监控指标
- 启动本地运行的指标推送器
- 解析命令行参数

**日志记录点：**
- 系统级日志初始化（aptos_logger）
- 指标收集器注册
- 命令解析结果

### 1.2 命令行参数结构

**源码位置：** `src/main.rs:43-71`

```rust
#[derive(Debug, Parser)]
struct ReplayERC20HistoricOpt{
    #[clap(long)]
    pub skip_parallel: bool,
    #[clap(long)]
    pub skip_sequential: bool,
    #[clap(long, default_value_t = 0)]
    pub num_warmups: usize,
    #[clap(long, default_value_t = 1)]
    pub num_runs: usize,
    #[clap(long)]
    pub maybe_block_gas_limit: Option<u64>,
    #[clap(long, default_value="../data/USDT_240101_240331_data_100000.csv")]
    pub data_path:String,
    #[clap(long,default_value_t=93000)]
    pub num_accounts:usize,
    #[clap(long)]
    pub output_file: Option<String>,
    #[clap(long)]
    pub concurrency_level: Option<usize>,
}
```

**解析结果：**
- `data_path`: "data/ETH_2401_100000.csv"
- `concurrency_level`: 4
- `num_warmups`: 0
- `num_runs`: 1
- `skip_parallel`: false
- `skip_sequential`: false

## 第二阶段：ERC20历史数据重放初始化

### 2.1 replay_erc20_historic函数调用

**源码位置：** `src/main.rs:164-192`

```rust
fn replay_erc20_historic(opt: ReplayERC20HistoricOpt) -> Result<(), Box<dyn Error>> {
    // 如果指定了输出文件，确保目录存在
    if let Some(ref output_path) = opt.output_file {
        if let Some(parent_dir) = Path::new(output_path).parent() {
            fs::create_dir_all(parent_dir)?;
        }
    }

    let mut simulator = Simulator::with_account_nums(opt.num_accounts);
    let concurrency_level = opt.concurrency_level.unwrap_or_else(|| num_cpus::get());
    let result = simulator.replay_erc20_historic(
        opt.data_path,
        opt.skip_parallel,
        opt.skip_sequential,
        opt.num_warmups,
        opt.num_runs,
        opt.maybe_block_gas_limit,
        concurrency_level,
    );

    // 如果指定了输出文件，将结果写入文件
    if let Some(output_path) = opt.output_file {
        let mut file = fs::File::create(&output_path)?;
        writeln!(file, "Replay ERC20 Historic benchmark completed successfully")?;
        println!("Results written to: {}", output_path);
    }

    result
}
```

**执行状态：**
- 创建输出目录（如果指定）
- 初始化Simulator实例，账户数量为93000（默认值）
- 设置并发级别为4
- 调用simulator的replay_erc20_historic方法

### 2.2 Simulator初始化

**源码位置：** `src/simulator.rs:47-67`

```rust
pub fn with_account_nums(num_accounts:usize) -> Self {
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
    
    // Use FakeExecutor's state_store to ensure proper VM initialization with gas schedule
    let universe = universe_gen.setup_gas_cost_stability(executor.state_store());

    Self {
        account_universe: universe,
        executor,
    }
}
```

**执行状态：**
- 创建93000个测试账户
- 每个账户初始余额：2,500,000,000,000 单位
- 初始化FakeExecutor（模拟执行环境）
- 设置Gas成本稳定性

## 第三阶段：Block-STM日志系统初始化

### 3.1 日志配置读取

**源码位置：** `src/simulator.rs:691-708`

```rust
// Initialize Block-STM logger from environment variables
let logging_config = LoggingConfig::default();
println!("Block-STM logging config: enabled={}, log_dir={:?}, log_level={:?}", 
         logging_config.enabled, logging_config.log_dir, logging_config.log_level);

if logging_config.enabled {
    match init_global_logger(logging_config) {
        Ok(()) => {
            println!("Block-STM logging initialized successfully");
        },
        Err(e) => {
            eprintln!("Failed to initialize Block-STM logger: {}", e);
            eprintln!("Continuing without logging...");
        }
    }
} else {
    println!("Block-STM logging disabled (BLOCK_STM_LOG_LEVEL not set or invalid)");
}
```

**执行状态：**
- 从环境变量读取日志配置
  - `BLOCK_STM_LOG_LEVEL=DEBUG`
  - `BLOCK_STM_LOG_DIR=./test_logs_new`
- 初始化全局日志记录器
- 创建日志输出目录

### 3.2 日志系统架构

**源码位置：** `block-executor/src/block_stm_logger.rs:184-228`

```rust
pub struct BlockSTMLogger {
    config: LoggingConfig,
    writers: Arc<Mutex<HashMap<LogFileType, BufWriter<File>>>>,
    start_time: Instant,
}

pub fn new(config: LoggingConfig) -> std::io::Result<Self> {
    std::fs::create_dir_all(&config.log_dir)?;
    
    let mut writers = HashMap::new();
    for file_type in [LogFileType::Execution, LogFileType::Concurrency, 
                      LogFileType::ReadWrite, LogFileType::Performance, 
                      LogFileType::Summary] {
        let filename = format!(
            "{}_{}.jsonl",
            file_type.filename(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        let file_path = config.log_dir.join(filename);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file_path)?;
        writers.insert(file_type, BufWriter::with_capacity(config.buffer_size, file));
    }

    Ok(Self {
        config,
        writers: Arc::new(Mutex::new(writers)),
        start_time: Instant::now(),
    })
}
```

**日志文件类型：**
- `execution_*.jsonl`: 交易执行日志
- `concurrency_*.jsonl`: 并发控制日志
- `readwrite_*.jsonl`: 读写集变化日志
- `performance_*.jsonl`: 性能指标日志
- `summary_*.jsonl`: 汇总统计日志

## 第四阶段：交易数据加载与预处理

### 4.1 CSV数据读取

**源码位置：** `src/simulator.rs:710-728`

```rust
println!("Reading ERC20 historic data from: {}", data_path);

// Read CSV file
let file = File::open(&data_path)?;
let reader = BufReader::new(file);
let mut transaction_graph = Vec::new();

// Skip header line and parse CSV
for (line_num, line) in reader.lines().enumerate() {
    if line_num == 0 { continue; } // Skip header
    let line = line?;
    let parts: Vec<&str> = line.split(',').collect();
    if parts.len() >= 3 {
        if let (Ok(from), Ok(to)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
            transaction_graph.push((from, to));
        }
    }
}

println!("Loaded {} transactions from CSV", transaction_graph.len());
```

**执行状态：**
- 打开CSV文件：`data/ETH_2401_100000.csv`
- 跳过标题行
- 解析每行数据，提取发送方和接收方账户ID
- 构建交易图：`Vec<(usize, usize)>`
- 加载100,000笔交易记录

### 4.2 交易生成

**源码位置：** `src/simulator.rs:69-98`

```rust
pub fn gen_transaction_for_erc20(&mut self,transaction_graph:Vec<(usize,usize)>) -> Vec<SignatureVerifiedTransaction> {
    let mut account_num_max = 0;
    let mut seq_map: HashMap<usize, usize> = HashMap::new();
    let mut signed_transactions = Vec::new();
    
    for tuple in &transaction_graph{
        account_num_max = account_num_max.max(tuple.0).max(tuple.1);
        let sender = self.account_universe.account(tuple.0);
        let receiver = self.account_universe.account(tuple.1);
        let entry = seq_map.entry(tuple.0);
        match entry {
            std::collections::hash_map::Entry::Occupied(mut occupied)=>{
                *occupied.get_mut()+=1;
            }
            std::collections::hash_map::Entry::Vacant(vacant) => {
                vacant.insert(sender.sequence_number() as usize);
            }
        };
        let txn = peer_to_peer_txn(
            sender.account(), 
            receiver.account(), 
            seq_map[&tuple.0] as u64, 
            1,
            100
        );
        signed_transactions.push(into_signature_verified_block(vec![txn]).pop().unwrap());
    }
    signed_transactions
}
```

**执行状态：**
- 遍历交易图，为每个交易对生成P2P转账交易
- 维护每个账户的序列号映射
- 转账金额：1单位
- Gas费用：100单位
- 生成100,000个SignatureVerifiedTransaction

**日志记录点：**
```rust
println!("Generated {} signature verified transactions", transactions.len());
```

## 第五阶段：预热执行（Warmup）

### 5.1 预热循环

**源码位置：** `src/simulator.rs:738-747`

```rust
// Run warmups
for i in 0..num_warmups {
    println!("Warmup run {}/{}", i + 1, num_warmups);
    let _ = self.execute_blockstm_benchmark(
        transactions.clone(),
        !skip_parallel,
        !skip_sequential,
        concurrency_level,
        maybe_block_gas_limit,
    );
}
```

**执行状态：**
- 由于`num_warmups=0`，跳过预热阶段
- 如果有预热，会执行相同的基准测试但忽略结果

## 第六阶段：Block-STM并行执行核心阶段

### 6.1 基准测试执行入口

**源码位置：** `src/simulator.rs:749-760`

```rust
// Run actual benchmarks
for i in 0..num_runs {
    println!("Benchmark run {}/{}", i + 1, num_runs);
    let (_par_tps, _seq_tps) = self.execute_blockstm_benchmark(
        transactions.clone(),
        !skip_parallel,
        !skip_sequential,
        concurrency_level,
        maybe_block_gas_limit,
    );
    // TPS results are already printed inside execute_blockstm_benchmark
}
```

**执行状态：**
- 执行1次基准测试（`num_runs=1`）
- 并行执行：启用（`!skip_parallel=true`）
- 串行执行：启用（`!skip_sequential=true`）
- 并发级别：4

### 6.2 execute_blockstm_benchmark核心逻辑

**源码位置：** `src/simulator.rs:272-307`

```rust
pub fn execute_blockstm_benchmark(
    &mut self,
    transactions: Vec<SignatureVerifiedTransaction>,
    run_par: bool,
    run_seq: bool,
    concurrency_level_per_shard: usize,
    maybe_block_gas_limit: Option<u64>,
) -> (usize, usize) {
    let (output, par_tps) = if run_par {
        if concurrency_level_per_shard == 1 {
            // For single core, use sequential execution path
            println!("Parallel execution starts...");
            let (output, tps) =
                self.execute_benchmark_sequential(&transactions, maybe_block_gas_limit);
            println!("Parallel execution finishes, TPS = {}", tps);
            (output, tps)
        } else {
            // For multi-core, use parallel execution path
            println!("Parallel execution starts...");
            let (output, tps) =
                self.execute_benchmark_parallel(
                    &transactions, 
                    concurrency_level_per_shard,
                    maybe_block_gas_limit
                );
            println!("Parallel execution finishes, TPS = {}", tps);
            (output, tps)
        }
    }else{
        (vec![],0)
    };
    // ... 验证交易输出状态
    // ... 串行执行部分
    (par_tps, seq_tps)
}
```

**执行状态：**
- 由于`concurrency_level_per_shard=4 > 1`，选择并行执行路径
- 调用`execute_benchmark_parallel`方法

### 6.3 并行执行核心实现

**源码位置：** `src/simulator.rs:208-270`

```rust
fn execute_benchmark_parallel(
    &self,
    transactions: &[SignatureVerifiedTransaction],
    concurrency_level_per_shard: usize,
    maybe_block_gas_limit: Option<u64>,
) -> (Vec<TransactionOutput>, usize) {
    use aptos_block_executor::counters;
    use aptos_types::block_executor::config::{
        BlockExecutorConfig, BlockExecutorLocalConfig, BlockExecutorModuleCacheLocalConfig
    };
    
    let block_size = transactions.len();
    
    // Reset counters before execution
    let execution_before = counters::TASK_EXECUTE_SECONDS.get_sample_count();
    let validation_before = counters::TASK_VALIDATE_SECONDS.get_sample_count();
    let abort_before = counters::SPECULATIVE_ABORT_COUNT.get();
    let suspend_before = counters::DEPENDENCY_WAIT_SECONDS.get_sample_count();
    let _suspend_time_before = counters::DEPENDENCY_WAIT_SECONDS.get_sample_sum();
    
    let timer = Instant::now();
    let txn_provider = DefaultTxnProvider::new_without_info(transactions.to_vec());
    let block_executor = AptosVMBlockExecutor::new();
    
    let config = BlockExecutorConfig {
        local: BlockExecutorLocalConfig {
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
    let suspend = counters::DEPENDENCY_WAIT_SECONDS.get_sample_count() - suspend_before;
    let suspend_time_total = counters::DEPENDENCY_WAIT_SECONDS.get_sample_sum() - _suspend_time_before;
    let avg_suspend_time = if suspend > 0 {
        suspend_time_total / suspend as f64
    } else {
        0.0
    };
    
    println!("execution_total:{}, validation_total:{}, abort:{}, suspend:{}, avg_suspend_time:{:.2} us, suspend_time_total:{:.2} us", 
        execution_total,
        validation_total,
        abort,
        suspend,
        avg_suspend_time * 1000000.0,
        suspend_time_total * 1000000.0
    );
    (output, block_size * 1000 / exec_time as usize)
}
```

**执行状态：**
- 重置性能计数器
- 创建交易提供者（DefaultTxnProvider）
- 创建Block-STM执行器（AptosVMBlockExecutor）
- 配置执行参数：
  - 并发级别：4
  - 允许回退：true
  - 丢弃失败块：false
- 调用`execute_block_with_config`进入Block-STM核心执行逻辑

**日志记录点：**
- 执行开始时间戳
- 性能计数器重置
- 执行结束后的统计信息

## 第七阶段：Block-STM核心执行引擎

### 7.1 BlockExecutor入口

**源码位置：** `block-executor/src/executor.rs:100-150`

```rust
pub struct BlockExecutor<T, E, S, L, TP> {
    config: BlockExecutorConfig,
    executor_thread_pool: Arc<ThreadPool>,
    maybe_block_limit_processor: Option<BlockGasLimitProcessor<'static, T, S>>,
    transaction_commit_hook: Option<L>,
    phantom: PhantomData<(T, E, S, TP)>,
}
```

**执行状态：**
- 初始化线程池（4个工作线程）
- 设置Block Gas限制处理器
- 配置交易提交钩子

### 7.2 execute_block_with_config核心流程

**源码位置：** `block-executor/src/executor.rs:200-300`（简化版本）

```rust
pub fn execute_block_with_config<S: TStateView<Key = T::Key> + Sync>(
    &self,
    txn_provider: &TP,
    state_view: &S,
    config: BlockExecutorConfig,
    transaction_slice_metadata: TransactionSliceMetadata,
) -> Result<BlockOutput<T::Output>, BlockExecutionError> {
    // 1. 初始化多版本数据结构
    let versioned_cache = MVHashMap::new();
    
    // 2. 初始化调度器V2
    let scheduler = SchedulerV2::new(txn_provider.num_txns());
    
    // 3. 初始化全局模块缓存
    let global_module_cache = GlobalModuleCache::empty();
    
    // 4. 创建共享同步参数
    let shared_params = SharedSyncParams {
        base_view: state_view,
        scheduler: &scheduler,
        versioned_cache: &versioned_cache,
        global_module_cache: &global_module_cache,
        // ... 其他参数
    };
    
    // 5. 启动并行执行
    self.execute_transactions_parallel(
        txn_provider,
        &shared_params,
        config.local.concurrency_level,
    )?;
    
    // 6. 收集执行结果
    let final_results = shared_params.final_results.into_inner();
    
    Ok(BlockOutput::new(final_results))
}
```

**执行状态：**
- 创建多版本哈希映射（MVHashMap）
- 初始化调度器V2（SchedulerV2）
- 设置全局模块缓存
- 启动4个并行工作线程

**日志记录点：**
```rust
// 在Block-STM执行器中的关键日志点
log_transaction_start!(txn_idx, incarnation);
log_transaction_finish!(txn_idx, incarnation, "Success", duration, gas_used, read_set_size, write_set_size);
log_transaction_abort!(txn_idx, incarnation, "Validation failed", retry_count, dependencies);
```

### 7.3 并行执行工作线程

**源码位置：** `block-executor/src/executor.rs:400-500`（简化版本）

```rust
fn execute_transactions_parallel<TP: TxnProvider<Txn = T>>(
    &self,
    txn_provider: &TP,
    shared_params: &SharedSyncParams<T, E, S>,
    concurrency_level: usize,
) -> Result<(), BlockExecutionError> {
    let executor_thread_pool = &self.executor_thread_pool;
    
    executor_thread_pool.scope(|s| {
        for _ in 0..concurrency_level {
            s.spawn(|_| {
                self.worker_loop(
                    txn_provider,
                    shared_params,
                );
            });
        }
    });
    
    Ok(())
}
```

**执行状态：**
- 启动4个工作线程
- 每个线程执行worker_loop
- 使用Rayon线程池进行并行调度

### 7.4 工作线程循环

**源码位置：** `block-executor/src/executor.rs:600-800`（简化版本）

```rust
fn worker_loop<TP: TxnProvider<Txn = T>>(
    &self,
    txn_provider: &TP,
    shared_params: &SharedSyncParams<T, E, S>,
) {
    loop {
        // 1. 从调度器获取任务
        let task = shared_params.scheduler.next_task();
        
        match task {
            SchedulerTask::ExecutionTask(txn_idx, incarnation) => {
                // 2. 执行交易
                self.execute_transaction(
                    txn_idx,
                    incarnation,
                    txn_provider,
                    shared_params,
                );
            },
            SchedulerTask::ValidationTask(txn_idx) => {
                // 3. 验证交易
                self.validate_transaction(
                    txn_idx,
                    txn_provider,
                    shared_params,
                );
            },
            SchedulerTask::Done => {
                // 4. 所有任务完成，退出循环
                break;
            },
        }
    }
}
```

**执行状态：**
- 工作线程从调度器获取任务
- 处理执行任务和验证任务
- 循环直到所有任务完成

**日志记录点：**
- 任务获取时间戳
- 任务类型和交易索引
- 任务执行开始和结束时间

## 第八阶段：交易执行与验证

### 8.1 交易执行阶段

**源码位置：** `block-executor/src/executor.rs:900-1100`（简化版本）

```rust
fn execute_transaction<TP: TxnProvider<Txn = T>>(
    &self,
    txn_idx: TxnIndex,
    incarnation: Incarnation,
    txn_provider: &TP,
    shared_params: &SharedSyncParams<T, E, S>,
) {
    // 1. 记录执行开始
    log_transaction_start!(txn_idx, incarnation);
    let start_time = Instant::now();
    
    // 2. 创建多版本视图
    let latest_view = LatestView::new(
        shared_params.base_view,
        shared_params.versioned_cache,
        txn_idx,
    );
    
    // 3. 获取交易
    let txn = txn_provider.get_transaction(txn_idx);
    
    // 4. 执行交易
    let execution_result = self.execute_transaction_impl(
        &txn,
        &latest_view,
        shared_params,
    );
    
    match execution_result {
        Ok((output, read_set, write_set)) => {
            // 5. 记录读写集
            shared_params.versioned_cache.record(
                txn_idx,
                incarnation,
                read_set,
                write_set,
            );
            
            // 6. 更新调度器状态
            shared_params.scheduler.finish_execution(
                txn_idx,
                incarnation,
                output,
            );
            
            // 7. 记录执行完成
            let duration = start_time.elapsed();
            log_transaction_finish!(
                txn_idx, 
                incarnation, 
                "Success", 
                duration, 
                output.gas_used(), 
                read_set.len(), 
                write_set.len()
            );
        },
        Err(dependency) => {
            // 8. 处理依赖等待
            shared_params.scheduler.wait_for_dependency(
                txn_idx,
                dependency,
            );
            
            log_dependency_stall!(txn_idx, vec![dependency]);
        }
    }
}
```

**执行状态：**
- 为每个交易创建多版本视图
- 乐观执行交易逻辑
- 记录读写集到多版本存储
- 处理数据依赖和冲突

**日志记录点：**
- 交易执行开始：`log_transaction_start!`
- 交易执行完成：`log_transaction_finish!`
- 依赖等待：`log_dependency_stall!`
- 读写集变化：`log_readwrite_set_change!`

### 8.2 交易验证阶段

**源码位置：** `block-executor/src/executor.rs:1200-1400`（简化版本）

```rust
fn validate_transaction<TP: TxnProvider<Txn = T>>(
    &self,
    txn_idx: TxnIndex,
    txn_provider: &TP,
    shared_params: &SharedSyncParams<T, E, S>,
) {
    let start_time = Instant::now();
    
    // 1. 获取已记录的读集
    let recorded_reads = shared_params.versioned_cache
        .get_reads(txn_idx);
    
    // 2. 验证读集一致性
    let validation_result = self.validate_reads(
        txn_idx,
        &recorded_reads,
        shared_params,
    );
    
    let duration = start_time.elapsed();
    
    match validation_result {
        Ok(()) => {
            // 3. 验证成功，提交交易
            shared_params.scheduler.finish_validation(
                txn_idx,
                true,
            );
            
            log_validation!(txn_idx, true, duration);
        },
        Err(conflict_txn) => {
            // 4. 验证失败，中止交易
            shared_params.scheduler.abort_transaction(
                txn_idx,
                conflict_txn,
            );
            
            log_transaction_abort!(
                txn_idx, 
                incarnation, 
                "Validation failed", 
                retry_count, 
                vec![conflict_txn]
            );
            
            log_validation!(txn_idx, false, duration);
        }
    }
}
```

**执行状态：**
- 验证交易读集的一致性
- 检测读写冲突
- 决定提交或中止交易
- 触发依赖交易的重新执行

**日志记录点：**
- 验证开始和结果：`log_validation!`
- 交易中止：`log_transaction_abort!`
- 读写冲突：`log_readwrite_conflict!`

## 第九阶段：调度器V2核心逻辑

### 9.1 SchedulerV2架构

**源码位置：** `block-executor/src/scheduler_v2.rs:50-100`

```rust
pub struct SchedulerV2 {
    num_txns: usize,
    execution_idx: AtomicU32,
    validation_idx: AtomicU32,
    abort_manager: AbortManager,
    task_queue: Mutex<VecDeque<SchedulerTask>>,
    finished_marker: AtomicBool,
}

impl SchedulerV2 {
    pub fn new(num_txns: usize) -> Self {
        Self {
            num_txns,
            execution_idx: AtomicU32::new(0),
            validation_idx: AtomicU32::new(0),
            abort_manager: AbortManager::new(num_txns),
            task_queue: Mutex::new(VecDeque::new()),
            finished_marker: AtomicBool::new(false),
        }
    }
}
```

**执行状态：**
- 管理100,000个交易的调度
- 维护执行和验证索引
- 处理交易中止和重试
- 协调4个工作线程

### 9.2 任务调度逻辑

**源码位置：** `block-executor/src/scheduler_v2.rs:200-300`

```rust
pub fn next_task(&self) -> SchedulerTask {
    loop {
        // 1. 检查是否有待处理的中止任务
        if let Some(abort_task) = self.abort_manager.next_abort_task() {
            return abort_task;
        }
        
        // 2. 尝试获取执行任务
        let execution_idx = self.execution_idx.load(Ordering::Acquire);
        if execution_idx < self.num_txns {
            if self.execution_idx.compare_exchange_weak(
                execution_idx,
                execution_idx + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                return SchedulerTask::ExecutionTask(
                    execution_idx as TxnIndex,
                    0, // incarnation
                );
            }
        }
        
        // 3. 尝试获取验证任务
        let validation_idx = self.validation_idx.load(Ordering::Acquire);
        if validation_idx < execution_idx {
            if self.validation_idx.compare_exchange_weak(
                validation_idx,
                validation_idx + 1,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ).is_ok() {
                return SchedulerTask::ValidationTask(
                    validation_idx as TxnIndex,
                );
            }
        }
        
        // 4. 检查是否所有任务完成
        if self.all_tasks_finished() {
            return SchedulerTask::Done;
        }
        
        // 5. 短暂等待后重试
        std::thread::yield_now();
    }
}
```

**执行状态：**
- 优先处理中止任务
- 按顺序分配执行任务
- 按顺序分配验证任务
- 确保执行-验证的正确顺序

**日志记录点：**
- 任务分配：记录任务类型和交易索引
- 调度器状态变化
- 工作线程负载均衡

## 第十阶段：多版本存储管理

### 10.1 MVHashMap核心结构

**源码位置：** `mvhashmap/src/lib.rs:100-200`

```rust
pub struct MVHashMap<K, T, V, D> {
    data: DashMap<K, VersionedValue<T, V>>,
    delayed_fields: DashMap<D, VersionedDelayedField<T>>,
    phantom: PhantomData<(K, T, V, D)>,
}

pub struct VersionedValue<T, V> {
    versioned_map: BTreeMap<T, ValueWithLayout<V>>,
    read_set: RwLock<BTreeSet<T>>,
}
```

**执行状态：**
- 为每个存储键维护多个版本
- 支持并发读写操作
- 管理读集和写集
- 处理延迟字段更新

### 10.2 读写操作处理

**源码位置：** `mvhashmap/src/lib.rs:300-500`

```rust
pub fn read(&self, key: &K, txn_idx: T) -> ReadResult<V> {
    match self.data.get(key) {
        Some(versioned_value) => {
            // 1. 查找小于等于txn_idx的最大版本
            let version_map = &versioned_value.versioned_map;
            
            match version_map.range(..=txn_idx).next_back() {
                Some((version, value)) => {
                    // 2. 记录读依赖
                    versioned_value.read_set
                        .write()
                        .unwrap()
                        .insert(txn_idx);
                    
                    // 3. 返回读取的值
                    ReadResult::Value(value.clone())
                },
                None => {
                    // 4. 未找到版本，从基础存储读取
                    ReadResult::Unresolved(Dependency::new(key.clone()))
                }
            }
        },
        None => {
            // 5. 键不存在，从基础存储读取
            ReadResult::Unresolved(Dependency::new(key.clone()))
        }
    }
}

pub fn write(&self, key: K, txn_idx: T, value: V) {
    // 1. 获取或创建版本化值
    let versioned_value = self.data
        .entry(key)
        .or_insert_with(|| VersionedValue::new());
    
    // 2. 插入新版本
    versioned_value.versioned_map
        .insert(txn_idx, ValueWithLayout::new(value));
    
    // 3. 记录写操作日志
    log_readwrite_set_change!(
        txn_idx,
        incarnation,
        vec![], // read_keys
        vec![format!("{:?}", key)], // write_keys
        0, 1, 0, 0, 0, 0 // 各种计数
    );
}
```

**执行状态：**
- 实现多版本并发控制
- 支持乐观读取
- 记录读写依赖关系
- 处理版本冲突

**日志记录点：**
- 读写操作时间戳
- 版本号和交易索引
- 读写集大小统计
- 冲突检测结果

## 第十一阶段：性能统计与结果输出

### 11.1 性能计数器收集

**源码位置：** `src/simulator.rs:250-270`

```rust
// Calculate deltas for this execution
let execution_total = counters::TASK_EXECUTE_SECONDS.get_sample_count() - execution_before;
let validation_total = counters::TASK_VALIDATE_SECONDS.get_sample_count() - validation_before;
let abort = counters::SPECULATIVE_ABORT_COUNT.get() - abort_before;
let suspend = counters::DEPENDENCY_WAIT_SECONDS.get_sample_count() - suspend_before;
let suspend_time_total = counters::DEPENDENCY_WAIT_SECONDS.get_sample_sum() - _suspend_time_before;
let avg_suspend_time = if suspend > 0 {
    suspend_time_total / suspend as f64
} else {
    0.0
};

println!("execution_total:{}, validation_total:{}, abort:{}, suspend:{}, avg_suspend_time:{:.2} us, suspend_time_total:{:.2} us", 
    execution_total,
    validation_total,
    abort,
    suspend,
    avg_suspend_time * 1000000.0,
    suspend_time_total * 1000000.0
);
```

**统计指标：**
- `execution_total`: 总执行任务数
- `validation_total`: 总验证任务数
- `abort`: 投机中止次数
- `suspend`: 依赖等待次数
- `avg_suspend_time`: 平均等待时间
- `suspend_time_total`: 总等待时间

### 11.2 TPS计算

**源码位置：** `src/simulator.rs:270`

```rust
(output, block_size * 1000 / exec_time as usize)
```

**计算公式：**
```
TPS = (交易数量 × 1000) / 执行时间(毫秒)
```

**执行状态：**
- 记录执行开始和结束时间
- 计算总执行时间（毫秒）
- 基于100,000笔交易计算TPS

### 11.3 结果验证

**源码位置：** `src/simulator.rs:295-306`

```rust
output.iter().for_each(|txn_output| {
    assert_eq!(
        txn_output.status(),
        &TransactionStatus::Keep(ExecutionStatus::Success)
    );
});
```

**验证内容：**
- 所有交易执行成功
- 无失败或中止的交易
- 输出状态一致性检查

## 第十二阶段：日志输出与清理

### 12.1 日志文件生成

**日志文件结构：**
```
./test_logs_new/
├── execution_<timestamp>.jsonl     # 交易执行日志
├── concurrency_<timestamp>.jsonl   # 并发控制日志
├── readwrite_<timestamp>.jsonl     # 读写集日志
├── performance_<timestamp>.jsonl   # 性能指标日志
└── summary_<timestamp>.jsonl       # 汇总统计日志
```

### 12.2 日志内容示例

**execution_*.jsonl:**
```json
{"event_type":"TransactionStart","transaction_id":0,"incarnation":0,"thread_id":123,"timestamp":1640995200000}
{"event_type":"TransactionFinish","transaction_id":0,"incarnation":0,"thread_id":123,"timestamp":1640995200100,"execution_result":"Success","duration_us":100000,"gas_used":100,"read_set_size":2,"write_set_size":1}
```

**concurrency_*.jsonl:**
```json
{"event_type":"TransactionAbort","transaction_id":42,"incarnation":1,"thread_id":124,"timestamp":1640995200200,"abort_reason":"Validation failed","retry_count":1,"dependencies":[41]}
{"event_type":"DependencyStall","transaction_id":43,"thread_id":125,"timestamp":1640995200300,"stalled_by":[42]}
```

### 12.3 资源清理

**源码位置：** `block-executor/src/block_stm_logger.rs:460-464`

```rust
impl Drop for BlockSTMLogger {
    fn drop(&mut self) {
        self.flush();
    }
}
```

**清理操作：**
- 刷新所有缓冲区
- 关闭日志文件
- 释放内存资源
- 清理临时数据

## 总结

### 完整调用链概览

```
main()
├── Args::parse()                           # 命令行解析
├── replay_erc20_historic()
│   ├── Simulator::with_account_nums()      # 初始化93000个账户
│   └── Simulator::replay_erc20_historic()
│       ├── init_global_logger()            # 初始化Block-STM日志系统
│       ├── CSV数据读取                      # 加载100,000笔交易
│       ├── gen_transaction_for_erc20()     # 生成签名验证交易
│       └── execute_blockstm_benchmark()
│           └── execute_benchmark_parallel()
│               ├── DefaultTxnProvider::new() # 创建交易提供者
│               ├── AptosVMBlockExecutor::new() # 创建Block-STM执行器
│               └── execute_block_with_config()
│                   ├── MVHashMap::new()    # 多版本存储初始化
│                   ├── SchedulerV2::new()  # 调度器V2初始化
│                   ├── 启动4个工作线程
│                   │   ├── worker_loop()   # 工作线程循环
│                   │   │   ├── next_task() # 获取任务
│                   │   │   ├── execute_transaction() # 执行交易
│                   │   │   └── validate_transaction() # 验证交易
│                   │   └── 日志记录点
│                   └── 收集执行结果
└── 性能统计输出
```

### 关键执行阶段总结

1. **初始化阶段**：命令解析、账户创建、日志系统初始化
2. **数据加载阶段**：CSV读取、交易生成、签名验证
3. **并行执行阶段**：多线程调度、乐观执行、冲突检测
4. **验证提交阶段**：读集验证、状态提交、结果收集
5. **统计输出阶段**：性能计算、日志输出、资源清理

### 日志系统集成点

Block-STM日志系统在以下关键点进行数据收集：

- **交易生命周期**：开始、执行、验证、提交/中止
- **并发控制**：依赖等待、冲突检测、重试机制
- **读写操作**：多版本读取、写入记录、集合变化
- **性能指标**：执行时间、TPS计算、资源使用
- **调度决策**：任务分配、线程负载、完成状态

通过这个详细的阶段分析，我们可以清楚地看到Block-STM如何通过乐观并发控制、多版本存储和智能调度来实现高性能的并行交易执行。