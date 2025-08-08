// Copyright © Aptos Foundation
// Parts of the project are originally copyright © Meta Platforms, Inc.
// SPDX-License-Identifier: Apache-2.0

use aptos_crypto::_once_cell::sync::Lazy;
use aptos_language_e2e_tests::account_universe::P2PTransferGen;
use aptos_metrics_core::{register_int_gauge, IntGauge};
use aptos_push_metrics::MetricsPusher;
use aptos_transaction_benchmarks::transactions::TransactionBencher;
use aptos_transaction_benchmarks::simulator::Simulator;

use aptos_vm_logging::disable_speculative_logging;
use clap::{Parser, Subcommand};
use proptest::prelude::*;
use std::{
    error::Error, fs, io::Write, net::SocketAddr, path::Path, time::{SystemTime, UNIX_EPOCH}
};

/// This is needed for filters on the Grafana dashboard working as its used to populate the filter
/// variables.
pub static START_TIME: Lazy<IntGauge> =
    Lazy::new(|| register_int_gauge!("node_process_start_time", "Start time").unwrap());

#[derive(Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    command: BenchmarkCommand,
}

#[derive(Subcommand, Debug)]
enum BenchmarkCommand {
    ParamSweep(ParamSweepOpt),
    Execute(ExecuteOpt),
    ReplayERC20(ReplayERC20HistoricOpt),
    ReplayERC20Full(ReplayERC20FullOpt),
    Airdrop(CommonOpt),
    Ballot(CommonOpt),
    BallotSharding(CommonShardingOpt),
    Kitty(CommonOpt),
    MillionPixel(CommonOpt),
    Empty(CommonOpt),
}

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

#[derive(Debug, Parser)]
struct ReplayERC20FullOpt{
    #[clap(long, default_value_t = 0)]
    pub num_warmups: usize,

    #[clap(long, default_value_t = 1)]
    pub num_runs: usize,

    #[clap(long, default_value="data/ETH_2401_100000.csv")]
    pub data_path: String,

    #[clap(long, default_value_t=93000)]
    pub num_accounts: usize,

    #[clap(long)]
    pub output_file: Option<String>,

    #[clap(long)]
    pub concurrency_level: Option<usize>,
}

#[derive(Debug, Parser)]
struct CommonOpt{
    #[clap(long, default_value_t = 0)]
    pub num_warmups: usize,
    
    #[clap(long, default_value_t = 1)]
    pub num_runs: usize,

    #[clap(long,default_value_t = 1000)]
    pub num_accounts: usize,

    #[clap(long,default_value_t = 100000)]
    pub num_transactions: usize,
}

#[derive(Debug, Parser)]
struct CommonShardingOpt{
    #[clap(long, default_value_t = 0)]
    pub num_warmups: usize,
    
    #[clap(long, default_value_t = 1)]
    pub num_runs: usize,

    #[clap(long,default_value_t = 1000)]
    pub num_accounts: usize,

    #[clap(long,default_value_t = 100000)]
    pub num_transactions: usize,

    #[clap(long,default_value_t = 1)]
    pub num_shardings: usize,
}


#[derive(Debug, Parser)]
struct ParamSweepOpt {
    #[clap(long, default_value = "1000")]
    pub num_accounts: Vec<usize>,

    #[clap(long)]
    pub block_sizes: Option<Vec<usize>>,

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
}

#[derive(Debug, Parser)]
struct ExecuteOpt {
    #[clap(long, default_value_t = 1000)]
    pub num_accounts: usize,

    #[clap(long, default_value_t = 0)]
    pub num_warmups: usize,

    #[clap(long, default_value_t = 100000)]
    pub block_size: usize,

    #[clap(long, default_value_t = 15)]
    pub num_blocks: usize,

    #[clap(long, default_value_t = 8)]
    pub concurrency_level_per_shard: usize,

    #[clap(long, default_value_t = 1)]
    pub num_executor_shards: usize,

    #[clap(long, num_args = 1.., conflicts_with = "num_executor_shards")]
    pub remote_executor_addresses: Option<Vec<SocketAddr>>,

    #[clap(long, default_value_t = true)]
    pub no_conflict_txns: bool,

    #[clap(long)]
    pub maybe_block_gas_limit: Option<u64>,

    #[clap(long, default_value_t = false)]
    pub generate_then_execute: bool,
}

