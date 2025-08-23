#!/bin/bash
default_filename="./result/eth_historical_single.log"
filename=${1:-$default_filename}

echo "run eth historical transfer tasks (single core)" > $filename

echo "============================== running with single cpu core ==============================" >> $filename
cargo run --release -- replay-erc20 --data-path "./data/ETH_2404_100000.csv"  >> $filename
echo "" >> $filename
echo "" >> $filename

echo "============================== finalize ==============================" >> $filename

parallel_tps_data=($(cat $filename | grep "Parallel execution finishes" | awk -F '=' '{print $2}'))
parallel_tps=$(IFS=,; echo "${parallel_tps_data[*]}")

sequential_tps_data=($(cat $filename | grep "Sequential execution finishes" | awk -F '=' '{print $2}'))
sequential_tps=$(IFS=,; echo "${sequential_tps_data[*]}")

echo "parallel execution tps: [$parallel_tps]" >> $filename
echo "sequential execution tps: [$sequential_tps]" >> $filename