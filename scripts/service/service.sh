#!/bin/bash
source "$(dirname "$0")/../_lib.sh" 2>/dev/null || source /opt/work/dm/scripts/_lib.sh
FOLLOW=0 EXPORT=""
while [[ $# -gt 0 ]]; do case $1 in -f) FOLLOW=1;; -export) EXPORT="${2:-.}"; shift;; esac; shift; done
classify(){ local c="$1"; case "$c" in *java*|*jar*|*tomcat*) echo "Java";; *nginx*) echo "Nginx";; *redis*) echo "Redis";; *mysql*|*mariadb*) echo "MySQL";; *kafka*) echo "Kafka";; *elastic*|*kibana*) echo "ES";; *docker*|*containerd*) echo "Docker";; *caddy*) echo "Caddy";; *node*|*pm2*) echo "Node";; *python*|*gunicorn*) echo "Python";; *sshd*) echo "SSH";; *) echo "Other";; esac; }
run() {
    [ -z "$EXPORT" ] && printf '\033[H\033[2J'
    local W=$(get_cols); local C1=6 C2=12 C3=8 C4=8 C5=8 C6=10 C7=30 C8=16
    C1=$((W*5/100)); C2=$((W*8/100)); C3=10
    [ $C1 -lt 4 ] && C1=4; [ $C7 -lt 20 ] && C7=20
    header "服务状态  $(date '+%Y-%m-%d %H:%M:%S')" $W
    top_border $C1 $C2 $C3 $C4 $C5 $C6 $C7 $C8
    table_row " ID" $C1 " 服务" $C2 " PID" $C3 " CPU%" $C4 " MEM%" $C5 " RSS" $C6 " 命令" $C7 " 端口/异常" $C8
    mid_border $C1 $C2 $C3 $C4 $C5 $C6 $C7 $C8
    local PF=$(mktemp)
    ss -tulnp 2>/dev/null|tail -n +2|while read l; do local port=$(echo "$l"|awk '{print $5}'|rev|cut -d: -f1|rev) pid=$(echo "$l"|grep -oP 'pid=\K\d+'|head -1) proto=$(echo "$l"|awk '{print $1}'); [ -n "$pid" ] && echo "$pid $proto:$port">>"$PF"; done
    local idx=0
    ps aux --sort=-%cpu 2>/dev/null|awk 'NR>1&&$11!~/^\[/&&$11!~/ps aux/'|head -40|while read u pid c m v r t stat st time cmd; do
        fc="$cmd"; [ -r "/proc/$pid/cmdline" ] && fc=$(tr '\0' ' ' < "/proc/$pid/cmdline" 2>/dev/null|head -c 60); [ -z "$fc" ] && fc="$cmd"
        svc=$(classify "$fc")
        ports=$(grep "^$pid " "$PF" 2>/dev/null|awk '{print $2}'|tr '\n' ','|sed 's/,$//'); [ -z "$ports" ] && ports="-"
        local err="$ports"
        [ "$svc" != "Other" ] && [ "$svc" != "Node" ] && [ "$svc" != "SSH" ] && [ "$ports" = "-" ] && err="⚠ 无端口"
        local sc="$G" state="●"
        if [ "$stat" = "Z" ]; then sc="$RD"; state="Z"; fi; if echo "$stat"|grep -q 'T'; then sc="$Y"; state="T"; fi
        idx=$((idx+1))
        printf -v row '  │%s│%s│%s│%s│%s│%s│%s│%b%s%b│\n' "$(pad " $idx" $C1)" "$(pad " $svc" $C2)" "$(pad " $pid" $C3)" "$(pad " ${c}%" $C4)" "$(pad " ${m}%" $C5)" "$(pad " ${r}K" $C6)" "$(pad " ${fc:0:$((C7-3))}" $C7)" "$sc" "$(pad " $err" $C8)" "$R"
        echo -n "$row"
    done
    rm -f "$PF"; bot_border $C1 $C2 $C3 $C4 $C5 $C6 $C7 $C8
    echo -e "  ${D}进程总数: $(ps aux|awk 'NR>1&&$11!~/^\[/'|wc -l)${R}"
    [ -n "$EXPORT" ] && { bash "$0" > "$EXPORT/process-$(date '+%Y%m%d-%H%M%S').txt" 2>/dev/null; echo "已导出: $EXPORT"; }
    [ "$FOLLOW" -eq 1 ] && echo -e "  ${D}每5秒刷新 | Ctrl+C 退出${R}"
}
if [ "$FOLLOW" -eq 1 ]; then while true; do run; sleep 5; done; else run; fi
exit 0