fn replay_erc20_historic(opt: ReplayERC20HistoricOpt) -> Result<(), Box<dyn Error>> {
    // 如果指定了输出文件，确保目录存在
    if let Some(ref output_path) = opt.output_file {
        if let Some(parent_dir) = Path::new(output_path).parent() {
            fs::create_dir_all(parent_dir)?;
        }
    }

    // 检查是否启用日志功能
    let log_enabled = std::env::var("BLOCK_STM_LOG_LEVEL").is_ok();
    let log_output_dir = std::env::var("BLOCK_STM_LOG_DIR").ok();
    
    // 使用新的支持日志功能的构造函数
    let mut simulator = if log_enabled {
        println!("Creating Simulator with logging enabled, output dir: {:?}", log_output_dir);
        Simulator::new_with_logging(
            opt.num_accounts,
            true,
            log_output_dir,
        )
    } else {
        println!("Creating Simulator without logging");
        Simulator::with_account_nums(opt.num_accounts)
    };
    
    let concurrency_level = opt.concurrency_level.unwrap_or_else(|| num_cpus::get());
    let result = simulator.replay_erc20_historic(
        opt.data_path,
        !opt.skip_parallel,
        !opt.skip_sequential,
        opt.num_warmups,
        opt.num_runs,
        opt.maybe_block_gas_limit,
        concurrency_level,
    );
    
    match result {
        Ok(_) => println!("ERC20 historic replay completed successfully"),
        Err(e) => println!("Error during ERC20 historic replay: {}", e),
    }

    // 如果指定了输出文件，将结果写入文件
    if let Some(output_path) = opt.output_file {
        let mut file = fs::File::create(&output_path)?;
        writeln!(file, "Replay ERC20 Historic benchmark completed successfully")?;
        println!("Results written to: {}", output_path);
    }

    Ok(())
}

fn replay_erc20_full(opt: ReplayERC20FullOpt) -> Result<(), Box<dyn Error>> {
    // 如果指定了输出文件，确保目录存在
    if let Some(ref output_path) = opt.output_file {
        if let Some(parent_dir) = Path::new(output_path).parent() {
            fs::create_dir_all(parent_dir)?;
        }
    }

    // 强制启用日志功能
    std::env::set_var("BLOCK_STM_LOG_LEVEL", "DEBUG");
    if std::env::var("BLOCK_STM_LOG_DIR").is_err() {
        std::env::set_var("BLOCK_STM_LOG_DIR", "./test_logs_stage4_full");
    }
    
    let log_output_dir = std::env::var("BLOCK_STM_LOG_DIR").ok();
    
    println!("Creating Simulator with full logging enabled, output dir: {:?}", log_output_dir);
    let mut simulator = Simulator::new_with_logging(
        opt.num_accounts,
        true,
        log_output_dir,
    );
    
    let concurrency_level = opt.concurrency_level.unwrap_or_else(|| num_cpus::get());
    
    println!("Starting full CSV replay with detailed logging...");
    let metrics_results = simulator.replay_with_full_logging(
        &opt.data_path,
        concurrency_level,
        opt.num_warmups,
        opt.num_runs,
    )?;
    
    // 输出详细的执行指标
    println!("\n=== Full CSV Replay Results ===");
    for (i, metrics) in metrics_results.iter().enumerate() {
        println!("Run {}: TPS={}, Execution={}ms, Aborts={}, Suspends={}, Avg Suspend Time={:.2}us",
            i + 1,
            metrics.tps,
            metrics.execution_time_ms,
            metrics.abort_count,
            metrics.suspend_count,
            metrics.avg_suspend_time_us
        );
    }
    
    let avg_tps = metrics_results.iter().map(|m| m.tps).sum::<usize>() / metrics_results.len();
    println!("Average TPS: {}", avg_tps);
    
    // 如果指定了输出文件，将结果写入文件
    if let Some(output_path) = opt.output_file {
        let mut file = fs::File::create(&output_path)?;
        writeln!(file, "Full CSV Replay benchmark completed successfully")?;
        writeln!(file, "Average TPS: {}", avg_tps)?;
        for (i, metrics) in metrics_results.iter().enumerate() {
            writeln!(file, "Run {}: TPS={}, Execution={}ms, Aborts={}, Suspends={}, Avg Suspend Time={:.2}us",
                i + 1,
                metrics.tps,
                metrics.execution_time_ms,
                metrics.abort_count,
                metrics.suspend_count,
                metrics.avg_suspend_time_us
            )?;
        }
        println!("Results written to: {}", output_path);
    }

    Ok(())
}

