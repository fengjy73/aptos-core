# 文档4：编译运行与环境配置指南 - 详细编写计划

## 创建时间

2025-08-22 14:35

## 文档基本信息

- **目标文档**:`20250822-1430-compilation-and-deployment-guide.md`
- **重点**: 环境配置、编译运行、大文件管理、实用操作指南

## 章节结构规划

### 第一章：环境准备与配置 (2-3页)

#### 1.1 不同平台的Rust环境配置

**支持平台**:

- **Linux** (Ubuntu 20.04+, CentOS 8+)
- **macOS** (10.15+, Apple Silicon支持)
- **Windows** (WSL2推荐)

**Rust工具链安装**:

```bash
# 官方推荐安装方式
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 版本要求
rustc --version  # 需要1.70.0+
cargo --version  # 需要对应版本
```

**平台特定配置**:

- **Linux**: 系统依赖包安装
- **macOS**: Xcode Command Line Tools配置
- **Windows**: WSL2环境设置和性能优化

#### 1.2 依赖库安装与版本要求

**系统级依赖**:

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev

# macOS (使用Homebrew)  
brew install pkg-config openssl

# CentOS/RHEL
sudo yum groupinstall -y "Development Tools"
sudo yum install -y openssl-devel pkg-config
```

**Rust crate依赖分析**:
从 `Cargo.toml`文件解析关键依赖：

- `clap` - 命令行参数解析
- `rayon` - 并行计算框架
- `serde` - 序列化框架
- `tokio` - 异步运行时 (如果使用)

#### 1.3 开发环境最佳实践

**IDE推荐配置**:

- **VS Code** + rust-analyzer插件
- **IntelliJ IDEA** + Rust插件
- **Vim/Neovim** + rust.vim

**开发工具链**:

```bash
# 代码格式化
rustup component add rustfmt

# 静态分析
rustup component add clippy

# 文档生成
cargo doc --no-deps --open
```

### 第二章：编译配置与优化 (2页)

#### 2.1 Cargo配置优化

**项目级配置** (`.cargo/config.toml`):

```toml
[build]
# 优化编译性能
jobs = 8                    # 并行编译任务数
rustc-wrapper = "sccache"   # 编译缓存 (可选)

[profile.release]
# 发布版本优化
lto = true                  # 链接时优化
codegen-units = 1          # 代码生成单元
panic = "abort"            # panic处理方式
```

**全局配置** (`~/.cargo/config.toml`):

```toml
[net]
retry = 3
git-fetch-with-cli = true

[registries.crates-io]
protocol = "sparse"        # 使用稀疏索引加速
```

#### 2.2 编译参数调优

**性能优化编译**:

```bash
# 发布版本编译 (推荐用于性能测试)
cargo build --release

# 启用CPU特定优化
RUSTFLAGS="-C target-cpu=native" cargo build --release

# 内存优化编译 (适用于内存受限环境)
cargo build --release --config profile.release.opt-level='"s"'
```

**调试版本编译**:

```bash
# 快速编译用于开发调试
cargo build

# 启用调试符号
cargo build --config profile.dev.debug=true
```

#### 2.3 Target目录管理策略

**目录结构优化**:

```
project/
├── Cargo.toml
├── src/
├── target/          # 本地target (可删除)
└── .cargo/
    └── config.toml  # 重定向target配置
```

**磁盘空间管理**:

```bash
# 清理编译产物
cargo clean

# 清理所有项目的编译缓存
cargo cache --autoclean
```

### 第三章：单次测试运行指南 (2-3页)

#### 3.1 基本命令格式与参数说明

**核心命令模板**:

```bash
cd aptos-core/aptos-move/aptos-transaction-benchmarks

BLOCK_STM_LOG_LEVEL=DEBUG \
BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S) \
cargo run --release -- replay-erc20 \
    --data-path data/ETH_2401_100.csv \
    --concurrency-level 4 \
    --num-runs 1 \
    --num-warmups 0
