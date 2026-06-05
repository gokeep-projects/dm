#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local CW=$(( (W-7)/4 ))
    header "网络诊断  $(date '+%Y-%m-%d %H:%M:%S')" $W
    top_border $CW $CW $CW $CW
    table_row " 接口" $CW " IPv4" $CW " MAC" $CW " 状态/MTU" $CW
    mid_border $CW $CW $CW $CW
    ip -o link show 2>/dev/null|awk -F': ' '{print $2}'|grep -v lo|grep -v 'docker\|br-\|veth\|virbr\|bond\|tun\|tap\|wg'|head -8|while read iface; do
        ipv4=$(ip -4 addr show "$iface" 2>/dev/null|grep -oP 'inet \K[\d.]+'|head -1) mac=$(ip link show "$iface" 2>/dev/null|grep -oP 'link/ether \K[\w:]+'|head -1) st=$(ip link show "$iface" 2>/dev/null|grep -oP 'state \K\w+') mtu=$(ip link show "$iface" 2>/dev/null|grep -oP 'mtu \K\d+'|head -1)
        [ -z "$ipv4" ] && ipv4="-"; [ -z "$mac" ] && mac="-"
        table_row " $iface" $CW " $ipv4" $CW " $mac" $CW " $st/$mtu" $CW
    done
    local DNS=$(grep '^nameserver' /etc/resolv.conf|awk '{print $2}'|head -1) GW=$(ip route show default 2>/dev/null|awk '{print $3}'|head -1)
    mid_border $CW $CW $CW $CW
    table_row " DNS" $CW " $DNS" $CW " 网关" $CW " ${GW:- -}" $CW
    mid_border $CW $CW $CW $CW
    table_row " 端口" $CW " 协议" $CW " 状态" $CW " 进程" $CW
    mid_border $CW $CW $CW $CW
    ss -tulnp 2>/dev/null|tail -n +2|sort -t: -k2 -n|head -20|while read l; do local proto=$(echo "$l"|awk '{print $1}') port=$(echo "$l"|awk '{print $5}'|rev|cut -d: -f1|rev) proc=$(echo "$l"|grep -oP 'users:\(\("\K[^"]+'|head -1) pid=$(echo "$l"|grep -oP 'pid=\K\d+'|head -1)
        [ -z "$proc" ] && proc="-"; [ -z "$pid" ] && pid="-"
        table_row " $port" $CW " $proto" $CW " OPEN" $CW " $proc($pid)" $CW
    done
    mid_border $CW $CW $CW $CW
    # SSL检查
    local DOMAIN="${1:-}"
    if [ -n "$DOMAIN" ]; then
        table_row " SSL域名" $CW " $DOMAIN" $CW " 证书" $CW " 检查中..." $CW
        local cert=$(echo|openssl s_client -servername "$DOMAIN" -connect "$DOMAIN:443" 2>/dev/null|openssl x509 -noout -dates 2>/dev/null)
        if [ -n "$cert" ]; then local after=$(echo "$cert"|grep 'notAfter='|cut -d= -f2); local days=$((($(date -d "$after" +%s)-$(date +%s))/86400)); table_row " 过期时间" $CW " $after" $CW " 剩余天数" $CW " ${days}天" $CW; else table_row " SSL" $CW " 无法获取" $CW " " $CW " " $CW; fi
    fi
    bot_border $CW $CW $CW $CW
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/network-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
    [ "$FOLLOW" -eq 1 ] && echo -e "  ${D}每5秒刷新 | Ctrl+C 退出${R}"
}
if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi
exit 0
