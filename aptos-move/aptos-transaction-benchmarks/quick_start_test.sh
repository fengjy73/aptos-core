#!/bin/bash

# Block-STM 集成测试快速开始脚本
# 用于验证测试环境和运行简单的验证测试

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  Block-STM 集成测试快速开始${NC}"
    echo -e "${BLUE}========================================${NC}"
    echo
}

print_step() {
    echo -e "${BLUE}[步骤 $1]${NC} $2"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# 检查当前目录
check_directory() {
    print_step "1" "检查当前目录"
    
    if [ ! -f "Cargo.toml" ]; then
        print_error "当前目录不是 aptos-transaction-benchmarks 目录"
        echo "请切换到正确的目录:"
        echo "cd /path/to/aptos-core/aptos-move/aptos-transaction-benchmarks"
        exit 1
    fi
    
    if [ ! -d "data" ]; then
        print_warning "data 目录不存在，将创建示例目录"
        mkdir -p data
    fi
    
    print_success "目录检查通过"
    echo
}

# 检查依赖项
check_dependencies() {
    print_step "2" "检查系统依赖"
    
    local missing_deps=()
    
    # 检查 cargo
    if ! command -v cargo &> /dev/null; then
        missing_deps+=("cargo")
    else
        print_success "cargo 已安装"
    fi
    
    # 检查 jq
    if ! command -v jq &> /dev/null; then
        print_warning "jq 未安装 - 将使用基础日志分析"
        echo "  安装命令: brew install jq (macOS) 或 apt-get install jq (Ubuntu)"
    else
        print_success "jq 已安装"
    fi
    
    # 检查 bc
    if ! command -v bc &> /dev/null; then
        missing_deps+=("bc")
    else
        print_success "bc 已安装"
    fi
    
    # 检查必需的脚本
    if [ ! -f "scripts/cpu_info.sh" ]; then
        print_error "scripts/cpu_info.sh 不存在"
        missing_deps+=("cpu_info.sh")
    else
        print_success "cpu_info.sh 脚本存在"
    fi
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        print_error "缺少必需的依赖: ${missing_deps[*]}"
        echo "请安装缺少的依赖后重新运行此脚本"
        exit 1
    fi
    
    print_success "所有依赖检查通过"
    echo
}

# 检查数据文件
check_data_files() {
    print_step "3" "检查数据文件"
    
    local csv_files=(data/*.csv)
    
    if [ ! -f "${csv_files[0]}" ]; then
        print_warning "未找到 CSV 数据文件"
        echo "将创建示例数据文件用于测试..."
        create_sample_data
    else
        local file_count=$(ls data/*.csv 2>/dev/null | wc -l)
        print_success "找到 $file_count 个 CSV 数据文件"
        
        # 显示文件信息
        echo "数据文件列表:"
        for file in data/*.csv; do
            if [ -f "$file" ]; then
                local size=$(ls -lh "$file" | awk '{print $5}')
                local lines=$(wc -l < "$file")
                echo "  $(basename "$file") - $size ($lines 行)"
            fi
        done
    fi
    
    echo
}

# 创建示例数据
create_sample_data() {
    local sample_file="data/sample_test_data.csv"
    
    echo "创建示例数据文件: $sample_file"
    
    # 创建 CSV 头部
    echo "from_address,to_address,amount,timestamp" > "$sample_file"
    
    # 生成示例交易数据
    for i in {1..50}; do
        local from_addr="0x$(printf '%040x' $((RANDOM * RANDOM)))"
        local to_addr="0x$(printf '%040x' $((RANDOM * RANDOM)))"
        local amount=$((RANDOM % 1000 + 1))
        local timestamp=$((1640995200 + RANDOM % 86400))
        
        echo "$from_addr,$to_addr,$amount,$timestamp" >> "$sample_file"
    done
    
    print_success "示例数据文件已创建 (50 笔交易)"
}

# 测试编译
test_compilation() {
    print_step "4" "测试项目编译"
    
    echo "正在编译项目..."
    if cargo build --release --quiet; then
        print_success "项目编译成功"
    else
        print_error "项目编译失败"
        echo "请检查 Rust 环境和项目依赖"
        exit 1
    fi
    
    echo
}

# 运行快速测试
run_quick_test() {
    print_step "5" "运行快速验证测试"
    
    echo "运行基础集成测试 (10 笔交易)..."
    
    # 设置测试参数
    export BLOCK_STM_LOG_DIR="./block_stm_logs"
    export BLOCK_STM_LOG_LEVEL="INFO"
    export BLOCK_STM_LOG_MAX_SIZE="10"
    export BLOCK_STM_LOG_DETAILED="true"
    
    # 创建日志目录
    mkdir -p block_stm_logs result temp_data
    
    # 获取第一个 CSV 文件
    local test_file=$(ls data/*.csv | head -1)
    local basename_file=$(basename "$test_file" .csv)
    
    # 创建小样本用于快速测试
    local temp_csv="temp_data/quick_test_sample.csv"
    head -n 1 "$test_file" > "$temp_csv"
    tail -n +2 "$test_file" | head -n 10 >> "$temp_csv"
    
    echo "使用数据文件: $test_file (采样 10 笔交易)"
    
    # 运行测试
    local log_file="result/quick_test_$(date +%Y%m%d_%H%M%S).log"
    
    echo "Block-STM 快速验证测试" > "$log_file"
    echo "测试时间: $(date)" >> "$log_file"
    echo "数据文件: $test_file" >> "$log_file"
    echo "交易数量: 10" >> "$log_file"
    echo "" >> "$log_file"
    
    if cargo run --release -- replay-erc20 --data-path "$temp_csv" --concurrency-level 2 >> "$log_file" 2>&1; then
        print_success "快速测试执行成功"
        
        # 检查结果
        if grep -q "Parallel execution finishes" "$log_file"; then
            local tps=$(grep "Parallel execution finishes" "$log_file" | awk -F '=' '{print $2}' | head -1)
            print_success "并行执行 TPS: $tps"
        fi
        
        if grep -q "Sequential execution finishes" "$log_file"; then
            local seq_tps=$(grep "Sequential execution finishes" "$log_file" | awk -F '=' '{print $2}' | head -1)
            print_success "顺序执行 TPS: $seq_tps"
        fi
        
        echo "测试日志保存到: $log_file"
        
    else
        print_error "快速测试执行失败"
        echo "请查看日志文件: $log_file"
        return 1
    fi
    
    # 检查 Block-STM 日志
    echo
    echo "Block-STM 日志文件:"
    for logfile in block_stm_logs/*.log; do
        if [ -f "$logfile" ]; then
            local lines=$(wc -l < "$logfile")
            echo "  $(basename "$logfile"): $lines 事件"
        fi
    done
    
    # 清理临时文件
    rm -f "$temp_csv"
    
    echo
}

# 显示下一步操作
show_next_steps() {
    print_step "6" "下一步操作"
    
    echo "环境验证完成！现在可以运行完整的集成测试:"
    echo
    echo -e "${YELLOW}基础测试:${NC}"
    echo "  ./run_integrated_historical_test.sh"
    echo
    echo -e "${YELLOW}详细测试:${NC}"
    echo "  ./run_integrated_historical_test.sh -s detailed -l DEBUG"
    echo
    echo -e "${YELLOW}压力测试:${NC}"
    echo "  ./run_integrated_historical_test.sh -s stress"
    echo
    echo -e "${YELLOW}清理并重新测试:${NC}"
    echo "  ./run_integrated_historical_test.sh -c -s basic"
    echo
    echo -e "${YELLOW}仅分析现有结果:${NC}"
    echo "  ./run_integrated_historical_test.sh -a"
    echo
    echo "更多信息请参考: INTEGRATED_TESTING_GUIDE.md"
    echo
}

# 主执行流程
main() {
    print_header
    
    check_directory
    check_dependencies
    check_data_files
    test_compilation
    
    if run_quick_test; then
        print_success "所有验证测试通过！"
        show_next_steps
    else
        print_error "验证测试失败，请检查配置"
        exit 1
    fi
    
    echo -e "${GREEN}快速开始验证完成！${NC}"
}

# 解析命令行参数
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            echo "Block-STM 集成测试快速开始脚本"
            echo
            echo "用法: $0 [选项]"
            echo
            echo "选项:"
            echo "  -h, --help    显示此帮助信息"
            echo
            echo "此脚本将:"
            echo "  1. 检查当前目录和依赖项"
            echo "  2. 验证数据文件"
            echo "  3. 测试项目编译"
            echo "  4. 运行快速验证测试"
            echo "  5. 显示下一步操作指南"
            echo
            exit 0
            ;;
        *)
            echo "未知选项: $1"
            echo "使用 -h 或 --help 查看帮助"
            exit 1
            ;;
    esac
done

# 运行主程序
main