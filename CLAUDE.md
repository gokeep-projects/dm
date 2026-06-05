# DM 项目 AI 知识库

## 项目概述

DM（现场维护工具）是一个离线运维工具，支持 CLI 和 Web 两种方式管理和执行运维脚本。单文件二进制，零外部依赖。

## 技术栈

- **后端**: Rust + Axum + Tokio
- **前端**: Svelte 5 + Vite
- **数据库**: SQLite (rusqlite)
- **嵌入**: rust-embed
- **编译**: musl 静态编译

## 项目结构

```
src/
├── main.rs          # 入口 + CLI 分发
├── config.rs        # 配置文件支持（~/.dm/config.toml）
├── checks/          # Rust-native 检查模块
│   ├── mod.rs       # JSON schema + 插件系统
│   ├── system.rs    # 系统综合检查
│   ├── security.rs  # 安全策略检查
│   ├── network.rs   # 网络诊断
│   ├── resource.rs  # 硬件资源
│   ├── service.rs   # 服务状态
│   ├── environment.rs # 环境信息
│   ├── container.rs # Docker 容器
│   ├── middleware.rs # 中间件
│   ├── schedule.rs  # 定时任务
│   └── smart_check.rs # 智能全量体检
├── cli/             # CLI 命令
│   ├── mod.rs       # CLI 定义
│   ├── check.rs     # dm check 命令
│   ├── list.rs      # dm list 命令
│   ├── info.rs      # dm info 命令
│   ├── run.rs       # dm run 命令
│   ├── serve.rs     # dm serve 命令
│   ├── stats.rs     # dm stats 命令
│   ├── duplicate.rs # dm duplicate 命令
│   ├── clean.rs     # dm clean 命令
│   ├── logs.rs      # dm logs 命令
│   ├── version.rs   # dm version 命令
│   ├── docs_cmd.rs  # dm docs 命令
│   └── util.rs      # 共享工具函数
├── web/             # Web 服务
│   ├── api.rs       # REST API 端点
│   ├── ws.rs        # WebSocket 处理
│   └── mod.rs       # 路由配置
├── db/              # SQLite 数据库
│   └── mod.rs       # 数据库操作
├── docs/            # 维护文档模块
│   └── mod.rs       # 文档 CRUD
├── dashboard/       # 系统信息采集
│   └── mod.rs       # 系统信息
└── script/          # 脚本管理
    ├── mod.rs       # 脚本发现
    ├── executor.rs  # 脚本执行器
    └── metadata.rs  # 元数据解析
```

## 核心架构

### 检查系统

所有检查返回结构化 JSON（CheckResult），包含 sections 和 items。

**Item 类型**: label, bar, table, sparkline, info, warning, error, success, divider

**插件系统**: 放置在 `~/.dm/plugins/` 的可执行文件，返回 JSON 格式的检查结果。

### CLI 命令

| 命令 | 说明 |
|------|------|
| dm check <id> | 执行检查（支持 --json） |
| dm list | 列出脚本 |
| dm info <id> | 脚本详情 |
| dm run <id> | 执行脚本（支持 --timeout） |
| dm stats <id> | 执行统计 |
| dm duplicate <id> <new_id> | 复制脚本 |
| dm clean | 清空执行历史 |
| dm logs <id> | 执行历史 |
| dm docs list/info/create/delete | 维护文档 |
| dm serve | 启动 Web 服务 |

### API 端点

| 端点 | 方法 | 说明 |
|------|------|------|
| /api/checks | GET | 列出所有检查项 |
| /api/checks/:id | GET | 执行检查 |
| /api/scripts | GET | 脚本列表 |
| /api/scripts/:id | GET | 脚本详情 |
| /api/scripts/:id/source | GET | 脚本源码 |
| /api/scripts/:id/stats | GET | 执行统计 |
| /api/scripts/:id/run | POST | 执行脚本 |
| /api/scripts/:id/duplicate | POST | 复制脚本 |
| /api/dashboard/stats | GET | 仪表盘统计 |
| /api/dashboard/history | GET/DELETE | 执行历史 |
| /api/system/info | GET | 系统信息 |
| /ws/exec/:id | WS | 实时日志 |

### SQLite 数据库

存储在 `~/.dm/logs/dm.db`，表结构：

```sql
CREATE TABLE exec_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    script_id TEXT NOT NULL,
    script_name TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    exit_code INTEGER,
    duration_ms INTEGER,
    output_lines INTEGER DEFAULT 0
);
```

### 插件开发

插件是 `~/.dm/plugins/` 下的可执行文件，返回 JSON 到 stdout。支持所有 Item 类型。

### 配置文件

优先级：CLI 参数 > ~/.dm/config.toml > 项目目录/.dm.toml > 默认值

### 关键设计决策

1. **SQLite 丢失不影响可用性** - 所有功能在数据库文件缺失时仍正常工作
2. **检查结果 JSON 格式** - 统一的结构化数据，前端/CLI 都能渲染
3. **插件系统** - 外部可执行文件通过 JSON stdout 与系统集成
4. **WebSocket 心跳** - 每 30 秒 ping 防止连接断开
5. **执行历史持久化** - SQLite + 文件双写，确保数据不丢失
