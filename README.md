# DM 现场维护工具

DM 是面向现场维护、故障定位和问题恢复的一体化运维工具。它提供 Web 控制台和 CLI 两种入口，把脚本执行、常规检查、服务管理、系统告警、规则引擎、维护文档和检查数据导入导出整合到一个 Linux 静态二进制中，适合离线环境、老旧系统和多架构服务器部署。

核心目标很明确：现场先看问题、快速定位证据、给出处理建议，再执行修复动作。所有检查结果都返回结构化数据，Web 负责完整渲染，CLI 负责终端高亮和表格化输出。

## 能力概览

| 模块 | 能力 |
|------|------|
| 仪表盘 | CPU、内存、负载、网络收发趋势，趋势数据持久化保留最近 2 小时 |
| 常规检查 | 系统、资源、网络、安全、服务、中间件、ES、Redis、Nginx、Keepalived、MySQL、Java 服务、业务综合检查 |
| 规则引擎 | 规则列表、检索定位、编辑覆盖、JSON 导入，所有启用规则参与告警和健康体检 |
| 系统告警 | 增量告警、去重归并、级别排序、全字段检索、清空告警、告警铃铛同步 |
| 服务管理 | 自动识别 systemd unit、进程、端口、CPU、内存、类型，支持日志、健康、启动、停止、重启 |
| 服务日志 | 自动尝试 systemd journal、PID journal、进程名、命令行日志参数、程序目录、配置路径和常见日志目录 |
| 脚本中心 | 内置脚本、用户脚本上传、更新、删除、执行、历史记录、编号搜索 |
| 维护管理 | 维护文档和维护记录分离，支持文档增删改查和记录闭环 |
| 设置 | 监听配置、目录迁移、主题、统一连接配置 JSON 模板下载和导入 |
| CLI | 与 Web 共享检查、规则、配置、脚本和导出能力，支持命令补全 |
| 发布 | GitHub Actions 自动多架构 musl 静态打包并发布 Release |

## 快速安装

下载对应架构的 zip 包后解压安装：

```bash
unzip dm-x86_64-unknown-linux-musl.zip
cd dm-x86_64-unknown-linux-musl
sudo bash install.sh
```

安装后默认路径：

| 内容 | 路径 |
|------|------|
| 二进制 | `/usr/bin/dm` |
| 用户脚本 | `~/.dm/scripts/` |
| 日志目录 | `~/.dm/logs/` |
| 数据目录 | `~/.dm/data/` |
| 配置文件 | `~/.dm/.dm.toml` |

## 启动 Web 控制台

```bash
dm serve
```

默认监听：

```text
0.0.0.0:3399
```

指定端口或仅本机访问：

```bash
dm serve --port 3399 --bind 0.0.0.0
dm serve --port 3399 --bind 127.0.0.1
```

启动后浏览器访问：

```text
http://服务器IP:3399
```

## 常用 CLI

```bash
# 脚本
dm list
dm list --search nginx
dm info security
dm run security

# 常规检查
dm check system
dm check elasticsearch
dm check redis
dm check mysql
dm check business-check
dm check-export -o dm-checks.json

# 连接配置
dm check-config get elasticsearch
dm check-config set elasticsearch url=http://127.0.0.1:9200 username=elastic password=secret
dm check-config template -o dm-check-config-template.json
dm check-config import dm-check-config-template.json

# 文档和记录
dm docs list
dm maintenance list

# shell 补全
dm completions bash > /etc/bash_completion.d/dm
```

## 统一连接配置

ES、Redis、MySQL、Nginx、Keepalived、Java 服务等检查会读取数据库中的连接配置。路径类字段可以留空，系统会根据进程、systemd、命令行参数、配置文件和常见目录自动推断。

在 Web 中进入“系统设置”，点击“下载模板”，编辑后点击“导入 JSON”。CLI 也可以直接生成模板：

```bash
dm check-config template -o dm-check-config-template.json
```

模板结构：

```json
{
  "version": 1,
  "configs": {
    "elasticsearch": {
      "url": "http://127.0.0.1:9200",
      "host": "127.0.0.1",
      "port": "9200",
      "username": "",
      "password": "",
      "config_path": "",
      "data_path": "",
      "log_path": "",
      "program_path": ""
    },
    "redis": {
      "host": "127.0.0.1",
      "port": "6379",
      "password": ""
    },
    "mysql": {
      "host": "127.0.0.1",
      "port": "3306",
      "username": "root",
      "password": ""
    },
    "nginx": {},
    "keepalived": {},
    "java-service": {
      "service_prefix": "order-,pay-,gateway-"
    }
  }
}
```

导入后配置会持久化到 SQLite，Web 和 CLI 的所有相关检查都会读取同一份配置。

