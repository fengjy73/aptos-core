#!/bin/bash

# Get the data directory path
data_dir="./data"
result_dir="./result"
temp_dir="./temp_data"

# Create result and temp directories if they don't exist
mkdir -p $result_dir
mkdir -p $temp_dir

# Get CPU core count information
count=$(./scripts/cpu_info.sh | awk '{print NF}')

# Define transaction counts to test
transaction_counts=(10 100 1000 10000 100000)

echo "Starting historical data replay tests with different transaction window sizes..."
echo "Found $(ls $data_dir/*.csv | wc -l) CSV files to process"
echo "Transaction counts to test: ${transaction_counts[*]}"
echo ""

# Function to detect operating system and choose appropriate random sampling method
detect_random_command() {
    if command -v shuf >/dev/null 2>&1; then
        echo "shuf"
    elif [[ "$(uname)" == "Darwin" ]] || command -v sort >/dev/null 2>&1; then
        echo "sort"
    else
        echo "none"
    fi
}

# Function to randomly sample lines from CSV file
random_sample_csv() {
    local input_file="$1"
    local output_file="$2"
    local sample_size="$3"
    
    echo "Sampling $sample_size transactions from $input_file to $output_file"
    
    # Check if input file exists
    if [ ! -f "$input_file" ]; then
        echo "Error: Input file $input_file does not exist"
        return 1
    fi
    
    # Get header line
    head -n 1 "$input_file" > "$output_file"
    
    # Get total number of data lines (excluding header)
    local total_lines=$(tail -n +2 "$input_file" | wc -l | tr -d ' ')
    echo "Total data lines available: $total_lines"
    
    # If sample size is greater than or equal to total lines, just copy all data
    if [ $sample_size -ge $total_lines ]; then
        echo "Sample size >= total lines, copying all data"
        tail -n +2 "$input_file" >> "$output_file"
    else
        # Randomly sample lines from the data (excluding header)
        echo "Randomly sampling $sample_size lines from $total_lines total lines"
        
        # Detect the best random sampling method for current system
        local random_cmd=$(detect_random_command)
        
        case $random_cmd in
            "shuf")
                echo "Using shuf command (Linux/Ubuntu system detected)"
                tail -n +2 "$input_file" | shuf -n $sample_size >> "$output_file"
                ;;
            "sort")
                echo "Using sort -R command (macOS system or shuf not available)"
                tail -n +2 "$input_file" | sort -R | head -n $sample_size >> "$output_file"
                ;;
            "none")
                echo "Error: Neither shuf nor sort command is available for random sampling"
                return 1
                ;;
        esac
    fi
    
    # Verify the output file
    local output_lines=$(tail -n +2 "$output_file" | wc -l | tr -d ' ')
    echo "Output file contains $output_lines data lines"
    
    return 0
}

# Loop through all CSV files in the data directory
for csv_file in $data_dir/*.csv; do
    # Extract filename without path and extension
    basename_file=$(basename "$csv_file" .csv)
    
    echo "Processing: $csv_file"
    echo ""
    
    # Loop through different transaction counts
    for tx_count in "${transaction_counts[@]}"; do
        echo "Testing with $tx_count transactions..."
        
        # Create temporary CSV file with sampled data
        temp_csv="$temp_dir/${basename_file}_${tx_count}.csv"
        echo "Creating temporary file: $temp_csv"
        random_sample_csv "$csv_file" "$temp_csv" $tx_count
        
        # Check if sampling was successful
        if [ ! -f "$temp_csv" ] || [ $(tail -n +2 "$temp_csv" | wc -l | tr -d ' ') -eq 0 ]; then
            echo "Error: Failed to create valid sample file for $tx_count transactions"
            continue
        fi
        
        # Create log filename: window + csv filename + transaction count + .log
        log_filename="$result_dir/window_${basename_file}_${tx_count}.log"
        
        echo "Output log: $log_filename"
        
        # Initialize log file
        echo "run historical transfer tasks for $basename_file with $tx_count transactions" > $log_filename
        echo "Original data file: $csv_file" >> $log_filename
        echo "Sampled data file: $temp_csv" >> $log_filename
        echo "Transaction count: $tx_count" >> $log_filename
        echo "" >> $log_filename
        
        # Run tests with different CPU core configurations
        for ((i=1; i<=$count; i++)); do
            cores=$(./scripts/cpu_info.sh|cut -d ' ' -f$i)
            # Count the number of cores by counting commas and adding 1
            core_count=$(echo $cores | tr ',' '\n' | wc -l)
            # Ensure core_count is at least 1
            if [ $core_count -eq 0 ]; then
                core_count=1
            fi
            echo "============================== running with cpu cores: (${cores}) count: ${core_count}, transactions: ${tx_count} ==============================" >> $log_filename
            cargo run --release -- replay-erc20 --data-path "$temp_csv" --concurrency-level $core_count >> $log_filename
            echo "" >> $log_filename
            echo "" >> $log_filename
            sleep 3s
        done
        
        echo "============================== finalize ==============================" >> $log_filename
        
        # Extract and process performance data
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
        
        echo "Completed testing $tx_count transactions for $basename_file"
        echo "Results saved to: $log_filename"
        echo "----------------------------------------"
        echo ""
        
        # Clean up temporary file
        rm -f "$temp_csv"
    done
    
    echo "Completed all transaction count tests for: $basename_file"
    echo "========================================"
    echo ""
done

# Clean up temp directory
rmdir $temp_dir 2>/dev/null

echo "All CSV files and transaction counts have been processed!"
echo "Log files are saved in the $result_dir directory with naming pattern: window_[csv_filename]_[transaction_count].log"