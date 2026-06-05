#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "Docker 容器  $(date '+%Y-%m-%d %H:%M:%S')" $W
    command -v docker >/dev/null 2>&1 || { echo "  Docker 未安装"; return; }
    top_border $CW $CW $CW $CW
    table_row " 容器ID" $CW " 名称" $CW " 镜像" $CW " 状态" $CW
    mid_border $CW $CW $CW $CW
    docker ps -a --format '{{.ID}}|{{.Names}}|{{.Image}}|{{.Status}}' 2>/dev/null|head -15|while IFS='|' read id name img st; do table_row " ${id:0:14}" $CW " $name" $CW " ${img:0:20}" $CW " $st" $CW; done
    mid_border $CW $CW $CW $CW
    table_row " 镜像" $CW " 标签" $CW " 大小" $CW " 创建时间" $CW
    mid_border $CW $CW $CW $CW
    docker images --format '{{.Repository}}|{{.Tag}}|{{.Size}}|{{.CreatedSince}}' 2>/dev/null|head -10|while IFS='|' read repo tag sz created; do table_row " ${repo:0:20}" $CW " $tag" $CW " $sz" $CW " $created" $CW; done
    bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/container-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
    [ "$FOLLOW" -eq 1 ] && echo -e "  ${D}每5秒刷新 | Ctrl+C 退出${R}"
}
if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi
exit 0
