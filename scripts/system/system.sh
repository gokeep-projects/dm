#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "系统综合检查  $(date '+%Y-%m-%d %H:%M:%S')" $W
    local HOST=$(hostname) OS=$(grep PRETTY_NAME /etc/os-release|sed 's/PRETTY_NAME=//'|tr -d '"')
    local KER=$(uname -r) ARCH=$(uname -m) UP=$(uptime -p 2>/dev/null|sed 's/up //')
    local LOAD=$(awk '{printf "%s/%s/%s",$1,$2,$3}' /proc/loadavg)
    local CPU=$(grep -m1 'model name' /proc/cpuinfo|cut -d: -f2|xargs) CORES=$(nproc)
    local MT=$(awk '/MemTotal:/{print $2}' /proc/meminfo) MA=$(awk '/MemAvailable:/{print $2}' /proc/meminfo)
    local MU=$((MT-MA)) MPCT=0; [ $MT -gt 0 ]&&MPCT=$((MU*100/MT))
    local ST=$(awk '/SwapTotal:/{print $2}' /proc/meminfo) SF=$(awk '/SwapFree:/{print $2}' /proc/meminfo)
    local SPCT=0; [ $ST -gt 0 ]&&SPCT=$(((ST-SF)*100/ST))
    local IP=$(curl -s --connect-timeout 2 ifconfig.me 2>/dev/null || ip -4 addr show scope global 2>/dev/null|grep -oP 'inet \K[\d.]+'|grep -v '^127\.\|^172\.\|^192\.168\.'|head -1); [ -z "$IP" ] && IP="-"
    local DISK=$(cat /sys/block/sda/device/model 2>/dev/null || lsblk -ndo MODEL /dev/sda 2>/dev/null || echo "-")
    local DU=$(df -h /|awk 'NR==2{print $3}') DT=$(df -h /|awk 'NR==2{print $2}') DP=$(df /|awk 'NR==2{gsub(/%/,"");print $5}')
    top_border $CW $CW $CW $CW
    table_row " 主机名" $CW " $HOST" $CW " 系统" $CW " $OS" $CW
    table_row " 内核" $CW " $KER" $CW " 架构" $CW " $ARCH" $CW
    table_row " CPU" $CW " $CPU" $CW " 核心/内存" $CW " ${CORES}核 $(fmt_b $((MU*1024)))/$(fmt_b $((MT*1024)))" $CW
    table_row " 运行时间" $CW " $UP" $CW " 负载" $CW " $LOAD" $CW
    table_row " 公网IP" $CW " $IP" $CW " 磁盘型号" $CW " $DISK" $CW
    mid_border $CW $CW $CW $CW
    table_row " CPU" $CW " ${MPCT}% ($(fmt_b $((MU*1024)))/$(fmt_b $((MT*1024))))" $CW " 内存" $CW " ${MPCT}% ($(fmt_b $((MU*1024)))/$(fmt_b $((MT*1024))))" $CW
    table_row " Swap" $CW " ${SPCT}% ($(fmt_b $((SF*1024)))/$(fmt_b $((ST*1024))))" $CW " 磁盘" $CW " ${DP}% (${DU}/${DT})" $CW
    mid_border $CW $CW $CW $CW
    ip -o link show 2>/dev/null|awk -F': ' '{print $2}'|grep -v lo|grep -v 'docker\|br-\|veth\|virbr\|bond\|tun\|tap\|wg'|head -3|while read iface; do
        ipv4=$(ip -4 addr show "$iface" 2>/dev/null|grep -oP 'inet \K[\d.]+'|head -1); st=$(ip link show "$iface" 2>/dev/null|grep -oP 'state \K\w+')
        [ -z "$ipv4" ] && ipv4="-"; table_row " 网卡" $CW " $iface" $CW " IPv4/状态" $CW " $ipv4 ($st)" $CW
    done
    mid_border $CW $CW $CW $CW
    table_row " 类型" $CW " 进程/PID" $CW " 端口" $CW " CPU%/MEM%" $CW
    mid_border $CW $CW $CW $CW
    local PF=$(mktemp)
    ss -tulnp 2>/dev/null|tail -n +2|while read l; do local port=$(echo "$l"|awk '{print $5}'|rev|cut -d: -f1|rev) pid=$(echo "$l"|grep -oP 'pid=\K\d+'|head -1) proto=$(echo "$l"|awk '{print $1}'); [ -n "$pid" ] && echo "$pid $proto:$port">>"$PF"; done
    ps aux --sort=-%cpu 2>/dev/null|awk 'NR>1&&$11!~/^\[/&&$11!~/ps aux/'|head -25|while read u pid c m x r t stat st time cmd; do
        fc="$cmd"; [ -r "/proc/$pid/cmdline" ] && fc=$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null|head -c 50); [ -z "$fc" ] && fc="$cmd"
        ports=$(grep "^$pid " "$PF" 2>/dev/null|awk '{print $2}'|tr '\n' ' '|sed 's/ $//'); [ -z "$ports" ] && ports="-"
        ptype="Other" pri=99
        case "$fc" in *java*|*jar*|*tomcat*) ptype="Java"; pri=1;; *nginx*) ptype="Nginx"; pri=2;; *redis*) ptype="Redis"; pri=2;; *mysql*|*mariadb*) ptype="MySQL"; pri=2;; *kafka*) ptype="Kafka"; pri=2;; *elastic*|*kibana*) ptype="ES"; pri=2;; *docker*|*containerd*) ptype="Docker"; pri=2;; *caddy*) ptype="Caddy"; pri=2;; *node*|*pm2*) ptype="Node"; pri=3;; *python*|*gunicorn*) ptype="Python"; pri=3;; esac
        [ "$ports" != "-" ] && pri=$((pri*10+5)) || pri=$((pri*10+6))
        echo "$pri|$ptype|$fc|$pid|$ports|${c}%/${m}%"
    done|sort -t'|' -k1,1n|head -12|while IFS='|' read pr ptype cmd pid ports cpumem; do table_row " $ptype" $CW " ${pid} ${cmd:0:$((CW-8))}" $CW " $ports" $CW " $cpumem" $CW; done
    rm -f "$PF"; bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/sys-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
    [ "$FOLLOW" -eq 1 ] && echo -e "  ${D}每5秒刷新 | Ctrl+C 退出${R}"
}; if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi; exit 0
