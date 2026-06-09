# DM

> 现场维护中枢。一个静态二进制，接管体检、脚本、服务、流量、告警和交付记录。

DM 面向离线机房、内网服务器和应急排障现场。它把 Web 控制台、CLI、维护脚本、系统体检、服务管理、流量分析和告警规则收敛到同一套运行时里，部署轻，启动快，信息密度高。

```text
OBSERVE -> DIAGNOSE -> EXECUTE -> VERIFY -> RECORD
```

![DM Dashboard](docs/images/dm-dashboard.png)

## 核心特性

| 能力 | 说明 |
| --- | --- |
| 系统体检 | 一键体检、实时日志、结构化结果、告警去重 |
| 维护脚本 | 上传、编辑、重传文件、参数表单、执行历史、CLI 复制 |
| 服务管理 | 进程、systemd、端口、CPU、内存、日志、健康检查 |
| 流量分析 | TCP/UDP/HTTP 明文解析、PCAP 导入导出、JSON/XML 美化 |
| 系统告警 | 规则命中、聚合去重、详情追踪、处理建议 |
| 维护闭环 | 文档、记录、执行结果、检查证据统一留存 |
| 离线部署 | x86_64 / ARM64 Linux musl 静态包，支持本地 offline 依赖 |

## 快速启动

```bash
unzip dm-x86_64-unknown-linux-musl.zip
cd dm-x86_64-unknown-linux-musl
sudo bash install.sh
```

前台启动：

```bash
dm serve --bind 0.0.0.0 --port 3399
```

后台启动：

```bash
dm serve -d --bind 0.0.0.0 --port 3399
```

访问：

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

## CLI

```bash
dm list
dm run security
dm check system
dm check redis
dm check-config template -o dm-check-config-template.json
dm check-config import dm-check-config-template.json
```

## 维护脚本中心

维护脚本是 DM 的核心。用户脚本默认独立展示，内置脚本只有手动勾选后才会一起显示。

支持上传和识别：

```text
sh, bash, zsh, ksh, py, python, pl, perl, js, mjs, rb, lua, php, awk, expect, run, bin
```

用户脚本目录：

```text
~/.dm/scripts/
└── restart-nginx/
    ├── restart-nginx.sh
    └── .dm.toml
```

`.dm.toml` 示例：

```toml
name = "restart-nginx"
description = "重启 Nginx 并检查端口"
feature = "服务重启、端口确认、日志提示"
example = "dm run restart-nginx"
version = "1.0.0"
author = "ops"
category = "服务管理"
```

## 流量边界

DM 会尽量显示原始明文：

| 类型 | 展示 |
| --- | --- |
| HTTP | 方法、路径、Host、Header、Body、状态码 |
| TCP/UDP 明文 | 原始可读 payload |
| JSON/XML | 格式化、自动换行 |
| PCAP | 导入分析、导出原始文件 |
| HTTPS/TLS | SNI、TLS 状态、HEX、ASCII |

HTTPS 原包通常是 TLS 密文。没有 session keys、`SSLKEYLOGFILE` 或受信任解密代理时，抓包里不存在 HTTP 明文，DM 不会把密文伪装成明文。

## 构建

本地构建默认自动选择依赖来源：

- 检测到 `offline/npm-cache` 和 `offline/cargo/vendor` 时优先使用项目内离线依赖。
- 本地没有完整 offline 依赖时联网下载。
- GitHub Actions 默认联网下载依赖并构建 x86_64 / ARM64 包。

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
