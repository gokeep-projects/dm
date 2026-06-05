#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "中间件一览  $(date '+%Y-%m-%d %H:%M:%S')" $W
    top_border $CW $CW $CW $CW
    table_row " 中间件" $CW " 版本/信息" $CW " 连接状态" $CW " 异常信息" $CW
    mid_border $CW $CW $CW $CW
    # Nginx
    local ng_ver="-" ng_stat="未安装" ng_err="-"
    if command -v nginx >/dev/null 2>&1; then ng_ver=$(nginx -v 2>&1|sed 's/nginx version://'); ng_stat="已安装"; ng_err="-"; fi
    table_row " Nginx" $CW " $ng_ver" $CW " $ng_stat" $CW " $ng_err" $CW
    # Redis
    local rd_ver="-" rd_stat="未连接" rd_err="-"
    if command -v redis-cli >/dev/null 2>&1; then
        local rd_info=$(redis-cli -h 127.0.0.1 -p 6379 INFO 2>/dev/null)
        if [ -n "$rd_info" ]; then rd_ver=$(echo "$rd_info"|grep '^redis_version:'|cut -d: -f2|tr -d '\r'); rd_stat="${G}已连接${R}"; else rd_stat="${Y}无法连接${R}"; rd_err="检查服务"; fi
    fi
    table_row " Redis" $CW " $rd_ver" $CW " $rd_stat" $CW " $rd_err" $CW
    # MySQL
    local my_ver="-" my_stat="未连接" my_err="-"
    if command -v mysql >/dev/null 2>&1; then my_ver=$(echo "SELECT VERSION();"|mysql -N 2>/dev/null|head -1); [ -n "$my_ver" ] && my_stat="${G}已连接${R}" || { my_stat="${Y}无法连接${R}"; my_err="检查服务"; }; fi
    table_row " MySQL" $CW " $my_ver" $CW " $my_stat" $CW " $my_err" $CW
    # Kafka
    local kf_stat="未连接" kf_err="-"
    if command -v kafka-topics.sh >/dev/null 2>&1; then kf_out=$(kafka-topics.sh --bootstrap-server 127.0.0.1:9092 --list 2>/dev/null|head -3|tr '\n' ' '); [ -n "$kf_out" ] && kf_stat="${G}已连接${R}" || { kf_stat="${Y}无法连接${R}"; kf_err="检查服务"; }; fi
    table_row " Kafka" $CW " -" $CW " $kf_stat" $CW " $kf_err" $CW
    # ES
    local es_stat="未连接" es_err="-"
    if command -v curl >/dev/null 2>&1; then es_out=$(curl -s --connect-timeout 3 127.0.0.1:9200 2>/dev/null); [ -n "$es_out" ] && es_stat="${G}已连接${R}" || { es_stat="${Y}无法连接${R}"; es_err="检查服务"; }; fi
    table_row " Elasticsearch" $CW " -" $CW " $es_stat" $CW " $es_err" $CW
    bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/middleware-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
}
if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi
exit 0
