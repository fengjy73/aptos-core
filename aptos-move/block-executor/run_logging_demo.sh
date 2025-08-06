#!/bin/bash

# Block-STM Logging Demo Script
# This script demonstrates how to run Block-STM with logging enabled
# and provides basic log analysis capabilities

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
DEFAULT_LOG_DIR="./block_stm_logs"
DEFAULT_LOG_LEVEL="INFO"
DEFAULT_TXN_COUNT="50"
DEFAULT_MAX_SIZE="10"

print_header() {
    echo -e "${BLUE}================================${NC}"
    echo -e "${BLUE}  Block-STM Logging Demo${NC}"
    echo -e "${BLUE}================================${NC}"
    echo
}

print_usage() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  -d, --log-dir DIR        Log directory (default: $DEFAULT_LOG_DIR)"
    echo "  -l, --log-level LEVEL    Log level: DEBUG|INFO|WARN|ERROR (default: $DEFAULT_LOG_LEVEL)"
    echo "  -t, --txn-count COUNT    Number of transactions to simulate (default: $DEFAULT_TXN_COUNT)"
    echo "  -s, --max-size SIZE      Max log file size in MB (default: $DEFAULT_MAX_SIZE)"
    echo "  -c, --clean              Clean existing logs before running"
    echo "  -a, --analyze-only       Only analyze existing logs, don't run demo"
    echo "  -h, --help               Show this help message"
    echo
    echo "Examples:"
    echo "  $0                                    # Run with default settings"
    echo "  $0 -d /tmp/logs -l DEBUG -t 100      # Custom settings"
    echo "  $0 -c -t 200                         # Clean logs and run 200 transactions"
    echo "  $0 -a                                 # Only analyze existing logs"
}

check_dependencies() {
    echo -e "${YELLOW}Checking dependencies...${NC}"
    
    # Check if cargo is available
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: cargo is not installed or not in PATH${NC}"
        echo "Please install Rust and Cargo: https://rustup.rs/"
        exit 1
    fi
    
    # Check if jq is available (optional)
    if command -v jq &> /dev/null; then
        echo -e "${GREEN}✓ jq found - advanced log analysis available${NC}"
        JQ_AVAILABLE=true
    else
        echo -e "${YELLOW}⚠ jq not found - basic log analysis only${NC}"
        echo "  Install jq for advanced log analysis: brew install jq (macOS) or apt-get install jq (Ubuntu)"
        JQ_AVAILABLE=false
    fi
    
    echo
}

setup_environment() {
    echo -e "${YELLOW}Setting up environment...${NC}"
    
    # Export environment variables
    export BLOCK_STM_LOG_DIR="$LOG_DIR"
    export BLOCK_STM_LOG_LEVEL="$LOG_LEVEL"
    export BLOCK_STM_LOG_MAX_SIZE="$MAX_SIZE"
    export BLOCK_STM_LOG_DETAILED="true"
    export DEMO_TXN_COUNT="$TXN_COUNT"
    
    # Create log directory
    mkdir -p "$LOG_DIR"
    
    echo "Environment configured:"
    echo "  Log Directory: $LOG_DIR"
    echo "  Log Level: $LOG_LEVEL"
    echo "  Transaction Count: $TXN_COUNT"
    echo "  Max File Size: ${MAX_SIZE}MB"
    echo
}

