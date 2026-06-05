#!/bin/bash
#
# DM 现场维护工具 — 安装脚本
# 安装到 /usr/bin/dm，脚本部署到 ~/.dm/scripts/
#

set -e

DM_BIN="${DM_BIN:-dm}"
DM_HOME="${DM_HOME:-$HOME/.dm}"
BIN_DIR="/usr/bin"

# 颜色
R='\033[0m'; G='\033[32m'; Y='\033[33m'; RD='\033[31m'; CY='\033[36m'

# 检查 root
if [ "$(id -u)" -ne 0 ]; then
    echo -e "${RD}错误: 需要 root 权限执行 (sudo)${R}"
    exit 1
fi

echo -e "${CY}╔════════════════════════════════════════════╗${R}"
echo -e "${CY}║       DM 现场维护工具 — 安装              ║${R}"
echo -e "${CY}╚════════════════════════════════════════════╝${R}"
echo ""

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── 1. 安装二进制 ──
echo -e "${Y}[1/4]${R} 安装 dm 到 ${BIN_DIR}/ ..."
if [ -f "$SCRIPT_DIR/dm" ]; then
    cp "$SCRIPT_DIR/dm" "${BIN_DIR}/dm"
    chmod 755 "${BIN_DIR}/dm"
    echo -e "  ${G}✓${R} ${BIN_DIR}/dm"
else
    echo -e "  ${RD}✗ 未找到 dm 二进制文件${R}"
    echo "    请将 dm 二进制放在脚本同目录下"
    exit 1
fi

# ── 2. 部署默认配置 ──
echo -e "${Y}[2/4]${R} 创建配置目录 ${DM_HOME}/ ..."
mkdir -p "${DM_HOME}/logs"
mkdir -p "${DM_HOME}/scripts"

# 写入默认配置
if [ -f "$SCRIPT_DIR/dm.conf.default" ]; then
    cp "$SCRIPT_DIR/dm.conf.default" "${DM_HOME}/.dm.toml"
    echo -e "  ${G}✓${R} ${DM_HOME}/.dm.toml"
else
    # 生成默认配置
    cat > "${DM_HOME}/.dm.toml" << 'EOF'
[dm]
# DM_HOME 目录（脚本、日志、配置存放位置）
# 留空则使用 ~/.dm

[scripts]
# 脚本搜索路径（多个用逗号分隔）
# 当前目录 scripts/ 优先级高于此路径
dirs = ["~/.dm/scripts"]

[server]
port = 3399
bind = "0.0.0.0"
EOF
    echo -e "  ${G}✓${R} ${DM_HOME}/.dm.toml (默认)"
fi

# ── 3. 部署脚本 ──
echo -e "${Y}[3/4]${R} 部署脚本到 ${DM_HOME}/scripts/ ..."
if [ -d "$SCRIPT_DIR/scripts" ]; then
    cp -r "$SCRIPT_DIR/scripts/"* "${DM_HOME}/scripts/"
    chmod -R 755 "${DM_HOME}/scripts/"*.pl "${DM_HOME}/scripts/"*.sh 2>/dev/null || true
    echo -e "  ${G}✓${R} $(find "${DM_HOME}/scripts" -maxdepth 2 -name '*.pl' -o -name '*.sh' -o -name '*.py' -o -name '*.js' | wc -l) 个脚本已部署"
else
    echo -e "  ${Y}⚠ 未找到 scripts/ 目录，跳过脚本部署${R}"
fi

# ── 4. 安装命令补全 ──
echo -e "${Y}[4/5]${R} 安装命令补全..."
if command -v dm &>/dev/null; then
    if [ -d /etc/bash_completion.d ]; then
        dm completions bash > /etc/bash_completion.d/dm 2>/dev/null || true
        echo -e "  ${G}✓${R} bash 补全"
    fi
    if [ -d /usr/share/zsh/site-functions ]; then
        dm completions zsh > /usr/share/zsh/site-functions/_dm 2>/dev/null || true
        echo -e "  ${G}✓${R} zsh 补全"
    fi
    if [ -d /usr/share/fish/vendor_completions.d ]; then
        dm completions fish > /usr/share/fish/vendor_completions.d/dm.fish 2>/dev/null || true
        echo -e "  ${G}✓${R} fish 补全"
    fi
else
    echo -e "  ${Y}⚠ dm 暂不可执行，跳过补全安装${R}"
fi

# ── 5. 验证 ──
echo -e "${Y}[5/5]${R} 验证安装..."
if command -v dm &>/dev/null; then
    echo -e "  ${G}✓${R} dm 命令可用"
    echo ""
    dm list 2>/dev/null || echo -e "  ${Y}⚠ 运行 dm list 时出错（可能当前目录没有脚本）${R}"
    echo ""
    echo -e "${G}╔════════════════════════════════════════════╗${R}"
    echo -e "${G}║       安装完成！                          ║${R}"
    echo -e "${G}╚════════════════════════════════════════════╝${R}"
    echo ""
    echo "  用法:"
    echo "    dm list             列出所有脚本"
    echo "    dm run <脚本名>     执行脚本"
    echo "    dm info <脚本名>    查看脚本详情"
    echo "    dm serve            启动 Web 服务"
    echo ""
    echo "  脚本管理:"
    echo "    自定义脚本放到 ${DM_HOME}/scripts/<名称>/ 下"
    echo "    创建 .dm.toml 声明元数据"
else
    echo -e "  ${RD}✗ dm 命令不可用，请检查 PATH${R}"
    exit 1
fi
