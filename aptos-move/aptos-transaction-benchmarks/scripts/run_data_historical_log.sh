#!/bin/bash

# 获取数据目录路径
data_dir="./data_easy"
result_dir="./result"
log_base_dir="./batch_logs"

# 如果结果目录不存在则创建
mkdir -p $result_dir

# 如果基础日志目录不存在则创建
mkdir -p $log_base_dir

# 获取CPU核心数信息
count=$(./scripts/cpu_info.sh | awk '{print NF}')

# 设置日志和运行次数的默认值
BLOCK_STM_LOG_LEVEL=${BLOCK_STM_LOG_LEVEL:-"DEBUG"}
NUM_RUNS=${NUM_RUNS:-1}

echo "Starting historical data replay tests with logging for all CSV files..."
echo "Found $(ls $data_dir/*.csv | wc -l) CSV files to process"
echo "Log level: $BLOCK_STM_LOG_LEVEL"
echo "Number of runs per test: $NUM_RUNS"
echo "Base log directory: $log_base_dir"
echo ""

# 遍历数据目录中的所有CSV文件
for csv_file in $data_dir/*.csv; do
    # 提取不含路径和扩展名的文件名
    basename_file=$(basename "$csv_file" .csv)
    
    # 创建日志文件名：historical + csv文件名 + .log
    log_filename="$result_dir/historical_${basename_file}.log"
    
    # 为此CSV文件创建专用日志目录
    csv_log_dir="$log_base_dir/${basename_file}"
    mkdir -p $csv_log_dir
    
    echo "Processing: $csv_file"
    echo "Output log: $log_filename"
    echo "Block-STM logs directory: $csv_log_dir"
    echo ""
    
    # 初始化日志文件
    echo "run historical transfer tasks for $basename_file with logging" > $log_filename
    echo "Data file: $csv_file" >> $log_filename
    echo "Log level: $BLOCK_STM_LOG_LEVEL" >> $log_filename
    echo "Number of runs: $NUM_RUNS" >> $log_filename
    echo "Block-STM logs directory: $csv_log_dir" >> $log_filename
    echo "" >> $log_filename
    
    # 使用不同的CPU核心配置运行测试
    for ((i=1; i<=$count; i++)); do
        cores=$(./scripts/cpu_info.sh|cut -d ' ' -f$i)
        # 通过计算逗号数量加1来统计核心数
        if [[ "$cores" == "0" ]]; then
            core_count=1
        else
            core_count=$(echo $cores | tr ',' '\n' | wc -l | tr -d ' ')
        fi
        
        # 跳过核心数为0的情况
        if [ $core_count -eq 0 ]; then
            continue
        fi
        
        # 为此核心配置创建专用日志子目录
        core_log_dir="$csv_log_dir/cores_${core_count}"
        mkdir -p $core_log_dir
        
        echo "============================== running with cpu cores: (${cores}) count: ${core_count} ==============================" >> $log_filename
        echo "Block-STM logs for this run: $core_log_dir" >> $log_filename
        
        # 使用日志环境变量运行测试
        BLOCK_STM_LOG_LEVEL=$BLOCK_STM_LOG_LEVEL BLOCK_STM_LOG_DIR=$core_log_dir cargo run --release -- replay-erc20 --data-path "$csv_file" --concurrency-level $core_count --num-runs $NUM_RUNS >> $log_filename
        
        echo "" >> $log_filename
        echo "" >> $log_filename
        sleep 5s
    done
    
    echo "============================== finalize ==============================" >> $log_filename
    
    # 提取和处理性能数据
    parallel_tps_data=($(cat $log_filename | grep "Parallel execution finishes" | awk -F '=' '{print $2}'))
    parallel_tps=$(IFS=,; echo "${parallel_tps_data[*]}")
    
    speed_up_data=()
    for num in "${parallel_tps_data[@]}"; do
        result=$(bc <<< "scale=2; $num / ${parallel_tps_data[0]}")
        formatted_result=$(printf "%.2f" $result)
        speed_up_data+=("$formatted_result")
    done
    speed_up=$(IFS=,; echo "${speed_up_data[*]}")
    
    sequential_tps_data=($(cat $log_filename | grep "Sequential execution finishes" | awk -F '=' '{print $2}'))
    sequential_tps=$(IFS=,; echo "${sequential_tps_data[*]}")
    
    echo "parallel execution tps: [$parallel_tps]" >> $log_filename
    echo "speed up: [$speed_up]" >> $log_filename
    echo "sequential execution tps: [$sequential_tps]" >> $log_filename
    
    echo "Completed processing: $basename_file"
    echo "Results saved to: $log_filename"
    echo "Block-STM logs saved to: $csv_log_dir"
    echo "----------------------------------------"
    echo ""
done

echo "All CSV files have been processed!"
echo "Performance log files are saved in the $result_dir directory with naming pattern: historical_[csv_filename].log"
echo "Block-STM debug logs are saved in the $log_base_dir directory, organized by CSV file and core count"
echo ""
echo "Usage examples:"
echo "  Default run: ./scripts/run_data_historical_log.sh"
echo "  Custom log level: BLOCK_STM_LOG_LEVEL=INFO ./scripts/run_data_historical_log.sh"
echo "  Multiple runs: NUM_RUNS=3 ./scripts/run_data_historical_log.sh"
echo "  Combined: BLOCK_STM_LOG_LEVEL=INFO NUM_RUNS=5 ./scripts/run_data_historical_log.sh"