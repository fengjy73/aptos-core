#!/bin/bash

# Block-STM 历史转账重放综合测试脚本
# 集成批量性能测试与细粒度日志收集功能
# 基于 run_data_window_historical.sh 和 block-executor 的日志功能

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 默认配置
data_dir="./data"
result_dir="./result"
temp_dir="./temp_data"
log_dir="./block_stm_logs"
block_executor_dir="../block-executor"

# Block-STM 日志配置
DEFAULT_LOG_LEVEL="INFO"
DEFAULT_MAX_SIZE="50"

# 测试配置
transaction_counts=(10 100 1000 10000 100000)
test_scenarios=("basic" "detailed" "stress")

print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Block-STM 历史转账重放综合测试${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  -s, --scenario SCENARIO  Test scenario: basic|detailed|stress (default: basic)"
    echo "  -l, --log-level LEVEL    Log level: DEBUG|INFO|WARN|ERROR (default: $DEFAULT_LOG_LEVEL)"
    echo "  -d, --data-dir DIR       Data directory (default: $data_dir)"
    echo "  -r, --result-dir DIR     Result directory (default: $result_dir)"
    echo "  -c, --clean              Clean existing logs and results before running"
    echo "  -a, --analyze-only       Only analyze existing results, don't run tests"
    echo "  -h, --help               Show this help message"
    echo
    echo "Test Scenarios:"
    echo "  basic    - Standard performance testing with INFO logging"
    echo "  detailed - Comprehensive testing with DEBUG logging and read/write tracking"
    echo "  stress   - Stress testing with large transaction counts"
    echo
    echo "Examples:"
    echo "  $0                                    # Run basic scenario"
    echo "  $0 -s detailed -l DEBUG              # Run detailed scenario with debug logging"
    echo "  $0 -c -s stress                      # Clean and run stress test"
    echo "  $0 -a                                 # Only analyze existing results"
}

check_dependencies() {
    echo -e "${YELLOW}检查依赖项...${NC}"
    
    # 检查 cargo
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}错误: cargo 未安装或不在 PATH 中${NC}"
        echo "请安装 Rust 和 Cargo: https://rustup.rs/"
        exit 1
    fi
    
    # 检查 jq (可选)
    if command -v jq &> /dev/null; then
        echo -e "${GREEN}✓ jq 已找到 - 可进行高级日志分析${NC}"
        JQ_AVAILABLE=true
    else
        echo -e "${YELLOW}⚠ jq 未找到 - 仅基础日志分析${NC}"
        echo "  安装 jq 以获得高级日志分析: brew install jq (macOS) 或 apt-get install jq (Ubuntu)"
        JQ_AVAILABLE=false
    fi
    
    # 检查 bc
    if ! command -v bc &> /dev/null; then
        echo -e "${RED}错误: bc 未安装，用于计算加速比${NC}"
        echo "请安装 bc: brew install bc (macOS) 或 apt-get install bc (Ubuntu)"
        exit 1
    fi
    
    # 检查 CPU 信息脚本
    if [ ! -f "./scripts/cpu_info.sh" ]; then
        echo -e "${RED}错误: ./scripts/cpu_info.sh 不存在${NC}"
        exit 1
    fi
    
    echo
}

setup_environment() {
    echo -e "${YELLOW}设置环境...${NC}"
    
    # 创建目录
    mkdir -p "$result_dir"
    mkdir -p "$temp_dir"
    mkdir -p "$log_dir"
    
    # 设置 Block-STM 日志环境变量
    export BLOCK_STM_LOG_DIR="$log_dir"
    export BLOCK_STM_LOG_LEVEL="$LOG_LEVEL"
    export BLOCK_STM_LOG_MAX_SIZE="$MAX_SIZE"
    export BLOCK_STM_LOG_DETAILED="true"
    export BLOCK_STM_LOG_INCLUDE_READWRITE="true"
    
    # 获取 CPU 核心数信息
    cpu_count=$(./scripts/cpu_info.sh | awk '{print NF}')
    
    echo "环境配置:"
    echo "  数据目录: $data_dir"
    echo "  结果目录: $result_dir"
    echo "  日志目录: $log_dir"
    echo "  日志级别: $LOG_LEVEL"
    echo "  测试场景: $SCENARIO"
    echo "  CPU 核心配置数: $cpu_count"
    echo
}

