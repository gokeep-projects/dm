# DM — 现场维护工具

DM 是一个离线运维工具，支持 CLI 和 Web 两种方式管理和执行运维脚本。单文件二进制，零外部依赖。

## 快速安装

```bash
# 1. 下载对应平台的 zip，解压
unzip dm-x86_64-unknown-linux-musl.zip

# 2. 运行安装脚本（需要 root）
sudo bash install.sh

# 3. 验证
dm list
```

安装后：
- 二进制 → `/usr/bin/dm`
- 脚本 → `~/.dm/scripts/`
- 配置 → `~/.dm/.dm.toml`
- 日志 → `~/.dm/logs/`

## 卸载

```bash
sudo bash uninstall.sh
```

清除 `/usr/bin/dm` + `~/.dm/` 全部内容。

## 用法

```bash
dm list                  # 列出所有脚本
dm list -c 系统安全      # 按分类筛选
dm list -s security      # 搜索脚本名/描述

dm info security         # 查看脚本详情（参数、示例）

dm run security          # 执行脚本
dm run security -f       # 每5秒持续刷新
dm run log               # 日志异常聚合分析

dm serve                 # 启动 Web 服务（默认 :3399）
dm serve --port 8080     # 指定端口
```

## 内置脚本

| 脚本 | 功能 | 分类 |
|------|------|------|
| `security` | 系统安全策略检查：SELinux/AppArmor/防火墙/内核加固/端口放行状态 | 系统安全 |
| `log` | 日志异常聚合：按 Java→中间件→系统优先级，进程维度统计 | 日志管理 |
| `sys-info` | 系统综合检查：CPU/内存/磁盘/网络/进程 | 系统检查 |
| `service-info` | 系统服务查询：Java/中间件/系统服务状态 | 服务管理 |

## 脚本优先级顺序

`dm run log` 按 3 级优先级扫描异常日志：

```
优先级 1 — Java（tomcat/spring/gc/catalina 等）
优先级 2 — 中间件（ES/Kafka/MySQL/Nginx/Redis/Caddy/Docker 等）
优先级 3 — 系统（syslog/auth.log/kern.log/messages 等）
```

输出两个表格：
- **分类概览**：每个分类的文件数、错误/警告统计
- **进程维度统计**：按进程聚合，显示异常数、来源日志、关键消息摘要

---

## 增加自定义脚本

### 目录结构

每个脚本是一个独立目录，放在 `~/.dm/scripts/` 或项目 `scripts/` 下：

```
~/.dm/scripts/
├── mycheck/              ← 脚本名 = 目录名
│   ├── mycheck.sh        ← 脚本文件（.sh .pl .py .js 或 shebang 可执行文件）
│   └── .dm.toml          ← 元数据（可选）
└── ...
```

### 支持的脚本语言

| 扩展名 | 解释器 | 备注 |
|--------|--------|------|
| `.sh` | bash | |
| `.pl` | perl | |
| `.py` | python3 | |
| `.js` | node | |
| 无扩展名 | 系统 exec | 依赖 shebang（`#!/usr/bin/ruby` 等） |

### 示例

```bash
mkdir -p ~/.dm/scripts/health-check
```

`~/.dm/scripts/health-check/health-check.sh`:

```bash
#!/bin/bash
# 本脚本检测磁盘健康状态
echo "磁盘使用率:"
df -h /
echo ""
echo "SMART 状态:"
smartctl -H /dev/sda 2>/dev/null || echo "  (smartctl 不可用)"
```

`~/.dm/scripts/health-check/.dm.toml`:

```toml
name = "health-check"
description = "磁盘健康检查：使用率、SMART 状态"
feature = "磁盘健康检测"
example = "dm run health-check"
version = "1.0.0"
author = "运维团队"
category = "系统检查"
```

完成后：

```bash
dm list                # 看到 health-check
dm info health-check   # 查看详情
dm run health-check    # 执行
```

### 使用 Perl（支持中文表格）

参考 `scripts/security/security.pl`，复用边框和表格函数：

```perl
use POSIX qw(strftime);
my ($R, $B, $D, $CY, $G, $Y, $RD, $BL) = (
    "\e[0m", "\e[1m", "\e[2m", "\e[36m",
    "\e[32m", "\e[33m", "\e[31m", "\e[34m"
);
# 然后直接用 top_border、table_row、bot_border 等函数
```

---

## 二次开发

### 技术栈

| 层 | 技术 |
|----|------|
| 后端 | Rust + Axum |
| 前端 | Svelte + Vite + TailwindCSS（可选，Web 模式） |
| 脚本 | bash / Perl / Python / Node / Ruby 等 |
| 编译 | musl 静态编译，零依赖部署 |

### 项目结构

```
dm/
├── src/
│   ├── main.rs           # 入口
│   ├── config.rs          # 配置加载（支持 DM_HOME 环境变量）
│   ├── cli/               # CLI 命令（list/info/run/serve）
│   ├── script/            # 脚本发现、元数据、执行
│   ├── web/               # Web API + WebSocket
│   └── dashboard/         # 系统仪表盘
├── scripts/               # 内置脚本
├── target/packages/       # 编译产物
└── Cargo.toml
```

### 构建

```bash
# 开发模式
cargo build --release

# Linux musl 静态交叉编译
cargo build --release --target x86_64-unknown-linux-musl
cross build --release --target aarch64-unknown-linux-musl
```

### 脚本发现机制

程序启动时按以下顺序搜索脚本：

1. 当前工作目录 `./scripts/`
2. 可执行文件同目录 `scripts/`
3. `$DM_HOME/scripts/`（默认 `~/.dm/scripts/`）
4. `$HOME/.dm/scripts/`

同名脚本以先出现的目录为准。

### 扩展新的脚本解释器

在 `src/script/executor.rs` 的 `resolve_interpreter()` 添加映射：

```rust
Some("rb") => ("ruby".to_string(), vec![script_str]),
```

或在 `src/script/mod.rs` 的 `find_script_file()` 添加扩展名扫描。

---

## 下载

| 平台 | 架构 | 包 |
|------|------|-----|
| Linux (musl, 静态) | x86_64 | `dm-x86_64-unknown-linux-musl.zip` |
| Linux (musl, 静态) | ARM64 | `dm-aarch64-unknown-linux-musl.zip` |

仅发布 Linux musl 静态包，不再提供 Windows 或 glibc/GNU 包。musl 静态编译版本不依赖 glibc 等系统动态库，适合离线和老旧发行版环境部署。
