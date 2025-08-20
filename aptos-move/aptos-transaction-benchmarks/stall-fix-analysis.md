# Stall时间计算修复分析

## 问题识别

在Block-STM执行过程中，观察到stall时间显示为整数值：
```
avg_stall_time:1000.00 us, stall_time_total:16000.00 us
```

这表明计算存在问题，因为真实的等待时间应该有更多精度。

## 根本原因分析

通过代码分析发现问题在`executor.rs`中：

```rust
// 问题代码（已修复）
counters::DEPENDENCY_WAIT_SECONDS.observe(0.001); // 硬编码1ms
```

### 问题机制：
1. **真实计时**: `view.rs`中正确使用`start_timer()`记录实际依赖等待时间
2. **污染数据**: `executor.rs`中每次`NextTask`都硬编码添加1ms（0.001秒）
3. **统计偏差**: 硬编码值污染了真实的等待时间统计

## 修复方案

### 1. 删除硬编码值
```rust
// 修复前
counters::DEPENDENCY_WAIT_SECONDS.observe(0.001); // 1ms to represent stall period

// 修复后  
// NextTask indicates no tasks are available due to dependencies or completion
// Don't artificially inflate dependency wait statistics with fixed values
// Real dependency wait times are recorded in view.rs with start_timer()
```

### 2. 保留真实计时
保持`view.rs`中的正确实现：
```rust
let _timer = counters::DEPENDENCY_WAIT_SECONDS.start_timer();
// 等待依赖解决...
// timer在作用域结束时自动记录真实时间
```

## 修复效果

### 修复前：
- 每个stall事件贡献固定的1ms
- 16个stall事件 = 16ms总时间
- 平均时间 = 16ms / 16 = 1ms = 1000μs
- 显示为整数倍的1000μs

### 修复后：
- 只记录真实的依赖等待时间
- 时间精度应该显示为微秒级别的小数
- 更准确反映Block-STM的实际等待特性

## Block-STM特性影响

### 乐观执行特点：
- **短暂依赖**: 大多数依赖等待应该很短（微秒级）
- **冲突解决**: 依赖解决通常很快，除非有严重冲突
- **并发效率**: 真实等待时间是衡量并发效率的关键指标

### 预期结果：
修复后应该看到类似：
```
avg_stall_time:23.45 us, stall_time_total:1234.56 us
```

## 验证方法

运行基准测试并观察输出：
```bash
BLOCK_STM_LOG_LEVEL=DEBUG BLOCK_STM_LOG_DIR=./test_logs_stall_fixed \
cargo run --release -- replay-erc20 \
--data-path data/ETH_2401_100.csv --concurrency-level 4 --num-runs 1
```

观察是否：
1. stall时间不再是1000的整数倍
2. 时间精度提升到小数位
3. 总时间与平均时间的关系更合理

## 相关文件修改

- `executor.rs:1859`: 删除硬编码的`observe(0.001)`调用
- 保持`view.rs:486`: 真实的`start_timer()`计时
- 保持`counters.rs:168`: DEPENDENCY_WAIT_SECONDS计数器定义

这个修复确保了Block-STM依赖等待统计的准确性，为性能分析提供更可靠的数据基础。