# DM 插件开发指南

## 概述

DM 支持外部插件扩展。插件是放置在 `~/.dm/plugins/` 目录下的可执行文件，执行后将 JSON 结果输出到 stdout，前端自动解析渲染。

## 快速开始

### 1. 创建插件目录

```bash
mkdir -p ~/.dm/plugins
```

### 2. 编写插本

插件必须满足：
- 文件位于 `~/.dm/plugins/` 目录
- 文件具有可执行权限（`chmod +x`）
- 执行后输出合法 JSON 到 stdout
- JSON 格式符合 CheckResult schema

### 3. 最小示例

```bash
#!/bin/bash
echo '{
  "id": "my-check",
  "name": "我的检查",
  "description": "示例插件",
  "category": "自定义",
  "version": "1.0.0",
  "status": "ok",
  "sections": [
    {
      "title": "基本信息",
      "icon": "📋",
      "items": [
        {"type": "label", "key": "状态", "value": "正常", "status": "ok"},
        {"type": "info", "text": "一切正常运行"}
      ]
    }
  ]
}'
```

```bash
chmod +x ~/.dm/plugins/my-check
dm check my-check
```

## JSON Schema

### CheckResult（顶层）

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | string | 是 | 唯一标识 |
| name | string | 是 | 显示名称 |
| description | string | 是 | 简短描述 |
| category | string | 是 | 分类 |
| version | string | 是 | 版本号 |
| status | string | 是 | ok/warn/error/info |
| sections | Section[] | 是 | 检查结果分组 |

### Section

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| title | string | 是 | 分组标题 |
| icon | string | 否 | 图标（emoji） |
| description | string | 否 | 分组描述 |
| items | Item[] | 是 | 数据项 |

### Item 类型

#### label - 键值对
```json
{"type": "label", "key": "CPU", "value": "45%", "status": "ok"}
```

#### bar - 进度条
```json
{"type": "bar", "key": "内存", "value": 45.2, "max": 100, "unit": "%", "status": "ok"}
```

#### table - 表格
```json
{"type": "table", "headers": ["名称", "状态"], "rows": [["nginx", "运行中"], ["redis", "已停止"]]}
```

#### info/warning/error/success - 消息
```json
{"type": "info", "text": "信息提示"}
{"type": "warning", "text": "警告信息"}
{"type": "error", "text": "错误信息"}
{"type": "success", "text": "成功信息"}
```

#### divider - 分割线
```json
{"type": "divider"}
```

### status 字段值

| 值 | 显示 | 用途 |
|----|------|------|
| ok | ✓ 绿色 | 正常 |
| warn | ⚠ 黄色 | 警告 |
| error | ✗ 红色 | 错误 |
| info | ▸ 灰色 | 信息 |

## 样式模板

### 系统检查模板
```json
{
  "sections": [
    {
      "title": "系统信息",
      "icon": "🖥️",
      "items": [
        {"type": "label", "key": "主机名", "value": "server01"},
        {"type": "label", "key": "操作系统", "value": "Ubuntu 24.04"},
        {"type": "label", "key": "内核", "value": "6.8.0-117-generic"}
      ]
    },
    {
      "title": "资源使用",
      "icon": "📊",
      "items": [
        {"type": "bar", "key": "CPU", "value": 45.2, "max": 100, "unit": "%", "status": "ok"},
        {"type": "bar", "key": "内存", "value": 78.5, "max": 100, "unit": "%", "status": "warn"},
        {"type": "bar", "key": "磁盘", "value": 92.1, "max": 100, "unit": "%", "status": "error"}
      ]
    },
    {
      "title": "建议",
      "icon": "💡",
      "items": [
        {"type": "warning", "text": "磁盘使用率超过 90%，建议清理"},
        {"type": "info", "text": "系统运行正常"}
      ]
    }
  ]
}
```

## 最佳实践

1. **id 命名**：使用英文，语义明确，不要太长（如 `disk-check`、`ssl-cert`）
2. **输出编码**：确保 stdout 输出 UTF-8 编码
3. **错误处理**：插件应尽量返回有效 JSON，即使检查失败
4. **执行时间**：插件应在 30 秒内完成
5. **幂等性**：插件应多次执行结果一致
6. **安全性**：不要在插件中硬编码敏感信息
