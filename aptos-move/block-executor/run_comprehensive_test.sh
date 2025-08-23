#!/bin/bash

# Block-STM 综合测试脚本
# 结合批量性能测试和细粒度日志记录功能
# 实现对历史转账重放的详细测试和分析

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 默认配置
DEFAULT_DATA_DIR="../aptos-transaction-benchmarks/data"
DEFAULT_RESULT_DIR="./comprehensive_test_results"
DEFAULT_LOG_DIR="./block_stm_detailed_logs"
DEFAULT_TEMP_DIR="./temp_comprehensive_test"
DEFAULT_TEST_SCENARIO="basic"
DEFAULT_LOG_LEVEL="INFO"
DEFAULT_MAX_LOG_SIZE="50"

# 测试场景配置
declare -A SCENARIO_CONFIGS
SCENARIO_CONFIGS["basic"]="10,100,1000 INFO false 3"
SCENARIO_CONFIGS["concurrency"]="1000,10000 DEBUG true 5"
SCENARIO_CONFIGS["conflict"]="10000 DEBUG true 3"
SCENARIO_CONFIGS["stress"]="100000 WARN false 1"
SCENARIO_CONFIGS["full"]="10,100,1000,10000,100000 DEBUG true 2"

print_header() {
    echo -e "${BLUE}================================================${NC}"
    echo -e "${BLUE}     Block-STM 综合测试框架${NC}"
    echo -e "${BLUE}================================================${NC}"
    echo -e "${CYAN}集成批量性能测试和细粒度日志记录功能${NC}"
    echo
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  -d, --data-dir DIR           数据目录 (default: $DEFAULT_DATA_DIR)"
    echo "  -r, --result-dir DIR         结果目录 (default: $DEFAULT_RESULT_DIR)"
    echo "  -l, --log-dir DIR            日志目录 (default: $DEFAULT_LOG_DIR)"
    echo "  -t, --temp-dir DIR           临时目录 (default: $DEFAULT_TEMP_DIR)"
    echo "  -s, --scenario SCENARIO      测试场景 (default: $DEFAULT_TEST_SCENARIO)"
    echo "  -v, --log-level LEVEL        日志级别: DEBUG|INFO|WARN|ERROR (default: $DEFAULT_LOG_LEVEL)"
    echo "  -m, --max-log-size SIZE      最大日志文件大小(MB) (default: $DEFAULT_MAX_LOG_SIZE)"
    echo "  -c, --clean                  清理现有结果"
    echo "  -a, --analyze-only           仅分析现有结果"
    echo "  -h, --help                   显示帮助信息"
    echo
    echo "测试场景:"
    echo "  basic        基础性能测试 (10, 100, 1K transactions)"
    echo "  concurrency  并发行为分析 (1K, 10K transactions)"
    echo "  conflict     冲突模式研究 (10K transactions)"
    echo "  stress       压力测试 (100K transactions)"
    echo "  full         完整测试 (所有规模)"
    echo
    echo "Examples:"
    echo "  $0                                    # 运行基础测试"
    echo "  $0 -s concurrency -v DEBUG           # 运行并发分析测试"
    echo "  $0 -s full -c                        # 清理并运行完整测试"
    echo "  $0 -a                                 # 仅分析现有结果"
}

check_dependencies() {
    echo -e "${YELLOW}检查依赖项...${NC}"
    
    # 检查必要的命令
    local missing_deps=()
    
    if ! command -v cargo &> /dev/null; then
        missing_deps+=("cargo")
    fi
    
    if ! command -v jq &> /dev/null; then
        echo -e "${YELLOW}⚠ jq 未找到 - 将使用基础分析功能${NC}"
        echo "  建议安装 jq 以获得高级分析功能: brew install jq (macOS) 或 apt-get install jq (Ubuntu)"
        JQ_AVAILABLE=false
    else
        echo -e "${GREEN}✓ jq 已找到 - 高级分析功能可用${NC}"
        JQ_AVAILABLE=true
    fi
    
    if ! command -v bc &> /dev/null; then
        missing_deps+=("bc")
    fi
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        echo -e "${RED}错误: 缺少必要的依赖项: ${missing_deps[*]}${NC}"
        exit 1
    fi
    
    # 检查CPU信息脚本
    if [ ! -f "../aptos-transaction-benchmarks/scripts/cpu_info.sh" ]; then
        echo -e "${RED}错误: 未找到 cpu_info.sh 脚本${NC}"
        echo "请确保在正确的目录中运行此脚本"
        exit 1
    fi
    
    echo -e "${GREEN}✓ 所有依赖项检查通过${NC}"
    echo
}

