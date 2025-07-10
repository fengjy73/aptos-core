#!/bin/bash

# Get the data directory path
data_dir="./data"
result_dir="./result"

# Create result directory if it doesn't exist
mkdir -p $result_dir

# Get CPU core count information
count=$(./scripts/cpu_info.sh | awk '{print NF}')

echo "Starting historical data replay tests for all CSV files..."
echo "Found $(ls $data_dir/*.csv | wc -l) CSV files to process"
echo ""

# Loop through all CSV files in the data directory
for csv_file in $data_dir/*.csv; do
    # Extract filename without path and extension
    basename_file=$(basename "$csv_file" .csv)
    
    # Create log filename: historical + csv filename + .log
    log_filename="$result_dir/historical_${basename_file}.log"
    
    echo "Processing: $csv_file"
    echo "Output log: $log_filename"
    echo ""
    
    # Initialize log file
    echo "run historical transfer tasks for $basename_file" > $log_filename
    echo "Data file: $csv_file" >> $log_filename
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
        echo "============================== running with cpu cores: (${cores}) count: ${core_count} ==============================" >> $log_filename
        cargo run --release -- replay-erc20 --data-path "$csv_file" --concurrency-level $core_count >> $log_filename
        echo "" >> $log_filename
        echo "" >> $log_filename
        sleep 5s
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
    
    echo "Completed processing: $basename_file"
    echo "Results saved to: $log_filename"
    echo "----------------------------------------"
    echo ""
done

echo "All CSV files have been processed!"
echo "Log files are saved in the $result_dir directory with naming pattern: historical_[csv_filename].log"