fn param_sweep(opt: ParamSweepOpt) {
    disable_speculative_logging();

    let block_sizes = opt.block_sizes.unwrap_or_else(|| vec![1000, 10000, 50000]);
    let concurrency_level = num_cpus::get();

    let bencher = TransactionBencher::new(any_with::<P2PTransferGen>((1_000, 1_000_000)));

    let mut par_measurements: Vec<Vec<usize>> = Vec::new();
    let mut seq_measurements: Vec<Vec<usize>> = Vec::new();

    let run_parallel = !opt.skip_parallel;
    let run_sequential = !opt.skip_sequential;

    let maybe_block_gas_limit = opt.maybe_block_gas_limit;

    assert!(
        run_sequential || run_parallel,
        "Must run at least one of parallel or sequential"
    );

    for block_size in &block_sizes {
        for num_accounts in &opt.num_accounts {
            let (mut par_tps, mut seq_tps) = bencher.blockstm_benchmark(
                *num_accounts,
                *block_size,
                run_parallel,
                run_sequential,
                opt.num_warmups,
                opt.num_runs,
                1,
                concurrency_level,
                None,
                false,
                maybe_block_gas_limit,
                false,
            );
            par_tps.sort();
            seq_tps.sort();
            par_measurements.push(par_tps);
            seq_measurements.push(seq_tps);
        }
    }

    println!("\nconcurrency_level = {}\n", concurrency_level);

    let mut i = 0;
    for block_size in &block_sizes {
        for num_accounts in &opt.num_accounts {
            println!(
                "PARAMS: num_account = {}, block_size = {}",
                *num_accounts, *block_size
            );

            let mut seq_tps = 1;
            if run_sequential {
                println!("Sequential TPS: {:?}", seq_measurements[i]);
                let mut seq_sum = 0;
                for m in &seq_measurements[i] {
                    seq_sum += m;
                }
                seq_tps = seq_sum / seq_measurements[i].len();
                println!("Avg Sequential TPS = {:?}", seq_tps,);
            }

            if run_parallel {
                println!("Parallel TPS: {:?}", par_measurements[i]);
                let mut par_sum = 0;
                for m in &par_measurements[i] {
                    par_sum += m;
                }
                let par_tps = par_sum / par_measurements[i].len();
                println!("Avg Parallel TPS = {:?}", par_tps,);
                if run_sequential {
                    println!("Speed up {}x over sequential", par_tps / seq_tps);
                }
            }
            i += 1;
        }
        println!();
    }
}

fn execute(opt: ExecuteOpt) {
    disable_speculative_logging();
    let bencher = TransactionBencher::new(any_with::<P2PTransferGen>((1_000, 1_000_000)));

    let (par_tps, _) = bencher.blockstm_benchmark(
        opt.num_accounts,
        opt.block_size,
        true,
        false,
        opt.num_warmups,
        opt.num_blocks,
        opt.num_executor_shards,
        opt.concurrency_level_per_shard,
        opt.remote_executor_addresses,
        opt.no_conflict_txns,
        opt.maybe_block_gas_limit,
        opt.generate_then_execute,
    );

    let sum: usize = par_tps.iter().sum();
    println!("Avg Parallel TPS = {:?}", sum / par_tps.len())
}

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

    // TODO: Check if I need DisplayChain here in the error case.
    match args.command {
        BenchmarkCommand::ParamSweep(opt) => param_sweep(opt),
        BenchmarkCommand::Execute(opt) => execute(opt),
        BenchmarkCommand::ReplayERC20(opt) => {
            if let Err(e) = replay_erc20_historic(opt) {
                eprintln!("Error in replay_erc20_historic: {}", e);
            }
        },
        BenchmarkCommand::ReplayERC20Full(opt) => {
            if let Err(e) = replay_erc20_full(opt) {
                eprintln!("Error in replay_erc20_full: {}", e);
            }
        },
        BenchmarkCommand::Airdrop(opt) => {
            let mut simulator = Simulator::with_account_nums(opt.num_accounts);
            simulator.run_airdrop(opt.num_transactions);
        },
        BenchmarkCommand::Ballot(opt) => {
            let mut simulator = Simulator::with_account_nums(opt.num_accounts);
            simulator.run_ballot(opt.num_transactions);
        },
        BenchmarkCommand::BallotSharding(opt) => {
            let mut simulator = Simulator::with_account_nums(opt.num_accounts);
            simulator.run_ballot_sharding(opt.num_transactions, opt.num_shardings);
        },
        BenchmarkCommand::Kitty(opt) => {
            let mut simulator = Simulator::with_account_nums(opt.num_accounts);
            simulator.run_kitty(opt.num_transactions);
        },
        BenchmarkCommand::MillionPixel(opt) => {
            let mut simulator = Simulator::with_account_nums(opt.num_accounts);
            simulator.run_mp(opt.num_transactions);
        },
        BenchmarkCommand::Empty(opt) => {
            let mut simulator = Simulator::with_account_nums(opt.num_accounts);
            simulator.run_empty(opt.num_transactions);
        },
    }
}

#[test]
fn verify_tool() {
    use clap::CommandFactory;
    Args::command().debug_assert()
}