```

**参数详细说明**:

- `--data-path`: CSV历史数据文件路径
  - 支持相对路径和绝对路径
  - 文件格式要求: CSV with specific columns
- `--concurrency-level`: 并行度设置
  - 推荐值: CPU核心数的50%-100%
  - 影响: 内存使用和性能表现
- `--num-runs`: 运行次数
  - 性能测试建议: 3-5次取平均值
  - 开发调试建议: 1次即可
- `--num-warmups`: 预热次数
  - JIT优化考虑: 1-2次预热
  - 冷启动测试: 0次预热

#### 3.2 环境变量配置详解

**日志控制变量**:

```bash
# 日志级别 (影响输出详细程度和性能)
BLOCK_STM_LOG_LEVEL=DEBUG    # 最详细，性能影响较大
BLOCK_STM_LOG_LEVEL=INFO     # 平衡选择
BLOCK_STM_LOG_LEVEL=WARN     # 仅警告和错误
BLOCK_STM_LOG_LEVEL=ERROR    # 仅错误信息

# 输出目录 (建议使用时间戳避免冲突)
BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S)
BLOCK_STM_LOG_DIR=/mist/block_stm_logs/$(whoami)

# 文件大小限制 (防止单个日志文件过大)
BLOCK_STM_LOG_MAX_SIZE=100   # 100MB限制
```

**性能相关变量**:

```bash
# Rust运行时配置
RUST_MIN_STACK=8388608       # 8MB栈大小，防止栈溢出
RAYON_NUM_THREADS=8          # Rayon线程数控制
RUST_BACKTRACE=1             # 错误时显示堆栈跟踪
```

#### 3.3 常见运行问题排查

**编译问题**:

- **错误**: "failed to run custom build command"
- **解决**: 检查系统依赖，更新Rust工具链
- **验证**:`cargo check` 检查依赖完整性

**运行时问题**:

- **内存不足**: 降低concurrency-level或增加系统内存
- **文件权限**: 确保日志目录可写
- **数据文件**: 验证CSV文件格式和路径正确性

**性能问题**:

- **TPS过低**: 检查CPU使用率，调整并行度
- **内存泄漏**: 使用`valgrind`或`heaptrack`分析

### 第四章：批量测试与自动化 (2页)

#### 4.1 官方批量脚本使用

**run_data_historical_log.sh 解析**:

```bash
#!/bin/bash
# 脚本位置: scripts/run_data_historical_log.sh

# 基本用法
./scripts/run_data_historical_log.sh

# 自定义配置
export CONCURRENCY_LEVELS="2 4 8 16"
export DATA_FILES="ETH_2401_100.csv ETH_2401_1000.csv"  
export NUM_RUNS=3
./scripts/run_data_historical_log.sh
```

**脚本功能分析**:

- 多并发度自动测试
- 多数据集批量处理
- 结果自动收集和汇总
- 错误处理和重试机制

#### 4.2 自定义批量测试脚本

**基础批量脚本模板**:

```bash
#!/bin/bash
# custom_batch_test.sh

CONCURRENCY_LEVELS=(2 4 8 16)
DATA_FILES=("data/ETH_2401_100.csv" "data/ETH_2401_1000.csv")
NUM_RUNS=3

for concurrency in "${CONCURRENCY_LEVELS[@]}"; do
    for data_file in "${DATA_FILES[@]}"; do
        for run in $(seq 1 $NUM_RUNS); do
            echo "Running: concurrency=$concurrency, data=$data_file, run=$run"
          
            LOG_DIR="./results/c${concurrency}_$(basename $data_file .csv)_run${run}"
          
            BLOCK_STM_LOG_LEVEL=INFO \
            BLOCK_STM_LOG_DIR="$LOG_DIR" \
            timeout 300 cargo run --release -- replay-erc20 \
                --data-path "$data_file" \
                --concurrency-level "$concurrency" \
                --num-runs 1 \
                --num-warmups 0 \
            || echo "Failed: $LOG_DIR"
        done
    done
done
```

#### 4.3 结果收集与分析方法

**结果汇总脚本**:

```bash
#!/bin/bash
# collect_results.sh

echo "Concurrency,DataSet,Run,TPS,AvgLatency,AbortCount" > results_summary.csv

