#!/bin/bash
default_filename="./result/eth_historical.log"
filename=${1:-$default_filename}
count=$(./scripts/cpu_info.sh | awk '{print NF}')

echo "run eth historical transfer tasks" > $filename

for ((i=1; i<=$count; i++)); do
    cores=$(./scripts/cpu_info.sh|cut -d ' ' -f$i)
    # Count the number of cores by counting commas and adding 1
    core_count=$(echo $cores | tr ',' '\n' | wc -l)
    # Ensure core_count is at least 1
    if [ $core_count -eq 0 ]; then
        core_count=1
    fi
    echo "============================== running with cpu cores: (${cores}) count: ${core_count} ==============================" >> $filename
    cargo run --release -- replay-erc20 --data-path "./data/ETH_2404_100000.csv" --concurrency-level $core_count >> $filename
    echo "" >> $filename
    echo "" >> $filename
    sleep 5s
done

echo "============================== finalize ==============================" >> $filename

parallel_tps_data=($(cat $filename | grep "Parallel execution finishes" | awk -F '=' '{print $2}'))
parallel_tps=$(IFS=,; echo "${parallel_tps_data[*]}")

speed_up_data=()
for num in "${parallel_tps_data[@]}"; do
    result=$(bc <<< "scale=2; $num / ${parallel_tps_data[0]}")
    formatted_result=$(printf "%.2f" $result)
    speed_up_data+=("$formatted_result")
done
speed_up=$(IFS=,; echo "${speed_up_data[*]}")

sequential_tps_data=($(cat $filename | grep "Sequential execution finishes" | awk -F '=' '{print $2}'))
sequential_tps=$(IFS=,; echo "${sequential_tps_data[*]}")

echo "parallel execution tps: [$parallel_tps]" >> $filename
echo "speed up: [$speed_up]" >> $filename
echo "sequential execution tps: [$sequential_tps]" >> $filename
