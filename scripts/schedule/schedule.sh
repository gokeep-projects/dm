#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "定时任务  $(date '+%Y-%m-%d %H:%M:%S')" $W
    # 用户cron表格
    top_border $CW $CW $CW $CW
    table_row " 用户" $CW " 定时任务" $CW " 备注" $CW " " $CW
    mid_border $CW $CW $CW $CW
    local found=0
    crontab -l 2>/dev/null|while read l; do
        [ -z "$l" ] && continue; echo "$l"|grep -q '^#' && continue
        table_row " $(whoami)" $CW " $l" $CW " 用户级" $CW " " $CW; found=1
    done
    [ $found -eq 0 ] 2>/dev/null && table_row " $(whoami)" $CW " (无定时任务)" $CW " 用户级" $CW " " $CW
    mid_border $CW $CW $CW $CW
    # 系统cron
    table_row " 系统" $CW " 系统定时任务" $CW " 路径" $CW " " $CW
    mid_border $CW $CW $CW $CW
    for f in /etc/crontab /etc/cron.d/*; do
        [ -f "$f" ] && grep -v '^#' "$f" 2>/dev/null|grep -v '^$'|while read l; do
            table_row " root" $CW " $l" $CW " ${f##*/}" $CW " " $CW
        done
    done
    mid_border $CW $CW $CW $CW
    # 服务状态
    table_row " cron服务" $CW " 状态" $CW " 进程" $CW " 开机启动" $CW
    mid_border $CW $CW $CW $CW
    local cron_stat="未运行" cron_pid="-" cron_enable="-"
    if systemctl is-active cron >/dev/null 2>&1; then cron_stat="${G}运行中${R}"; cron_pid=$(systemctl show -p MainPID cron 2>/dev/null|cut -d= -f2); else
        systemctl is-active crond >/dev/null 2>&1 && { cron_stat="${G}运行中${R}"; cron_pid=$(systemctl show -p MainPID crond 2>/dev/null|cut -d= -f2); }
    fi
    if systemctl is-enabled cron >/dev/null 2>&1; then cron_enable="是"; else
        systemctl is-enabled crond >/dev/null 2>&1 && cron_enable="是"
    fi
    table_row " cron" $CW " $cron_stat" $CW " PID:$cron_pid" $CW " $cron_enable" $CW
    bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/schedule-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
    [ "$FOLLOW" -eq 1 ] && echo -e "  ${D}每5秒刷新 | Ctrl+C 退出${R}"
}; if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi; exit 0