for log_dir in ./results/*/; do
    if [[ -f "$log_dir/block_stm_summary.log" ]]; then
        # 从汇总日志提取关键指标
        tps=$(grep "average_tps" "$log_dir/block_stm_summary.log" | tail -1 | jq -r '.average_tps')
        # ... 其他指标提取
        echo "$concurrency,$dataset,$run,$tps,$latency,$aborts" >> results_summary.csv
    fi
done
```

### 第五章：大文件管理方案 (1-2页)

#### 5.1 源码与编译产物分离策略

**问题分析**:

- Aptos-core完整编译产物可能超过10GB
- 日志文件快速增长可能占满磁盘
- 家目录空间有限，需要使用外部存储

**分离方案设计**:

```
HOME目录结构:
~/aptos-projects/
├── aptos-core/          # 源码仓库 (git clone)
│   ├── .cargo/
│   │   └── config.toml  # 重定向配置
│   └── ... 其他源码文件

/mist/aptos-build/$(whoami)/  # 大文件存储区
├── target/              # 编译产物
├── logs/               # 日志文件
└── cache/              # 编译缓存
```

#### 5.2 符号链接与目录映射

**实施步骤**:

```bash
# 1. 创建外部存储目录
mkdir -p /mist/aptos-build/$(whoami)/{target,logs,cache}

# 2. 配置cargo重定向
cat > ~/.cargo/config.toml << EOF
[build]
target-dir = "/mist/aptos-build/$(whoami)/target"

[env]
CARGO_HOME = "/mist/aptos-build/$(whoami)/cache"
EOF

# 3. 创建日志目录软链接  
ln -sf /mist/aptos-build/$(whoami)/logs ~/aptos-projects/logs

# 4. 验证配置
cargo build --release
ls -la /mist/aptos-build/$(whoami)/target/
```

#### 5.3 磁盘空间优化建议

**自动清理策略**:

```bash
#!/bin/bash
# cleanup_old_files.sh

# 清理7天前的日志文件
find /mist/aptos-build/$(whoami)/logs -name "*.log" -mtime +7 -delete

# 清理过期的编译缓存
find /mist/aptos-build/$(whoami)/cache -name "*.rlib" -atime +30 -delete

# 压缩重要的历史日志
find /mist/aptos-build/$(whoami)/logs -name "*.log" -mtime +1 -exec gzip {} \;
```

**监控脚本**:

```bash
# 磁盘使用监控
du -sh /mist/aptos-build/$(whoami)/* | sort -hr
```

## 写作要求

### 实操性要求

- 所有命令都必须可以直接复制执行
- 提供完整的错误排查流程
- 包含性能调优的具体建议
- 给出实际的配置文件示例

### 兼容性要求

- 覆盖主流操作系统和版本
- 考虑不同硬件配置的适配
- 提供低配置环境的优化方案
- 包含企业环境的特殊需求

### 安全性要求

- 避免使用不安全的配置
- 提供权限管理的最佳实践
- 包含网络安全相关的注意事项
- 考虑多用户环境的隔离需求

## 质量控制检查点

### 可操作性检查

- [ ] 所有命令在目标平台测试通过
- [ ] 配置文件格式正确有效
- [ ] 路径和权限设置合理
- [ ] 错误处理逻辑完善

### 完整性检查

- [ ] 覆盖完整的安装配置流程
- [ ] 包含常见问题的解决方案
- [ ] 提供性能优化指导
- [ ] 考虑不同使用场景需求

### 实用性检查

- [ ] 新手可以按文档成功配置
- [ ] 高级用户可以找到优化建议
- [ ] 运维人员可以进行批量部署
- [ ] 开发人员可以快速上手开发

## 预期成果

完成的文档将为用户提供：

1. **完整的环境配置指南**
2. **优化的编译运行方案**
3. **实用的批量测试工具**
4. **智能的大文件管理策略**
5. **专业的运维最佳实践**

这将是一份注重实操性和实用性的配置指南，帮助各类用户在不同环境下高效地使用Aptos Block-STM系统。
