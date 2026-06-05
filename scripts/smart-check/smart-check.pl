#!/usr/bin/perl
use strict;
use warnings;
use POSIX qw(strftime);

# ═══════════════════════════════════════════════════════════════════════
# 智能综合检查 v2.0 - 全量体检 + 炫酷报告
# ═══════════════════════════════════════════════════════════════════════

my ($R, $B, $D, $CY, $G, $Y, $RD, $BL) = (
    "\e[0m", "\e[1m", "\e[2m", "\e[36m",
    "\e[32m", "\e[33m", "\e[31m", "\e[34m"
);

# ─── 表格 ───
sub _dw {
    my ($s) = @_;
    $s =~ s/\e\[[0-9;]*m//g;
    my $len = length($s); my $w = 0;
    for my $i (0 .. $len - 1) {
        my $b = ord(substr($s, $i, 1));
        $b < 0x80 ? $w++ : ($b >= 0xC0 ? $w += 2 : ());
    }
    $w;
}
sub _pad {
    my ($s, $tw) = @_;
    my $plain = $s; $plain =~ s/\e\[[0-9;]*m//g;
    my $plen = length($plain); my $cw = 0; my $out = '';
    for (my $i = 0; $i < $plen; $i++) {
        last if $cw >= $tw - 1;
        my $b = ord(substr($plain, $i, 1));
        if    ($b < 0x80)   { $out .= substr($plain, $i, 1); $cw += 1 }
        elsif ($b >= 0xF0) { last if $cw+2>$tw-1; $out .= substr($plain, $i, 4); $cw += 2; $i += 3 }
        elsif ($b >= 0xE0) { last if $cw+2>$tw-1; $out .= substr($plain, $i, 3); $cw += 2; $i += 2 }
        elsif ($b >= 0xC0) { last if $cw+2>$tw-1; $out .= substr($plain, $i, 2); $cw += 2; $i += 1 }
    }
    if (length($out) < $plen) { $out .= '..'; $cw += 2 }
    my $diff = $tw - $cw; $diff = 0 if $diff < 0;
    $out . (' ' x $diff);
}
sub top_border  { my $s="  ┌";my $i=0;for my $w(@_){$s.="─"x $w;$s.=($i++<$#_)?"┬":"┐"}print"$s\n"}
sub mid_border  { my $s="  ├";my $i=0;for my $w(@_){$s.="─"x $w;$s.=($i++<$#_)?"┼":"┤"}print"$s\n"}
sub bot_border  { my $s="  └";my $i=0;for my $w(@_){$s.="─"x $w;$s.=($i++<$#_)?"┴":"┘"}print"$s\n"}
sub table_row   { my $s="  │";while(@_>=2){$s.=_pad(shift,shift).'│'}print"$s\n"}
sub get_cols    { my$tw=(`tput cols 2>/dev/null`||140)+0;my$w=int($tw*0.9);$w=110 if$w<110;$w=200 if$w>200;$w}

# ─── 工具 ───
sub _cmd  { my$c=shift;my$o=`$c 2>/dev/null`;chomp$o;$o }
sub _fst  { my$p=shift;return''unless -r $p;open my$fh,'<:raw',$p or return'';my$l=<$fh>;close$fh;defined$l&&chomp$l;$l//'' }
sub _read { my$p=shift;return''unless -r $p;open my$fh,'<:raw',$p or return'';my$c=do{local$/;<$fh>};close$fh;chomp$c;$c }
sub _act  { my$s=shift;system("systemctl is-active $s >/dev/null 2>&1");$?>>8==0 }
sub _glb  { my$p=shift; my@f; for(glob($p)){push@f,$_ if -r && -f} @f }

sub start_loading { my ($m) = @_; print "  ${CY}⟳ ${m}...${R}" }
sub stop_loading  { print "${G} 完成${R}\n" }

# ─── 结果收集 ───
my @results;
sub add_r { my ($cat,$name,$status,$detail)=@_; my$s=$status eq'PASS'?100:$status eq'WARN'?50:0; push@results,{category=>$cat,name=>$name,status=>$status,detail=>$detail,score=>$s} }

# ─── 健康条 ───
sub health_bar {
    my ($score, $width) = @_;
    $width ||= 30;
    my $filled = int($score * $width / 100);
    my $empty  = $width - $filled;
    my $color  = $score >= 80 ? $G : $score >= 60 ? $CY : $score >= 40 ? $Y : $RD;
    "${color}" . ("█" x $filled) . "${D}" . ("░" x $empty) . "${R}";
}

# ═══════════════════════════════════════════════════════════════════════
# 1. 系统配置 (25项)
# ═══════════════════════════════════════════════════════════════════════
sub check_system_config {
    start_loading("检查系统配置...");

    add_r('系统配置', "操作系统",   'PASS', _cmd("grep PRETTY_NAME /etc/os-release|cut -d= -f2|tr -d '\"'") . " / " . _cmd('uname -r') . " " . _cmd('uname -m'));
    add_r('系统配置', "主机名",     'PASS', _cmd('hostname'));
    add_r('系统配置', "运行时间",   'PASS', _cmd('uptime -p') || _cmd('uptime|awk "{print \$3\$4}"'));
    my $load = _cmd("awk '{printf \"%.2f\",\$1}' /proc/loadavg");
    my $ncpu = _cmd('nproc');
    add_r('系统配置', "系统负载",   $load>$ncpu*0.7?'WARN':'PASS', "load: $load / $ncpu 核");

    # 安全子系统
    my $se = _cmd('getenforce');
    add_r('系统配置', 'SELinux', $se eq 'Enforcing'?'PASS':'WARN', $se||'未安装');
    add_r('系统配置', 'AppArmor', _act('apparmor')?'PASS':'WARN', _act('apparmor')?'启用':'未启用');

    # 内核加固 (12项)
    my $aslr = _fst('/proc/sys/kernel/randomize_va_space');
    add_r('系统配置', 'ASLR', $aslr eq '2'?'PASS':$aslr eq '1'?'WARN':'FAIL', $aslr eq '2'?'完全随机化':$aslr eq '1'?'部分随机化':'关闭');
    my $yama = _fst('/proc/sys/kernel/yama/ptrace_scope');
    add_r('系统配置', 'Yama ptrace_scope', $yama eq '1'||$yama eq '2'||$yama eq '3'?'PASS':'WARN', "值: $yama");
    add_r('系统配置', 'kptr_restrict', (_fst('/proc/sys/kernel/kptr_restrict')||'0')>=1?'PASS':'WARN', "值: "._fst('/proc/sys/kernel/kptr_restrict'));
    add_r('系统配置', 'dmesg_restrict', (_fst('/proc/sys/kernel/dmesg_restrict')||'0') eq '1'?'PASS':'WARN', "值: "._fst('/proc/sys/kernel/dmesg_restrict'));
    my $core = _fst('/proc/sys/kernel/core_pattern');
    add_r('系统配置', 'Core dump 策略', $core=~/^\|/?'PASS':'WARN', $core=~/^\|/?'管道捕获(安全)':"文件落盘: $core");
    add_r('系统配置', 'kexec 禁用', (_fst('/proc/sys/kernel/kexec_load_disabled')||'0') eq '1'?'PASS':'WARN', _fst('/proc/sys/kernel/kexec_load_disabled') eq '1'?'已禁用':'未禁用');
    add_r('系统配置', '模块加载禁用', (_fst('/proc/sys/kernel/modules_disabled')||'0') eq '1'?'PASS':'WARN', _fst('/proc/sys/kernel/modules_disabled') eq '1'?'已禁用':'未禁用');
    add_r('系统配置', '硬链接保护', (_fst('/proc/sys/fs/protected_hardlinks')||'0') eq '1'?'PASS':'FAIL', _fst('/proc/sys/fs/protected_hardlinks') eq '1'?'已开启':'未开启');
    add_r('系统配置', '软链接保护', (_fst('/proc/sys/fs/protected_symlinks')||'0') eq '1'?'PASS':'FAIL', _fst('/proc/sys/fs/protected_symlinks') eq '1'?'已开启':'未开启');

    # 网络加固
    add_r('系统配置', 'TCP Syncookies', (_fst('/proc/sys/net/ipv4/tcp_syncookies')||'0') eq '1'?'PASS':'FAIL', _fst('/proc/sys/net/ipv4/tcp_syncookies') eq '1'?'已开启':'未开启');
    add_r('系统配置', 'TCP SACK', (_fst('/proc/sys/net/ipv4/tcp_sack')||'0') eq '1'?'PASS':'WARN', _fst('/proc/sys/net/ipv4/tcp_sack') eq '1'?'开启':'关闭');
    my $rp = _fst('/proc/sys/net/ipv4/conf/all/rp_filter');
    add_r('系统配置', 'rp_filter', $rp eq '1'?'PASS':'WARN', "值: $rp");
    add_r('系统配置', 'IP 转发', (_fst('/proc/sys/net/ipv4/ip_forward')||'0') eq '1'?'WARN':'PASS', _fst('/proc/sys/net/ipv4/ip_forward') eq '1'?'已开启(路由模式)':'已关闭');
    add_r('系统配置', 'ICMP 广播忽略', (_fst('/proc/sys/net/ipv4/icmp_echo_ignore_broadcasts')||'0') eq '1'?'PASS':'FAIL', _fst('/proc/sys/net/ipv4/icmp_echo_ignore_broadcasts') eq '1'?'已忽略':'未忽略');
    add_r('系统配置', 'TCP SYN Retries', 'PASS', _fst('/proc/sys/net/ipv4/tcp_syn_retries')||'N/A');
    add_r('系统配置', 'TCP FIN Timeout', 'PASS', _fst('/proc/sys/net/ipv4/tcp_fin_timeout')||'N/A');

    my $lock = _read('/sys/kernel/security/lockdown') || 'none';
    $lock =~ s/^\s+|\s+$//g;
    add_r('系统配置', '系统锁定(Lockdown)', $lock ne 'none'?'PASS':'WARN', $lock);
    add_r('系统配置', 'FTrace', (_fst('/proc/sys/kernel/ftrace_enabled')||'1') eq '0'?'PASS':'WARN', _fst('/proc/sys/kernel/ftrace_enabled') eq '0'?'已关闭':'已开启');
    add_r('系统配置', 'unprivileged BPF', (_fst('/proc/sys/kernel/unprivileged_bpf_disabled')||'0') eq '1'?'PASS':'WARN', _fst('/proc/sys/kernel/unprivileged_bpf_disabled') eq '1'?'已禁用':'未禁用');

    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 2. 服务状态 (15项)
# ═══════════════════════════════════════════════════════════════════════
sub check_services {
    start_loading("检查服务状态...");
    for (qw(ssh cron rsyslog systemd-journald auditd caddy docker containerd)) {
        my $a = _act($_);
        add_r('服务状态', $_, $a?'PASS':'WARN', $a?'运行中':'未运行');
    }
    my $failed = _cmd("systemctl --failed --no-pager 2>/dev/null | grep -c 'failed'");
    if ($failed && $failed > 0) {
        my $flist = _cmd("systemctl --failed --no-pager 2>/dev/null | grep loaded | head -10 | awk '{print \$1}' | tr '\n' ' '");
        add_r('服务状态', "异常服务", 'FAIL', "${failed} 个失败: $flist");
    } else {
        add_r('服务状态', "异常服务", 'PASS', '无');
    }
    # systemd 版本 / 启动时间
    add_r('服务状态', 'systemd 版本', 'PASS', _cmd('systemctl --version 2>/dev/null | head -1'));

    my $users = _cmd("who | wc -l");
    add_r('服务状态', "在线用户", 'PASS', "$users 人");

    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 3. 防火墙 (8项)
# ═══════════════════════════════════════════════════════════════════════
sub check_firewall {
    start_loading("检查防火墙...");
    my $ipt = _cmd('iptables -L -n 2>/dev/null');
    my $default_action = 'ACCEPT'; my $chains=0; my $rules_total=0; my $accept=0; my $drop=0;
    for (split("\n", $ipt)) {
        if (/^Chain (\S+) \(policy (\S+)/ && $1 eq 'INPUT') { $default_action = $2 }
        $chains++ if /^Chain /;
        next unless /^\s*\d+\s+\d+\s+\d+/;
        $rules_total++;
        $accept++ if /\bACCEPT\b/; $drop++ if /\bDROP\b/;
    }
    add_r('防火墙', "INPUT 默认策略", $default_action eq 'DROP'?'PASS':'WARN', $default_action eq 'DROP'?'DROP(默认拒绝)':"ACCEPT(默认放行)");
    add_r('防火墙', "iptables 规则统计", 'PASS', "链: $chains, 总规则: $rules_total, ACCEPT: $accept, DROP: $drop");
    add_r('防火墙', 'FORWARD 策略', $ipt=~/^Chain FORWARD \(policy DROP/?'PASS':'WARN', $ipt=~/^Chain FORWARD \(policy DROP/?'DROP':'ACCEPT');

    if (_act('firewalld')) {
        my $z = _cmd('firewall-cmd --get-default-zone');
        my $s = _cmd("firewall-cmd --zone=$z --list-services 2>/dev/null") || '无';
        my $p = _cmd("firewall-cmd --zone=$z --list-ports 2>/dev/null")   || '无';
        add_r('防火墙', 'firewalld', 'PASS', "运行中(区域:$z) 服务:$s 端口:$p");
    } elsif (_act('ufw')) {
        my $s = _cmd('ufw status verbose');
        add_r('防火墙', 'UFW', $s=~/active/i?'PASS':'WARN', $s=~/active/i?'运行中':'未启用');
    } else {
        add_r('防火墙', '防火墙服务', 'WARN', '仅 iptables 规则,无管理服务');
    }

    # 检查 Docker 链
    my $docker_chains = _cmd("iptables -L -n 2>/dev/null | grep -c '^Chain DOCKER'");
    add_r('防火墙', "Docker 防火墙链", $docker_chains>0?'PASS':'WARN', "${docker_chains} 条Docker链");
    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 4. SSH 安全 (8项)
# ═══════════════════════════════════════════════════════════════════════
sub check_ssh {
    start_loading("检查 SSH 安全...");
    my $cfg = '/etc/ssh/sshd_config';
    return stop_loading() unless -r $cfg;
    my $root   = _cmd("grep -i '^PermitRootLogin' $cfg|awk '{print \$2}'|tail -1") || 'prohibit-password';
    my $pw     = _cmd("grep -i '^PasswordAuthentication' $cfg|awk '{print \$2}'|tail -1") || 'yes';
    my $pubkey = _cmd("grep -i '^PubkeyAuthentication' $cfg|awk '{print \$2}'|tail -1")   || 'yes';
    my $port   = _cmd("grep -i '^Port' $cfg|awk '{print \$2}'|tail -1")                   || '22';
    my $empty  = _cmd("grep -i '^PermitEmptyPasswords' $cfg|awk '{print \$2}'|tail -1")   || 'no';
    my $ga     = _cmd("grep -i '^UsePAM' $cfg|awk '{print \$2}'|tail -1")                 || 'yes';
    my $maxauth= _cmd("grep -i '^MaxAuthTries' $cfg|awk '{print \$2}'|tail -1")           || '6';
    add_r('SSH 安全', "端口",       'PASS', "$port");
    add_r('SSH 安全', "SSH 协议",   'PASS', _cmd("grep -i '^Protocol' $cfg|awk '{print \$2}'|tail -1")||'2');
    add_r('SSH 安全', "Root 登录",  lc($root) eq 'no'?'PASS':lc($root) eq 'prohibit-password'?'PASS':'WARN', $root);
    add_r('SSH 安全', "密码认证",   lc($pw) eq 'no'?'PASS':'WARN', $pw);
    add_r('SSH 安全', "公钥认证",   lc($pubkey) eq 'yes'?'PASS':'WARN', $pubkey);
    add_r('SSH 安全', "空密码",     lc($empty) eq 'no'?'PASS':'FAIL', $empty);
    add_r('SSH 安全', "PAM 认证",   'PASS', $ga);
    add_r('SSH 安全', "最大认证次数", $maxauth<=3?'PASS':'WARN', $maxauth);
    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 5. 密码策略 (5项)
# ═══════════════════════════════════════════════════════════════════════
sub check_password_policy {
    start_loading("检查密码策略...");
    my $minlen   = _cmd("grep '^minlen' /etc/security/pwquality.conf 2>/dev/null|awk -F= '{print \$2}'|tr -d ' '");
    my $minclass = _cmd("grep '^minclass' /etc/security/pwquality.conf 2>/dev/null|awk -F= '{print \$2}'|tr -d ' '");
    my $maxdays  = _cmd("grep '^PASS_MAX_DAYS' /etc/login.defs 2>/dev/null|awk '{print \$2}'");
    my $mindays  = _cmd("grep '^PASS_MIN_DAYS' /etc/login.defs 2>/dev/null|awk '{print \$2}'");
    my $warnage  = _cmd("grep '^PASS_WARN_AGE' /etc/login.defs 2>/dev/null|awk '{print \$2}'");
    add_r('密码策略', "密码最短长度", $minlen&&$minlen>=8?'PASS':'WARN', $minlen||'默认(6)');
    add_r('密码策略', "字符类数",     $minclass&&$minclass>=3?'PASS':'WARN', $minclass||'默认(1)');
    add_r('密码策略', "密码过期天数", $maxdays&&$maxdays<=90?'PASS':'WARN', $maxdays||'默认(99999)');
    add_r('密码策略', "密码修改间隔", $mindays&&$mindays>=1?'PASS':'WARN', $mindays||'默认(0)');
    add_r('密码策略', "过期提醒",     $warnage&&$warnage>=7?'PASS':'WARN', $warnage||'默认(7)');
    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 6. 日志分析（详细内容提取）
# ═══════════════════════════════════════════════════════════════════════
sub check_logs {
    start_loading("分析日志异常...");

    my @logfiles = (
        '/var/log/syslog', '/var/log/syslog.1',
        '/var/log/auth.log', '/var/log/auth.log.1',
        '/var/log/kern.log', '/var/log/kern.log.1',
        '/var/log/messages', '/var/log/messages.1',
        '/var/log/daemon.log', '/var/log/daemon.log.1',
        '/var/log/dmesg', '/var/log/bootstrap.log',
    );

    my ($total_err, $total_warn) = (0, 0);
    my (%err_by_type, %err_by_proc);
    my (@ssh_fails, %ssh_ips, %ssh_users, $ssh_attempts);
    my (@oom_lines, @panic_lines, @disk_err_lines, @svc_fail_lines);
    my (@kernel_err_lines, @docker_err_lines, @dpkg_err_lines);
    my (@network_err_lines, @caddy_err_lines, @pam_err_lines);
    my $seen_line = {};

    for my $path (@logfiles) {
        next unless -r $path;
        open my $fh, '<:raw', $path or next;
        my $n = 0;
        while (<$fh>) {
            $n++; last if $n > 5000;
            chomp;
            my $line = $_;
            my $dk = substr($line, 0, 120); $dk =~ s/\e\[[0-9;]*m//g;
            next if $seen_line->{$dk}++;

            # 级别统计
            if    (/\bERROR\b/i)        { $total_err++; $err_by_type{'ERROR'}++ }
            elsif (/\bFATAL\b/i)        { $total_err++; $err_by_type{'FATAL'}++ }
            elsif (/\bSEVERE\b/i)       { $total_err++; $err_by_type{'SEVERE'}++ }
            elsif (/\bCRITICAL\b/i)     { $total_err++; $err_by_type{'CRITICAL'}++ }
            elsif (/\bCRIT\b/i)         { $total_err++; $err_by_type{'CRIT'}++ }
            elsif (/\bPANIC\b/i)        { $total_err++; $err_by_type{'PANIC'}++ }
            elsif (/\b(Error|Failed|Failure)\b/i) { $total_err++; $err_by_type{'Error'}++ }
            elsif (/\b(WARN|WARNING|Warning)\b/i) { $total_warn++; $err_by_type{'WARN'}++ }
            elsif (/\btimeout\b/i)      { $total_warn++; $err_by_type{'timeout'}++ }

            # 进程提取
            if (/^(\S+\s+\S+\s+\d+\s+\d+:\d+:\d+\s+\S+)\s+(\S+?)(?:\[\d+\])?:\s+/) {
                my $proc = $2; $proc =~ s/[^a-zA-Z0-9_\.\-]//g;
                $err_by_proc{$proc}++ if $proc;
            }

            # SSH 爆破
            if (/Failed password/i) {
                $ssh_attempts++;
                if (/from\s+(\S+)/) { $ssh_ips{$1}++ }
                if (/for\s+(\S+)/)  { $ssh_users{$1}++ }
                push @ssh_fails, $line if @ssh_fails < 5;
            }

            # PAM 错误
            if (/pam_unix|pam_sss|authentication failure/i && /error|fail/i) {
                push @pam_err_lines, $line if @pam_err_lines < 3;
            }

            # OOM
            if (/Out of memory|oom_kill|OOM|invoked oom-killer|Killed process/i) {
                $total_err++; $err_by_type{'OOM'}++;
                my $d = $1 if /Killed process (\d+.*?) /;
                $d = $1 if /(invoked oom-killer.*)/;
                push @oom_lines, $d || $line if @oom_lines < 5;
            }

            # Kernel
            if (/kernel panic|Kernel Panic/i) { push @panic_lines, $line if @panic_lines < 5 }
            if (/segfault|general protection fault|BUG:/i && !/error/i) {
                my $d = $1 if /(\S+\[\d+\]:.*segfault.*)/; $d ||= $line;
                push @kernel_err_lines, $d if @kernel_err_lines < 5;
                $total_err++; $err_by_type{'Kernel'}++;
            }

            # 磁盘/文件系统
            if (/I\/O error|no space left|read error|write error|bad sector|mount error|
                 fsck.*fail|ext\d+.*error|xfs.*fail|journal.*failed/i) {
                push @disk_err_lines, $line if @disk_err_lines < 8;
                $total_err++; $err_by_type{'Disk'}++;
            }

            # Systemd 服务失败
            if (/systemd.*Failed to start|unit.*failed|entered failed state|watchdog timeout/i) {
                my $s = $1 if /(Failed to start.*?\.service)/;
                $s ||= $1 if /(Unit\s+\S+\.service)/i;
                push @svc_fail_lines, $s || $line if @svc_fail_lines < 5;
                $total_err++; $err_by_type{'Systemd'}++;
            }

            # 网络错误
            if (/link down|carrier lost|TX errors|RX errors|Network is unreachable|
                 Connection refused|route.*failed|DHCP.*fail/i) {
                push @network_err_lines, $line if @network_err_lines < 5;
                $total_err++; $err_by_type{'Network'}++;
            }

            # Caddy
            if (/caddy.*error|reverseproxy.*fail|EOF.*request|http\.log\.error/i) {
                push @caddy_err_lines, $line if @caddy_err_lines < 5;
                $total_err++; $err_by_type{'Caddy'}++;
            }

            # Docker
            if (/dockerd.*error|failed to.*container|Error response|Deleting.*rules.*error/i) {
                push @docker_err_lines, $line if @docker_err_lines < 5;
                $total_err++; $err_by_type{'Docker'}++;
            }

            # dpkg
            if (/dpkg.*error|dpkg.*warning.*parsing|subprocess.*error/i) {
                push @dpkg_err_lines, $line if @dpkg_err_lines < 5;
                $total_err++; $err_by_type{'dpkg'}++;
            }
        }
        close $fh;
    }

    # Docker journal
    if (_act('docker')) {
        my $dlog = _cmd("journalctl -u docker --since \"24 hours ago\" --no-pager 2>/dev/null | grep -i 'error\\|fail' | tail -5");
        for my $dl (split("\n", $dlog)) { chomp $dl; push @docker_err_lines, $dl if @docker_err_lines < 5 }
    }

    my $total_issues = $total_err + $total_warn + $ssh_attempts;
    add_r('日志分析', "异常总览", $total_issues>100?'FAIL':$total_issues>20?'WARN':'PASS',
           "错误: ${total_err} / 警告: ${total_warn} / 含SSH: ${ssh_attempts}");

    if (%err_by_type) {
        my @sorted = sort { $err_by_type{$b} <=> $err_by_type{$a} } keys %err_by_type;
        add_r('日志分析', "错误类型分布", $total_err>0?'WARN':'PASS', join(', ', map { "$_: $err_by_type{$_}" } @sorted));
    }
    if (%err_by_proc) {
        my @sorted = sort { $err_by_proc{$b} <=> $err_by_proc{$a} } keys %err_by_proc;
        add_r('日志分析', "异常进程 Top5", 'WARN', join(', ', map { "$_: $err_by_proc{$_}" } @sorted>5 ? @sorted[0..4] : @sorted));
    }

    # SSH 详情
    if ($ssh_attempts > 0) {
        my $unique_ips = scalar(keys %ssh_ips);
        my ($tip, $tcnt) = ('', 0); for (keys %ssh_ips) { ($tip,$tcnt)=($_,$ssh_ips{$_}) if $ssh_ips{$_}>$tcnt }
        my ($tuser, $tucnt) = ('', 0); for (keys %ssh_users) { ($tuser,$tucnt)=($_,$ssh_users{$_}) if $ssh_users{$_}>$tucnt }
        add_r('日志分析', "SSH 爆破攻击", $ssh_attempts>100?'FAIL':$ssh_attempts>10?'WARN':'PASS',
               "共 ${ssh_attempts} 次, ${unique_ips} 个IP | 最活跃IP: ${tip}(${tcnt}) | 最活跃用户: ${tuser}(${tucnt})");
        for my $sf (@ssh_fails) {
            $sf =~ s/^\w{3}\s+\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+\S+\s+//;
            add_r('日志分析', "  SSH 示例", 'FAIL', $sf);
        }
        my @tips = sort { $ssh_ips{$b} <=> $ssh_ips{$a} } keys %ssh_ips;
        add_r('日志分析', "  Top 攻击IP", 'FAIL', join(', ', map { "$_($ssh_ips{$_})" } @tips>3 ? @tips[0..2] : @tips));
        my @tus = sort { $ssh_users{$b} <=> $ssh_users{$a} } keys %ssh_users;
        add_r('日志分析', "  Top 用户", 'WARN', join(', ', map { "$_($ssh_users{$_})" } @tus>5 ? @tus[0..4] : @tus));
    } else { add_r('日志分析', "SSH 爆破", 'PASS', '未检测到') }

    # 各类错误（使用 scalar 取值，不用 ${$#}）
    my $n = scalar(@oom_lines);
    if ($n) { add_r('日志分析', "OOM Killer", 'FAIL', "${n} 次"); for (@oom_lines) { add_r('日志分析', "  OOM", 'FAIL', $_) } } else { add_r('日志分析', "OOM", 'PASS', '无') }
    $n = scalar(@panic_lines);
    if ($n) { add_r('日志分析', "Kernel Panic", 'FAIL', "${n} 次"); for (@panic_lines) { add_r('日志分析', "  Panic", 'FAIL', $_) } } else { add_r('日志分析', "Kernel Panic", 'PASS', '无') }
    $n = scalar(@kernel_err_lines);
    if ($n) { add_r('日志分析', "Kernel 异常", 'FAIL', "${n} 次"); for (@kernel_err_lines) { add_r('日志分析', "  Kernel", 'FAIL', $_) } }
    $n = scalar(@disk_err_lines);
    if ($n) { add_r('日志分析', "磁盘/文件系统错误", 'FAIL', "${n} 条"); for (@disk_err_lines) { add_r('日志分析', "  磁盘错误", 'FAIL', $_) } } else { add_r('日志分析', "磁盘错误", 'PASS', '无') }
    $n = scalar(@svc_fail_lines);
    if ($n) { add_r('日志分析', "Systemd 服务失败", 'FAIL', "${n} 个"); for (@svc_fail_lines) { add_r('日志分析', "  服务失败", 'FAIL', $_) } } else { add_r('日志分析', "Systemd 服务", 'PASS', '无失败') }
    $n = scalar(@docker_err_lines);
    if ($n) { add_r('日志分析', "Docker 错误", 'WARN', "${n} 条"); for (@docker_err_lines) { add_r('日志分析', "  Docker", 'WARN', $_) } }
    $n = scalar(@caddy_err_lines);
    if ($n) { add_r('日志分析', "Caddy/Proxy 错误", 'WARN', "${n} 条"); for (@caddy_err_lines) { s/.*\"msg\":\"([^\"]+).*/$1/; add_r('日志分析', "  Proxy", 'WARN', $_) } }
    $n = scalar(@network_err_lines);
    if ($n) { add_r('日志分析', "网络错误", 'FAIL', "${n} 条"); for (@network_err_lines) { add_r('日志分析', "  网络", 'FAIL', $_) } }
    $n = scalar(@pam_err_lines);
    if ($n) { add_r('日志分析', "PAM 认证失败", 'WARN', "${n} 条"); for (@pam_err_lines) { add_r('日志分析', "  PAM", 'WARN', $_) } }
    $n = scalar(@dpkg_err_lines);
    if ($n) { add_r('日志分析', "dpkg/apt 错误", 'WARN', "${n} 条"); for (@dpkg_err_lines) { add_r('日志分析', "  dpkg", 'WARN', $_) } }

    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 7. 硬件健康 (12项) — 始终显示完整数值
# ═══════════════════════════════════════════════════════════════════════
sub check_hardware {
    start_loading("检查硬件健康...");

    my $meminfo = _read('/proc/meminfo');
    my ($mem_total, $mem_avail, $swap_total, $swap_free);
    ($mem_total)   = $meminfo =~ /MemTotal:\s+(\d+)/;
    ($mem_avail)   = $meminfo =~ /MemAvailable:\s+(\d+)/;
    ($swap_total)  = $meminfo =~ /SwapTotal:\s+(\d+)/;
    ($swap_free)   = $meminfo =~ /SwapFree:\s+(\d+)/;
    $mem_total  ||= 0; $mem_avail  ||= 0; $swap_total ||= 0; $swap_free  ||= 0;
    my $mem_used  = $mem_total - $mem_avail;
    my $mem_pct   = $mem_total>0 ? int($mem_used*100/$mem_total) : 0;
    my $mem_free_pct = $mem_total>0 ? int($mem_avail*100/$mem_total) : 0;
    my $mem_total_mb = int($mem_total/1024);
    my $mem_used_mb  = int($mem_used/1024);
    my $mem_avail_mb = int($mem_avail/1024);
    add_r('硬件健康', "内存", $mem_pct>90?'FAIL':$mem_pct>75?'WARN':'PASS',
           "${mem_used_mb}MB / ${mem_total_mb}MB (${mem_pct}%)  空闲: ${mem_avail_mb}MB (${mem_free_pct}%)");

    if ($swap_total>0) {
        my $sp = int(($swap_total-$swap_free)*100/$swap_total);
        my $swap_used_mb = int(($swap_total-$swap_free)/1024);
        my $swap_total_mb = int($swap_total/1024);
        add_r('硬件健康', "Swap", $sp>50?'WARN':'PASS', "${swap_used_mb}MB / ${swap_total_mb}MB (${sp}%)");
    } else { add_r('硬件健康', "Swap", 'PASS', '未配置 Swap') }

    my $cpu_model = _cmd("grep -m1 'model name' /proc/cpuinfo|cut -d: -f2|xargs")||'N/A';
    my $cores     = _cmd('nproc');
    my $cpu_usage = _cmd("top -bn1 2>/dev/null | grep 'Cpu(s)' | awk '{print \$2}'") || _cmd("grep '^cpu ' /proc/stat 2>/dev/null|awk '{usage=(\$2+\$4)*100/(\$2+\$4+\$5+\$6+\$7+\$8)} END {printf \"%.1f\", usage}'") || 'N/A';
    add_r('硬件健康', 'CPU', 'PASS', "$cpu_model ($cores 核)  使用率: ${cpu_usage}%");

    for my $mp (qw(/ /var /home /data /opt /boot)) {
        next unless -d $mp;
        my $df = _cmd("df -h $mp 2>/dev/null | tail -1");
        my ($fs,$size,$used,$avail,$pct,$mnt) = split(/\s+/, $df);
        next unless $mnt; $pct =~ s/%//;
        my $sev = $pct>90?'FAIL':$pct>80?'WARN':'PASS';
        add_r('硬件健康', "磁盘 $mnt", $sev, "总量: ${size}, 已用: ${used}, 剩余: ${avail} (${pct}%)  文件系统: ${fs}");
    }

    my $smart = _cmd("smartctl -H /dev/sda 2>/dev/null|grep 'SMART overall-health'|awk -F': ' '{print \$2}'");
    my $smart_raw = _cmd("smartctl -a /dev/sda 2>/dev/null | grep -E 'Reallocated_Sector|Current_Pending_Sector|Offline_Uncorrectable' | head -5") || '';
    $smart_raw =~ s/\n/; /g; chomp $smart_raw;
    add_r('硬件健康', "磁盘 SMART", $smart eq 'PASSED'?'PASS':'WARN', $smart||'不支持');
    if ($smart_raw) { add_r('硬件健康', "  SMART 属性", 'PASS', $smart_raw) }

    my $io_wait = _cmd("iostat -c 1 2 2>/dev/null|tail -1|awk '{print \$4}'") || 'N/A';
    add_r('硬件健康', "CPU I/O Wait", $io_wait ne 'N/A' && $io_wait>30?'WARN':'PASS', "${io_wait}%");

    my $uptime_sec = (_read('/proc/uptime') =~ /^(\d+)/ ? $1 : 0);
    my $uptime_days = int($uptime_sec / 86400);
    add_r('硬件健康', "系统运行", 'PASS', "${uptime_days} 天");

    my $procs = _cmd("ps aux --no-headers 2>/dev/null | wc -l");
    my $fds   = _cmd("cat /proc/sys/fs/file-nr 2>/dev/null|awk '{print \$1}'") || 'N/A';
    my $fdmax = _cmd("cat /proc/sys/fs/file-max 2>/dev/null") || 'N/A';
    add_r('硬件健康', "进程数", $procs<500?'PASS':$procs<1000?'WARN':'FAIL', "${procs} 个");
    add_r('硬件健康', "文件句柄", 'PASS', "已用: ${fds} / 最大: ${fdmax}");

    my $zombie = _cmd("ps aux --no-headers 2>/dev/null | awk '{print \$8}' | grep -c Z");
    add_r('硬件健康', "僵尸进程", $zombie==0?'PASS':'FAIL', "${zombie} 个");

    stop_loading();
}

# ═══════════════════════════════════════════════════════════════════════
# 报告渲染（严格闭合边框，炫酷仪表盘风格）
# ═══════════════════════════════════════════════════════════════════════
sub render_report {
    my $W  = get_cols();
    my $TW = $W - 2;
    my $time = strftime('%Y-%m-%d %H:%M:%S', localtime);
    my $inner = $TW - 2;

    # ── 统计 ──
    my %groups; for my $r (@results) { push @{$groups{$r->{category}}}, $r }
    my @cat_order = qw(系统配置 服务状态 防火墙 SSH安全 密码策略 日志分析 硬件健康);
    my %cat_scores; my %cat_counts;
    for my $cat (@cat_order) {
        next unless $groups{$cat};
        my $items = $groups{$cat};
        my $pass = scalar(grep{$_->{status}eq'PASS'}@$items);
        my $warn = scalar(grep{$_->{status}eq'WARN'}@$items);
        my $fail = scalar(grep{$_->{status}eq'FAIL'}@$items);
        my $total = scalar(@$items);
        $cat_scores{$cat}=$total>0?int(($pass*100+$warn*50)/$total):0;
        $cat_counts{$cat}={pass=>$pass,warn=>$warn,fail=>$fail,total=>$total};
    }
    my $total_items = scalar(@results);
    my $total_pass  = scalar(grep{$_->{status}eq'PASS'}@results);
    my $total_warn  = scalar(grep{$_->{status}eq'WARN'}@results);
    my $total_fail  = scalar(grep{$_->{status}eq'FAIL'}@results);
    my $total_score = $total_items>0?int(($total_pass*100+$total_warn*50)/$total_items):0;

    my $grade = $total_score>=90?'S':$total_score>=80?'A':$total_score>=70?'B':$total_score>=60?'C':$total_score>=40?'D':'F';
    my $grade_text = $total_score>=90?'完美':$total_score>=80?'优秀':$total_score>=70?'良好':$total_score>=60?'一般':$total_score>=40?'较差':'危险';
    my $grade_color = $total_score>=80?$G:$total_score>=60?$CY:$total_score>=40?$Y:$RD;

    # ═══ 标题 ═══
    print "\n";
    print "${B}${BL}╔${R}" . ("${BL}═${R}" x $inner) . "${BL}╗${R}\n";
    print "${BL}║${R}" . (' ' x $inner) . "${BL}║${R}\n";
    my $title = "🔍 智能综合检查报告";
    my $td = _dw($title);
    my $pad_l = int(($inner - $td) / 2);
    my $pad_r = $inner - $td - $pad_l;
    print "${BL}║${R}" . (' ' x $pad_l) . "${B}${CY}${title}${R}" . (' ' x $pad_r) . "${BL}║${R}\n";
    $title = "生成时间: $time";
    $td = _dw($title);
    $pad_l = int(($inner - $td) / 2);
    $pad_r = $inner - $td - $pad_l;
    print "${BL}║${R}" . (' ' x $pad_l) . "${D}${title}${R}" . (' ' x $pad_r) . "${BL}║${R}\n";
    print "${BL}║${R}" . (' ' x $inner) . "${BL}║${R}\n";
    print "${BL}╚${R}" . ("${BL}═${R}" x $inner) . "${BL}╝${R}\n\n";

    # ═══ 总评分仪表盘 ═══
    my $mw = int(($TW - 7) / 3);
    my $bar_str = health_bar($total_score, 25);
    top_border($mw, $mw, $mw);
    table_row(" ${B}${grade_color}总评分: ${total_score}/100${R}", $mw,
              " ${B}${grade_color}等级: ${grade} (${grade_text})${R}", $mw,
              " ${B}统计${R}", $mw);
    mid_border($mw, $mw, $mw);
    table_row(" $bar_str", $mw,
              " ${G}✔ ${total_pass}${R}  ${Y}△ ${total_warn}${R}  ${RD}✘ ${total_fail}${R}", $mw,
              " ${total_items} 项检查", $mw);
    bot_border($mw, $mw, $mw);
    print "\n";

    # ═══ 分类评分卡片 ═══
    my $cw_card = int(($TW - 7) / 4);
    top_border($cw_card, $cw_card, $cw_card, $cw_card);
    table_row("${B}分类${R}", $cw_card, "${B}评分${R}", $cw_card, "${B}通过/警告/失败${R}", $cw_card*2);
    mid_border($cw_card, $cw_card, $cw_card, $cw_card);
    for my $cat (@cat_order) {
        next unless $groups{$cat};
        my $sc = $cat_scores{$cat};
        my $ct = $cat_counts{$cat};
        my $sc_color = $sc>=80?$G:$sc>=60?$CY:$sc>=40?$Y:$RD;
        my $bar = $sc>=80?"${G}████████${R}":$sc>=60?"${CY}██████${R}${D}██${R}":$sc>=40?"${Y}████${R}${D}████${R}":"${RD}██${R}${D}██████${R}";
        table_row(" ${cat}", $cw_card, " ${sc_color}${sc}${R}", $cw_card,
                  " ${bar}  " . "${G}" . $ct->{pass} . "${R}/${Y}" . $ct->{warn} . "${R}/${RD}" . $ct->{fail} . "${R}", $cw_card*2);
    }
    bot_border($cw_card, $cw_card, $cw_card, $cw_card);
    print "\n";

    # ═══ 详细结果（严格闭合边框） ═══
    for my $cat (@cat_order) {
        next unless $groups{$cat};
        my $items = $groups{$cat};
        my $sc = $cat_scores{$cat};
        my $ct = $cat_counts{$cat};

        # 分类标题行 — 严格匹配 inner 宽度
        my $h_line = "  ▸ ${cat}  ";
        my $h_score = " [ ${sc}/100 ] ";
        my $h_stat = " ✔" . $ct->{pass} . " △" . $ct->{warn} . " ✘" . $ct->{fail} . " ";
        my $h_content = "${B}${CY}${h_line}${R}${B}${h_score}${R}${D}${h_stat}${R}";
        my $h_dw = _dw($h_content);
        my $h_pad = $inner - $h_dw;
        $h_pad = 0 if $h_pad < 0;
        print "  ${B}${CY}┌${R}" . ("${CY}─${R}" x $inner) . "${CY}┐${R}\n";
        print "  ${CY}│${R} ${h_content}" . (' ' x $h_pad) . " ${CY}│${R}\n";
        print "  ${CY}├${R}" . ("${CY}─${R}" x $inner) . "${CY}┤${R}\n";

        for my $r (@$items) {
            my $icon = $r->{status} eq 'PASS' ? "${G}✔${R}" : $r->{status} eq 'WARN' ? "${Y}△${R}" : "${RD}✘${R}";
            my $tag  = $r->{status} eq 'PASS' ? "${B}${G}PASS${R}" : $r->{status} eq 'WARN' ? "${B}${Y}WARN${R}" : "${B}${RD}FAIL${R}";
            my $detail = $r->{detail} || '';
            # 内容行：固定在边框内
            my $content = " ${icon} ${B}$r->{name}${R} ${tag}";
            $content .= "  ${D}${detail}${R}" if $detail;
            my $cw = _dw($content);
            my $diff = $inner - $cw;
            if ($diff < 0) {
                # 截断到 inner-3
                $diff = 3;
                $content = " ${icon} ${B}$r->{name}${R} ...";
                $cw = _dw($content);
                $diff = $inner - $cw;
            }
            $diff = 0 if $diff < 0;
            print "  ${CY}│${R}${content}" . (' ' x $diff) . "${CY}│${R}\n";
        }

        print "  ${CY}└${R}" . ("${CY}─${R}" x $inner) . "${CY}┘${R}\n\n";
    }

    # ═══ 高风险 ═══
    my @fails = grep{$_->{status}eq'FAIL'}@results;
    if (@fails) {
        print "  ${B}${RD}┌${R}" . ("${RD}─${R}" x $inner) . "${RD}┐${R}\n";
        my $ht = " ⚠ 高风险项目 (FAIL) — 需立即处理 ";
        my $htw = _dw($ht);
        my $htp = $inner - $htw;
        $htp = 0 if $htp < 0;
        print "  ${RD}│${R}${B}${RD}${ht}${R}" . (' ' x $htp) . "${RD}│${R}\n";
        print "  ${RD}├${R}" . ("${RD}─${R}" x $inner) . "${RD}┤${R}\n";
        for my $f (@fails) {
            my $line = " ${RD}✘${R} ${B}[$f->{category}]${R} $f->{name}";
            $line .= " — ${D}$f->{detail}${R}" if $f->{detail};
            my $lw = _dw($line);
            my $diff = $inner - $lw;
            $diff = 0 if $diff < 0;
            print "  ${RD}│${R}${line}" . (' ' x $diff) . "${RD}│${R}\n";
        }
        print "  ${RD}└${R}" . ("${RD}─${R}" x $inner) . "${RD}┘${R}\n\n";
    }

    # ═══ 建议关注 ═══
    my @warns = grep{$_->{status}eq'WARN'}@results;
    if (@warns) {
        print "  ${B}${Y}┌${R}" . ("${Y}─${R}" x $inner) . "${Y}┐${R}\n";
        my $ht = " ⚑ 建议关注 (WARN) — 建议优化 ";
        my $htw = _dw($ht);
        my $htp = $inner - $htw;
        $htp = 0 if $htp < 0;
        print "  ${Y}│${R}${B}${Y}${ht}${R}" . (' ' x $htp) . "${Y}│${R}\n";
        print "  ${Y}├${R}" . ("${Y}─${R}" x $inner) . "${Y}┤${R}\n";
        for my $w (@warns) {
            my $line = " ${Y}△${R} ${B}[$w->{category}]${R} $w->{name}";
            $line .= " — ${D}$w->{detail}${R}" if $w->{detail};
            my $lw = _dw($line);
            my $diff = $inner - $lw;
            $diff = 0 if $diff < 0;
            print "  ${Y}│${R}${line}" . (' ' x $diff) . "${Y}│${R}\n";
        }
        print "  ${Y}└${R}" . ("${Y}─${R}" x $inner) . "${Y}┘${R}\n\n";
    }

    # ═══ 页脚 ═══
    print "  ${D}${BL}┌${R}" . ("${BL}─${R}" x $inner) . "${BL}┐${R}\n";
    my $footer = " DM 智能检查引擎 v2.0 | ${total_items} 项检查 | ${total_pass} 通过 | ${total_warn} 建议 | ${total_fail} 高风险 ";
    my $fw = _dw($footer);
    my $fp = int(($inner - $fw) / 2);
    $fp = 0 if $fp < 0;
    my $fr = $inner - $fw - $fp;
    printf "  ${BL}│${R}${D}%s${footer}%s${BL}│${R}\n", ' ' x $fp, ' ' x $fr;
    print "  ${BL}└${R}" . ("${BL}─${R}" x $inner) . "${BL}┘${R}\n";
    print "\n";
}

# ═══════════════════════════════════════════════════════════════════════
# 入口
# ═══════════════════════════════════════════════════════════════════════
my $EXPORT = '';
while (@ARGV) {
    my $a = shift @ARGV;
    $EXPORT = shift @ARGV if $a eq '-export';
}

sub run_checks {
    check_system_config();
    check_services();
    check_firewall();
    check_ssh();
    check_password_policy();
    check_logs();
    check_hardware();
    render_report();
}

if ($EXPORT) {
    # 导出模式：重新执行自身（无 -export 参数），去色写文件
    my @clean_args;
    my $skip = 0;
    for my $a (@ARGV) {
        if ($a eq '-export') { $skip = 1; next }
        if ($skip) { $skip = 0; next }
        push @clean_args, $a;
    }
    my $plain = `perl $0 @clean_args 2>/dev/null | perl -pe 's/\\e\\[[0-9;]*m//g'`;
    if ($plain) {
        `mkdir -p "$EXPORT"`;
        my $fname = "$EXPORT/smart-check-" . strftime('%Y%m%d-%H%M%S', localtime) . ".txt";
        open my $fh, '>:utf8', $fname or die "无法写入: $!";
        print $fh $plain;
        close $fh;
        print "已导出: $fname (" . int(length($plain)/1024) . "KB)\n";
    }
    exit 0;
}

printf "\e[H\e[2J";
print "${B}${CY}╔══════════════════════════════════════════════════════════════╗${R}\n";
print "${B}${CY}║             智能综合检查启动中...                           ║${R}\n";
print "${B}${CY}╚══════════════════════════════════════════════════════════════╝${R}\n\n";

run_checks();
exit 0;
