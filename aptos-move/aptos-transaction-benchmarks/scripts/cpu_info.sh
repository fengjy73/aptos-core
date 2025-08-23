#!/bin/bash

# macOS compatible version using sysctl
if [[ "$OSTYPE" == "darwin"* ]]; then
    # Get number of physical cores
    physical_cores=$(sysctl -n hw.physicalcpu)
    # Get number of logical cores
    logical_cores=$(sysctl -n hw.logicalcpu)
    
    # Create array of core numbers (0 to physical_cores-1)
    selected_logical_cores=()
    for ((i=0; i<$physical_cores; i++)); do
        selected_logical_cores+=($i)
    done
else
    # Linux version using lscpu
    cpu_info=$(lscpu --all --extended)
    
    declare -A physical_to_logical
    
    while IFS= read -r line; do
        if [[ $line =~ ^CPU ]]; then
            continue
        fi
        logical_id=$(echo $line | awk '{print $1}')
        core_id=$(echo $line | awk '{print $4}')
        physical_to_logical["$core_id"]+="$logical_id "
    done <<< "$cpu_info"
    
    selected_logical_cores=()
    for key in "${!physical_to_logical[@]}"; do
        logical_cores=(${physical_to_logical[$key]})
        selected_logical_cores+=(${logical_cores[0]})
    done
    
    IFS=$'\n' selected_logical_cores=($(sort -n <<<"${selected_logical_cores[*]}"))
    unset IFS
fi

length=${#selected_logical_cores[@]}
power=1

while [ $((2 ** power)) -le $length ]; do
    power=$((power + 1))
done

power=$((power - 1))
result=()

for ((i=0; i<=power; i++)); do
    limit=$((2 ** i))
    subarray=(${selected_logical_cores[@]:0:$limit})
    result+=("$(IFS=,; echo "${subarray[*]}")") 
done

echo "${result[*]}"

