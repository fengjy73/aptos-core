# 分离式Stall统计设计规范

## 创建时间
2025-08-22 08:40

## 设计目标

实现Transaction-Level Stall和System-Level Stall的分离统计，提供清晰的性能监控数据。

## 1. 数据结构设计

### 1.1 核心统计结构
```rust
#[derive(Debug)]
pub struct SeparatedStallStatistics {
    // Transaction-Level Stall统计
    pub transaction_stalls: AtomicU32,
    pub transaction_stall_duration_total_us: AtomicU64,
    pub transaction_stall_count_by_txn: DashMap<u32, u32>, // txn_id -> stall_count
    
    // System-Level Stall统计  
    pub system_stalls: AtomicU32,
    pub system_stall_duration_total_us: AtomicU64,
    pub system_stall_count_by_worker: DashMap<u32, u32>, // worker_id -> stall_count
    
    // 综合统计
    pub total_stall_events: AtomicU32,
    pub stall_statistics_start_time: std::time::Instant,
}
```

### 1.2 SystemStall事件结构
```rust
#[derive(Debug, Clone, Serialize)]
pub struct SystemStallEvent {
    pub event_type: String,           // "SystemStallStart" | "SystemStallEnd"
    pub timestamp_us: u64,
    pub thread_id: u64,
    pub worker_id: u32,
    pub total_workers: u32,
    pub reason: String,               // "NextTask_no_available_work"
    pub scheduler_state: String,      // "ALL_TRANSACTIONS_WAITING"
    pub system_stall_count_before: u32,
    pub system_stall_count_after: u32,
    pub stall_transition: String,     // "ACTIVE_TO_STALLED" | "STALLED_TO_ACTIVE"
}
```

### 1.3 改进的TransactionStall事件结构
```rust
#[derive(Debug, Clone, Serialize)]
pub struct TransactionStallEvent {
    pub event_type: String,           // "TransactionStallAdd" | "TransactionStallRemove"
    pub timestamp_us: u64,
    pub thread_id: u64,
    pub txn_id: u32,
    pub incarnation: u32,
    pub by_tx: u32,
    pub transaction_stall_count_before: u32,
    pub transaction_stall_count_after: u32,
    pub first_stall: Option<bool>,    // 只在StallAdd时存在
    pub became_unstalled: Option<bool>, // 只在StallRemove时存在
    pub stall_transition: String,     // "UNSTALLED_TO_STALLED" | "STALLED_TO_UNSTALLED"
}
```

## 2. API设计

### 2.1 BlockSTMLogger新增方法
```rust
impl BlockSTMLogger {
    // Transaction Stall API
    pub fn log_transaction_stall_add(&self, txn_id: u32, incarnation: u32, by_tx: u32, first_stall: bool);
    pub fn log_transaction_stall_remove(&self, txn_id: u32, incarnation: u32, by_tx: u32, became_unstalled: bool);
    pub fn increment_transaction_stalls(&self) -> u32;
    pub fn record_transaction_stall_duration(&self, duration_us: u64);
    
    // System Stall API
    pub fn log_system_stall_start(&self, worker_id: u32, total_workers: u32, reason: &str);
    pub fn log_system_stall_end(&self, worker_id: u32, total_workers: u32, reason: &str);
    pub fn increment_system_stalls(&self) -> u32;
    pub fn record_system_stall_duration(&self, duration_us: u64);
    
    // 统计查询API
    pub fn get_transaction_stall_count(&self) -> u32;
    pub fn get_system_stall_count(&self) -> u32;
    pub fn get_total_stall_count(&self) -> u32;
    pub fn get_stall_statistics_summary(&self) -> StallStatisticsSummary;
}
```

### 2.2 统计摘要结构
```rust
#[derive(Debug, Clone, Serialize)]
pub struct StallStatisticsSummary {
    pub transaction_stalls: u32,
    pub system_stalls: u32,
    pub total_stalls: u32,
    pub transaction_stall_avg_duration_us: f64,
    pub system_stall_avg_duration_us: f64,
    pub overall_stall_avg_duration_us: f64,
    pub transaction_stall_percentage: f64,
    pub system_stall_percentage: f64,
}
```

## 3. 日志文件输出格式

### 3.1 文件结构
```
test_logs_xxx/
├── transaction_stalls.ndjson     # Transaction-Level stall事件
├── system_stalls.ndjson          # System-Level stall事件  
├── stall_statistics_summary.json # 综合统计摘要
└── block_stm_execution.log       # 保持原有的综合日志
```

