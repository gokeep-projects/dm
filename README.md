<p align="center">
  <strong>DM</strong>
</p>

<p align="center">
  <strong>现场维护中枢 / Linux 运维控制台 / 离线应急排障平台</strong>
</p>

<p align="center">
  <a href="https://github.com/gokeep-projects/dm/actions/workflows/build.yml">
    <img alt="Build Packages" src="https://github.com/gokeep-projects/dm/actions/workflows/build.yml/badge.svg">
  </a>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-static%20binary-111827?logo=rust">
  <img alt="Linux" src="https://img.shields.io/badge/Linux-x86__64%20%7C%20ARM64-0f172a?logo=linux">
  <img alt="Offline" src="https://img.shields.io/badge/Offline-ready-06b6d4">
</p>

```text
OBSERVE  ->  DIAGNOSE  ->  EXECUTE  ->  VERIFY  ->  RECORD
```

DM 把 Web 控制台、CLI、系统体检、维护脚本、服务管理、流量分析、告警规则和维护记录压进一个 Linux 静态二进制。它面向内网、离线机房、现场交付和生产故障处理，目标是把“看到问题、定位证据、执行动作、留下记录”压缩成一个清晰的操作链路。

![DM Dashboard](docs/images/dm-dashboard.png)

## 一眼看懂

| 场景 | DM 做什么 |
| --- | --- |
| 现场巡检 | 系统概要、资源趋势、Top 进程、健康状态集中展示 |
| 故障定位 | 一键体检、实时日志、结构化结果、告警自动去重 |
| 脚本执行 | 上传脚本、参数表单、重传文件、执行历史、CLI 复制 |
| 服务处置 | 识别进程、systemd、端口、CPU、内存、日志和健康状态 |
| 流量分析 | 导入 PCAP，解析 TCP/UDP/HTTP，格式化 JSON/XML |
| 闭环交付 | 文档、维护记录、检查证据、处理建议统一留存 |

## 启动

```bash
unzip dm-x86_64-unknown-linux-musl.zip
cd dm-x86_64-unknown-linux-musl
sudo bash install.sh
```

```bash
dm serve --bind 0.0.0.0 --port 3399
```

后台模式：

```bash
dm serve -d --bind 0.0.0.0 --port 3399
```

```text
http://服务器IP:3399
```

后台模式文件：

```text
~/.dm/logs/dm-serve-3399.pid
~/.dm/logs/dm-serve-3399.log
```

停止后台服务：

```bash
kill "$(cat ~/.dm/logs/dm-serve-3399.pid)"
```

## CLI 快速入口

```bash
dm list
dm run security
dm check system
dm check redis
dm check-config template -o dm-check-config-template.json
dm check-config import dm-check-config-template.json
```

## 维护脚本

维护脚本是 DM 的核心能力。用户脚本默认独立显示，内置脚本只有手动勾选后才会合并展示。

```text
~/.dm/scripts/
└── restart-nginx/
    ├── restart-nginx.sh
    └── .dm.toml
```

```toml
name = "restart-nginx"
description = "重启 Nginx 并检查端口"
feature = "服务重启、端口确认、日志提示"
example = "dm run restart-nginx"
version = "1.0.0"
author = "ops"
category = "服务管理"
```

支持脚本类型：

```text
sh, bash, zsh, ksh, py, python, pl, perl, js, mjs, rb, lua, php, awk, expect, run, bin
```

## 流量分析边界

| 类型 | 展示方式 |
| --- | --- |
| HTTP | 方法、路径、Host、Header、Body、状态码 |
| TCP/UDP 明文 | 原始可读 payload |
| JSON/XML | 格式化、自动换行 |
| PCAP | 导入分析、导出原始文件 |
| HTTPS/TLS | SNI、TLS 状态、HEX、ASCII |

HTTPS 原包通常是 TLS 密文。没有 session keys、`SSLKEYLOGFILE` 或受信任解密代理时，抓包里不存在 HTTP 明文，DM 不会把密文伪装成明文。

## 构建

本地构建优先使用项目内离线依赖；GitHub Actions 默认联网下载依赖并自动构建 x86_64 和 ARM64 Linux musl 包。

```bash
PACKAGE_TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl" ./package.sh
```

强制离线：

```bash
USE_OFFLINE_DEPS=1 PACKAGE_TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl" ./package.sh
```

输出：

```text
target/packages/dm-x86_64-unknown-linux-musl.zip
target/packages/dm-aarch64-unknown-linux-musl.zip
```

## 默认路径

| 内容 | 路径 |
| --- | --- |
| 二进制 | `/usr/bin/dm` |
| 用户脚本 | `~/.dm/scripts/` |
| 数据目录 | `~/.dm/data/` |
| 日志目录 | `~/.dm/logs/` |
| 配置文件 | `~/.dm/.dm.toml` |

## License

MIT
