#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "硬件资源详情  $(date '+%Y-%m-%d %H:%M:%S')" $W
    # CPU
    local model=$(grep -m1 'model name' /proc/cpuinfo|cut -d: -f2|xargs)
    local arch=$(uname -m) cores=$(nproc) freq=$(grep -m1 'cpu MHz' /proc/cpuinfo|cut -d: -f2|xargs)
    local cache=$(grep -m1 'cache size' /proc/cpuinfo|cut -d: -f2|xargs) bogomips=$(grep -m1 'bogomips' /proc/cpuinfo|cut -d: -f2|xargs)
    top_border $CW $CW $CW $CW
    table_row " CPU型号" $CW " $model" $CW " 架构" $CW " $arch" $CW
    table_row " 核心数" $CW " ${cores}核" $CW " 主频" $CW " ${freq} MHz" $CW
    table_row " 缓存" $CW " $cache" $CW " Bogomips" $CW " $bogomips" $CW
    mid_border $CW $CW $CW $CW
    awk '/^cpu[0-9]/{t=$2+$3+$4+$5+$6+$7+$8+$9;u=$2+$3+$4+$7+$8+$9;p=0;if(t>0)p=int(u*100/t);printf "%s|%s\n",substr($1,4),p}' /proc/stat|head -16|while IFS='|' read c p; do table_row " CPU$c" $CW " ${p}%" $CW " CPU$c" $CW "${p}%" $CW 2>/dev/null; done
    mid_border $CW $CW $CW $CW
    # 内存
    local MT=$(awk '/MemTotal:/{print $2}' /proc/meminfo) MA=$(awk '/MemAvailable:/{print $2}' /proc/meminfo)
    local MF=$(awk '/MemFree:/{print $2}' /proc/meminfo) MB=$(awk '/Buffers:/{print $2}' /proc/meminfo)
    local MC=$(awk '/^Cached:/{print $2}' /proc/meminfo) ST=$(awk '/SwapTotal:/{print $2}' /proc/meminfo)
    local SF=$(awk '/SwapFree:/{print $2}' /proc/meminfo) MPCT=0 SPCT=0
    [ $MT -gt 0 ]&&MPCT=$(((MT-MA)*100/MT)); [ $ST -gt 0 ]&&SPCT=$(((ST-SF)*100/ST))
    table_row " 总内存" $CW " $(fmt_b $((MT*1024)))" $CW " 已使用" $CW " $(fmt_b $(((MT-MA)*1024))) (${MPCT}%)" $CW
    table_row " 空闲" $CW " $(fmt_b $((MF*1024)))" $CW " 可用" $CW " $(fmt_b $((MA*1024)))" $CW
    table_row " Buffers" $CW " $(fmt_b $((MB*1024)))" $CW " Cached" $CW " $(fmt_b $((MC*1024)))" $CW
    table_row " Swap总量" $CW " $(fmt_b $((ST*1024)))" $CW " Swap使用" $CW " ${SPCT}%" $CW
    mid_border $CW $CW $CW $CW
    # 磁盘
    df -h --output=target,size,used,avail,pcent -x tmpfs -x devtmpfs -x squashfs -x overlay 2>/dev/null|tail -n +2|while read mp sz us av pc; do table_row " 磁盘" $CW " $mp" $CW " 使用率" $CW " ${us}/${sz} ($pc)" $CW; done
    mid_border $CW $CW $CW $CW
    # IO
    table_row " 设备" $CW " 读/s" $CW " 写/s" $CW " 读KB 写KB" $CW
    iostat -d -x 1 1 2>/dev/null|awk 'NR>3&&$1!=""&&$1!~/Device/{printf "%s|%s|%s|%s %s\n",$1,$2,$3,$4,$5}'|head -5|while IFS='|' read d r w rw; do table_row " $d" $CW " $r" $CW " $w" $CW " $rw" $CW; done
    bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/hardware-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
    [ "$FOLLOW" -eq 1 ] && echo -e "  ${D}每5秒刷新 | Ctrl+C 退出${R}"
}
if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi
exit 0