### 3.2 Transaction Stall日志示例
```json
{"event_type":"TransactionStallAdd","timestamp_us":1755849588950537,"thread_id":3320665455366264189,"txn_id":23,"incarnation":1,"by_tx":22,"transaction_stall_count_before":0,"transaction_stall_count_after":1,"first_stall":true,"stall_transition":"UNSTALLED_TO_STALLED"}
{"event_type":"TransactionStallRemove","timestamp_us":1755849588951143,"thread_id":3673300442962989464,"txn_id":23,"incarnation":2,"by_tx":22,"transaction_stall_count_before":1,"transaction_stall_count_after":0,"became_unstalled":true,"stall_transition":"STALLED_TO_UNSTALLED"}
```

### 3.3 System Stall日志示例  
```json
{"event_type":"SystemStallStart","timestamp_us":1755849588955000,"thread_id":12318721104400761032,"worker_id":2,"total_workers":4,"reason":"NextTask_no_available_work","scheduler_state":"ALL_TRANSACTIONS_WAITING","system_stall_count_before":0,"system_stall_count_after":1,"stall_transition":"ACTIVE_TO_STALLED"}
{"event_type":"SystemStallEnd","timestamp_us":1755849588956500,"thread_id":12318721104400761032,"worker_id":2,"total_workers":4,"reason":"NewTask_available","scheduler_state":"TASK_DISPATCHED","system_stall_count_before":1,"system_stall_count_after":0,"stall_transition":"STALLED_TO_ACTIVE"}
```

### 3.4 统计摘要示例
```json
{
  "execution_summary": {
    "total_transactions": 100000,
    "concurrency_level": 4,
    "execution_time_ms": 5432
  },
  "stall_statistics": {
    "transaction_stalls": 3,
    "system_stalls": 96, 
    "total_stalls": 99,
    "transaction_stall_avg_duration_us": 640.26,
    "system_stall_avg_duration_us": 125.43,
    "overall_stall_avg_duration_us": 145.67,
    "transaction_stall_percentage": 3.03,
    "system_stall_percentage": 96.97
  },
  "performance_insights": {
    "primary_bottleneck": "system_scheduling",
    "transaction_conflict_level": "low",
    "scheduler_efficiency": "needs_optimization"
  }
}
```

## 4. 程序输出格式设计

### 4.1 详细统计输出
```
执行统计摘要:
执行次数: 133, 验证次数: 366, 中止次数: 33
停滞统计详情:
  - 交易停滞次数: 3 (3.03%)
  - 系统停滞次数: 96 (96.97%) 
  - 总停滞次数: 99
性能分析:
  - 交易平均停滞时间: 640.26 us
  - 系统平均停滞时间: 125.43 us  
  - 整体平均停滞时间: 145.67 us
  - 总停滞时间: 2920.79 us
```

### 4.2 简化统计输出（兼容性）
```
执行次数:133, 验证次数:366, 中止次数:33, 停滞次数:99 (交易:3, 系统:96), 平均停滞时间:145.67 us, 总停滞时间:2920.79 us
```

## 5. 实现策略

### 5.1 向后兼容性
- 保持现有的`increment_stall_events()`方法，但标记为deprecated
- 现有的stall_events.ndjson文件将包含两种类型的事件
- 添加新的分类日志文件作为补充

### 5.2 渐进式迁移
1. **阶段1**: 添加新的统计结构和API，保持现有逻辑不变
2. **阶段2**: 修改executor.rs中的System stall记录
3. **阶段3**: 更新日志输出格式
4. **阶段4**: 完善测试和文档

### 5.3 性能考虑
- 使用原子操作避免锁竞争
- DashMap提供高效的并发哈希表
- 批量写入日志减少I/O开销

## 6. 测试验证方案

### 6.1 单元测试
- 测试各种stall事件的正确分类
- 验证统计计数的准确性
- 检查日志格式的正确性

### 6.2 集成测试  
- 使用简单的依赖链测试Transaction stall
- 使用高并发场景测试System stall
- 验证混合工作负载下的统计准确性

### 6.3 性能测试
- 确保新的日志记录不影响执行性能
- 验证分离统计的额外开销在可接受范围内

## 7. 文档更新

需要更新的文档：
- CLAUDE.md中的日志文件说明
- cookbook中的stall机制解释
- 性能分析指南中的stall统计解读

通过这种设计，我们能够：
1. 清晰区分两种不同类型的性能瓶颈
2. 提供更精确的性能分析数据
3. 保持良好的向后兼容性
4. 支持未来的性能优化工作