setup_environment() {
    echo -e "${YELLOW}设置测试环境...${NC}"
    
    # 创建必要的目录
    mkdir -p "$RESULT_DIR"
    mkdir -p "$LOG_DIR"
    mkdir -p "$TEMP_DIR"
    
    # 设置Block-STM日志环境变量
    export BLOCK_STM_LOG_DIR="$LOG_DIR"
    export BLOCK_STM_LOG_LEVEL="$LOG_LEVEL"
    export BLOCK_STM_LOG_MAX_SIZE="$MAX_LOG_SIZE"
    export BLOCK_STM_LOG_DETAILED="$DETAILED_LOGGING"
    
    # 获取CPU核心信息
    if [ -f "../aptos-transaction-benchmarks/scripts/cpu_info.sh" ]; then
        CPU_COUNT=$(../aptos-transaction-benchmarks/scripts/cpu_info.sh | awk '{print NF}')
    else
        CPU_COUNT=1
        echo -e "${YELLOW}⚠ 无法获取CPU信息，使用默认值 1${NC}"
    fi
    
    echo "环境配置:"
    echo "  数据目录: $DATA_DIR"
    echo "  结果目录: $RESULT_DIR"
    echo "  日志目录: $LOG_DIR"
    echo "  临时目录: $TEMP_DIR"
    echo "  测试场景: $TEST_SCENARIO"
    echo "  日志级别: $LOG_LEVEL"
    echo "  详细日志: $DETAILED_LOGGING"
    echo "  CPU核心数: $CPU_COUNT"
    echo
}

