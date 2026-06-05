#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "环境与用户  $(date '+%Y-%m-%d %H:%M:%S')" $W
    top_border $CW $CW $CW $CW
    table_row " 主机名" $CW " $(hostname)" $CW " 系统" $CW " $(grep PRETTY_NAME /etc/os-release|sed 's/PRETTY_NAME=//'|tr -d '"')" $CW
    table_row " 内核" $CW " $(uname -r)" $CW " 架构" $CW " $(uname -m)" $CW
    table_row " 运行时间" $CW " $(uptime -p 2>/dev/null|sed 's/up //')" $CW " 负载" $CW " $(awk '{printf "%s/%s/%s",$1,$2,$3}' /proc/loadavg)" $CW
    table_row " 用户" $CW " $(whoami)" $CW " 登录" $CW " $(who|wc -l) 个会话" $CW
    mid_border $CW $CW $CW $CW
    table_row " 变量" $CW " 值" $CW " 变量" $CW " 值" $CW
    mid_border $CW $CW $CW $CW
    for v in PATH HOME SHELL LANG; do table_row " $v" $CW " $(printenv $v 2>/dev/null|cut -c1-$((CW-4)))" $CW " " $CW " " $CW; done
    mid_border $CW $CW $CW $CW
    table_row " 软件" $CW " 版本" $CW " 软件" $CW " 版本" $CW
    mid_border $CW $CW $CW $CW
    for cmd in python3 java node npm docker git gcc curl wget; do local ver=$($cmd --version 2>/dev/null|head -1|cut -c1-$((CW-6))); [ -n "$ver" ] && table_row " $cmd" $CW " $ver" $CW " " $CW " " $CW; done
    bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/env-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
}
if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi
exit 0
