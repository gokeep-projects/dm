<div align="center">

```
 ██████╗ ███╗   ███╗
 ██╔══██╗████╗ ████║
 ██║  ██║██╔████╔██║
 ██║  ██║██║╚██╔╝██║
 ██████╔╝██║ ╚═╝ ██║
 ╚═════╝ ╚═╝     ╚═╝
```

# DM — Linux 现场维护中枢

**单一二进制 · 零外部依赖 · 完全离线运行**

<br/>

<img src="https://img.shields.io/badge/Rust-1.77+-DEA584?style=for-the-badge&logo=rust&logoColor=white" alt="Rust">&nbsp;
<img src="https://img.shields.io/badge/Linux-x86__64_|_ARM64-FCC624?style=for-the-badge&logo=linux&logoColor=black" alt="Linux">&nbsp;
<img src="https://img.shields.io/badge/OFFLINE-Ready-06B6D4?style=for-the-badge&logo=wifi-off&logoColor=white" alt="Offline">&nbsp;
<img src="https://img.shields.io/badge/STATIC-Musl_Binary-FF6B6B?style=for-the-badge&logo=ghost&logoColor=white" alt="Static">

<br/>

<img src="https://img.shields.io/badge/Svelte-5-FF3E00?style=flat-square&logo=svelte&logoColor=white" alt="Svelte">&nbsp;
<img src="https://img.shields.io/badge/Axum-0.7-111827?style=flat-square&logo=rocket&logoColor=white" alt="Axum">&nbsp;
<img src="https://img.shields.io/badge/SQLite-Bundled-003B57?style=flat-square&logo=sqlite&logoColor=white" alt="SQLite">&nbsp;
<img src="https://img.shields.io/badge/Tokio-Runtime-000000?style=flat-square&logo=tokio&logoColor=white" alt="Tokio">&nbsp;
<img src="https://img.shields.io/badge/Java-Observer-ED8B00?style=flat-square&logo=openjdk&logoColor=white" alt="Java">

<br/><br/>

```
  ◈ OBSERVE ───► DIAGNOSE ───► EXECUTE ───► VERIFY ───► RECORD ◈
```

</div>

---

> 💡 **DM 面向内网机房、离线环境、交付现场和生产故障窗口。**
> Web 可视化操作 + CLI 终端应急，两端共享同一套 Rust 后端能力，无需安装额外排障工具。

<br/>

## ◈ 核心能力

<div align="center">

| | 模块 | 能力描述 |
|:---:|:---:|:---|
| 🖥️ | **仪表盘** | 系统概要 · 资源趋势 · Top 进程 · 告警态势 |
| 🩺 | **系统体检** | 一键体检 · 实时日志 · 去重告警 · 结构化详情 |
| 📜 | **维护脚本** | 用户/内置脚本 · 分类筛选 · 上传执行 · 历史追溯 · CLI 复制 |
| ⚙️ | **服务管理** | 进程识别 · 端口映射 · systemd 管理 · 健康检查 · 状态一致性 |
| 🌐 | **流量分析** | 网卡优选 · PCAP 导入 · HTTP/TCP/UDP 明文视图 · 原始包导出 |
| ☕ | **Java 运行时** | 线程转储 · 堆概要 · 调用树 · CPU 热点 · 锁竞争 · OOM 线索 |
| 📋 | **文档记录** | 维护文档 · 交付记录 · 上传解析 · 证据沉淀 |

</div>

---

## ◈ 架构全览

<div align="center">

```
 ╔═══════════════════════════════════════════════════════════════╗
 ║                         DM Binary                            ║
 ║                 static · musl · zero-dep · single-file       ║
 ╠═════════════════════╦═════════════════════════════════════════╣
 ║      Web UI         ║              CLI                        ║
 ║     Svelte 5        ║    clap · colored · tabled              ║
 ╠═════════════════════╩═════════════════════════════════════════╣
 ║                   Axum · Tokio · WebSocket                   ║
 ╠════════╦═══════════╦═══════════╦═══════════╦═════════════════╣
 ║ Checks ║ Scripts   ║   Docs    ║ Traffic   ║ Java Analyzer   ║
 ╠════════╩═══════════╩═══════════╩═══════════╩═════════════════╣
 ║              SQLite · rust-embed · sysinfo · PCAP            ║
 ╚═══════════════════════════════════════════════════════════════╝
```

</div>

---

## ◈ 快速启动

```bash
# ─── 前台启动 ─────────────────────────────────
dm serve --bind 0.0.0.0 --port 3399

# ─── 后台守护 ─────────────────────────────────
dm serve -d --bind 0.0.0.0 --port 3399
```

<div align="center">

🌐 浏览器访问 **`http://<服务器IP>:3399`**

</div>

<details>
<summary>📂 后台运行时文件</summary>

| 文件 | 路径 |
|:---:|:---|
| PID 文件 | `~/.dm/logs/dm-serve-<port>.pid` |
| 运行日志 | `~/.dm/logs/dm-serve-<port>.log` |