## 服务管理和日志定位

服务管理会优先识别 systemd unit，例如 `nginx.service`、`redis@6379.service`，不会只显示进程路径。服务日志查看会自动尝试以下来源：

| 来源 | 说明 |
|------|------|
| `journalctl -u <unit>` | systemd unit 日志 |
| `journalctl _PID=<pid>` | 指定 PID 日志 |
| `journalctl _COMM=<process>` | 进程名日志 |
| 命令行参数 | `--log-file`、`-Dlogging.file.name`、`path.logs` 等 |
| 配置文件 | ES、Nginx、Redis、MySQL 等配置中的日志路径 |
| 常见目录 | `/var/log`、`/opt/*/logs`、`/data/*/logs` |

## Elasticsearch 检查

ES 检查优先支持 7.x，并兼容旧版本常用接口。检查内容包括：

| 内容 | 数据来源 |
|------|----------|
| 连接信息 | 配置数据库、默认端口、HTTP API |
| 集群健康 | `/_cluster/health`，`green=正常`、`yellow=告警`、`red=错误` |
| 节点与存储 | `/_nodes/process,jvm,settings,fs,http` |
| 索引状态 | `/_cat/indices` |
| 分片状态 | `/_cat/shards` |
| 当前任务 | `/_tasks` |
| 备份还原 | `/_snapshot` |
| 配置路径 | 显式配置、进程参数、默认路径 |
| 数据路径 | `path.data`、进程参数、默认路径 |
| 日志路径 | `path.logs`、进程参数、默认日志目录 |

## 自定义脚本

每个脚本是一个独立目录，放在 `~/.dm/scripts/` 或项目 `scripts/` 下：

```text
~/.dm/scripts/
└── health-check/
    ├── health-check.sh
    └── .dm.toml
```

`.dm.toml` 示例：

```toml
name = "health-check"
description = "磁盘健康检查"
feature = "磁盘、SMART、容量检查"
example = "dm run health-check"
version = "1.0.0"
author = "ops"
category = "系统检查"
```

支持 `.sh`、`.pl`、`.py`、`.js` 以及带 shebang 的可执行文件。

## 构建

本地 x86_64 musl 打包：

```bash
PACKAGE_TARGETS="x86_64-unknown-linux-musl" ./package.sh
```

多架构打包：

```bash
./package.sh
```

默认目标：

| 架构 | Rust target | 包名 |
|------|-------------|------|
| x86_64 | `x86_64-unknown-linux-musl` | `dm-x86_64-unknown-linux-musl.zip` |
| ARM64 | `aarch64-unknown-linux-musl` | `dm-aarch64-unknown-linux-musl.zip` |
| LoongArch64 | `loongarch64-unknown-linux-musl` | `dm-loongarch64-unknown-linux-musl.zip` |
| MIPS | `mips-unknown-linux-musl` | `dm-mips-unknown-linux-musl.zip` |
| MIPS little-endian | `mipsel-unknown-linux-musl` | `dm-mipsel-unknown-linux-musl.zip` |
| MIPS64 | `mips64-unknown-linux-muslabi64` | `dm-mips64-unknown-linux-muslabi64.zip` |
| MIPS64 little-endian | `mips64el-unknown-linux-muslabi64` | `dm-mips64el-unknown-linux-muslabi64.zip` |

MIPS 目标没有预编译 `rust-std` 时会自动使用 nightly `build-std`。CI 使用 `cargo-zigbuild + Zig` 做跨架构链接。

## GitHub Actions Release

仓库已配置 `.github/workflows/release.yml`。推送 `v*` tag 后会自动：

1. 安装 Node、Rust、Zig 和打包工具。
2. 构建前端资源。
3. 对 x86_64 执行 `cargo fmt --check` 和 `cargo test --locked`。
4. 矩阵构建 x86_64、ARM64、LoongArch64、MIPS、MIPS64 等 Linux musl 静态包。
5. 上传 zip 产物。
6. 自动创建 GitHub Release。
7. 自动生成 Release Notes，包含实现的功能、修复的问题、下载包和变更记录。

发布：

```bash
git tag v0.1.0
git push origin v0.1.0
```

也可以在 GitHub Actions 页面手动运行 `Build and Release` workflow，填写 `tag_name`。

建议提交信息：

```text
feat: 新增功能说明
fix: 修复问题说明
新增: 中文功能说明
修复: 中文修复说明
```

## 卸载

```bash
sudo bash uninstall.sh
```

## 项目信息

| 项目 | 内容 |
|------|------|
| 版本 | `0.1.0` |
| 作者 | `xuning` |
| 邮箱 | `gokeeps@qq.com` |
| 许可证 | `MIT` |