clean_previous_results() {
    if [ "$CLEAN_LOGS" = true ]; then
        echo -e "${YELLOW}清理之前的结果和日志...${NC}"
        
        if [ -d "$result_dir" ]; then
            rm -f "$result_dir"/*.log
            echo -e "${GREEN}✓ 结果文件已清理${NC}"
        fi
        
        if [ -d "$log_dir" ]; then
            rm -f "$log_dir"/*.log
            echo -e "${GREEN}✓ Block-STM 日志已清理${NC}"
        fi
        
        if [ -d "$temp_dir" ]; then
            rm -f "$temp_dir"/*.csv
            echo -e "${GREEN}✓ 临时文件已清理${NC}"
        fi
        
        echo
    fi
}

# 检测操作系统并选择合适的随机采样方法
detect_random_command() {
    if command -v shuf >/dev/null 2>&1; then
        echo "shuf"
    elif [[ "$(uname)" == "Darwin" ]] || command -v sort >/dev/null 2>&1; then
        echo "sort"
    else
        echo "none"
    fi
}

# 从 CSV 文件随机采样
random_sample_csv() {
    local input_file="$1"
    local output_file="$2"
    local sample_size="$3"
    
    echo "从 $input_file 采样 $sample_size 笔交易到 $output_file"
    
    # 检查输入文件是否存在
    if [ ! -f "$input_file" ]; then
        echo "错误: 输入文件 $input_file 不存在"
        return 1
    fi
    
    # 获取头部行
    head -n 1 "$input_file" > "$output_file"
    
    # 获取数据行总数（不包括头部）
    local total_lines=$(tail -n +2 "$input_file" | wc -l | tr -d ' ')
    echo "可用数据行总数: $total_lines"
    
    # 如果采样大小大于等于总行数，直接复制所有数据
    if [ $sample_size -ge $total_lines ]; then
        echo "采样大小 >= 总行数，复制所有数据"
        tail -n +2 "$input_file" >> "$output_file"
    else
        # 从数据中随机采样（不包括头部）
        echo "从 $total_lines 总行数中随机采样 $sample_size 行"
        
        # 检测当前系统的最佳随机采样方法
        local random_cmd=$(detect_random_command)
        
        case $random_cmd in
            "shuf")
                echo "使用 shuf 命令 (检测到 Linux/Ubuntu 系统)"
                tail -n +2 "$input_file" | shuf -n $sample_size >> "$output_file"
                ;;
            "sort")
                echo "使用 sort -R 命令 (macOS 系统或 shuf 不可用)"
                tail -n +2 "$input_file" | sort -R | head -n $sample_size >> "$output_file"
                ;;
            "none")
                echo "错误: shuf 和 sort 命令都不可用于随机采样"
                return 1
                ;;
        esac
    fi
    
    # 验证输出文件
    local output_lines=$(tail -n +2 "$output_file" | wc -l | tr -d ' ')
    echo "输出文件包含 $output_lines 数据行"
    
    return 0
}

# 运行单个测试
run_single_test() {
    local csv_file="$1"
    local tx_count="$2"
    local basename_file="$3"
    local cores="$4"
    local core_count="$5"
    local log_filename="$6"
    
    echo "============================== 运行 CPU 核心: (${cores}) 数量: ${core_count}, 交易数: ${tx_count} ==============================" >> "$log_filename"
    
    # 清理之前的 Block-STM 日志
    rm -f "$log_dir"/*.log
    
    # 运行测试
    echo "开始执行测试: $tx_count 笔交易，使用 $core_count 个核心"
    cargo run --release -- replay-erc20 --data-path "$csv_file" --concurrency-level $core_count >> "$log_filename" 2>&1
    
    echo "" >> "$log_filename"
    
    # 分析 Block-STM 日志并添加到结果文件
    analyze_block_stm_logs "$log_filename" "$tx_count" "$core_count"
    
    echo "" >> "$log_filename"
    sleep 3s
}

# 分析 Block-STM 日志
analyze_block_stm_logs() {
    local log_filename="$1"
    local tx_count="$2"
    local core_count="$3"
    
    echo "=== Block-STM 详细日志分析 (交易数: $tx_count, 核心数: $core_count) ===" >> "$log_filename"
    
    local exec_log="$log_dir/block_stm_execution.log"
    local conc_log="$log_dir/block_stm_concurrency.log"
    local perf_log="$log_dir/block_stm_performance.log"
    local rw_log="$log_dir/block_stm_readwrite.log"
    local summary_log="$log_dir/block_stm_summary.log"
    
    # 日志文件摘要
    echo "日志文件摘要:" >> "$log_filename"
    for logfile in "$exec_log" "$conc_log" "$perf_log" "$rw_log" "$summary_log"; do
        if [ -f "$logfile" ]; then
            local line_count=$(wc -l < "$logfile")
            local basename_log=$(basename "$logfile")
            echo "  $basename_log: $line_count 事件" >> "$log_filename"
        fi
    done
    
    if [ "$JQ_AVAILABLE" = true ]; then
        # 高级分析
        if [ -f "$exec_log" ]; then
            echo "" >> "$log_filename"
            echo "交易执行统计:" >> "$log_filename"
            
            # 统计交易开始和完成
            local starts=$(jq -r 'select(.event_type == "TransactionStart")' "$exec_log" 2>/dev/null | wc -l || echo "0")
            local finishes=$(jq -r 'select(.event_type == "TransactionFinish")' "$exec_log" 2>/dev/null | wc -l || echo "0")
            echo "  交易开始: $starts" >> "$log_filename"
            echo "  交易完成: $finishes" >> "$log_filename"
            
            # 计算平均执行时间
            if [ "$finishes" -gt 0 ]; then
                local avg_time=$(jq -r 'select(.event_type == "TransactionFinish") | .duration_us' "$exec_log" 2>/dev/null | \
                    awk '{sum+=$1; count++} END {if(count>0) print sum/count; else print "0"}')
                echo "  平均执行时间: ${avg_time} 微秒" >> "$log_filename"
            fi
            
            # 显示执行结果分布
            echo "  执行结果分布:" >> "$log_filename"
            jq -r 'select(.event_type == "TransactionFinish") | .result' "$exec_log" 2>/dev/null | \
                sort | uniq -c | while read count result; do
                    echo "    $result: $count" >> "$log_filename"
                done
        fi
        
        if [ -f "$conc_log" ]; then
            echo "" >> "$log_filename"
            echo "并发统计:" >> "$log_filename"
            
            # 统计中止
            local aborts=$(jq -r 'select(.event_type == "TransactionAbort")' "$conc_log" 2>/dev/null | wc -l || echo "0")
            echo "  交易中止: $aborts" >> "$log_filename"
            
            # 显示中止原因
            if [ "$aborts" -gt 0 ]; then
                echo "  中止原因:" >> "$log_filename"
                jq -r 'select(.event_type == "TransactionAbort") | .reason' "$conc_log" 2>/dev/null | \
                    sort | uniq -c | while read count reason; do
                        echo "    $reason: $count" >> "$log_filename"
                    done
            fi
        fi
        
        if [ -f "$perf_log" ]; then
            echo "" >> "$log_filename"
            echo "性能指标:" >> "$log_filename"
            local perf_count=$(wc -l < "$perf_log")
            echo "  性能事件: $perf_count" >> "$log_filename"
            
            if [ "$perf_count" -gt 0 ]; then
                echo "  指标类型:" >> "$log_filename"
                jq -r '.metric_name' "$perf_log" 2>/dev/null | \
                    sort | uniq -c | while read count metric; do
                        echo "    $metric: $count" >> "$log_filename"
                    done
            fi
        fi
    else
        # 基础分析
        echo "" >> "$log_filename"
        echo "基础日志分析 (前5行):" >> "$log_filename"
        
        for logfile in "$exec_log" "$conc_log" "$perf_log" "$rw_log" "$summary_log"; do
            if [ -f "$logfile" ]; then
                local basename_log=$(basename "$logfile")
                echo "" >> "$log_filename"
                echo "$basename_log:" >> "$log_filename"
                head -n 5 "$logfile" | while IFS= read -r line; do
                    echo "  $line" >> "$log_filename"
                done
                
                local total_lines=$(wc -l < "$logfile")
                if [ "$total_lines" -gt 5 ]; then
                    echo "  ... 还有 $((total_lines - 5)) 行" >> "$log_filename"
                fi
            fi
        done
    fi
    
    echo "=== Block-STM 日志分析结束 ===" >> "$log_filename"
}

# 运行测试场景
run_test_scenario() {
    local scenario="$1"
    
    echo -e "${BLUE}开始运行测试场景: $scenario${NC}"
    
    # 根据场景调整交易数量
    case $scenario in
        "basic")
            local test_counts=(10 100 1000)
            ;;
        "detailed")
            local test_counts=(100 1000 10000)
            ;;
        "stress")
            local test_counts=(1000 10000 100000)
            ;;
        *)
            local test_counts=("${transaction_counts[@]}")
            ;;
    esac
    
    echo "测试交易数量: ${test_counts[*]}"
    echo "找到 $(ls $data_dir/*.csv | wc -l) 个 CSV 文件需要处理"
    echo
    
    # 遍历所有 CSV 文件
    for csv_file in $data_dir/*.csv; do
        # 提取不带路径和扩展名的文件名
        basename_file=$(basename "$csv_file" .csv)
        
        echo "处理: $csv_file"
        echo
        
        # 遍历不同的交易数量
        for tx_count in "${test_counts[@]}"; do
            echo "测试 $tx_count 笔交易..."
            
            # 创建临时 CSV 文件
            temp_csv="$temp_dir/${basename_file}_${tx_count}.csv"
            echo "创建临时文件: $temp_csv"
            random_sample_csv "$csv_file" "$temp_csv" $tx_count
            
            # 检查采样是否成功
            if [ ! -f "$temp_csv" ] || [ $(tail -n +2 "$temp_csv" | wc -l | tr -d ' ') -eq 0 ]; then
                echo "错误: 为 $tx_count 笔交易创建有效采样文件失败"
                continue
            fi
            
            # 创建日志文件名
            log_filename="$result_dir/integrated_${scenario}_${basename_file}_${tx_count}.log"
            
            echo "输出日志: $log_filename"
            
            # 初始化日志文件
            echo "Block-STM 历史转账重放综合测试 - $scenario 场景" > "$log_filename"
            echo "原始数据文件: $csv_file" >> "$log_filename"
            echo "采样数据文件: $temp_csv" >> "$log_filename"
            echo "交易数量: $tx_count" >> "$log_filename"
            echo "测试场景: $scenario" >> "$log_filename"
            echo "日志级别: $LOG_LEVEL" >> "$log_filename"
            echo "开始时间: $(date)" >> "$log_filename"
            echo "" >> "$log_filename"
            
            # 使用不同的 CPU 核心配置运行测试
            for ((i=1; i<=cpu_count; i++)); do
                cores=$(./scripts/cpu_info.sh|cut -d ' ' -f$i)
                # 通过计算逗号数量加1来统计核心数
                core_count=$(echo $cores | tr ',' '\n' | wc -l)
                # 确保核心数至少为1
                if [ $core_count -eq 0 ]; then
                    core_count=1
                fi
                
                run_single_test "$temp_csv" "$tx_count" "$basename_file" "$cores" "$core_count" "$log_filename"
            done
            
            echo "============================== 完成 ==============================" >> "$log_filename"
            
            # 提取和处理性能数据
            parallel_tps_data=($(cat "$log_filename" | grep "Parallel execution finishes" | awk -F '=' '{print $2}'))
            parallel_tps=$(IFS=,; echo "${parallel_tps_data[*]}")
            
            speed_up_data=()
            for num in "${parallel_tps_data[@]}"; do
                if [ ${#parallel_tps_data[@]} -gt 0 ] && [ "${parallel_tps_data[0]}" != "0" ]; then
                    result=$(bc <<< "scale=2; $num / ${parallel_tps_data[0]}")
                    formatted_result=$(printf "%.2f" $result)
                    speed_up_data+=("$formatted_result")
                fi
            done
            speed_up=$(IFS=,; echo "${speed_up_data[*]}")
            
            sequential_tps_data=($(cat "$log_filename" | grep "Sequential execution finishes" | awk -F '=' '{print $2}'))
            sequential_tps=$(IFS=,; echo "${sequential_tps_data[*]}")
            
            echo "" >> "$log_filename"
            echo "=== 性能摘要 ===" >> "$log_filename"
            echo "并行执行 TPS: [$parallel_tps]" >> "$log_filename"
            echo "加速比: [$speed_up]" >> "$log_filename"
            echo "顺序执行 TPS: [$sequential_tps]" >> "$log_filename"
            echo "结束时间: $(date)" >> "$log_filename"
            
            echo "完成 $basename_file 的 $tx_count 笔交易测试"
            echo "结果保存到: $log_filename"
            echo "----------------------------------------"
            echo
            
            # 清理临时文件
            rm -f "$temp_csv"
        done
        
        echo "完成 $basename_file 的所有交易数量测试"
        echo "========================================"
        echo
    done
}

# 分析现有结果
analyze_existing_results() {
    echo -e "${BLUE}=== 分析现有测试结果 ===${NC}"
    
    if [ ! -d "$result_dir" ]; then
        echo -e "${RED}错误: 结果目录 $result_dir 不存在${NC}"
        return 1
    fi
    
    local result_files=("$result_dir"/integrated_*.log)
    
    if [ ${#result_files[@]} -eq 0 ] || [ ! -f "${result_files[0]}" ]; then
        echo -e "${YELLOW}警告: 未找到集成测试结果文件${NC}"
        return 1
    fi
    
    echo "找到 ${#result_files[@]} 个结果文件"
    echo
    
    # 生成摘要报告
    local summary_file="$result_dir/test_summary_$(date +%Y%m%d_%H%M%S).md"
    
    echo "# Block-STM 历史转账重放综合测试摘要报告" > "$summary_file"
    echo "" >> "$summary_file"
    echo "生成时间: $(date)" >> "$summary_file"
    echo "" >> "$summary_file"
    
    echo "## 测试概览" >> "$summary_file"
    echo "" >> "$summary_file"
    
    for result_file in "${result_files[@]}"; do
        if [ -f "$result_file" ]; then
            local basename_result=$(basename "$result_file" .log)
            echo "### $basename_result" >> "$summary_file"
            echo "" >> "$summary_file"
            
            # 提取关键信息
            local scenario=$(grep "测试场景:" "$result_file" | head -1 | cut -d ':' -f2 | tr -d ' ')
            local tx_count=$(grep "交易数量:" "$result_file" | head -1 | cut -d ':' -f2 | tr -d ' ')
            local log_level=$(grep "日志级别:" "$result_file" | head -1 | cut -d ':' -f2 | tr -d ' ')
            
            echo "- **测试场景**: $scenario" >> "$summary_file"
            echo "- **交易数量**: $tx_count" >> "$summary_file"
            echo "- **日志级别**: $log_level" >> "$summary_file"
            
            # 提取性能数据
            local parallel_tps=$(grep "并行执行 TPS:" "$result_file" | tail -1 | cut -d '[' -f2 | cut -d ']' -f1)
            local speed_up=$(grep "加速比:" "$result_file" | tail -1 | cut -d '[' -f2 | cut -d ']' -f1)
            
            if [ -n "$parallel_tps" ]; then
                echo "- **并行执行 TPS**: $parallel_tps" >> "$summary_file"
            fi
            if [ -n "$speed_up" ]; then
                echo "- **加速比**: $speed_up" >> "$summary_file"
            fi
            
            echo "" >> "$summary_file"
        fi
    done
    
    echo "## 分析建议" >> "$summary_file"
    echo "" >> "$summary_file"
    echo "1. **性能分析**: 比较不同核心数配置下的 TPS 和加速比" >> "$summary_file"
    echo "2. **并发分析**: 查看交易中止率和中止原因分布" >> "$summary_file"
    echo "3. **冲突分析**: 分析读写集合冲突模式" >> "$summary_file"
    echo "4. **优化建议**: 基于日志数据提出系统优化建议" >> "$summary_file"
    echo "" >> "$summary_file"
    
    echo "摘要报告已生成: $summary_file"
    echo
    
    # 显示最近的测试结果
    echo "最近的测试结果:"
    for result_file in "${result_files[@]}"; do
        if [ -f "$result_file" ]; then
            local basename_result=$(basename "$result_file")
            local file_size=$(ls -lh "$result_file" | awk '{print $5}')
            echo "  $basename_result ($file_size)"
        fi
    done
}

# 解析命令行参数
SCENARIO="basic"
LOG_LEVEL="$DEFAULT_LOG_LEVEL"
MAX_SIZE="$DEFAULT_MAX_SIZE"
CLEAN_LOGS=false
ANALYZE_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -s|--scenario)
            SCENARIO="$2"
            shift 2
            ;;
        -l|--log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        -d|--data-dir)
            data_dir="$2"
            shift 2
            ;;
        -r|--result-dir)
            result_dir="$2"
            shift 2
            ;;
        -c|--clean)
            CLEAN_LOGS=true
            shift
            ;;
        -a|--analyze-only)
            ANALYZE_ONLY=true
            shift
            ;;
        -h|--help)
            print_usage
            exit 0
            ;;
        *)
            echo -e "${RED}错误: 未知选项 $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

# 验证场景
if [[ ! " ${test_scenarios[*]} " =~ " ${SCENARIO} " ]]; then
    echo -e "${RED}错误: 无效的测试场景 '$SCENARIO'${NC}"
    echo "有效场景: ${test_scenarios[*]}"
    exit 1
fi

# 主执行流程
print_header

if [ "$ANALYZE_ONLY" = false ]; then
    check_dependencies
    setup_environment
    clean_previous_results
    
    echo -e "${GREEN}开始运行 Block-STM 历史转账重放综合测试...${NC}"
    echo
    
    run_test_scenario "$SCENARIO"
    
    echo -e "${GREEN}测试完成!${NC}"
    echo
fi

analyze_existing_results

echo
echo -e "${GREEN}综合测试完成!${NC}"
echo -e "结果保存在: ${BLUE}$result_dir${NC}"
echo -e "Block-STM 日志保存在: ${BLUE}$log_dir${NC}"
echo
echo "手动分析命令:"
echo "  # 查看所有结果文件:"
echo "  ls -la $result_dir/integrated_*.log"
echo
echo "  # 分析特定结果:"
echo "  grep -A 10 '性能摘要' $result_dir/integrated_*.log"
echo
echo "  # 查看 Block-STM 日志:"
echo "  ls -la $log_dir/*.log"

# 清理临时目录
rmdir "$temp_dir" 2>/dev/null || true

echo -e "${BLUE}测试脚本执行完毕!${NC}"