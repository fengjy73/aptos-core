# Aptos Block-STM 编译运行与环境配置指南

**文档版本**: 1.0  
**创建日期**: 2025-08-22  
**目标受众**: 开发者、运维工程师、性能测试人员  

## 概述

本文档提供Aptos Block-STM系统的完整编译运行指南，涵盖环境配置、编译优化、测试执行和大文件管理等关键环节。无论您是初次接触Block-STM的新手，还是需要优化性能的资深开发者，都可以在本指南中找到所需的操作步骤和最佳实践。

## 目录

- [第一章：环境准备与配置](#第一章环境准备与配置)
- [第二章：编译配置与优化](#第二章编译配置与优化)  
- [第三章：单次测试运行指南](#第三章单次测试运行指南)
- [第四章：批量测试与自动化](#第四章批量测试与自动化)
- [第五章：大文件管理方案](#第五章大文件管理方案)

---

## 第一章：环境准备与配置

### 1.1 不同平台的Rust环境配置

#### 支持平台

Aptos Block-STM系统支持在多个平台上编译运行，推荐配置如下：

- **Linux** (Ubuntu 20.04+, CentOS 8+, RHEL 8+)
- **macOS** (10.15+, 原生支持Apple Silicon M1/M2)
- **Windows** (推荐使用WSL2)

#### Rust工具链安装

**标准安装流程**：

```bash
# 官方推荐安装方式
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 重新加载环境变量
source ~/.cargo/env

# 验证安装
rustc --version  # 需要 1.70.0 或更高版本
cargo --version  # 验证 cargo 可用
```

**版本要求**：
- Rust: 1.70.0+ (推荐使用最新稳定版)
- Cargo: 对应Rust版本的配套版本
- LLVM: 系统自动处理，无需手动安装

#### 平台特定配置

##### Linux系统配置

**Ubuntu/Debian系列**：
```bash
# 更新包管理器
sudo apt update && sudo apt upgrade -y

# 安装编译依赖
sudo apt install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    cmake \
    clang \
    llvm-dev \
    libclang-dev

# 验证关键组件
gcc --version    # 应显示 9.0+ 版本
pkg-config --version
openssl version  # 应显示 1.1.1+ 版本
```

**CentOS/RHEL系列**：
```bash
# 启用PowerTools仓库 (CentOS 8)
sudo dnf config-manager --set-enabled powertools

# 安装开发工具包
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y \
    openssl-devel \
    pkg-config \
    git \
    cmake \
    clang \
    llvm-devel
```

##### macOS系统配置

```bash
# 安装 Xcode Command Line Tools
xcode-select --install

# 使用 Homebrew 安装依赖 (推荐)
brew install pkg-config openssl cmake

# 配置 OpenSSL 环境变量
echo 'export PATH="/opt/homebrew/opt/openssl@3/bin:$PATH"' >> ~/.zshrc
echo 'export LDFLAGS="-L/opt/homebrew/opt/openssl@3/lib"' >> ~/.zshrc
echo 'export CPPFLAGS="-I/opt/homebrew/opt/openssl@3/include"' >> ~/.zshrc
source ~/.zshrc
```

**Apple Silicon特别注意**：
```bash
# M1/M2芯片需要额外配置
export CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=clang
export CC=clang
export CXX=clang++
```

##### Windows WSL2配置

```bash
# 在WSL2 Ubuntu环境中
# 1. 启用WSL2并安装Ubuntu 20.04+
# 2. 按照Linux Ubuntu配置步骤执行
# 3. 配置Windows Terminal集成

# WSL2性能优化
echo 'export CARGO_TARGET_DIR=/mnt/c/aptos-build/target' >> ~/.bashrc
```

### 1.2 依赖库安装与版本要求

#### 系统级依赖分析

基于Aptos Block-STM的实际依赖，需要确保以下系统库可用：

**核心系统库**：
- OpenSSL 1.1.1+ (加密算法支持)
- zlib 1.2.11+ (数据压缩)
- libffi 3.2+ (外部函数接口)
- pkg-config 0.29+ (库配置管理)

**性能相关库**：
- libc 2.17+ (系统调用接口)
- pthread (多线程支持)
- libatomic (原子操作，某些平台需要)

#### Rust Crate依赖解析

通过分析 `Cargo.toml` 文件，Block-STM的关键依赖包括：

**并行计算框架**：
```toml
[dependencies]
rayon = "1.7"          # 数据并行处理
tokio = { version = "1", features = ["full"] }  # 异步运行时
crossbeam = "0.8"      # 无锁数据结构
parking_lot = "0.12"   # 高性能锁
```

**序列化与IO**：
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"     # JSON处理
bincode = "1.3"        # 二进制序列化
csv = "1.2"            # CSV文件处理
```

**系统交互**：
```toml
clap = { version = "4.0", features = ["derive"] }  # 命令行解析
log = "0.4"            # 日志框架
env_logger = "0.10"    # 环境变量日志配置
```

#### 依赖版本兼容性验证

```bash
# 检查依赖完整性
cargo check --all-targets

# 更新到兼容版本
cargo update

# 查看依赖树
cargo tree | head -20

# 检查过时依赖
cargo audit
```

### 1.3 开发环境最佳实践

#### IDE推荐配置

##### Visual Studio Code (推荐)

**必需扩展**：
```json
{
  "recommendations": [
    "rust-lang.rust-analyzer",    // Rust语言服务
    "vadimcn.vscode-lldb",        // 调试支持
    "serayuzgur.crates",          // Crate版本管理
    "dustypomerleau.rust-syntax"  // 语法高亮增强
  ]
}
```

**工作区配置** (`.vscode/settings.json`)：
```json
{
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.loadOutDirsFromCheck": true,
  "files.watcherExclude": {
    "**/target/**": true
  }
}
```

##### IntelliJ IDEA/CLion

```bash
# 安装Rust插件
# File -> Settings -> Plugins -> Marketplace -> "Rust"

# 配置工具链
# File -> Settings -> Languages & Frameworks -> Rust
# 工具链路径：~/.cargo/bin
```

##### Vim/Neovim配置

```vim
" .vimrc 或 init.vim 配置
Plug 'rust-lang/rust.vim'
Plug 'neoclide/coc.nvim', {'branch': 'release'}

" rust.vim 配置
let g:rustfmt_autosave = 1
let g:rust_clip_command = 'xclip -selection clipboard'
```

#### 开发工具链配置

```bash
# 安装额外组件
rustup component add rustfmt   # 代码格式化
rustup component add clippy    # 静态分析
rustup component add rls       # 语言服务器 (可选)

# 安装开发辅助工具
cargo install cargo-watch      # 文件变化监控
cargo install cargo-audit      # 安全审计
cargo install cargo-outdated   # 依赖更新检查
cargo install cargo-tree       # 依赖树查看

# 验证工具链完整性
rustfmt --version
cargo clippy --version
cargo audit --version
```

#### 环境变量配置

**基础环境变量** (`~/.bashrc` 或 `~/.zshrc`)：
```bash
# Rust环境
export PATH="$HOME/.cargo/bin:$PATH"
export RUST_SRC_PATH="$(rustc --print sysroot)/lib/rustlib/src/rust/src"

# Block-STM特定配置
export RUST_MIN_STACK=8388608          # 8MB栈大小
export RUST_BACKTRACE=1                # 错误堆栈追踪
export RAYON_NUM_THREADS=$(nproc)      # Rayon线程数

# 性能优化
export CARGO_INCREMENTAL=1             # 增量编译
export RUST_LOG=info                   # 默认日志级别
```

#### 开发工作流优化

**快速开发循环**：
```bash
# 监控文件变化并自动检查
cargo watch -x check

# 监控并自动运行测试
cargo watch -x test

# 监控并自动格式化
cargo watch -s 'cargo fmt && cargo clippy'
```

**代码质量保证**：
```bash
# 运行完整检查套件
cargo fmt --all           # 格式化代码
cargo clippy --all        # 静态分析
cargo test --all          # 运行测试
cargo doc --no-deps       # 生成文档
```

---

## 第二章：编译配置与优化

### 2.1 Cargo配置优化

#### 项目级配置

Aptos Block-STM项目的编译性能可以通过创建 `.cargo/config.toml` 文件进行显著优化：

**基础性能配置** (`.cargo/config.toml`)：
```toml
[build]
# 并行编译任务数 (根据CPU核心数调整)
jobs = 8                    # 推荐设置为 CPU核心数
rustc-wrapper = "sccache"   # 编译缓存 (需要安装 sccache)

# 目标平台特定设置
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]

# 发布版本优化配置
[profile.release]
lto = true                  # 链接时优化 (LTO)
codegen-units = 1          # 代码生成单元数
panic = "abort"            # panic处理方式
opt-level = 3              # 最高优化级别
debug = false              # 禁用调试符号
rpath = false              # 禁用运行时路径

# 开发版本配置
[profile.dev]
# 适度优化以平衡编译速度和运行性能
opt-level = 1              # 轻度优化
debug = true               # 保留调试符号
incremental = true         # 启用增量编译
```

**高级编译优化配置**：
```toml
# 自定义发布配置用于基准测试
[profile.bench]
inherits = "release"
lto = "thin"               # 瘦LTO，平衡编译时间和性能
codegen-units = 4

# 测试专用配置
[profile.test]
opt-level = 2              # 测试时适度优化
overflow-checks = true     # 保持溢出检查
debug-assertions = true    # 保持断言检查
```

#### 全局Cargo配置

**用户级全局配置** (`~/.cargo/config.toml`)：
```toml
[net]
retry = 3                  # 网络重试次数
git-fetch-with-cli = true  # 使用git CLI而非内置git
offline = false            # 允许网络访问

[registries.crates-io]
protocol = "sparse"        # 使用稀疏索引加速包下载

[cargo-new]
name = "Your Name"         # 默认作者信息
email = "your.email@example.com"
vcs = "git"

# 源码镜像配置 (中国用户推荐)
[source.crates-io]
replace-with = "ustc"      # 使用中科大镜像

[source.ustc]
registry = "https://mirrors.ustc.edu.cn/crates.io-index"
```

#### Sccache编译缓存设置

```bash
# 安装sccache编译缓存工具
cargo install sccache

# 配置sccache
export RUSTC_WRAPPER=sccache
export SCCACHE_CACHE_SIZE="10G"     # 设置缓存大小
export SCCACHE_DIR="$HOME/.sccache" # 缓存目录

# 验证sccache状态
sccache --show-stats

# 清理缓存 (如果需要)
sccache --zero-stats
sccache --stop-server
```

### 2.2 编译参数调优

#### 性能优化编译

**发布版本编译** (推荐用于性能测试)：
```bash
# 基础发布版本编译
cargo build --release

# 启用CPU特定优化 (重要！)
RUSTFLAGS="-C target-cpu=native" cargo build --release

# 最大化性能优化 (用于基准测试)
RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C lto=fat" \
  cargo build --release

# 多线程编译加速
cargo build --release -j $(nproc)
```

**基准测试专用编译**：
```bash
# 进入基准测试目录
cd aptos-move/aptos-transaction-benchmarks

# 针对Block-STM优化的编译
RUSTFLAGS="-C target-cpu=native -C opt-level=3 -C codegen-units=1" \
  CARGO_PROFILE_RELEASE_LTO=true \
  cargo build --release --bin aptos-transaction-benchmarks

# 验证编译结果
ls -la target/release/aptos-transaction-benchmarks
file target/release/aptos-transaction-benchmarks
```

#### 调试版本编译

**快速开发编译**：
```bash
# 快速编译用于开发调试
cargo build

# 启用详细调试信息
cargo build --config profile.dev.debug=true

# 仅检查代码而不生成二进制文件 (最快)
cargo check

# 并行检查所有目标
cargo check --all-targets -j $(nproc)
```

**内存优化编译** (适用于内存受限环境)：
```bash
# 优化编译内存使用
RUSTFLAGS="-C opt-level=s -C codegen-units=256" \
  cargo build --release

# 最小化二进制文件大小
RUSTFLAGS="-C opt-level=z -C strip=symbols" \
  cargo build --release
```

#### 特定平台编译调优

**Linux平台优化**：
```bash
# 使用现代链接器
RUSTFLAGS="-C link-arg=-fuse-ld=lld" cargo build --release

# 启用Intel CPU特定优化
RUSTFLAGS="-C target-cpu=skylake -C target-feature=+avx2" \
  cargo build --release
```

**macOS平台优化**：
```bash
# Apple Silicon优化
RUSTFLAGS="-C target-cpu=apple-m1" cargo build --release

# x86_64 Mac优化
RUSTFLAGS="-C target-cpu=haswell" cargo build --release
```

### 2.3 Target目录管理策略

#### 目录结构优化

**推荐项目结构**：
```
aptos-core/
├── Cargo.toml              # 工作空间配置
├── .cargo/
│   └── config.toml         # 项目级Cargo配置
├── aptos-move/
│   └── aptos-transaction-benchmarks/
│       ├── src/
│       ├── Cargo.toml
│       └── target/         # 本地编译输出 (可删除)
└── build/                  # 自定义构建脚本
```

**Target目录重定向配置**：
```toml
# .cargo/config.toml
[build]
# 重定向到外部存储 (推荐用于大文件管理)
target-dir = "/mist/aptos-build/target"

# 或者使用环境变量
# export CARGO_TARGET_DIR=/mist/aptos-build/target
```

#### 磁盘空间管理策略

**编译产物清理**：
```bash
# 清理当前项目编译产物
cargo clean

# 清理特定包的编译产物
cargo clean -p aptos-transaction-benchmarks

# 清理所有Cargo缓存 (慎用)
rm -rf ~/.cargo/registry/cache/
rm -rf ~/.cargo/git/

# 使用cargo-cache工具管理 (推荐)
cargo install cargo-cache
cargo cache --autoclean       # 自动清理过期缓存
cargo cache --autoclean-expensive  # 深度清理
```

**Target目录结构分析**：
```bash
# 分析target目录大小
du -sh target/*
# 典型输出：
# 2.1G    target/release      # 发布版本
# 8.5G    target/debug        # 调试版本
# 156M    target/deps         # 依赖库

# 查看最大文件
find target/ -type f -exec ls -lh {} \; | sort -nrk5 | head -10
```

**智能缓存策略**：
```bash
#!/bin/bash
# smart_clean.sh - 智能清理脚本

TARGET_DIR="${CARGO_TARGET_DIR:-./target}"
MAX_SIZE_GB=20  # 最大允许大小

# 检查target目录大小
SIZE_GB=$(du -sg "$TARGET_DIR" | cut -f1)

if [ "$SIZE_GB" -gt "$MAX_SIZE_GB" ]; then
    echo "Target directory size (${SIZE_GB}GB) exceeds limit (${MAX_SIZE_GB}GB)"
    
    # 清理调试版本 (通常最大)
    rm -rf "$TARGET_DIR/debug"
    echo "Cleaned debug build"
    
    # 保留最近的发布版本
    find "$TARGET_DIR/release" -name "*.rlib" -mtime +7 -delete
    echo "Cleaned old release artifacts"
fi
```

#### 多项目共享Target配置

**工作空间级别配置**：
```toml
# 根目录 Cargo.toml
[workspace]
members = [
    "aptos-move/aptos-vm",
    "aptos-move/block-executor", 
    "aptos-move/aptos-transaction-benchmarks"
]

[workspace.dependencies]
# 统一依赖版本管理
rayon = "1.7"
serde = { version = "1.0", features = ["derive"] }

[profile.release]
lto = true
codegen-units = 1
```

**环境变量统一管理**：
```bash
# aptos_build_env.sh - 构建环境脚本
#!/bin/bash

# 基础环境配置
export CARGO_TARGET_DIR="/mist/aptos-build/target"
export RUSTC_WRAPPER="sccache"
export RUST_MIN_STACK=8388608

# 性能优化标志
export RUSTFLAGS="-C target-cpu=native -C opt-level=3"
export RAYON_NUM_THREADS=$(nproc)

# 使用方式：
# source aptos_build_env.sh
# cargo build --release
```

---

## 第三章：单次测试运行指南

### 3.1 基本命令格式与参数说明

#### 核心命令模板

**Block-STM ERC20重放测试标准命令**：

```bash
# 基本运行模板
cd aptos-core/aptos-move/aptos-transaction-benchmarks

BLOCK_STM_LOG_LEVEL=DEBUG \
BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S) \
cargo run --release -- replay-erc20 \
    --data-path data/ETH_2401_100.csv \
    --concurrency-level 4 \
    --num-runs 1 \
    --num-warmups 0
```

**命令结构解析**：
1. **环境变量**: BLOCK_STM_LOG_LEVEL, BLOCK_STM_LOG_DIR
2. **编译模式**: cargo run --release
3. **主命令**: replay-erc20
4. **数据参数**: --data-path
5. **性能参数**: --concurrency-level, --num-runs, --num-warmups

#### 参数详细说明

##### 数据文件参数

**--data-path** ：指定历史交易数据CSV文件路径
```bash
# 相对路径 (推荐)
--data-path data/ETH_2401_100.csv
--data-path data/ETH_2401_1000.csv
--data-path data/ETH_2401_10000.csv

# 绝对路径
--data-path /mist/blockchain_data/ETH_2401_100000.csv

# 验证文件存在
ls -la data/ETH_2401_100.csv
file data/ETH_2401_100.csv  # 应显示 CSV 文本文件
```

**CSV文件格式要求**：
- 必须包含标准的ERC20交易字段
- 支持的字段：`block_number`, `transaction_hash`, `from_address`, `to_address`, `value`, `gas_used`
- 编码要求：UTF-8格式
- 文件大小：支持从100行到100万行不同规模

##### 并行化参数

**--concurrency-level** ：设置并行执行线程数
```bash
# CPU核心数映射关系
--concurrency-level 1   # 顺序执行 (基准线)
--concurrency-level 2   # 2线程并行
--concurrency-level 4   # 4线程并行 (推荐)
--concurrency-level 8   # 8线程并行
--concurrency-level 16  # 高并发测试

# 自动检测CPU核心数
nproc  # 查看系统核心数

# 推荐设置：
# - 开发调试：2-4线程
# - 性能测试：CPU核心数的 50%-100%
# - 极限测试：2x CPU核心数
```

**并行度影响分析**：
- **内存使用**: 线程数 × 单线程内存 (10-50MB)
- **CPU使用**: 接近线性增长，受交易依赖关系影响
- **磁盘IO**: 日志写入增加，可考虑SSD存储

##### 性能测试参数

**--num-runs** ：设置测试运行次数
```bash
--num-runs 1    # 单次运行 (开发调试)
--num-runs 3    # 3次运行 (基本性能测试)
--num-runs 5    # 5次运行 (精确性能测试)
--num-runs 10   # 10次运行 (统计分析)

# 多次运行优势：
# - 消除偶发性干扰
# - 获得平均、最大、最小值
# - 评估性能稳定性
```

**--num-warmups** ：设置预热运行次数
```bash
--num-warmups 0   # 无预热 (冷启动测试)
--num-warmups 1   # 1次预热 (推荐)
--num-warmups 2   # 2次预热 (JIT优化测试)

# 预热作用：
# - JIT编译器优化
# - 内存分配器预热
# - CPU缓存预加载
# - 消除首次运行的开销
```

#### 高级命令参数

**日志级别控制**：
```bash
# 基本日志参数
--log-level DEBUG    # 最详细日志
--log-level INFO     # 信息日志 (默认)
--log-level WARN     # 仅警告和错误
--log-level ERROR    # 仅错误信息

# 组合使用
BLOCK_STM_LOG_LEVEL=DEBUG cargo run --release -- replay-erc20 \
    --data-path data/ETH_2401_100.csv --log-level INFO
```

**输出格式控制** (如果支持)：
```bash
--output-format json     # JSON结构化输出
--output-format table    # 表格格式
--output-format summary  # 简要汇总

# 输出重定向
--output-file results.json  # 保存结果到文件
```

### 3.2 环境变量配置详解

#### 日志控制变量

##### BLOCK_STM_LOG_LEVEL
```bash
# 日志级别设置 (影响输出详细程度和性能)
export BLOCK_STM_LOG_LEVEL=DEBUG    # 最详细，性能影响较大
export BLOCK_STM_LOG_LEVEL=INFO     # 平衡选择 (推荐)
export BLOCK_STM_LOG_LEVEL=WARN     # 仅警告和错误
export BLOCK_STM_LOG_LEVEL=ERROR    # 仅错误信息
export BLOCK_STM_LOG_LEVEL=OFF      # 关闭所有日志

# 性能影响分析：
# DEBUG: TPS下降 10-15%，日志文件大小 50-100MB
# INFO:  TPS下降 3-5%，日志文件大小 10-20MB
# WARN:  TPS影响 <2%，日志文件大小 <5MB
```

##### BLOCK_STM_LOG_DIR
```bash
# 日志输出目录设置 (建议使用时间戳避免冲突)
export BLOCK_STM_LOG_DIR=./test_logs_$(date +%Y%m%d_%H%M%S)
export BLOCK_STM_LOG_DIR=./test_logs_concurrency_4
export BLOCK_STM_LOG_DIR=/mist/block_stm_logs/$(whoami)
export BLOCK_STM_LOG_DIR=/tmp/block_stm_logs_$  # 使用进程ID

# 目录命名最佳实践：
# - 包含时间戳避免覆盖
# - 包含测试参数便于分辨
# - 使用有意义的名称

# 目录权限检查
ls -ld "$BLOCK_STM_LOG_DIR" 2>/dev/null || mkdir -p "$BLOCK_STM_LOG_DIR"
touch "$BLOCK_STM_LOG_DIR/test_write" && rm "$BLOCK_STM_LOG_DIR/test_write"
```

##### BLOCK_STM_LOG_MAX_SIZE
```bash
# 日志文件大小限制 (防止单个日志文件过大)
export BLOCK_STM_LOG_MAX_SIZE=100   # 100MB限制 (推荐)
export BLOCK_STM_LOG_MAX_SIZE=50    # 50MB限制 (小型测试)
export BLOCK_STM_LOG_MAX_SIZE=500   # 500MB限制 (大型测试)

# 日志轮转机制：
# - 达到大小限制后自动创建新文件
# - 文件命名: block_stm_execution.log.1, .2, 以此类推
# - 保持最近3-5个文件防止磁盘占满
```

#### 性能相关变量

##### Rust运行时环境
```bash
# 栈大小设置 (防止栈溢出)
export RUST_MIN_STACK=8388608       # 8MB栈大小 (推荐)
export RUST_MIN_STACK=16777216      # 16MB栈大小 (复杂交易)

# 错误堆栈追踪
export RUST_BACKTRACE=1             # 显示堆栈追踪
export RUST_BACKTRACE=full          # 完整堆栈信息
export RUST_BACKTRACE=0             # 禁用堆栈追踪

# 日志设置
export RUST_LOG=info                # Rust系统日志级别
export RUST_LOG=debug               # 详细调试信息
export RUST_LOG=block_executor=debug # 针对模块的日志
```

##### 并行计算配置
```bash
# Rayon线程数控制
export RAYON_NUM_THREADS=$(nproc)   # 使用所有CPU核心
export RAYON_NUM_THREADS=8          # 固定线程数
export RAYON_NUM_THREADS=$(($(nproc) / 2))  # 使用一协核心

# Tokio异步运行时配置 (如果使用)
export TOKIO_WORKER_THREADS=4       # Tokio工作线程数
export TOKIO_THREAD_STACK_SIZE=2097152  # 2MB线程栈
```

##### 内存管理配置
```bash
# malloc调优 (Linux)
export MALLOC_ARENA_MAX=2           # 限制arena数量
export MALLOC_MMAP_THRESHOLD_=131072 # 128KB mmap阈值

# jemalloc配置 (如果使用)
export MALLOC_CONF="dirty_decay_ms:1000,muzzy_decay_ms:5000"

# 内存监控
export RUST_ALLOC_CONF="stats_print:true"  # 打印内存统计
```

### 3.3 常见运行问题排查

#### 编译问题诊断

##### 依赖缺失问题
```bash
# 错误信息: "failed to run custom build command"
# 原因: 缺少系统依赖

# 诊断步骤
cargo check --all-targets           # 检查依赖完整性
rustc --version && cargo --version  # 验证Rust版本
pkg-config --list-all | head -10    # 检查pkg-config可用性

# 解决方案
# Ubuntu/Debian:
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev

# CentOS/RHEL:
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y openssl-devel pkg-config

# macOS:
brew install pkg-config openssl cmake
```

##### 编译器版本问题
```bash
# 错误信息: "feature X is not stable"
# 原因: Rust版本过旧

# 诊断和修复
rustc --version                     # 检查当前版本
rustup update                       # 更新到最新版本
rustup default stable               # 设置稳定版为默认

# 依赖冲突问题
cargo clean                         # 清理编译产物
cargo update                        # 更新依赖版本
rm Cargo.lock                       # 重建锁定文件 (慎用)
```

#### 运行时问题诊断

##### 内存不足问题
```bash
# 错误信息: "Cannot allocate memory" 或 OOM Killer
# 原因: 并发级别过高或数据集过大

# 内存使用监控
free -h                             # 检查系统内存
top -p $(pgrep aptos-transaction)    # 监控进程内存

# 解决方案
# 1. 降低并发级别
--concurrency-level 2               # 清少4降到2

# 2. 使用更小数据集
--data-path data/ETH_2401_100.csv   # 使用100行代替1000行

# 3. 增加系统内存 或 配置Swap
sudo swapon --show                  # 检查Swap状态
```

##### 文件权限问题
```bash
# 错误信息: "Permission denied" 或 "No such file or directory"

# 诊断步骤
# 1. 检查数据文件
ls -la data/ETH_2401_100.csv
file data/ETH_2401_100.csv

# 2. 检查日志目录
ls -ld "$BLOCK_STM_LOG_DIR"
mkdir -p "$BLOCK_STM_LOG_DIR"
touch "$BLOCK_STM_LOG_DIR/test" && rm "$BLOCK_STM_LOG_DIR/test"

# 3. 检查可执行文件
ls -la target/release/aptos-transaction-benchmarks
chmod +x target/release/aptos-transaction-benchmarks

# 修复权限问题
chmod 755 "$BLOCK_STM_LOG_DIR"       # 目录权限
chmod 644 data/*.csv                # 数据文件权限
```

##### 数据文件问题
```bash
# 错误信息: "CSV parse error" 或 "Invalid format"

# CSV文件验证
head -5 data/ETH_2401_100.csv        # 检查文件格式
wc -l data/ETH_2401_100.csv          # 检查行数
file data/ETH_2401_100.csv           # 检查文件类型

# 字符编码检查
iconv -f utf-8 -t utf-8 data/ETH_2401_100.csv > /dev/null
echo $?  # 0表示UTF-8编码正确

# CSV格式修复
sed -i 's/\r$//' data/ETH_2401_100.csv  # 删除Windows换行符
```

#### 性能问题诊断

##### TPS过低问题
```bash
# 预期TPS < 实际TPS，需要排查性能瓶颈

# 1. CPU利用率检查
top -p $(pgrep aptos-transaction) -H  # 查看线程级CPU使用
htop                                 # 更直观的系统监控

# 2. 磁盘IO检查
iotop                                # 监控磁盘IO
df -h "$BLOCK_STM_LOG_DIR"            # 检查磁盘空间

# 3. 网络检查 (如果涉及远程数据)
ss -tuln | grep :8080                # 检查端口状态

# 优化建议
# - CPU约束: 增加并发级别
# - 内存约束: 降低并发级别
# - 磁盘约束: 使用SSD或降低日志级别
```

##### 内存泄漏检测
```bash
# 长时间运行内存使用量不断增加

# 使用valgrind检测 (仅Linux)
sudo apt install -y valgrind
valgrind --tool=memcheck --leak-check=full \
  target/release/aptos-transaction-benchmarks replay-erc20 \
  --data-path data/ETH_2401_100.csv --concurrency-level 2

# 使用heaptrack检测
sudo apt install -y heaptrack
heaptrack target/release/aptos-transaction-benchmarks replay-erc20 \
  --data-path data/ETH_2401_100.csv --concurrency-level 2

# 轻量级内存监控
# 在程序运行期间定期检查:
watch -n 5 "ps -p \$(pgrep aptos-transaction) -o pid,ppid,rss,vsz,command"
```

---

## 第四章：批量测试与自动化

### 4.1 官方批量脚本使用

#### run_data_historical_log.sh 脚本解析

**脚本位置和基本用法**：

```bash
# 脚本位置
./scripts/run_data_historical_log.sh

# 基本运行 (使用默认配置)
./scripts/run_data_historical_log.sh

# 检查脚本存在和权限
ls -la scripts/run_data_historical_log.sh
chmod +x scripts/run_data_historical_log.sh
```

**脚本功能分析**：

通过分析脚本内容，该脚本支持以下功能：

1. **多并发度自动测试**: 自动进行 1, 2, 4, 8, 16 线程的对比测试
2. **多数据集批量处理**: 支持同时测试多个CSV数据文件
3. **结果自动收集**: 自动收集和汇总测试结果
4. **错误处理和重试**: 内置错误处理和重试机制

#### 自定义环境变量配置

**并发度设置**：
```bash
# 设置要测试的并发级别
export CONCURRENCY_LEVELS="2 4 8"      # 仅测试 2, 4, 8 线程
export CONCURRENCY_LEVELS="1 2 4 8 16" # 完整对比测试
export CONCURRENCY_LEVELS="4"          # 仅测试 4 线程

# 运行脚本
./scripts/run_data_historical_log.sh
```

**数据文件设置**：
```bash
# 指定要测试的数据文件
export DATA_FILES="ETH_2401_100.csv ETH_2401_1000.csv"  
export DATA_FILES="ETH_2401_100.csv"                     # 仅测试单个文件
export DATA_FILES="ETH_2401_*.csv"                       # 通配符匹配

# 验证数据文件存在
for file in $DATA_FILES; do
    if [[ -f "data/$file" ]]; then
        echo "Found: data/$file"
    else
        echo "Missing: data/$file"
    fi
done
```

**测试轮次设置**：
```bash
# 设置每个配置的运行次数
export NUM_RUNS=3                       # 3次运行 (推荐)
export NUM_RUNS=5                       # 5次运行 (精确测试)
export NUM_RUNS=1                       # 单次运行 (快速测试)

# 预热设置
export NUM_WARMUPS=1                    # 默认预热
export NUM_WARMUPS=0                    # 无预热 (冷启动测试)
```

#### 完整批量测试命令示例

**基础批量测试**：
```bash
# 设置测试环境
export CONCURRENCY_LEVELS="2 4 8 16"
export DATA_FILES="ETH_2401_100.csv ETH_2401_1000.csv"
export NUM_RUNS=3
export NUM_WARMUPS=1
export BLOCK_STM_LOG_LEVEL=INFO

# 执行批量测试
./scripts/run_data_historical_log.sh
```

**高精度性能测试**：
```bash
# 高精度配置
export CONCURRENCY_LEVELS="1 2 4 8 16"
export DATA_FILES="ETH_2401_10000.csv ETH_2401_100000.csv"
export NUM_RUNS=5
export NUM_WARMUPS=2
export BLOCK_STM_LOG_LEVEL=DEBUG
export BLOCK_STM_LOG_MAX_SIZE=200

# 执行测试
./scripts/run_data_historical_log.sh
```

**快速验证测试**：
```bash
# 快速验证配置
export CONCURRENCY_LEVELS="4"
export DATA_FILES="ETH_2401_100.csv"
export NUM_RUNS=1
export NUM_WARMUPS=0
export BLOCK_STM_LOG_LEVEL=WARN

# 执行测试
./scripts/run_data_historical_log.sh
```

### 4.2 自定义批量测试脚本

#### 基础批量测试脚本模板

**custom_batch_test.sh** - 完整批量测试脚本：

```bash
#!/bin/bash
# custom_batch_test.sh - 自定义批量测试脚本

set -euo pipefail  # 严格错误处理

# ===========================================
# 配置参数
# ===========================================

# 测试配置
CONCURRENCY_LEVELS=(1 2 4 8 16)
DATA_FILES=(
    "data/ETH_2401_100.csv"
    "data/ETH_2401_1000.csv"
    "data/ETH_2401_10000.csv"
)
NUM_RUNS=3
NUM_WARMUPS=1

# 日志配置
LOG_LEVEL="INFO"
BASE_LOG_DIR="./batch_results_$(date +%Y%m%d_%H%M%S)"
TIMEOUT_SECONDS=600  # 10分钟超时

# 结果输出
RESULTS_CSV="$BASE_LOG_DIR/batch_results.csv"
SUMMARY_FILE="$BASE_LOG_DIR/test_summary.md"

# ===========================================
# 助手函数
# ===========================================

# 初始化结果文件
init_results() {
    mkdir -p "$BASE_LOG_DIR"
    
    # 创建CSV表头
    cat > "$RESULTS_CSV" << EOF
Timestamp,Concurrency,DataFile,DataSize,Run,TPS,AvgLatency,AbortCount,Status,LogDir
EOF

    # 创建汇总文件
    cat > "$SUMMARY_FILE" << EOF
# Batch Test Summary

**Test Start**: $(date)
**Configuration**:
- Concurrency Levels: ${CONCURRENCY_LEVELS[*]}
- Data Files: ${DATA_FILES[*]}
- Runs per Config: $NUM_RUNS
- Warmups: $NUM_WARMUPS

## Results

EOF
}

# 记录测试结果
log_result() {
    local timestamp="$1"
    local concurrency="$2"
    local data_file="$3"
    local data_size="$4"
    local run="$5"
    local status="$6"
    local log_dir="$7"
    local tps="${8:-N/A}"
    local latency="${9:-N/A}"
    local aborts="${10:-N/A}"
    
    echo "$timestamp,$concurrency,$data_file,$data_size,$run,$tps,$latency,$aborts,$status,$log_dir" >> "$RESULTS_CSV"
}

# 提取性能指标
extract_metrics() {
    local log_dir="$1"
    local tps="N/A"
    local latency="N/A"
    local aborts="N/A"
    
    # 从汇总日志中提取TPS
    if [[ -f "$log_dir/block_stm_summary.log" ]]; then
        tps=$(grep -o '"average_tps":[0-9.]*' "$log_dir/block_stm_summary.log" | tail -1 | cut -d':' -f2 || echo "N/A")
        latency=$(grep -o '"average_latency_ms":[0-9.]*' "$log_dir/block_stm_summary.log" | tail -1 | cut -d':' -f2 || echo "N/A")
        aborts=$(grep -o '"total_aborts":[0-9]*' "$log_dir/block_stm_summary.log" | tail -1 | cut -d':' -f2 || echo "N/A")
    fi
    
    echo "$tps,$latency,$aborts"
}

# 计算数据文件大小
get_data_size() {
    local file="$1"
    if [[ -f "$file" ]]; then
        wc -l < "$file" | tr -d ' '
    else
        echo "0"
    fi
}

# ===========================================
# 主测试循环
# ===========================================

# 初始化
init_results

echo "Starting batch test at $(date)"
echo "Results will be saved to: $BASE_LOG_DIR"

# 主循环: 并发度 x 数据文件 x 运行次数
for concurrency in "${CONCURRENCY_LEVELS[@]}"; do
    echo "\n=== Testing concurrency level: $concurrency ==="
    
    for data_file in "${DATA_FILES[@]}"; do
        if [[ ! -f "$data_file" ]]; then
            echo "Warning: Data file not found: $data_file"
            continue
        fi
        
        data_size=$(get_data_size "$data_file")
        data_name=$(basename "$data_file" .csv)
        
        echo "  Testing data file: $data_file ($data_size lines)"
        
        for run in $(seq 1 $NUM_RUNS); do
            echo "    Run $run/$NUM_RUNS..."
            
            # 准备日志目录
            timestamp=$(date +%Y%m%d_%H%M%S)
            log_dir="$BASE_LOG_DIR/c${concurrency}_${data_name}_run${run}_${timestamp}"
            mkdir -p "$log_dir"
            
            # 执行测试
            if timeout $TIMEOUT_SECONDS env \
                BLOCK_STM_LOG_LEVEL="$LOG_LEVEL" \
                BLOCK_STM_LOG_DIR="$log_dir" \
                cargo run --release -- replay-erc20 \
                    --data-path "$data_file" \
                    --concurrency-level "$concurrency" \
                    --num-runs 1 \
                    --num-warmups "$NUM_WARMUPS" \
                > "$log_dir/stdout.log" 2> "$log_dir/stderr.log"; then
                
                # 成功：提取指标
                metrics=$(extract_metrics "$log_dir")
                IFS=',' read -r tps latency aborts <<< "$metrics"
                
                log_result "$timestamp" "$concurrency" "$data_file" "$data_size" "$run" "SUCCESS" "$log_dir" "$tps" "$latency" "$aborts"
                echo "      ✓ TPS: $tps, Latency: ${latency}ms, Aborts: $aborts"
            else
                # 失败：记录错误
                log_result "$timestamp" "$concurrency" "$data_file" "$data_size" "$run" "FAILED" "$log_dir"
                echo "      ✗ Test failed (timeout or error)"
            fi
        done
    done
done

echo "\nBatch test completed at $(date)"
echo "Results saved to: $RESULTS_CSV"
echo "Summary: $SUMMARY_FILE"
```

#### 轻量级批量测试脚本

**simple_batch.sh** - 简化批量测试：

```bash
#!/bin/bash
# simple_batch.sh - 简化的批量测试脚本

# 基本配置
CONCURRENCIES="2 4 8"
DATA_FILE="data/ETH_2401_100.csv"
RUNS=3

# 结果目录
RESULT_DIR="./simple_batch_$(date +%H%M%S)"
mkdir -p "$RESULT_DIR"

echo "Starting simple batch test with $DATA_FILE"

for concurrency in $CONCURRENCIES; do
    echo "Testing concurrency: $concurrency"
    
    for run in $(seq 1 $RUNS); do
        log_dir="$RESULT_DIR/c${concurrency}_run${run}"
        
        echo "  Run $run: $(date)"
        
        BLOCK_STM_LOG_LEVEL=INFO \
        BLOCK_STM_LOG_DIR="$log_dir" \
        cargo run --release -- replay-erc20 \
            --data-path "$DATA_FILE" \
            --concurrency-level "$concurrency" \
            --num-runs 1 \
            --num-warmups 0 \
        && echo "    ✓ Success" || echo "    ✗ Failed"
    done
done

echo "Simple batch test completed: $RESULT_DIR"
```

#### 参数化批量测试脚本

**parametric_batch.sh** - 支持命令行参数：

```bash
#!/bin/bash
# parametric_batch.sh - 参数化批量测试

# 默认参数
CONCURRENCY_LEVELS="${1:-2 4 8}"
DATA_PATH="${2:-data/ETH_2401_100.csv}"
NUM_RUNS="${3:-3}"
LOG_LEVEL="${4:-INFO}"

# 使用方式
if [[ "$1" == "-h" || "$1" == "--help" ]]; then
    cat << EOF
Usage: $0 [CONCURRENCY_LEVELS] [DATA_PATH] [NUM_RUNS] [LOG_LEVEL]

Parameters:
  CONCURRENCY_LEVELS : Space-separated list of concurrency levels (default: "2 4 8")
  DATA_PATH         : CSV data file path (default: "data/ETH_2401_100.csv")
  NUM_RUNS          : Number of runs per configuration (default: 3)
  LOG_LEVEL         : Log level DEBUG|INFO|WARN|ERROR (default: INFO)

Examples:
  $0                                    # Use all defaults
  $0 "4 8 16"                           # Custom concurrency levels
  $0 "4" data/ETH_2401_1000.csv 5      # 4 threads, larger dataset, 5 runs
  $0 "2 4" data/ETH_2401_100.csv 1 DEBUG # Debug mode
EOF
    exit 0
fi

# 验证数据文件
if [[ ! -f "$DATA_PATH" ]]; then
    echo "Error: Data file not found: $DATA_PATH"
    exit 1
fi

# 执行批量测试
echo "Starting parametric batch test:"
echo "  Concurrency Levels: $CONCURRENCY_LEVELS"
echo "  Data Path: $DATA_PATH"
echo "  Runs per Config: $NUM_RUNS"
echo "  Log Level: $LOG_LEVEL"

for concurrency in $CONCURRENCY_LEVELS; do
    for run in $(seq 1 $NUM_RUNS); do
        log_dir="./param_batch_c${concurrency}_run${run}_$(date +%H%M%S)"
        
        echo "Running: concurrency=$concurrency, run=$run"
        
        BLOCK_STM_LOG_LEVEL="$LOG_LEVEL" \
        BLOCK_STM_LOG_DIR="$log_dir" \
        cargo run --release -- replay-erc20 \
            --data-path "$DATA_PATH" \
            --concurrency-level "$concurrency" \
            --num-runs 1 \
            --num-warmups 1
    done
done

echo "Parametric batch test completed"
```

### 4.3 结果收集与分析方法

#### 结果汇总脚本

**collect_results.sh** - 结果自动汇总：

```bash
#!/bin/bash
# collect_results.sh - 批量测试结果汇总脚本

set -euo pipefail

# 参数
RESULT_DIR="${1:-.}"
OUTPUT_CSV="${2:-batch_results_summary.csv}"
OUTPUT_REPORT="${3:-batch_results_report.md}"

# 验证目录
if [[ ! -d "$RESULT_DIR" ]]; then
    echo "Error: Result directory not found: $RESULT_DIR"
    exit 1
fi

# 初始化汇总文件
cat > "$OUTPUT_CSV" << EOF
Concurrency,DataFile,Run,TPS,AvgLatency,AbortCount,Duration,Status,LogDir
EOF

echo "Collecting results from: $RESULT_DIR"
echo "Output CSV: $OUTPUT_CSV"
echo "Output Report: $OUTPUT_REPORT"

# 遍历所有日志目录
result_count=0
success_count=0
failed_count=0

for log_dir in "$RESULT_DIR"/*/; do
    if [[ ! -d "$log_dir" ]]; then
        continue
    fi
    
    # 解析目录名获取参数
    dir_name=$(basename "$log_dir")
    
    # 解析格式: c4_ETH_2401_100_run1_20250822_143000
    if [[ $dir_name =~ c([0-9]+)_(.+)_run([0-9]+)_([0-9_]+) ]]; then
        concurrency="${BASH_REMATCH[1]}"
        data_file="${BASH_REMATCH[2]}.csv"
        run="${BASH_REMATCH[3]}"
        timestamp="${BASH_REMATCH[4]}"
    else
        echo "Warning: Cannot parse directory name: $dir_name"
        continue
    fi
    
    # 提取性能指标
    tps="N/A"
    latency="N/A"
    aborts="N/A"
    duration="N/A"
    status="UNKNOWN"
    
    # 从汇总日志提取数据
    if [[ -f "$log_dir/block_stm_summary.log" ]]; then
        # 使用jq解析JSON (如果可用)
        if command -v jq &> /dev/null; then
            tps=$(tail -1 "$log_dir/block_stm_summary.log" | jq -r '.average_tps // "N/A"' 2>/dev/null || echo "N/A")
            latency=$(tail -1 "$log_dir/block_stm_summary.log" | jq -r '.average_latency_ms // "N/A"' 2>/dev/null || echo "N/A")
            aborts=$(tail -1 "$log_dir/block_stm_summary.log" | jq -r '.total_aborts // "N/A"' 2>/dev/null || echo "N/A")
            duration=$(tail -1 "$log_dir/block_stm_summary.log" | jq -r '.total_duration_ms // "N/A"' 2>/dev/null || echo "N/A")
        else
            # 使用grep提取 (备用方案)
            tps=$(grep -o '"average_tps":[0-9.]*' "$log_dir/block_stm_summary.log" | tail -1 | cut -d':' -f2 || echo "N/A")
            latency=$(grep -o '"average_latency_ms":[0-9.]*' "$log_dir/block_stm_summary.log" | tail -1 | cut -d':' -f2 || echo "N/A")
            aborts=$(grep -o '"total_aborts":[0-9]*' "$log_dir/block_stm_summary.log" | tail -1 | cut -d':' -f2 || echo "N/A")
        fi
        status="SUCCESS"
        ((success_count++))
    elif [[ -f "$log_dir/stderr.log" && -s "$log_dir/stderr.log" ]]; then
        status="FAILED"
        ((failed_count++))
    fi
    
    # 记录到CSV
    echo "$concurrency,$data_file,$run,$tps,$latency,$aborts,$duration,$status,$log_dir" >> "$OUTPUT_CSV"
    ((result_count++))
    
    echo "  Processed: $dir_name -> TPS=$tps, Status=$status"
done

echo "\nCollection completed:"
echo "  Total results: $result_count"
echo "  Successful: $success_count"
echo "  Failed: $failed_count"

# 生成Markdown报告
cat > "$OUTPUT_REPORT" << EOF
# Batch Test Results Report

**Generated**: $(date)  
**Source Directory**: $RESULT_DIR  
**Results Summary**: $result_count total, $success_count successful, $failed_count failed

## Performance Summary

### TPS by Concurrency Level

\`\`\`bash
# Generate TPS summary
grep -v "^Concurrency" "$OUTPUT_CSV" | grep "SUCCESS" | \
  awk -F',' '{sum[\$1] += \$4; count[\$1]++} END {for (c in sum) printf "Concurrency %d: Avg TPS %.2f\\n", c, sum[c]/count[c]}' | \
  sort -n
\`\`\`

### Detailed Results

See \`$OUTPUT_CSV\` for complete data.

## Failed Tests

EOF

# 添加失败测试详情
grep "FAILED" "$OUTPUT_CSV" | while IFS=',' read -r concurrency data_file run tps latency aborts duration status log_dir; do
    echo "- Concurrency $concurrency, $data_file, Run $run: Check \`$log_dir/stderr.log\`" >> "$OUTPUT_REPORT"
done

echo "\nResults report generated: $OUTPUT_REPORT"
```

#### 性能分析脚本

**analyze_performance.py** - Python数据分析：

```python
#!/usr/bin/env python3
# analyze_performance.py - 性能数据分析脚本

import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
import sys
from pathlib import Path

def load_results(csv_file):
    """Load batch test results from CSV"""
    try:
        df = pd.read_csv(csv_file)
        # 过滤成功的结果
        df = df[df['Status'] == 'SUCCESS']
        # 转换数据类型
        df['TPS'] = pd.to_numeric(df['TPS'], errors='coerce')
        df['AvgLatency'] = pd.to_numeric(df['AvgLatency'], errors='coerce')
        df['AbortCount'] = pd.to_numeric(df['AbortCount'], errors='coerce')
        return df
    except Exception as e:
        print(f"Error loading results: {e}")
        return None

def analyze_concurrency_scaling(df):
    """Analyze TPS scaling with concurrency"""
    scaling = df.groupby('Concurrency')['TPS'].agg(['mean', 'std', 'min', 'max'])
    print("\n=== Concurrency Scaling Analysis ===")
    print(scaling)
    
    # 计算并行加速比
    baseline_tps = scaling.loc[1, 'mean'] if 1 in scaling.index else scaling['mean'].min()
    scaling['Speedup'] = scaling['mean'] / baseline_tps
    scaling['Efficiency'] = scaling['Speedup'] / scaling.index
    
    print("\n=== Parallel Efficiency ===")
    print(scaling[['Speedup', 'Efficiency']])
    
    return scaling

def plot_performance(df, output_dir='./analysis'):
    """Generate performance plots"""
    Path(output_dir).mkdir(exist_ok=True)
    
    # TPS vs Concurrency
    plt.figure(figsize=(10, 6))
    for data_file in df['DataFile'].unique():
        data_subset = df[df['DataFile'] == data_file]
        concurrency_avg = data_subset.groupby('Concurrency')['TPS'].agg(['mean', 'std'])
        
        plt.errorbar(concurrency_avg.index, concurrency_avg['mean'], 
                    yerr=concurrency_avg['std'], 
                    label=data_file, marker='o', capsize=5)
    
    plt.xlabel('Concurrency Level')
    plt.ylabel('TPS (Transactions per Second)')
    plt.title('TPS vs Concurrency Level')
    plt.legend()
    plt.grid(True, alpha=0.3)
    plt.savefig(f'{output_dir}/tps_vs_concurrency.png', dpi=300, bbox_inches='tight')
    plt.close()
    
    # Latency vs Concurrency
    plt.figure(figsize=(10, 6))
    latency_avg = df.groupby('Concurrency')['AvgLatency'].agg(['mean', 'std'])
    plt.errorbar(latency_avg.index, latency_avg['mean'], 
                yerr=latency_avg['std'], 
                marker='s', capsize=5, color='red')
    
    plt.xlabel('Concurrency Level')
    plt.ylabel('Average Latency (ms)')
    plt.title('Latency vs Concurrency Level')
    plt.grid(True, alpha=0.3)
    plt.savefig(f'{output_dir}/latency_vs_concurrency.png', dpi=300, bbox_inches='tight')
    plt.close()
    
    print(f"\nPlots saved to: {output_dir}/")

def generate_report(df, scaling, output_file='performance_report.md'):
    """Generate comprehensive performance report"""
    with open(output_file, 'w') as f:
        f.write(f"# Performance Analysis Report\n\n")
        f.write(f"**Generated**: {pd.Timestamp.now()}\n\n")
        
        f.write(f"## Test Configuration\n\n")
        f.write(f"- **Concurrency Levels**: {sorted(df['Concurrency'].unique())}\n")
        f.write(f"- **Data Files**: {list(df['DataFile'].unique())}\n")
        f.write(f"- **Total Test Runs**: {len(df)}\n\n")
        
        f.write(f"## Key Findings\n\n")
        max_tps_row = df.loc[df['TPS'].idxmax()]
        f.write(f"- **Peak TPS**: {max_tps_row['TPS']:.2f} at concurrency {max_tps_row['Concurrency']}\n")
        
        if 'Speedup' in scaling.columns:
            max_speedup = scaling['Speedup'].max()
            max_speedup_concurrency = scaling['Speedup'].idxmax()
            f.write(f"- **Max Speedup**: {max_speedup:.2f}x at concurrency {max_speedup_concurrency}\n")
        
        f.write(f"\n## Detailed Results\n\n")
        f.write(scaling.to_markdown())
        
    print(f"\nComprehensive report saved to: {output_file}")

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 analyze_performance.py <results.csv>")
        sys.exit(1)
    
    csv_file = sys.argv[1]
    if not Path(csv_file).exists():
        print(f"Error: File not found: {csv_file}")
        sys.exit(1)
    
    # Load and analyze data
    df = load_results(csv_file)
    if df is None or len(df) == 0:
        print("No valid data to analyze")
        sys.exit(1)
    
    print(f"Loaded {len(df)} successful test results")
    
    # Perform analysis
    scaling = analyze_concurrency_scaling(df)
    
    # Generate plots
    try:
        plot_performance(df)
    except Exception as e:
        print(f"Warning: Could not generate plots: {e}")
    
    # Generate report
    generate_report(df, scaling)

if __name__ == '__main__':
    main()
```

#### 结果对比分析

**compare_results.sh** - 多次测试结果对比：

```bash
#!/bin/bash
# compare_results.sh - 对比多次测试结果

# 参数
COMPARE_DIRS=("$@")

if [[ ${#COMPARE_DIRS[@]} -lt 2 ]]; then
    echo "Usage: $0 <result_dir1> <result_dir2> [result_dir3] ..."
    echo "Example: $0 ./test_results_v1 ./test_results_v2"
    exit 1
fi

# 生成对比报告
OUTPUT_REPORT="comparison_report_$(date +%H%M%S).md"

cat > "$OUTPUT_REPORT" << EOF
# Test Results Comparison Report

**Generated**: $(date)
**Compared Directories**: ${COMPARE_DIRS[*]}

## TPS Comparison Summary

| Concurrency | $(printf "%-15s | " "${COMPARE_DIRS[@]}") Improvement |
|-------------|$(printf "%-15s-|-" "${COMPARE_DIRS[@]}")------------|
EOF

# 提取各目录的TPS数据
for concurrency in 1 2 4 8 16; do
    line="| $concurrency         |"
    
    tps_values=()
    for dir in "${COMPARE_DIRS[@]}"; do
        # 从该目录提取平均TPS
        avg_tps=$(find "$dir" -name "*c${concurrency}_*" -path "*/block_stm_summary.log" -exec grep "average_tps" {} \; 2>/dev/null | \
                 awk -F'[:"]' '{sum+=$4; count++} END {if(count>0) printf "%.2f", sum/count; else print "N/A"}')
        
        tps_values+=("$avg_tps")
        line+=" $(printf "%-13s" "$avg_tps") |"
    done
    
    # 计算改善百分比
    if [[ ${#tps_values[@]} -ge 2 && "${tps_values[0]}" != "N/A" && "${tps_values[1]}" != "N/A" ]]; then
        improvement=$(awk "BEGIN {printf \"%.1f%%\", (${tps_values[1]} - ${tps_values[0]}) / ${tps_values[0]} * 100}")
        line+=" $improvement |"
    else
        line+=" N/A |"
    fi
    
    echo "$line" >> "$OUTPUT_REPORT"
done

cat >> "$OUTPUT_REPORT" << EOF

## Detailed Analysis

### Configuration Comparison

EOF

# 为每个目录生成配置信息
for i in "${!COMPARE_DIRS[@]}"; do
    dir="${COMPARE_DIRS[$i]}"
    echo "#### $dir" >> "$OUTPUT_REPORT"
    
    # 统计信息
    total_tests=$(find "$dir" -maxdepth 1 -type d | wc -l)
    successful_tests=$(find "$dir" -name "block_stm_summary.log" | wc -l)
    
    echo "- Total tests: $((total_tests - 1))" >> "$OUTPUT_REPORT"
    echo "- Successful tests: $successful_tests" >> "$OUTPUT_REPORT"
    echo "" >> "$OUTPUT_REPORT"
done

echo "Comparison report generated: $OUTPUT_REPORT"
```

---

## 第五章：大文件管理方案

### 5.0 快速环境配置（针对用户dengliang）

为了简化配置过程，我们提供了一个一键配置脚本，专门针对用户dengliang的环境进行优化：

**一键配置脚本** (`quick_setup_dengliang.sh`)：

```bash
#!/bin/bash
# quick_setup_dengliang.sh - dengliang用户专用快速配置脚本

set -euo pipefail

echo "=== Aptos Block-STM 环境快速配置 (用户: dengliang) ==="

# 配置参数
USER_HOME="/home/dengliang"
EXTERNAL_STORAGE="/mist/dengliang"
APTOS_PROJECT_DIR="$USER_HOME/aptos-projects"
BUILD_ROOT="$EXTERNAL_STORAGE/aptos-build"

# 1. 检查并创建目录结构
echo "1. 创建目录结构..."
mkdir -p "$APTOS_PROJECT_DIR"
mkdir -p "$BUILD_ROOT"/{target,logs,cache,backup}
mkdir -p "$BUILD_ROOT/cache"/{cargo-registry,sccache}
mkdir -p "$BUILD_ROOT/logs"/{current,archived}
mkdir -p "$EXTERNAL_STORAGE/shared-data/blockchain-data"

echo "   ✓ 目录结构创建完成"

# 2. 配置Cargo全局设置
echo "2. 配置Cargo全局设置..."
mkdir -p "$USER_HOME/.cargo"

cat > "$USER_HOME/.cargo/config.toml" << 'EOF'
[build]
target-dir = "/mist/dengliang/aptos-build/target"
jobs = 8

[env]
CARGO_HOME = "/mist/dengliang/aptos-build/cache/cargo-registry"
SCCACHE_DIR = "/mist/dengliang/aptos-build/cache/sccache"
SCCACHE_CACHE_SIZE = "10G"

[net]
offline = false
retry = 3

[source.crates-io]
replace-with = "ustc"

[source.ustc]
registry = "https://mirrors.ustc.edu.cn/crates.io-index"
EOF

echo "   ✓ Cargo全局配置完成"

# 3. 创建环境变量脚本
echo "3. 创建环境变量脚本..."

cat > "$USER_HOME/aptos_env.sh" << 'EOF'
#!/bin/bash
# Aptos Block-STM 开发环境变量 (dengliang用户专用)

# 基础路径
export APTOS_BUILD_ROOT="/mist/dengliang/aptos-build"
export APTOS_PROJECT_DIR="/home/dengliang/aptos-projects"

# Cargo配置
export CARGO_TARGET_DIR="$APTOS_BUILD_ROOT/target"
export CARGO_HOME="$APTOS_BUILD_ROOT/cache/cargo-registry"

# Sccache配置
export RUSTC_WRAPPER="sccache"
export SCCACHE_DIR="$APTOS_BUILD_ROOT/cache/sccache"
export SCCACHE_CACHE_SIZE="10G"

# Block-STM日志配置
export BLOCK_STM_LOG_DIR="$APTOS_BUILD_ROOT/logs/current/$(date +%Y%m%d)"
export BLOCK_STM_LOG_MAX_SIZE="200"

# Rust性能优化
export RUST_MIN_STACK=8388608
export RUST_BACKTRACE=1
export RAYON_NUM_THREADS=$(nproc)

# 创建日志目录
mkdir -p "$BLOCK_STM_LOG_DIR"

echo "Aptos开发环境已配置:"
echo "  构建根目录: $APTOS_BUILD_ROOT"
echo "  项目目录: $APTOS_PROJECT_DIR"
echo "  目标目录: $CARGO_TARGET_DIR"
echo "  日志目录: $BLOCK_STM_LOG_DIR"
EOF

chmod +x "$USER_HOME/aptos_env.sh"
echo "   ✓ 环境变量脚本创建完成: $USER_HOME/aptos_env.sh"

# 4. 添加到bashrc
echo "4. 配置shell环境..."
if ! grep -q "aptos_env.sh" "$USER_HOME/.bashrc" 2>/dev/null; then
    echo "" >> "$USER_HOME/.bashrc"
    echo "# Aptos Block-STM 开发环境" >> "$USER_HOME/.bashrc"
    echo "source $USER_HOME/aptos_env.sh" >> "$USER_HOME/.bashrc"
    echo "   ✓ 已添加到 .bashrc"
else
    echo "   ✓ .bashrc 已包含环境配置"
fi

# 5. 创建快捷脚本
echo "5. 创建快捷脚本..."

# 快速编译脚本
cat > "$USER_HOME/build_aptos.sh" << 'EOF'
#!/bin/bash
# 快速编译Aptos Block-STM
source /home/dengliang/aptos_env.sh
cd /home/dengliang/aptos-projects/aptos-core/aptos-move/aptos-transaction-benchmarks
RUSTFLAGS="-C target-cpu=native -C opt-level=3" cargo build --release
EOF

# 快速测试脚本
cat > "$USER_HOME/test_aptos.sh" << 'EOF'
#!/bin/bash
# 快速测试Aptos Block-STM
source /home/dengliang/aptos_env.sh
cd /home/dengliang/aptos-projects/aptos-core/aptos-move/aptos-transaction-benchmarks

# 默认测试参数
DATA_FILE="${1:-data/ETH_2401_100.csv}"
CONCURRENCY="${2:-4}"
NUM_RUNS="${3:-1}"

echo "运行测试: 数据文件=$DATA_FILE, 并发度=$CONCURRENCY, 运行次数=$NUM_RUNS"

BLOCK_STM_LOG_LEVEL=INFO \
BLOCK_STM_LOG_DIR="$BLOCK_STM_LOG_DIR" \
cargo run --release -- replay-erc20 \
    --data-path "$DATA_FILE" \
    --concurrency-level "$CONCURRENCY" \
    --num-runs "$NUM_RUNS" \
    --num-warmups 1
EOF

chmod +x "$USER_HOME/build_aptos.sh" "$USER_HOME/test_aptos.sh"
echo "   ✓ 快捷脚本创建完成"

# 6. 设置权限
echo "6. 设置目录权限..."
chmod 755 "$BUILD_ROOT" "$EXTERNAL_STORAGE/shared-data"
chmod 750 "$BUILD_ROOT"/{target,logs,cache,backup}
echo "   ✓ 权限设置完成"

echo "
=== 配置完成 ==="
echo "使用方法:"
echo "1. 重新加载环境: source ~/.bashrc"
echo "2. 或手动加载: source ~/aptos_env.sh"
echo "3. 快速编译: ~/build_aptos.sh"
echo "4. 快速测试: ~/test_aptos.sh [数据文件] [并发度] [运行次数]"
echo "
示例:"
echo "  ~/test_aptos.sh data/ETH_2401_100.csv 4 3"
echo "
注意: 请确保aptos-core项目已克隆到 $APTOS_PROJECT_DIR/aptos-core"
```

**使用方法**：

```bash
# 下载并运行配置脚本
wget -O quick_setup_dengliang.sh [脚本URL]
chmod +x quick_setup_dengliang.sh
./quick_setup_dengliang.sh

# 重新加载环境
source ~/.bashrc

# 验证配置
echo $APTOS_BUILD_ROOT
echo $CARGO_TARGET_DIR
```

### 5.1 源码与编译产物分离策略

#### 问题分析

Aptos Block-STM的完整开发环境面临着显著的存储空间挑战：

**存储空间需求分析**：
- **源码仓库**: ~2-3GB (包含.git历史)
- **编译产物** (target/): 10-15GB (完整debug + release)
- **日志文件**: 1-10GB (根据测试规模)
- **缓存文件**: 2-5GB (cargo和sccache)
- **总计**: 15-35GB

**常见问题**：
- 家目录空间不足 (<20GB配额)
- SSD空间有限但传统硬盘容量大
- 多用户环境下的空间竞争
- 临时文件累积导致磁盘占满

#### 分离策略设计

**推荐目录结构**：

```
用户家目录 (~/) - 保存必要文件：
├── aptos-projects/
│   ├── aptos-core/              # 源码仓库 (git clone)
│   │   ├── .git/                 # Git历史 (必须)
│   │   ├── src/                  # 源代码 (必须)
│   │   ├── .cargo/
│   │   │   └── config.toml       # Cargo配置 (重定向)
│   │   └── Cargo.toml           # 项目配置
│   └── scripts/                 # 开发脚本
└── .cargo/
    └── config.toml             # 全局Cargo配置

外部存储 (/mist/aptos-build/) - 存放大文件：
├── $(whoami)/               # 用户专用目录
│   ├── target/              # 编译产物 (10-15GB)
│   │   ├── debug/           # 调试版本
│   │   ├── release/         # 发布版本
│   │   └── deps/            # 依赖库
│   ├── logs/                # 测试日志 (1-10GB)
│   │   ├── test_logs_**/
│   │   └── archived/        # 归档日志
│   ├── cache/               # 编译缓存
│   │   ├── cargo-registry/   # Cargo缓存
│   │   └── sccache/         # Sccache缓存
│   └── backup/              # 重要结果备份
└── shared-data/             # 共享数据集
    └── blockchain-data/
        ├── ETH_2401_*.csv
        └── USDT_*.csv
```

#### 实施步骤

**1. 初始化外部存储结构**：

```bash
#!/bin/bash
# setup_storage.sh - 存储结构初始化脚本

# 检查外部存储可用性
EXTERNAL_STORAGE="/mist/dengliang"
if [[ ! -d "$EXTERNAL_STORAGE" ]]; then
    echo "Error: External storage not available: $EXTERNAL_STORAGE"
    echo "Please mount external storage or choose different path"
    exit 1
fi

# 创建用户专用目录
USER_BUILD_DIR="$EXTERNAL_STORAGE/aptos-build"
mkdir -p "$USER_BUILD_DIR"/{target,logs,cache,backup}
mkdir -p "$USER_BUILD_DIR/cache"/{cargo-registry,sccache}
mkdir -p "$USER_BUILD_DIR/logs"/{current,archived}

echo "Created user build directory: $USER_BUILD_DIR"

# 设置目录权限
chmod 750 "$USER_BUILD_DIR"              # 用户和组可访问
chmod 755 "$USER_BUILD_DIR"/{target,logs,cache}

# 创建共享数据目录
SHARED_DATA_DIR="$EXTERNAL_STORAGE/shared-data"
mkdir -p "$SHARED_DATA_DIR/blockchain-data"
chmod 755 "$SHARED_DATA_DIR" "$SHARED_DATA_DIR/blockchain-data"

echo "Storage structure initialized successfully"
```

**2. 配置Cargo重定向**：

```toml
# ~/.cargo/config.toml - 全局配置
[build]
target-dir = "/mist/dengliang/aptos-build/target"

[env]
CARGO_HOME = "/mist/dengliang/aptos-build/cache/cargo-registry"

# sccache配置
SCCACHE_DIR = "/mist/dengliang/aptos-build/cache/sccache"
SCCACHE_CACHE_SIZE = "10G"

[net]
offline = false
retry = 3
```

```toml
# aptos-core/.cargo/config.toml - 项目级配置
[build]
# 继承全局配置或覆盖为项目特定路径
target-dir = "/mist/dengliang/aptos-build/target/aptos-core"

[env]
# Block-STM特定环境变量
BLOCK_STM_LOG_DIR = "/mist/dengliang/aptos-build/logs/current"
BLOCK_STM_LOG_MAX_SIZE = "200"
```

**3. 环境变量配置**：

```bash
# aptos_env.sh - Aptos开发环境配置脚本
#!/bin/bash

# 外部存储路径
export APTOS_BUILD_ROOT="/mist/dengliang/aptos-build"

# Cargo配置
export CARGO_TARGET_DIR="$APTOS_BUILD_ROOT/target"
export CARGO_HOME="$APTOS_BUILD_ROOT/cache/cargo-registry"

# Sccache配置
export RUSTC_WRAPPER="sccache"
export SCCACHE_DIR="$APTOS_BUILD_ROOT/cache/sccache"
export SCCACHE_CACHE_SIZE="10G"

# Block-STM日志配置
export BLOCK_STM_LOG_DIR="$APTOS_BUILD_ROOT/logs/current/$(date +%Y%m%d)"
export BLOCK_STM_LOG_MAX_SIZE="200"

# 创建必要目录
mkdir -p "$BLOCK_STM_LOG_DIR"

echo "Aptos development environment configured:"
echo "  Build root: $APTOS_BUILD_ROOT"
echo "  Target dir: $CARGO_TARGET_DIR" 
echo "  Log dir: $BLOCK_STM_LOG_DIR"

# 使用方式：
# source aptos_env.sh
# cd aptos-core && cargo build --release
```

### 5.2 符号链接与目录映射

#### 符号链接策略

**符号链接的优势**：
- 透明性：对应用程序无感，路径操作一致
- 灵活性：可以随时更改目标位置
- 空间效率：不占用额外空间
- 安全性：删除链接不会影响原文件

**创建符号链接**：

```bash
#!/bin/bash
# create_symlinks.sh - 创建符号链接脚本

set -euo pipefail

# 配置参数
APTOS_SOURCE_DIR="/home/dengliang/aptos-projects/aptos-core"
EXTERNAL_BUILD_DIR="/mist/dengliang/aptos-build"

# 检查前置条件
if [[ ! -d "$APTOS_SOURCE_DIR" ]]; then
    echo "Error: Source directory not found: $APTOS_SOURCE_DIR"
    exit 1
fi

if [[ ! -d "$EXTERNAL_BUILD_DIR" ]]; then
    echo "Error: External build directory not found: $EXTERNAL_BUILD_DIR"
    exit 1
fi

# 函数：安全创建符号链接
create_safe_symlink() {
    local target="$1"
    local link="$2"
    local description="$3"
    
    # 删除旧的链接或目录
    if [[ -L "$link" ]]; then
        echo "Removing existing symlink: $link"
        rm "$link"
    elif [[ -d "$link" ]]; then
        echo "Warning: Directory exists: $link"
        read -p "Move to backup? (y/n): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            mv "$link" "${link}_backup_$(date +%Y%m%d_%H%M%S)"
        else
            echo "Skipping: $description"
            return
        fi
    fi
    
    # 创建符号链接
    ln -sf "$target" "$link"
    echo "✓ Created symlink: $link -> $target"
}

# 创建主要符号链接
echo "Creating symlinks for Aptos development environment..."

# 1. target目录链接
create_safe_symlink \
    "$EXTERNAL_BUILD_DIR/target" \
    "$APTOS_SOURCE_DIR/target" \
    "Build target directory"

# 2. 日志目录链接  
create_safe_symlink \
    "$EXTERNAL_BUILD_DIR/logs" \
    "$HOME/aptos-projects/logs" \
    "Log directory"

# 3. 数据文件链接
DATA_DIR="$APTOS_SOURCE_DIR/aptos-move/aptos-transaction-benchmarks/data"
SHARED_DATA="/mist/dengliang/shared-data/blockchain-data"

if [[ -d "$SHARED_DATA" ]]; then
    # 备份现有数据 (如果存在)
    if [[ -d "$DATA_DIR" && ! -L "$DATA_DIR" ]]; then
        cp -r "$DATA_DIR" "${DATA_DIR}_backup_$(date +%Y%m%d_%H%M%S)"
    fi
    
    create_safe_symlink \
        "$SHARED_DATA" \
        "$DATA_DIR" \
        "Shared blockchain data directory"
fi

# 4. 缓存目录链接 (可选)
if [[ ! -d "$HOME/.cargo/registry" ]]; then
    create_safe_symlink \
        "$EXTERNAL_BUILD_DIR/cache/cargo-registry" \
        "$HOME/.cargo/registry" \
        "Cargo registry cache"
fi

echo "\nSymlink creation completed!"
echo "\nCurrent symlink status:"
ls -la "$APTOS_SOURCE_DIR/target" "$HOME/aptos-projects/logs" 2>/dev/null || true
ls -la "$DATA_DIR" 2>/dev/null || true
```

#### 目录映射验证

**验证脚本**：

```bash
#!/bin/bash
# verify_setup.sh - 验证目录映射设置

echo "=== Aptos Storage Setup Verification ==="

# 检查关键环境变量
echo "\n1. Environment Variables:"
echo "   CARGO_TARGET_DIR: ${CARGO_TARGET_DIR:-'Not set'}"
echo "   CARGO_HOME: ${CARGO_HOME:-'Not set'}"
echo "   BLOCK_STM_LOG_DIR: ${BLOCK_STM_LOG_DIR:-'Not set'}"

# 检查目录映射
echo "\n2. Directory Mappings:"

check_mapping() {
    local description="$1"
    local path="$2"
    
    if [[ -L "$path" ]]; then
        local target=$(readlink -f "$path")
        echo "   $description: $path -> $target"
        if [[ -d "$target" ]]; then
            echo "     ✓ Target exists and is accessible"
            du -sh "$target" 2>/dev/null | sed 's/^/     Size: /'
        else
            echo "     ✗ Target does not exist"
        fi
    elif [[ -d "$path" ]]; then
        echo "   $description: $path (regular directory)"
    else
        echo "   $description: $path (not found)"
    fi
}

check_mapping "Target directory" "/home/dengliang/aptos-projects/aptos-core/target"
check_mapping "Log directory" "/home/dengliang/aptos-projects/logs"
check_mapping "Data directory" "/home/dengliang/aptos-projects/aptos-core/aptos-move/aptos-transaction-benchmarks/data"

# 检查磁盘空间
echo "\n3. Disk Space Usage:"
echo "   Home directory:"
df -h /home/dengliang | sed 's/^/     /'
echo "   External storage:"
df -h /mist/dengliang 2>/dev/null | sed 's/^/     /' || echo "     External storage not accessible"

# 测试写入权限
echo "\n4. Write Permission Tests:"

test_write_permission() {
    local description="$1"
    local test_dir="$2"
    
    if [[ -d "$test_dir" ]]; then
        local test_file="$test_dir/.write_test_$"
        if touch "$test_file" 2>/dev/null; then
            rm -f "$test_file"
            echo "   $description: ✓ Writable"
        else
            echo "   $description: ✗ Not writable"
        fi
    else
        echo "   $description: Directory not found"
    fi
}

test_write_permission "Target directory" "${CARGO_TARGET_DIR:-/home/dengliang/aptos-projects/aptos-core/target}"
test_write_permission "Log directory" "${BLOCK_STM_LOG_DIR:-/home/dengliang/aptos-projects/logs}"

# 编译测试
echo "\n5. Compilation Test:"
if [[ -d "/home/dengliang/aptos-projects/aptos-core" ]]; then
    echo "   Running quick compilation check..."
    cd /home/dengliang/aptos-projects/aptos-core/aptos-move/aptos-transaction-benchmarks
    if timeout 30 cargo check --release &>/dev/null; then
        echo "   ✓ Compilation check passed"
    else
        echo "   ✗ Compilation check failed or timed out"
    fi
else
    echo "   Source directory not found"
fi

echo "\n=== Verification Complete ==="
```

### 5.3 磁盘空间优化建议

#### 自动清理策略

**智能清理脚本**：

```bash
#!/bin/bash
# smart_cleanup.sh - 智能磁盘空间清理

set -euo pipefail

# 配置参数
BUILD_ROOT="/mist/dengliang/aptos-build"
MAX_TARGET_SIZE_GB=15        # target目录最大允许大小
MAX_LOG_AGE_DAYS=7           # 日志文件保留天数  
MAX_CACHE_SIZE_GB=5          # 缓存最大允许大小
COMPRESS_LOGS_OLDER_THAN=1   # 压缩多少天前的日志

# 工具函数
get_dir_size_gb() {
    local dir="$1"
    if [[ -d "$dir" ]]; then
        du -sg "$dir" 2>/dev/null | cut -f1 || echo "0"
    else
        echo "0"
    fi
}

format_size() {
    local size_gb="$1"
    if [[ $size_gb -gt 1024 ]]; then
        echo "$(echo "scale=2; $size_gb / 1024" | bc)TB"
    else
        echo "${size_gb}GB"
    fi
}

log_action() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$BUILD_ROOT/cleanup.log"
}

# 检查环境
if [[ ! -d "$BUILD_ROOT" ]]; then
    echo "Error: Build root directory not found: $BUILD_ROOT"
    exit 1
fi

cd "$BUILD_ROOT"

log_action "Starting intelligent cleanup for $BUILD_ROOT"

# 1. 清理过旧的日志文件
echo "\n=== Log Files Cleanup ==="

if [[ -d "logs" ]]; then
    log_size_before=$(get_dir_size_gb "logs")
    
    # 删除过旧日志
    find logs -name "*.log" -type f -mtime +$MAX_LOG_AGE_DAYS -delete 2>/dev/null || true
    find logs -type d -empty -mtime +$MAX_LOG_AGE_DAYS -delete 2>/dev/null || true
    
    # 压缩较旧日志
    find logs -name "*.log" -type f -mtime +$COMPRESS_LOGS_OLDER_THAN ! -name "*.gz" -exec gzip {} \; 2>/dev/null || true
    
    log_size_after=$(get_dir_size_gb "logs")
    log_action "Log cleanup: $(format_size $log_size_before) -> $(format_size $log_size_after)"
fi

# 2. 清理编译产物
echo "\n=== Build Artifacts Cleanup ==="

if [[ -d "target" ]]; then
    target_size=$(get_dir_size_gb "target")
    
    if [[ $target_size -gt $MAX_TARGET_SIZE_GB ]]; then
        log_action "Target directory size ($(format_size $target_size)) exceeds limit ($(format_size $MAX_TARGET_SIZE_GB))"
        
        # 清理策略：保留release，清理debug
        if [[ -d "target/debug" ]]; then
            debug_size=$(get_dir_size_gb "target/debug")
            rm -rf "target/debug"
            log_action "Removed debug build ($(format_size $debug_size) freed)"
        fi
        
        # 清理旧的依赖库
        if [[ -d "target/release/deps" ]]; then
            find "target/release/deps" -name "*.rlib" -atime +30 -delete 2>/dev/null || true
            log_action "Cleaned old dependency libraries"
        fi
        
        # 清理中间文件
        find "target" -name "*.dSYM" -type d -exec rm -rf {} \; 2>/dev/null || true
        find "target" -name "*.pdb" -delete 2>/dev/null || true
        
        target_size_after=$(get_dir_size_gb "target")
        log_action "Target cleanup: $(format_size $target_size) -> $(format_size $target_size_after)"
    fi
fi

# 3. 清理缓存文件
echo "\n=== Cache Cleanup ==="

if [[ -d "cache" ]]; then
    cache_size=$(get_dir_size_gb "cache")
    
    if [[ $cache_size -gt $MAX_CACHE_SIZE_GB ]]; then
        log_action "Cache size ($(format_size $cache_size)) exceeds limit ($(format_size $MAX_CACHE_SIZE_GB))"
        
        # 清理sccache
        if [[ -d "cache/sccache" ]]; then
            sccache --stop-server 2>/dev/null || true
            find "cache/sccache" -type f -atime +14 -delete 2>/dev/null || true
        fi
        
        # 清理cargo缓存
        if [[ -d "cache/cargo-registry" ]]; then
            # 保留最近使用的包
            find "cache/cargo-registry" -name "*.crate" -atime +60 -delete 2>/dev/null || true
            find "cache/cargo-registry" -type d -empty -delete 2>/dev/null || true
        fi
        
        cache_size_after=$(get_dir_size_gb "cache")
        log_action "Cache cleanup: $(format_size $cache_size) -> $(format_size $cache_size_after)"
    fi
fi

# 4. 整理备份文件
echo "\n=== Backup Management ==="

if [[ -d "backup" ]]; then
    # 保留最近5个备份文件
    ls -t backup/ | tail -n +6 | xargs -r -I {} rm -f "backup/{}"
    log_action "Cleaned old backup files (kept latest 5)"
fi

# 5. 生成清理报告
echo "\n=== Cleanup Summary ==="

TOTAL_SIZE=$(get_dir_size_gb "$BUILD_ROOT")
log_action "Total build directory size: $(format_size $TOTAL_SIZE)"

# 显示各目录大小
echo "Directory breakdown:"
for dir in target logs cache backup; do
    if [[ -d "$dir" ]]; then
        size=$(get_dir_size_gb "$dir")
        printf "  %-10s: %s\n" "$dir" "$(format_size $size)"
    fi
done

log_action "Cleanup completed"
```

#### 监控和告警系统

**磁盘空间监控脚本**：

```bash
#!/bin/bash
# disk_monitor.sh - 磁盘空间监控和告警

# 配置参数
BUILD_ROOT="/mist/dengliang/aptos-build"
WARNING_THRESHOLD=80    # 告警阈值 (百分比)
CRITICAL_THRESHOLD=90   # 关键阈值 (百分比)
LOG_FILE="$BUILD_ROOT/disk_monitor.log"
EMAIL="dengliang@company.com"  # 告警邮箱 (可选)

# 检查磁盘使用率
check_disk_usage() {
    local path="$1"
    local description="$2"
    
    local usage=$(df "$path" | tail -1 | awk '{print $5}' | sed 's/%//')
    local available=$(df -h "$path" | tail -1 | awk '{print $4}')
    
    echo "$description: ${usage}% used, ${available} available"
    
    if [[ $usage -ge $CRITICAL_THRESHOLD ]]; then
        echo "CRITICAL: $description disk usage is ${usage}%" | tee -a "$LOG_FILE"
        # 自动触发清理
        echo "Triggering automatic cleanup..."
        "$(dirname "$0")/smart_cleanup.sh"
        return 2
    elif [[ $usage -ge $WARNING_THRESHOLD ]]; then
        echo "WARNING: $description disk usage is ${usage}%" | tee -a "$LOG_FILE"
        return 1
    else
        echo "OK: $description disk usage is ${usage}%"
        return 0
    fi
}

# 主监控逻辑
echo "=== Disk Space Monitoring $(date) ==="

# 检查家目录
home_status=0
if check_disk_usage "/home/dengliang" "Home directory"; then
    home_status=0
else
    home_status=$?
fi

# 检查外部存储
external_status=0
if [[ -d "/mist/dengliang" ]]; then
    if check_disk_usage "/mist/dengliang" "External storage"; then
        external_status=0
    else
        external_status=$?
    fi
else
    echo "External storage not accessible"
    external_status=0
fi

# 检查大文件
echo "\n=== Large Files Analysis ==="
if [[ -d "$BUILD_ROOT" ]]; then
    echo "Top 10 largest files/directories in build root:"
    du -ah "$BUILD_ROOT" 2>/dev/null | sort -hr | head -10 | sed 's/^/  /'
fi

# 生成告警
max_status=$((home_status > external_status ? home_status : external_status))

case $max_status in
    0) echo "\n✓ All disk usage levels are normal" ;;
    1) 
        echo "\n⚠ WARNING: Disk space is running low"
        echo "Consider running cleanup: $(dirname "$0")/smart_cleanup.sh"
        ;;
    2) 
        echo "\n❗ CRITICAL: Disk space is critically low"
        echo "Automatic cleanup has been triggered"
        ;;
esac

echo "Monitor log: $LOG_FILE"
```

#### 定期维护任务

**Cron任务配置**：

```bash
# 安装定期维护任务
# install_cron_jobs.sh

#!/bin/bash

# 获取脚本目录
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# 创建临时crontab文件
TMP_CRONTAB=$(mktemp)

# 导入现有的crontab任务
crontab -l 2>/dev/null > "$TMP_CRONTAB" || true

# 添加Aptos维护任务
cat >> "$TMP_CRONTAB" << EOF

# Aptos Build Environment Maintenance
# Daily cleanup at 2 AM
0 2 * * * $SCRIPT_DIR/smart_cleanup.sh >/dev/null 2>&1

# Hourly disk space monitoring during work hours
0 9-18 * * 1-5 $SCRIPT_DIR/disk_monitor.sh >/dev/null 2>&1

# Weekly deep cleanup on Sunday 3 AM  
0 3 * * 0 $SCRIPT_DIR/deep_cleanup.sh >/dev/null 2>&1

# Monthly cache rebuild on first Sunday 4 AM
0 4 1-7 * 0 $SCRIPT_DIR/rebuild_cache.sh >/dev/null 2>&1
EOF

# 安装新的crontab
crontab "$TMP_CRONTAB"
rm "$TMP_CRONTAB"

echo "Cron jobs installed successfully"
echo "Current crontab:"
crontab -l | grep -A 10 "Aptos Build Environment Maintenance"
```

**手动维护指南**：

```bash
#!/bin/bash
# maintenance_guide.sh - 手动维护指南

cat << 'EOF'
# Aptos Build Environment Manual Maintenance Guide

## Daily Tasks (5 minutes)
1. Check disk space: ./disk_monitor.sh
2. Review recent logs: tail -50 /mist/aptos-build/$(whoami)/cleanup.log
3. Quick cleanup if needed: ./smart_cleanup.sh

## Weekly Tasks (15 minutes)
1. Deep cleanup: ./deep_cleanup.sh
2. Update dependencies: cd aptos-core && cargo update
3. Rebuild if necessary: cargo build --release
4. Archive important results: cp -r important_results/ backup/

## Monthly Tasks (30 minutes)
1. Full cache rebuild: ./rebuild_cache.sh
2. Update Rust toolchain: rustup update
3. Review and optimize configurations
4. Clean up backup directory: ls -la backup/

## Emergency Procedures

### Disk Full Emergency
1. Immediate: rm -rf /mist/dengliang/aptos-build/target/debug
2. Quick wins: ./smart_cleanup.sh
3. Manual review: du -sh /mist/dengliang/aptos-build/* | sort -hr

### Performance Issues
1. Check if SSD is full: df -h /
2. Move logs to slower storage: mv logs/* /backup/logs/
3. Rebuild with optimizations: cargo build --release

### Recovery Procedures
1. Restore from backup: cp -r backup/important_results/ ./
2. Rebuild symlinks: ./create_symlinks.sh
3. Verify setup: ./verify_setup.sh

EOF
```

### 5.4 dengliang用户专用维护工具

为了方便用户dengliang的日常维护，我们提供了一套专门的维护脚本：

**日常维护脚本** (`daily_maintenance_dengliang.sh`)：

```bash
#!/bin/bash
# daily_maintenance_dengliang.sh - dengliang用户专用日常维护脚本

set -euo pipefail

# 配置参数
BUILD_ROOT="/mist/dengliang/aptos-build"
HOME_DIR="/home/dengliang"
LOG_FILE="$BUILD_ROOT/maintenance.log"

# 日志函数
log_message() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" | tee -a "$LOG_FILE"
}

echo "=== Aptos Block-STM 日常维护 (用户: dengliang) ==="
log_message "开始日常维护"

# 1. 检查磁盘空间
echo "\n1. 磁盘空间检查"
HOME_USAGE=$(df /home/dengliang | tail -1 | awk '{print $5}' | sed 's/%//')
MIST_USAGE=$(df /mist/dengliang | tail -1 | awk '{print $5}' | sed 's/%//')

echo "   家目录使用率: ${HOME_USAGE}%"
echo "   外部存储使用率: ${MIST_USAGE}%"

if [[ $HOME_USAGE -gt 80 ]]; then
    log_message "警告: 家目录使用率过高 (${HOME_USAGE}%)"
fi

if [[ $MIST_USAGE -gt 85 ]]; then
    log_message "警告: 外部存储使用率过高 (${MIST_USAGE}%)"
    echo "   触发自动清理..."
    
    # 自动清理debug构建
    if [[ -d "$BUILD_ROOT/target/debug" ]]; then
        rm -rf "$BUILD_ROOT/target/debug"
        log_message "已清理debug构建目录"
    fi
fi

# 2. 清理旧日志
echo "\n2. 日志文件清理"
if [[ -d "$BUILD_ROOT/logs" ]]; then
    # 删除7天前的日志
    find "$BUILD_ROOT/logs" -name "*.log" -mtime +7 -delete 2>/dev/null || true
    # 压缩3天前的日志
    find "$BUILD_ROOT/logs" -name "*.log" -mtime +3 ! -name "*.gz" -exec gzip {} \; 2>/dev/null || true
    
    log_count=$(find "$BUILD_ROOT/logs" -name "*.log*" | wc -l)
    echo "   当前日志文件数量: $log_count"
    log_message "日志清理完成，当前文件数: $log_count"
fi

# 3. 缓存管理
echo "\n3. 缓存管理"
if [[ -d "$BUILD_ROOT/cache" ]]; then
    cache_size=$(du -sh "$BUILD_ROOT/cache" | cut -f1)
    echo "   缓存目录大小: $cache_size"
    
    # 清理sccache统计
    if command -v sccache >/dev/null 2>&1; then
        sccache_stats=$(sccache --show-stats 2>/dev/null || echo "sccache不可用")
        echo "   Sccache状态: $sccache_stats"
    fi
fi

# 4. 检查构建状态
echo "\n4. 构建状态检查"
if [[ -d "$BUILD_ROOT/target/release" ]]; then
    target_size=$(du -sh "$BUILD_ROOT/target/release" | cut -f1)
    echo "   Release构建大小: $target_size"
    
    # 检查关键二进制文件
    benchmark_bin="$BUILD_ROOT/target/release/aptos-transaction-benchmarks"
    if [[ -f "$benchmark_bin" ]]; then
        bin_date=$(stat -c %y "$benchmark_bin" 2>/dev/null || stat -f %Sm "$benchmark_bin" 2>/dev/null || echo "未知")
        echo "   基准测试二进制文件: 存在 (修改时间: $bin_date)"
    else
        echo "   基准测试二进制文件: 不存在，可能需要重新编译"
        log_message "警告: 基准测试二进制文件不存在"
    fi
fi

# 5. 环境变量检查
echo "\n5. 环境变量检查"
source "$HOME_DIR/aptos_env.sh" 2>/dev/null || echo "   警告: 无法加载环境变量脚本"

echo "   CARGO_TARGET_DIR: ${CARGO_TARGET_DIR:-'未设置'}"
echo "   BLOCK_STM_LOG_DIR: ${BLOCK_STM_LOG_DIR:-'未设置'}"
echo "   SCCACHE_DIR: ${SCCACHE_DIR:-'未设置'}"

# 6. 生成维护报告
echo "\n6. 维护报告"
echo "=== 维护报告 $(date) ===" >> "$LOG_FILE"
echo "家目录使用率: ${HOME_USAGE}%" >> "$LOG_FILE"
echo "外部存储使用率: ${MIST_USAGE}%" >> "$LOG_FILE"
echo "构建目录大小: $(du -sh $BUILD_ROOT 2>/dev/null | cut -f1 || echo '未知')" >> "$LOG_FILE"
echo "" >> "$LOG_FILE"

log_message "日常维护完成"
echo "\n维护日志: $LOG_FILE"
```

**快速问题诊断脚本** (`diagnose_dengliang.sh`)：

```bash
#!/bin/bash
# diagnose_dengliang.sh - dengliang用户专用问题诊断脚本

echo "=== Aptos Block-STM 环境诊断 (用户: dengliang) ==="

# 基础路径
BUILD_ROOT="/mist/dengliang/aptos-build"
HOME_DIR="/home/dengliang"
PROJECT_DIR="$HOME_DIR/aptos-projects/aptos-core"

# 1. 目录结构检查
echo "\n1. 目录结构检查"
check_dir() {
    local dir="$1"
    local desc="$2"
    if [[ -d "$dir" ]]; then
        echo "   ✓ $desc: $dir (存在)"
    else
        echo "   ✗ $desc: $dir (不存在)"
    fi
}

check_dir "$BUILD_ROOT" "构建根目录"
check_dir "$BUILD_ROOT/target" "目标目录"
check_dir "$BUILD_ROOT/logs" "日志目录"
check_dir "$BUILD_ROOT/cache" "缓存目录"
check_dir "$PROJECT_DIR" "项目目录"

# 2. 权限检查
echo "\n2. 权限检查"
test_write() {
    local dir="$1"
    local desc="$2"
    if [[ -d "$dir" ]]; then
        if touch "$dir/.write_test" 2>/dev/null; then
            rm -f "$dir/.write_test"
            echo "   ✓ $desc: 可写"
        else
            echo "   ✗ $desc: 不可写"
        fi
    else
        echo "   - $desc: 目录不存在"
    fi
}

test_write "$BUILD_ROOT" "构建根目录"
test_write "$BUILD_ROOT/target" "目标目录"
test_write "$BUILD_ROOT/logs" "日志目录"

# 3. 环境变量检查
echo "\n3. 环境变量检查"
if [[ -f "$HOME_DIR/aptos_env.sh" ]]; then
    echo "   ✓ 环境脚本存在: $HOME_DIR/aptos_env.sh"
    source "$HOME_DIR/aptos_env.sh"
    
    echo "   CARGO_TARGET_DIR: ${CARGO_TARGET_DIR:-'未设置'}"
    echo "   BLOCK_STM_LOG_DIR: ${BLOCK_STM_LOG_DIR:-'未设置'}"
    echo "   APTOS_BUILD_ROOT: ${APTOS_BUILD_ROOT:-'未设置'}"
else
    echo "   ✗ 环境脚本不存在: $HOME_DIR/aptos_env.sh"
fi

# 4. Rust工具链检查
echo "\n4. Rust工具链检查"
if command -v rustc >/dev/null 2>&1; then
    echo "   ✓ Rust编译器: $(rustc --version)"
else
    echo "   ✗ Rust编译器未安装"
fi

if command -v cargo >/dev/null 2>&1; then
    echo "   ✓ Cargo: $(cargo --version)"
else
    echo "   ✗ Cargo未安装"
fi

if command -v sccache >/dev/null 2>&1; then
    echo "   ✓ Sccache: $(sccache --version)"
else
    echo "   - Sccache未安装 (可选)"
fi

# 5. 项目状态检查
echo "\n5. 项目状态检查"
if [[ -d "$PROJECT_DIR" ]]; then
    cd "$PROJECT_DIR"
    
    if [[ -f "Cargo.toml" ]]; then
        echo "   ✓ 项目根目录正确"
    else
        echo "   ✗ 项目根目录错误，未找到Cargo.toml"
    fi
    
    benchmark_dir="$PROJECT_DIR/aptos-move/aptos-transaction-benchmarks"
    if [[ -d "$benchmark_dir" ]]; then
        echo "   ✓ 基准测试目录存在"
        
        if [[ -f "$benchmark_dir/Cargo.toml" ]]; then
            echo "   ✓ 基准测试项目配置正确"
        else
            echo "   ✗ 基准测试项目配置错误"
        fi
    else
        echo "   ✗ 基准测试目录不存在"
    fi
else
    echo "   ✗ 项目目录不存在: $PROJECT_DIR"
fi

# 6. 编译测试
echo "\n6. 快速编译测试"
if [[ -d "$PROJECT_DIR/aptos-move/aptos-transaction-benchmarks" ]]; then
    cd "$PROJECT_DIR/aptos-move/aptos-transaction-benchmarks"
    echo "   正在进行编译检查..."
    
    if timeout 60 cargo check --release >/dev/null 2>&1; then
        echo "   ✓ 编译检查通过"
    else
        echo "   ✗ 编译检查失败或超时"
        echo "   建议运行: cd $PROJECT_DIR/aptos-move/aptos-transaction-benchmarks && cargo check"
    fi
else
    echo "   - 跳过编译测试 (项目目录不存在)"
fi

# 7. 磁盘空间检查
echo "\n7. 磁盘空间检查"
echo "   家目录: $(df -h /home/dengliang | tail -1 | awk '{print $3 "/" $2 " (" $5 ")'}' 2>/dev/null || echo '无法检查')"
echo "   外部存储: $(df -h /mist/dengliang | tail -1 | awk '{print $3 "/" $2 " (" $5 ")'}' 2>/dev/null || echo '无法检查')"

if [[ -d "$BUILD_ROOT" ]]; then
    echo "   构建目录大小: $(du -sh $BUILD_ROOT | cut -f1)"
fi

echo "\n=== 诊断完成 ==="
echo "如果发现问题，请参考维护指南或运行 ~/daily_maintenance_dengliang.sh"
```

**安装维护工具脚本**：

```bash
#!/bin/bash
# install_maintenance_tools_dengliang.sh - 安装维护工具

HOME_DIR="/home/dengliang"

# 创建维护脚本
echo "安装dengliang用户专用维护工具..."

# 将上述脚本内容写入文件
cat > "$HOME_DIR/daily_maintenance_dengliang.sh" << 'EOF'
# [这里插入daily_maintenance_dengliang.sh的完整内容]
EOF

cat > "$HOME_DIR/diagnose_dengliang.sh" << 'EOF'
# [这里插入diagnose_dengliang.sh的完整内容]
EOF

# 设置执行权限
chmod +x "$HOME_DIR/daily_maintenance_dengliang.sh"
chmod +x "$HOME_DIR/diagnose_dengliang.sh"

# 创建定时任务
echo "设置定时维护任务..."
(crontab -l 2>/dev/null; echo "0 2 * * * $HOME_DIR/daily_maintenance_dengliang.sh >/dev/null 2>&1") | crontab -

echo "维护工具安装完成！"
echo "使用方法:"
echo "  日常维护: ~/daily_maintenance_dengliang.sh"
echo "  问题诊断: ~/diagnose_dengliang.sh"
echo "  定时任务: 每天凌晨2点自动运行维护"
```

通过以上系统性的大文件管理方案，用户dengliang可以高效地管理Aptos Block-STM开发环境中的存储空间，确保项目开发的持续性和稳定性。

---

## 总结

本指南提供了Aptos Block-STM系统的完整编译运行解决方案，涵盖了从环境配置到性能优化、从单次测试到批量自动化、从小规模开发到大文件管理的全部流程。无论您是初学者还是资深开发者，都可以根据本指南快速上手并获得最佳的性能表现。

### 关键要点回顾

1. **环境配置**：正确的Rust环境和依赖库安装是成功的基础
2. **编译优化**：通过Cargo配置和编译参数可以显著提升性能
3. **测试执行**：掌握正确的参数设置和问题排查方法
4. **批量自动化**：高效的批量测试和结果分析能力
5. **空间管理**：合理的存储策略确保项目的可持续性

希望本指南能够为Aptos Block-STM的研究和开发工作提供有力支持！