</details>

---

## ◈ ☕ Java 运行时观测

Web 端进入 `Java 堆栈实时分析`，自动发现运行中的 Java 进程，展示 PID、服务名、路径、监听端口。

<div align="center">

```bash
dm java list                                                    # 发现 Java 进程
dm java analyze --pid 12345 --samples 4 --interval-ms 700      # 采样分析
dm java analyze --pid 12345 --json                              # JSON 输出
dm java export --pid 12345 --format raw    -o raw.json          # 原始数据导出
dm java export --pid 12345 --format report -o report.md         # Markdown 报告
dm java export --pid 12345 --format pdf    -o report.pdf        # PDF 报告导出
```

</div>

**分析维度：**

```
 ◈ 线程状态    ◈ CPU 热点    ◈ 调用树      ◈ 对象占用    ◈ 堆概要     ◈ 类直方图
 ◈ 锁竞争      ◈ 线程复杂度  ◈ JDBC 连接   ◈ ES 调用     ◈ Redis 调用 ◈ OOM 线索
 ◈ 综合结论
```

---

## ◈ 📜 维护脚本

支持 `sh` `bash` `python` `perl` `js` `ruby` `lua` `php` `awk` `expect` 及二进制可执行脚本。

**目录规范：**

```
~/.dm/scripts/
└── restart-nginx/
    ├── restart-nginx.sh     ← 可执行脚本
    └── .dm.toml             ← 元数据描述
```

**元数据 (`.dm.toml`)：**

```toml
name        = "restart-nginx"
description = "重启 Nginx 并检查端口"
feature     = "服务重启、端口确认、日志提示"
version     = "1.0.0"
category    = "服务管理"
```

**CLI 操作：**

```bash
dm list                                  # 列出全部脚本
dm info restart-nginx                    # 查看脚本详情
dm run restart-nginx --param force=true  # 执行脚本
dm logs restart-nginx                    # 查看执行历史
dm duplicate restart-nginx new-nginx     # 复制脚本
```

---

## ◈ 🩺 常规检查

```bash
dm check system                          # 系统综合检查
dm check redis                           # Redis 检查
dm check-export -o checks.json           # 导出检查结果
dm check-config template -o tpl.json     # 生成配置模板
dm check-config import tpl.json          # 导入检查配置
dm check-config export -o config.json    # 导出当前配置
```

---

## ◈ 🔧 构建打包

```bash
# ─── 标准构建（自动回落联网依赖）──────────────
PACKAGE_TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl" ./package.sh

# ─── 强制离线构建 ─────────────────────────────
USE_OFFLINE_DEPS=1 \
PACKAGE_TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl" ./package.sh
```

**输出产物：**

```
📦 target/packages/dm-x86_64-unknown-linux-musl.zip
📦 target/packages/dm-aarch64-unknown-linux-musl.zip
```

---

## ◈ 📂 数据路径

| 内容 | 路径 | 说明 |
|:---:|:---|:---|
| 📜 用户脚本 | `~/.dm/scripts/` | 脚本目录，每个脚本一个子目录 |
| 💾 数据目录 | `~/.dm/data/` | SQLite 数据库等持久化数据 |
| 📝 日志目录 | `~/.dm/logs/` | 运行日志与执行历史 |
| ⚙️ 配置文件 | `~/.dm/.dm.toml` | 用户级配置覆盖 |

---

## ◈ CLI 速查

<div align="center">

| 命令 | 说明 |
|:---|:---|
| `dm serve` | 启动 Web 服务 |
| `dm check <id>` | 执行检查（支持 `--json`） |
| `dm list` | 列出脚本 |
| `dm info <id>` | 脚本详情 |
| `dm run <id>` | 执行脚本（支持 `--timeout`） |
| `dm stats <id>` | 执行统计 |
| `dm logs <id>` | 执行历史 |
| `dm duplicate <id> <new>` | 复制脚本 |
| `dm clean` | 清空执行历史 |
| `dm java list` | 列出 Java 进程 |
| `dm java analyze --pid <PID>` | Java 运行时分析 |
| `dm docs list` | 列出维护文档 |

</div>

---

## ◈ API 速查

<div align="center">

| 端点 | 方法 | 说明 |
|:---|:---:|:---|
| `/api/checks` | GET | 列出所有检查项 |
| `/api/checks/:id` | GET | 执行检查 |
| `/api/scripts` | GET | 脚本列表 |
| `/api/scripts/:id` | GET | 脚本详情 |
| `/api/scripts/:id/run` | POST | 执行脚本 |
| `/api/scripts/:id/duplicate` | POST | 复制脚本 |
| `/api/dashboard/stats` | GET | 仪表盘统计 |
| `/api/dashboard/history` | GET | 执行历史 |
| `/api/system/info` | GET | 系统信息 |
| `/ws/exec/:id` | WS | 实时执行日志 |

</div>

---

<div align="center">

<br/>

**Built with ❤️ and Rust**

`MIT License`

</div>
