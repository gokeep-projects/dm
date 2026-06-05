#!/bin/bash
#
# DM 现场维护工具 — 卸载脚本
# 彻底清除：二进制、配置、脚本、日志
#

set -e

R='\033[0m'; G='\033[32m'; Y='\033[33m'; RD='\033[31m'; CY='\033[36m'

if [ "$(id -u)" -ne 0 ]; then
    echo -e "${RD}错误: 需要 root 权限执行 (sudo)${R}"
    exit 1
fi

DM_HOME="${DM_HOME:-$HOME/.dm}"

echo -e "${CY}╔════════════════════════════════════════════╗${R}"
echo -e "${CY}║       DM 现场维护工具 — 卸载              ║${R}"
echo -e "${CY}╚════════════════════════════════════════════╝${R}"
echo ""

# 确认
echo -e "${Y}将彻底清除以下内容:${R}"
echo "  • /usr/bin/dm"
echo "  • ${DM_HOME}（配置、脚本、日志）"
echo ""
read -p "确认卸载？(y/N) " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo -e "${Y}已取消${R}"
    exit 0
fi

# 1. 删除二进制
echo -ne "${Y}[1/3]${R} 删除 /usr/bin/dm ... "
if [ -f /usr/bin/dm ]; then
    rm -f /usr/bin/dm
    echo -e "${G}✓${R}"
else
    echo -e "${Y}不存在${R}"
fi

# 2. 删除用户数据
echo -ne "${Y}[2/3]${R} 删除 ${DM_HOME} ... "
if [ -d "$DM_HOME" ]; then
    rm -rf "$DM_HOME"
    echo -e "${G}✓${R}"
else
    echo -e "${Y}不存在${R}"
fi

# 3. 验证
echo -ne "${Y}[3/3]${R} 验证 ... "
if command -v dm &>/dev/null; then
    echo -e "${RD}✗ dm 仍在 PATH 中（可能有其他副本）${R}"
    which dm
else
    echo -e "${G}✓ 已彻底卸载${R}"
fi
echo ""