clean_logs() {
    if [ -d "$LOG_DIR" ]; then
        echo -e "${YELLOW}Cleaning existing logs in $LOG_DIR...${NC}"
        rm -f "$LOG_DIR"/*.log
        echo -e "${GREEN}✓ Logs cleaned${NC}"
    fi
}

run_demo() {
    echo -e "${YELLOW}Building and running Block-STM logging demo...${NC}"
    
    # Build the project
    echo "Building project..."
    if ! cargo build --example block_stm_logging_example 2>/dev/null; then
        echo -e "${RED}Error: Failed to build the demo${NC}"
        echo "Make sure you're in the block-executor directory and all dependencies are available"
        exit 1
    fi
    
    echo -e "${GREEN}✓ Build successful${NC}"
    
    # Run the demo
    echo "Running demo with $TXN_COUNT transactions..."
    if ! cargo run --example block_stm_logging_example; then
        echo -e "${RED}Error: Demo execution failed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ Demo completed successfully${NC}"
    echo
}

analyze_logs() {
    echo -e "${BLUE}=== Log Analysis ===${NC}"
    
    if [ ! -d "$LOG_DIR" ]; then
        echo -e "${RED}Error: Log directory $LOG_DIR does not exist${NC}"
        return 1
    fi
    
    local log_files=(
        "block_stm_execution.log:Execution Events"
        "block_stm_concurrency.log:Concurrency Events"
        "block_stm_readwrite.log:Read/Write Events"
        "block_stm_performance.log:Performance Metrics"
        "block_stm_summary.log:Summary Events"
    )
    
    echo "Log file summary:"
    for entry in "${log_files[@]}"; do
        IFS=':' read -r filename description <<< "$entry"
        local filepath="$LOG_DIR/$filename"
        
        if [ -f "$filepath" ]; then
            local line_count=$(wc -l < "$filepath")
            echo -e "  ${GREEN}✓${NC} $description: $line_count events"
        else
            echo -e "  ${YELLOW}⚠${NC} $description: No events logged"
        fi
    done
    
    echo
    
    if [ "$JQ_AVAILABLE" = true ]; then
        advanced_analysis
    else
        basic_analysis
    fi
}

advanced_analysis() {
    echo -e "${BLUE}=== Advanced Analysis (using jq) ===${NC}"
    
    local exec_log="$LOG_DIR/block_stm_execution.log"
    local conc_log="$LOG_DIR/block_stm_concurrency.log"
    local perf_log="$LOG_DIR/block_stm_performance.log"
    
    if [ -f "$exec_log" ]; then
        echo "Transaction Statistics:"
        
        # Count transaction starts and finishes
        local starts=$(jq -r 'select(.event_type == "TransactionStart")' "$exec_log" 2>/dev/null | wc -l || echo "0")
        local finishes=$(jq -r 'select(.event_type == "TransactionFinish")' "$exec_log" 2>/dev/null | wc -l || echo "0")
        echo "  Transaction Starts: $starts"
        echo "  Transaction Finishes: $finishes"
        
        # Calculate average execution time
        if [ "$finishes" -gt 0 ]; then
            local avg_time=$(jq -r 'select(.event_type == "TransactionFinish") | .duration_us' "$exec_log" 2>/dev/null | \
                awk '{sum+=$1; count++} END {if(count>0) print sum/count; else print "0"}')
            echo "  Average Execution Time: ${avg_time} microseconds"
        fi
        
        # Show execution result distribution
        echo "  Execution Results:"
        jq -r 'select(.event_type == "TransactionFinish") | .result' "$exec_log" 2>/dev/null | \
            sort | uniq -c | while read count result; do
                echo "    $result: $count"
            done
    fi
    
    if [ -f "$conc_log" ]; then
        echo
        echo "Concurrency Statistics:"
        
        # Count aborts
        local aborts=$(jq -r 'select(.event_type == "TransactionAbort")' "$conc_log" 2>/dev/null | wc -l || echo "0")
        echo "  Transaction Aborts: $aborts"
        
        # Show abort reasons
        if [ "$aborts" -gt 0 ]; then
            echo "  Abort Reasons:"
            jq -r 'select(.event_type == "TransactionAbort") | .reason' "$conc_log" 2>/dev/null | \
                sort | uniq -c | while read count reason; do
                    echo "    $reason: $count"
                done
        fi
    fi
    
    if [ -f "$perf_log" ]; then
        echo
        echo "Performance Metrics:"
        local perf_count=$(wc -l < "$perf_log")
        echo "  Performance Events: $perf_count"
        
        if [ "$perf_count" -gt 0 ]; then
            echo "  Metric Types:"
            jq -r '.metric_name' "$perf_log" 2>/dev/null | \
                sort | uniq -c | while read count metric; do
                    echo "    $metric: $count"
                done
        fi
    fi
}

basic_analysis() {
    echo -e "${BLUE}=== Basic Analysis ===${NC}"
    
    echo "Log file contents (first 5 lines of each):"
    
    for logfile in "$LOG_DIR"/*.log; do
        if [ -f "$logfile" ]; then
            echo
            echo -e "${YELLOW}$(basename "$logfile"):${NC}"
            head -n 5 "$logfile" | while IFS= read -r line; do
                echo "  $line"
            done
            
            local total_lines=$(wc -l < "$logfile")
            if [ "$total_lines" -gt 5 ]; then
                echo "  ... and $((total_lines - 5)) more lines"
            fi
        fi
    done
}

show_analysis_commands() {
    echo
    echo -e "${BLUE}=== Manual Analysis Commands ===${NC}"
    echo "You can analyze the logs manually using these commands:"
    echo
    
    if [ "$JQ_AVAILABLE" = true ]; then
        echo "Using jq (recommended):"
        echo "  # Count transaction starts:"
        echo "  jq 'select(.event_type == \"TransactionStart\")' $LOG_DIR/block_stm_execution.log | wc -l"
        echo
        echo "  # Show all abort events:"
        echo "  jq 'select(.event_type == \"TransactionAbort\")' $LOG_DIR/block_stm_concurrency.log"
        echo
        echo "  # Calculate average execution time:"
        echo "  jq -r 'select(.event_type == \"TransactionFinish\") | .duration_us' $LOG_DIR/block_stm_execution.log | awk '{sum+=\$1; count++} END {print \"Average: \" sum/count \" microseconds\"}'"
        echo
        echo "  # Show performance metrics:"
        echo "  jq -r '.metric_name + \": \" + (.value | tostring)' $LOG_DIR/block_stm_performance.log"
        echo
    fi
    
    echo "Using basic shell commands:"
    echo "  # Count total events in each log:"
    echo "  wc -l $LOG_DIR/*.log"
    echo
    echo "  # Search for specific events:"
    echo "  grep 'TransactionAbort' $LOG_DIR/block_stm_concurrency.log"
    echo
    echo "  # View recent events:"
    echo "  tail -f $LOG_DIR/block_stm_execution.log"
}

# Parse command line arguments
LOG_DIR="$DEFAULT_LOG_DIR"
LOG_LEVEL="$DEFAULT_LOG_LEVEL"
TXN_COUNT="$DEFAULT_TXN_COUNT"
MAX_SIZE="$DEFAULT_MAX_SIZE"
CLEAN_LOGS=false
ANALYZE_ONLY=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -d|--log-dir)
            LOG_DIR="$2"
            shift 2
            ;;
        -l|--log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        -t|--txn-count)
            TXN_COUNT="$2"
            shift 2
            ;;
        -s|--max-size)
            MAX_SIZE="$2"
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
            echo -e "${RED}Error: Unknown option $1${NC}"
            print_usage
            exit 1
            ;;
    esac
done

# Main execution
print_header

if [ "$ANALYZE_ONLY" = false ]; then
    check_dependencies
    setup_environment
    
    if [ "$CLEAN_LOGS" = true ]; then
        clean_logs
    fi
    
    run_demo
fi

analyze_logs
show_analysis_commands

echo
echo -e "${GREEN}Demo completed successfully!${NC}"
echo -e "Logs are available in: ${BLUE}$LOG_DIR${NC}"