clean_previous_results() {
    echo -e "${YELLOW}清理之前的测试结果...${NC}"
    
    if [ -d "$RESULT_DIR" ]; then
        rm -rf "$RESULT_DIR"/*
        echo -e "${GREEN}✓ 结果目录已清理${NC}"
    fi
    
    if [ -d "$LOG_DIR" ]; then
        rm -rf "$LOG_DIR"/*
        echo -e "${GREEN}✓ 日志目录已清理${NC}"
    fi
    
    if [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"/*
        echo -e "${GREEN}✓ 临时目录已清理${NC}"
    fi
    
    echo
}

# 检测随机采样命令
detect_random_command() {
    if command -v shuf >/dev/null 2>&1; then
        echo "shuf"
    elif [[ "$(uname)" == "Darwin" ]] || command -v sort >/dev/null 2>&1; then
        echo "sort"
    else
        echo "none"
    fi
}

# 随机采样CSV文件
random_sample_csv() {
    local input_file="$1"
    local output_file="$2"
    local sample_size="$3"
    
    echo "  从 $input_file 采样 $sample_size 个事务到 $output_file"
    
    if [ ! -f "$input_file" ]; then
        echo -e "${RED}错误: 输入文件 $input_file 不存在${NC}"
        return 1
    fi
    
    # 获取标题行
    head -n 1 "$input_file" > "$output_file"
    
    # 获取数据行总数（不包括标题）
    local total_lines=$(tail -n +2 "$input_file" | wc -l | tr -d ' ')
    echo "    可用数据行总数: $total_lines"
    
    if [ $sample_size -ge $total_lines ]; then
        echo "    采样大小 >= 总行数，复制所有数据"
        tail -n +2 "$input_file" >> "$output_file"
    else
        echo "    从 $total_lines 行中随机采样 $sample_size 行"
        
        local random_cmd=$(detect_random_command)
        
        case $random_cmd in
            "shuf")
                tail -n +2 "$input_file" | shuf -n $sample_size >> "$output_file"
                ;;
            "sort")
                tail -n +2 "$input_file" | sort -R | head -n $sample_size >> "$output_file"
                ;;
            "none")
                echo -e "${RED}错误: 无法找到随机采样命令${NC}"
                return 1
                ;;
        esac
    fi
    
    local output_lines=$(tail -n +2 "$output_file" | wc -l | tr -d ' ')
    echo "    输出文件包含 $output_lines 行数据"
    
    return 0
}

run_single_test() {
    local csv_file="$1"
    local tx_count="$2"
    local basename_file="$3"
    local test_id="$4"
    
    echo -e "${CYAN}运行测试: $basename_file, 事务数: $tx_count${NC}"
    
    # 创建临时CSV文件
    local temp_csv="$TEMP_DIR/${basename_file}_${tx_count}_${test_id}.csv"
    random_sample_csv "$csv_file" "$temp_csv" $tx_count
    
    if [ ! -f "$temp_csv" ] || [ $(tail -n +2 "$temp_csv" | wc -l | tr -d ' ') -eq 0 ]; then
        echo -e "${RED}错误: 无法创建有效的采样文件${NC}"
        return 1
    fi
    
    # 创建测试特定的日志目录
    local test_log_dir="$LOG_DIR/${basename_file}_${tx_count}_${test_id}"
    mkdir -p "$test_log_dir"
    
    # 设置测试特定的日志环境
    export BLOCK_STM_LOG_DIR="$test_log_dir"
    
    # 创建结果日志文件
    local result_log="$RESULT_DIR/test_${basename_file}_${tx_count}_${test_id}.log"
    
    echo "=== Block-STM 综合测试结果 ===" > "$result_log"
    echo "测试ID: ${test_id}" >> "$result_log"
    echo "数据文件: $csv_file" >> "$result_log"
    echo "采样文件: $temp_csv" >> "$result_log"
    echo "事务数量: $tx_count" >> "$result_log"
    echo "测试场景: $TEST_SCENARIO" >> "$result_log"
    echo "日志级别: $LOG_LEVEL" >> "$result_log"
    echo "详细日志: $DETAILED_LOGGING" >> "$result_log"
    echo "日志目录: $test_log_dir" >> "$result_log"
    echo "测试时间: $(date)" >> "$result_log"
    echo "" >> "$result_log"
    
    # 运行不同CPU核心配置的测试
    for ((i=1; i<=$CPU_COUNT; i++)); do
        if [ -f "../aptos-transaction-benchmarks/scripts/cpu_info.sh" ]; then
            cores=$(../aptos-transaction-benchmarks/scripts/cpu_info.sh | cut -d ' ' -f$i)
            core_count=$(echo $cores | tr ',' '\n' | wc -l)
        else
            cores="0"
            core_count=1
        fi
        
        if [ $core_count -eq 0 ]; then
            core_count=1
        fi
        
        echo "=== 运行配置: CPU核心 ($cores), 核心数: $core_count, 事务数: $tx_count ===" >> "$result_log"
        echo -e "${PURPLE}  测试配置: $core_count 核心, $tx_count 事务${NC}"
        
        # 切换到正确的目录并运行测试
        cd ../aptos-transaction-benchmarks
        
        # 运行测试并捕获输出
        if cargo run --release -- replay-erc20 --data-path "$temp_csv" --concurrency-level $core_count >> "$result_log" 2>&1; then
            echo -e "${GREEN}    ✓ 测试完成${NC}"
        else
            echo -e "${RED}    ✗ 测试失败${NC}"
        fi
        
        cd - > /dev/null
        
        echo "" >> "$result_log"
        echo "" >> "$result_log"
        
        sleep 2s
    done
    
    # 处理性能数据
    process_performance_data "$result_log"
    
    # 分析Block-STM日志
    if [ "$DETAILED_LOGGING" = "true" ]; then
        analyze_block_stm_logs "$test_log_dir" "$result_log"
    fi
    
    # 清理临时文件
    rm -f "$temp_csv"
    
    echo -e "${GREEN}✓ 测试完成: $basename_file ($tx_count 事务)${NC}"
    echo
}

process_performance_data() {
    local result_log="$1"
    
    echo "=== 性能数据分析 ===" >> "$result_log"
    
    # 提取并处理性能数据
    local parallel_tps_data=($(cat "$result_log" | grep "Parallel execution finishes" | awk -F '=' '{print $2}'))
    local parallel_tps=$(IFS=,; echo "${parallel_tps_data[*]}")
    
    if [ ${#parallel_tps_data[@]} -gt 0 ]; then
        local speed_up_data=()
        for num in "${parallel_tps_data[@]}"; do
            if [ "${parallel_tps_data[0]}" != "0" ]; then
                result=$(bc <<< "scale=2; $num / ${parallel_tps_data[0]}")
                formatted_result=$(printf "%.2f" $result)
                speed_up_data+=("$formatted_result")
            else
                speed_up_data+=("0.00")
            fi
        done
        local speed_up=$(IFS=,; echo "${speed_up_data[*]}")
        
        local sequential_tps_data=($(cat "$result_log" | grep "Sequential execution finishes" | awk -F '=' '{print $2}'))
        local sequential_tps=$(IFS=,; echo "${sequential_tps_data[*]}")
        
        echo "并行执行 TPS: [$parallel_tps]" >> "$result_log"
        echo "加速比: [$speed_up]" >> "$result_log"
        echo "顺序执行 TPS: [$sequential_tps]" >> "$result_log"
    else
        echo "警告: 未找到性能数据" >> "$result_log"
    fi
    
    echo "" >> "$result_log"
}

analyze_block_stm_logs() {
    local log_dir="$1"
    local result_log="$2"
    
    echo "=== Block-STM 详细日志分析 ===" >> "$result_log"
    
    if [ ! -d "$log_dir" ]; then
        echo "警告: 日志目录不存在: $log_dir" >> "$result_log"
        return
    fi
    
    # 分析各种日志文件
    local log_files=(
        "block_stm_execution.log:执行事件"
        "block_stm_concurrency.log:并发事件"
        "block_stm_readwrite.log:读写事件"
        "block_stm_performance.log:性能指标"
        "block_stm_summary.log:汇总事件"
    )
    
    echo "日志文件摘要:" >> "$result_log"
    for entry in "${log_files[@]}"; do
        IFS=':' read -r filename description <<< "$entry"
        local filepath="$log_dir/$filename"
        
        if [ -f "$filepath" ]; then
            local line_count=$(wc -l < "$filepath")
            echo "  ✓ $description: $line_count 个事件" >> "$result_log"
        else
            echo "  ⚠ $description: 无事件记录" >> "$result_log"
        fi
    done
    
    echo "" >> "$result_log"
    
    # 如果有jq，进行高级分析
    if [ "$JQ_AVAILABLE" = true ]; then
        perform_advanced_log_analysis "$log_dir" "$result_log"
    else
        perform_basic_log_analysis "$log_dir" "$result_log"
    fi
}

perform_advanced_log_analysis() {
    local log_dir="$1"
    local result_log="$2"
    
    echo "高级日志分析 (使用 jq):" >> "$result_log"
    
    local exec_log="$log_dir/block_stm_execution.log"
    local conc_log="$log_dir/block_stm_concurrency.log"
    
    if [ -f "$exec_log" ]; then
        echo "事务统计:" >> "$result_log"
        
        local starts=$(jq -r 'select(.event_type == "TransactionStart")' "$exec_log" 2>/dev/null | wc -l || echo "0")
        local finishes=$(jq -r 'select(.event_type == "TransactionFinish")' "$exec_log" 2>/dev/null | wc -l || echo "0")
        echo "  事务开始: $starts" >> "$result_log"
        echo "  事务完成: $finishes" >> "$result_log"
        
        if [ "$finishes" -gt 0 ]; then
            local avg_time=$(jq -r 'select(.event_type == "TransactionFinish") | .duration_us' "$exec_log" 2>/dev/null | \
                awk '{sum+=$1; count++} END {if(count>0) print sum/count; else print "0"}')
            echo "  平均执行时间: ${avg_time} 微秒" >> "$result_log"
        fi
        
        echo "  执行结果分布:" >> "$result_log"
        jq -r 'select(.event_type == "TransactionFinish") | .result' "$exec_log" 2>/dev/null | \
            sort | uniq -c | while read count result; do
                echo "    $result: $count" >> "$result_log"
            done
    fi
    
    if [ -f "$conc_log" ]; then
        echo "" >> "$result_log"
        echo "并发统计:" >> "$result_log"
        
        local aborts=$(jq -r 'select(.event_type == "TransactionAbort")' "$conc_log" 2>/dev/null | wc -l || echo "0")
        echo "  事务中止: $aborts" >> "$result_log"
        
        if [ "$aborts" -gt 0 ]; then
            echo "  中止原因:" >> "$result_log"
            jq -r 'select(.event_type == "TransactionAbort") | .reason' "$conc_log" 2>/dev/null | \
                sort | uniq -c | while read count reason; do
                    echo "    $reason: $count" >> "$result_log"
                done
        fi
    fi
    
    echo "" >> "$result_log"
}

perform_basic_log_analysis() {
    local log_dir="$1"
    local result_log="$2"
    
    echo "基础日志分析:" >> "$result_log"
    
    for logfile in "$log_dir"/*.log; do
        if [ -f "$logfile" ]; then
            echo "" >> "$result_log"
            echo "$(basename "$logfile") (前5行):" >> "$result_log"
            head -n 5 "$logfile" | while IFS= read -r line; do
                echo "  $line" >> "$result_log"
            done
            
            local total_lines=$(wc -l < "$logfile")
            if [ "$total_lines" -gt 5 ]; then
                echo "  ... 还有 $((total_lines - 5)) 行" >> "$result_log"
            fi
        fi
    done
    
    echo "" >> "$result_log"
}

run_test_scenario() {
    local scenario="$1"
    
    echo -e "${BLUE}=== 运行测试场景: $scenario ===${NC}"
    
    # 解析场景配置
    local config="${SCENARIO_CONFIGS[$scenario]}"
    if [ -z "$config" ]; then
        echo -e "${RED}错误: 未知的测试场景: $scenario${NC}"
        exit 1
    fi
    
    IFS=' ' read -r tx_counts log_level detailed_logging repeat_count <<< "$config"
    
    # 更新配置
    LOG_LEVEL="$log_level"
    DETAILED_LOGGING="$detailed_logging"
    
    # 重新设置环境
    export BLOCK_STM_LOG_LEVEL="$LOG_LEVEL"
    export BLOCK_STM_LOG_DETAILED="$DETAILED_LOGGING"
    
    echo "场景配置:"
    echo "  事务数量: $tx_counts"
    echo "  日志级别: $log_level"
    echo "  详细日志: $detailed_logging"
    echo "  重复次数: $repeat_count"
    echo
    
    # 转换事务数量字符串为数组
    IFS=',' read -ra TRANSACTION_COUNTS <<< "$tx_counts"
    
    # 检查数据目录
    if [ ! -d "$DATA_DIR" ]; then
        echo -e "${RED}错误: 数据目录不存在: $DATA_DIR${NC}"
        exit 1
    fi
    
    local csv_files=("$DATA_DIR"/*.csv)
    if [ ! -f "${csv_files[0]}" ]; then
        echo -e "${RED}错误: 在数据目录中未找到CSV文件: $DATA_DIR${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}找到 ${#csv_files[@]} 个CSV文件进行处理${NC}"
    echo
    
    # 运行测试
    local test_count=0
    for csv_file in "${csv_files[@]}"; do
        local basename_file=$(basename "$csv_file" .csv)
        echo -e "${CYAN}处理文件: $csv_file${NC}"
        
        for tx_count in "${TRANSACTION_COUNTS[@]}"; do
            for ((repeat=1; repeat<=repeat_count; repeat++)); do
                test_count=$((test_count + 1))
                local test_id="${scenario}_${test_count}_r${repeat}"
                
                echo -e "${YELLOW}测试 $test_count/$((${#csv_files[@]} * ${#TRANSACTION_COUNTS[@]} * repeat_count)): $basename_file, $tx_count 事务, 重复 $repeat/$repeat_count${NC}"
                
                run_single_test "$csv_file" "$tx_count" "$basename_file" "$test_id"
            done
        done
        
        echo -e "${GREEN}完成文件: $basename_file${NC}"
        echo "========================================"
        echo
    done
    
    echo -e "${GREEN}场景 '$scenario' 测试完成！${NC}"
    echo -e "${BLUE}总共运行了 $test_count 个测试${NC}"
    echo
}

generate_summary_report() {
    echo -e "${YELLOW}生成汇总报告...${NC}"
    
    local summary_file="$RESULT_DIR/comprehensive_test_summary.md"
    
    cat > "$summary_file" << EOF
# Block-STM 综合测试汇总报告

## 测试概览

- **测试时间**: $(date)
- **测试场景**: $TEST_SCENARIO
- **日志级别**: $LOG_LEVEL
- **详细日志**: $DETAILED_LOGGING
- **数据目录**: $DATA_DIR
- **结果目录**: $RESULT_DIR
- **日志目录**: $LOG_DIR

## 测试结果文件

EOF
    
    # 列出所有测试结果文件
    echo "### 详细测试结果" >> "$summary_file"
    echo "" >> "$summary_file"
    for result_file in "$RESULT_DIR"/test_*.log; do
        if [ -f "$result_file" ]; then
            local filename=$(basename "$result_file")
            echo "- [$filename](./$filename)" >> "$summary_file"
        fi
    done
    
    echo "" >> "$summary_file"
    echo "### Block-STM 详细日志" >> "$summary_file"
    echo "" >> "$summary_file"
    
    # 列出所有日志目录
    for log_subdir in "$LOG_DIR"/*/; do
        if [ -d "$log_subdir" ]; then
            local dirname=$(basename "$log_subdir")
            echo "- [$dirname](./../block_stm_detailed_logs/$dirname/)" >> "$summary_file"
        fi
    done
    
    # 如果有jq，添加快速统计
    if [ "$JQ_AVAILABLE" = true ]; then
        echo "" >> "$summary_file"
        echo "## 快速统计" >> "$summary_file"
        echo "" >> "$summary_file"
        
        local total_tests=$(ls "$RESULT_DIR"/test_*.log 2>/dev/null | wc -l)
        echo "- **总测试数**: $total_tests" >> "$summary_file"
        
        # 统计性能数据
        local total_parallel_tps=0
        local test_count=0
        for result_file in "$RESULT_DIR"/test_*.log; do
            if [ -f "$result_file" ]; then
                local tps=$(grep "并行执行 TPS:" "$result_file" | head -1 | sed 's/.*\[\([^,]*\).*/\1/' | tr -d ' ')
                if [[ "$tps" =~ ^[0-9]+$ ]]; then
                    total_parallel_tps=$((total_parallel_tps + tps))
                    test_count=$((test_count + 1))
                fi
            fi
        done
        
        if [ $test_count -gt 0 ]; then
            local avg_tps=$((total_parallel_tps / test_count))
            echo "- **平均并行 TPS**: $avg_tps" >> "$summary_file"
        fi
    fi
    
    cat >> "$summary_file" << EOF

## 分析建议

### 性能分析
1. 查看各个测试结果文件中的 "性能数据分析" 部分
2. 比较不同事务数量下的TPS和加速比
3. 关注CPU核心数对性能的影响

### 并发行为分析
1. 检查Block-STM详细日志中的并发统计
2. 分析事务中止率和中止原因
3. 识别性能瓶颈和热点

### 进一步分析
使用以下命令进行更深入的分析：

\`\`\`bash
# 分析所有测试的平均TPS
grep "并行执行 TPS:" $RESULT_DIR/test_*.log

# 查看加速比分布
grep "加速比:" $RESULT_DIR/test_*.log

# 分析日志事件（如果有jq）
find $LOG_DIR -name "block_stm_execution.log" -exec jq 'select(.event_type == "TransactionFinish") | .duration_us' {} \;
\`\`\`

---

*报告生成时间: $(date)*
EOF
    
    echo -e "${GREEN}✓ 汇总报告已生成: $summary_file${NC}"
}

analyze_existing_results() {
    echo -e "${BLUE}=== 分析现有测试结果 ===${NC}"
    
    if [ ! -d "$RESULT_DIR" ] || [ -z "$(ls -A "$RESULT_DIR" 2>/dev/null)" ]; then
        echo -e "${RED}错误: 结果目录为空或不存在: $RESULT_DIR${NC}"
        exit 1
    fi
    
    echo "找到的测试结果文件:"
    for result_file in "$RESULT_DIR"/test_*.log; do
        if [ -f "$result_file" ]; then
            echo "  - $(basename "$result_file")"
        fi
    done
    
    echo
    
    # 生成汇总报告
    generate_summary_report
    
    # 如果有jq，进行高级分析
    if [ "$JQ_AVAILABLE" = true ]; then
        echo -e "${YELLOW}执行高级分析...${NC}"
        
        # 分析性能趋势
        echo "性能趋势分析:"
        echo "事务数量 -> 平均TPS"
        for result_file in "$RESULT_DIR"/test_*.log; do
            if [ -f "$result_file" ]; then
                local filename=$(basename "$result_file")
                local tx_count=$(echo "$filename" | sed 's/.*_\([0-9]\+\)_.*/\1/')
                local tps=$(grep "并行执行 TPS:" "$result_file" | head -1 | sed 's/.*\[\([^,]*\).*/\1/' | tr -d ' ')
                if [[ "$tps" =~ ^[0-9]+$ ]]; then
                    echo "  $tx_count -> $tps"
                fi
            fi
        done
        
        echo
    fi
    
    echo -e "${GREEN}分析完成！${NC}"
}

# 解析命令行参数
DATA_DIR="$DEFAULT_DATA_DIR"
RESULT_DIR="$DEFAULT_RESULT_DIR"
LOG_DIR="$DEFAULT_LOG_DIR"
TEMP_DIR="$DEFAULT_TEMP_DIR"
TEST_SCENARIO="$DEFAULT_TEST_SCENARIO"
LOG_LEVEL="$DEFAULT_LOG_LEVEL"
MAX_LOG_SIZE="$DEFAULT_MAX_LOG_SIZE"
DETAILED_LOGGING="false"
CLEAN_RESULTS=false
ANALYZE_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--data-dir)
            DATA_DIR="$2"
            shift 2
            ;;
        -r|--result-dir)
            RESULT_DIR="$2"
            shift 2
            ;;
        -l|--log-dir)
            LOG_DIR="$2"
            shift 2
            ;;
        -t|--temp-dir)
            TEMP_DIR="$2"
            shift 2
            ;;
        -s|--scenario)
            TEST_SCENARIO="$2"
            shift 2
            ;;
        -v|--log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        -m|--max-log-size)
            MAX_LOG_SIZE="$2"
            shift 2
            ;;
        -c|--clean)
            CLEAN_RESULTS=true
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

# 主执行流程
print_header

if [ "$ANALYZE_ONLY" = true ]; then
    analyze_existing_results
else
    check_dependencies
    setup_environment
    
    if [ "$CLEAN_RESULTS" = true ]; then
        clean_previous_results
    fi
    
    run_test_scenario "$TEST_SCENARIO"
    generate_summary_report
fi

echo
echo -e "${GREEN}综合测试完成！${NC}"
echo -e "结果保存在: ${BLUE}$RESULT_DIR${NC}"
if [ "$DETAILED_LOGGING" = "true" ]; then
    echo -e "详细日志保存在: ${BLUE}$LOG_DIR${NC}"
fi

# 清理临时目录
if [ -d "$TEMP_DIR" ] && [ -z "$(ls -A "$TEMP_DIR" 2>/dev/null)" ]; then
    rmdir "$TEMP_DIR" 2>/dev/null
fi