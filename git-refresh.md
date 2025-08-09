# Git 合并计划 - git-refresh.md

## 概述
将上游 aptos-core 仓库的最新更新（commit 570ec4a79ea5b1e30fc6be7fe619275689c0402d）与本地的 block-stm 日志系统修改进行合并。

## 当前状态分析
- **当前分支**: `eval_modified` 
- **本地修改**: Block-STM日志系统升级，包括warning修复、suspend→stall字段重命名、计数器插桩等
- **上游更新**: 需要合并 commit 570ec4a79ea5b1e30fc6be7fe619275689c0402d 的变更

## Git 合并计划

### 第一阶段：备份和准备
```bash
# 1. 确认当前状态
git status
git log --oneline -10

# 2. 创建当前工作的备份分支
git checkout -b eval_modified_backup_$(date +%Y%m%d_%H%M%S)
git checkout eval_modified

# 3. 确认远程仓库配置
git remote -v
# 如果没有上游remote，添加：
# git remote add upstream https://github.com/aptos-labs/aptos-core.git
```

### 第二阶段：获取上游更新
```bash
# 4. 获取上游最新更新
git fetch upstream

# 5. 检查目标commit是否存在
git show 570ec4a79ea5b1e30fc6be7fe619275689c0402d --stat

# 6. 创建临时分支查看上游变更
git checkout -b temp_upstream_check upstream/main
git log --oneline -20 | grep -E "570ec4a|$(echo 570ec4a79ea5b1e30fc6be7fe619275689c0402d | cut -c1-7)"
```

### 第三阶段：准备合并
```bash
# 7. 回到工作分支
git checkout eval_modified

# 8. 创建合并分支
git checkout -b eval_modified_merge_$(date +%Y%m%d)

# 9. 查看即将合并的变更差异
git log --oneline HEAD..570ec4a79ea5b1e30fc6be7fe619275689c0402d --max-count=50
```

### 第四阶段：执行合并
```bash
# 10. 执行合并（可能遇到冲突）
git merge 570ec4a79ea5b1e30fc6be7fe619275689c0402d

# 如果有冲突，继续下面的步骤
# 11. 查看冲突文件
git status

# 12. 解决冲突（重点关注我们修改过的文件）
# 主要冲突可能出现在：
# - aptos-move/block-executor/src/executor.rs
# - aptos-move/block-executor/src/scheduler_status.rs
# - aptos-move/block-executor/src/scheduler_v2.rs
# - aptos-move/aptos-transaction-benchmarks/src/simulator.rs
# - aptos-move/aptos-transaction-benchmarks/src/main.rs
```

### 第五阶段：冲突解决策略

#### 关键修改保留清单：
```markdown
1. **executor.rs 修改**:
   - #[allow(dead_code)] 注解
   - DEPENDENCY_WAIT_SECONDS stall计数修复
   - TaskKind::NextTask中的stall记录

2. **scheduler_status.rs 修改**:
   - #[allow(dead_code)] 注解
   - counters模块导入
   - add_stall中的stall事件记录注释

3. **scheduler_v2.rs 修改**:
   - #[allow(dead_code)] 注解

4. **simulator.rs 修改**:
   - DetailedExecutionMetrics结构体中suspend→stall字段重命名
   - 所有函数中的stall计算和打印逻辑
   - 日志系统集成代码

5. **main.rs 修改**:
   - --num-warmups参数移除
   - stall字段引用修复

6. **mvhashmap相关修改**:
   - DelayedFieldID warning注释化
```

### 第六阶段：验证和测试
```bash
# 13. 完成冲突解决后
git add .
git commit -m "Merge upstream 570ec4a79ea5b1e30fc6be7fe619275689c0402d with block-stm logger enhancements

- Keep block-stm logger system upgrades
- Maintain suspend→stall field renaming
- Preserve warning fixes and dead_code annotations
- Retain stall counter instrumentation fixes"

# 14. 验证编译
cargo check

# 15. 运行基础测试
cargo run --release -- replay-erc20 --data-path data/ETH_2401_100.csv --concurrency-level 4 --num-runs 1
```

### 第七阶段：最终整理
```bash
# 16. 如果测试通过，合并回主工作分支
git checkout eval_modified
git merge eval_modified_merge_$(date +%Y%m%d)

# 17. 清理临时分支
git branch -d temp_upstream_check
git branch -d eval_modified_merge_$(date +%Y%m%d)

# 18. 更新远程分支（如果需要）
# git push origin eval_modified
```

## 潜在冲突点和解决策略

### 高风险冲突区域
1. **Cargo.toml/Cargo.lock**: 依赖版本冲突
   - 策略：优先使用上游版本，除非我们添加了新依赖
   
2. **executor.rs**: 核心执行逻辑变更
   - 策略：保留我们的stall计数器修复，合并上游功能改进
   
3. **scheduler相关文件**: 调度逻辑更新
   - 策略：保留dead_code注解和stall机制修复

### 低风险区域
1. **simulator.rs**: 我们的主要修改区域
   - 策略：保留所有我们的修改，除非上游有直接冲突的功能更新

2. **日志系统文件**: 我们新增的功能
   - 策略：完全保留我们的实现

## 回滚计划
如果合并出现严重问题：
```bash
# 紧急回滚到合并前状态
git reset --hard eval_modified_backup_[timestamp]

# 或者重新开始
git checkout eval_modified_backup_[timestamp]
git checkout -b eval_modified_fresh_start
```

## 验证检查清单
合并完成后验证：
- [ ] 编译成功 (`cargo check`)
- [ ] stall指标正确显示 (非0值)
- [ ] 日志系统功能正常
- [ ] 基准测试运行正常
- [ ] 所有warning已修复
- [ ] suspend→stall字段重命名完整

## 注意事项

1. **保持我们的核心修改**: 所有block-stm logger功能必须保留
2. **谨慎处理核心执行逻辑**: executor.rs的修改需要仔细验证
3. **测试先行**: 每步完成后都要进行编译和基础测试
4. **文档更新**: 如果上游有新的API变化，可能需要更新我们的集成代码

---
*执行此计划前，建议先在测试环境中进行试运行*