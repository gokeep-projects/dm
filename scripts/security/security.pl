#!/usr/bin/perl
use strict;
use warnings;
use POSIX qw(strftime);

# ──────────────────── 颜色 ────────────────────
my ($R, $B, $D, $CY, $G, $Y, $RD, $BL) = (
    "\e[0m", "\e[1m", "\e[2m", "\e[36m",
    "\e[32m", "\e[33m", "\e[31m", "\e[34m"
);

# ──────────────────── 表格绘制（字节级 UTF-8 宽度） ────────────────────
sub _dw {
    my ($s) = @_;
    $s =~ s/\e\[[0-9;]*m//g;
    my $len = length($s);
    my $w = 0;
    for my $i (0 .. $len - 1) {
        my $b = ord(substr($s, $i, 1));
        if ($b < 0x80) { $w += 1 }
        elsif ($b >= 0xC0) { $w += 2 }
    }
    return $w;
}
sub _pad {
    my ($s, $tw) = @_;
    my $plain = $s;
    $plain =~ s/\e\[[0-9;]*m//g;
    my $plen = length($plain);
    my $cw = 0;
    my $out = '';
    for (my $i = 0; $i < $plen; $i++) {
        last if $cw >= $tw - 1;
        my $b = ord(substr($plain, $i, 1));
        if ($b < 0x80) { $out .= substr($plain, $i, 1); $cw += 1 }
        elsif ($b >= 0xF0) { last if $cw + 2 > $tw - 1; $out .= substr($plain, $i, 4); $cw += 2; $i += 3 }
        elsif ($b >= 0xE0) { last if $cw + 2 > $tw - 1; $out .= substr($plain, $i, 3); $cw += 2; $i += 2 }
        elsif ($b >= 0xC0) { last if $cw + 2 > $tw - 1; $out .= substr($plain, $i, 2); $cw += 2; $i += 1 }
    }
    if (length($out) < $plen) { $out .= '..'; $cw += 2 }
    my $diff = $tw - $cw;
    $diff = 0 if $diff < 0;
    return $out . (' ' x $diff);
}
sub top_border {
    my $s = "  ┌";
    my $i = 0;
    for my $w (@_) {
        $s .= "─" x $w;
        $s .= ($i++ < $#_) ? "┬" : "┐";
    }
    print "$s\n";
}
sub mid_border {
    my $s = "  ├";
    my $i = 0;
    for my $w (@_) {
        $s .= "─" x $w;
        $s .= ($i++ < $#_) ? "┼" : "┤";
    }
    print "$s\n";
}
sub bot_border {
    my $s = "  └";
    my $i = 0;
    for my $w (@_) {
        $s .= "─" x $w;
        $s .= ($i++ < $#_) ? "┴" : "┘";
    }
    print "$s\n";
}
sub table_row {
    my $s = "  │";
    while (@_ >= 2) { $s .= _pad(shift, shift) . '│' }
    print "$s\n";
}
sub header {
    my ($title, $w) = @_;
    $w ||= 120;
    my $tdw = _dw($title);
    my $pad = $w - 2 - $tdw;
    $pad = 0 if $pad < 0;
    my $l = int($pad / 2);
    my $r = $pad - $l;
    print "${B}${BL}" . '╔' . ('═' x ($w - 2)) . "╗${R}\n";
    printf "%s║%s%s%s║%s\n", $B . $BL, ' ' x $l, $title, ' ' x $r, $R;
    print "${B}${BL}" . '╚' . ('═' x ($w - 2)) . "╝${R}\n\n";
}
sub get_cols {
    my $tw = (`tput cols 2>/dev/null` || 140) + 0;
    my $w  = int($tw * 0.85);
    $w = 100 if $w < 100; $w = 200 if $w > 200;
    return $w;
}

# ──────────────────── 工具函数 ────────────────────
sub _firstline {
    my ($path) = @_;
    return '' unless -r $path;
    open my $fh, '<:raw', $path or return '';
    my $l = <$fh>; close $fh;
    defined $l and chomp $l;
    return $l // '';
}
sub _readfile {
    my ($path) = @_;
    return '' unless -r $path;
    open my $fh, '<:raw', $path or return '';
    my $c = do { local $/; <$fh> }; close $fh;
    chomp $c; return $c;
}
sub _cmd {
    my ($cmd) = @_;
    my $out = `$cmd 2>/dev/null`; chomp $out; return $out;
}
sub _cmd_code {
    my ($cmd) = @_;
    system("$cmd >/dev/null 2>&1"); return $? >> 8;
}
sub _svc_active { my ($s) = @_; return _cmd_code("systemctl is-active $s") == 0 }

# ──────────────────── 防火墙/端口规则采集 ────────────────────

sub collect_iptables_rules {
    my $raw = _cmd('iptables -L -n -v --line-numbers 2>/dev/null');
    return () unless $raw;
    my @chains;
    my $current_chain = '';
    my $current_policy = '';
    my @rules;
    for my $line (split("\n", $raw)) {
        chomp $line;
        if ($line =~ /^Chain (\S+) \(policy (\S+)/) {
            if ($current_chain) {
                push @chains, { name => $current_chain, policy => $current_policy, rules => [@rules] };
                @rules = ();
            }
            $current_chain = $1;
            $current_policy = $2;
        } elsif ($line =~ /^\s*\d+\s+\d+\s+\d+\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)/) {
            my $target = $1; my $prot = $2; my $opt = $3; my $in = $4; my $out = $5; my $src = $6;
            my $rest = $'; my $dport = ''; my $ctstate = '';
            if ($rest =~ /dpt?:(\S+)/)   { $dport   = $1 }
            if ($rest =~ /ctstate\s+(\S+)/) { $ctstate = $1 }
            push @rules, { target=>$target, prot=>$prot, in=>$in, out=>$out, src=>$src, dport=>$dport, ctstate=>$ctstate, raw=>$line };
        }
    }
    if ($current_chain) {
        push @chains, { name => $current_chain, policy => $current_policy, rules => [@rules] };
    }
    return @chains;
}

sub collect_listening_ports {
    my $raw = _cmd("ss -tlnp 2>/dev/null");
    my @ports;
    for my $line (split("\n", $raw)) {
        chomp $line;
        next if $line =~ /^State/;
        if ($line =~ /(\S+):(\d+)\s+\S+\s+users:\(\((.*?)\)\)/) {
            my $addr = $1; my $port = $2; my $proc = $3 || '';
            $proc =~ s/",pid=\d+.*//; $proc =~ s/"//g;
            $proc = substr($proc, 0, 16) if length($proc) > 16;
            push @ports, { addr => $addr, port => $port, proc => $proc || '-' };
        } elsif ($line =~ /(\S+):(\d+)\s+\S+\s*$/) {
            push @ports, { addr => $1, port => $2, proc => '-' };
        }
    }
    return @ports;
}

sub check_port_in_rules {
    my ($port, $chains_ref) = @_;
    for my $chain (@$chains_ref) {
        next unless $chain->{name} eq 'INPUT' || $chain->{name} eq 'OUTPUT' || $chain->{name} =~ /^DOCKER/;
        for my $rule (@{$chain->{rules}}) {
            next unless $rule->{dport} && $rule->{dport} eq $port;
            next if $rule->{prot} ne 'tcp' && $rule->{prot} ne '6';
            return ($rule->{target}, $chain->{name});
        }
    }
    return ('', '');
}

# ──────────────────── 主采集 ────────────────────
my %sec;
my @chains = collect_iptables_rules();
my @listening = collect_listening_ports();
my $default_action = '';

for my $ch (@chains) {
    if ($ch->{name} eq 'INPUT') { $default_action = $ch->{policy}; last }
}

$sec{fw_name}   = 'iptables';
$sec{fw_status} = '';
if (_svc_active('firewalld')) {
    $sec{fw_name}   = 'firewalld';
    my $s = _cmd('firewall-cmd --state');
    $sec{fw_status} = ($s eq 'running') ? "${G}运行中${R}" : "${RD}关闭${R}";
} elsif (_svc_active('ufw')) {
    $sec{fw_name}   = 'UFW';
    my $s = _cmd('ufw status');
    $sec{fw_status} = ($s =~ /active/i) ? "${G}运行中${R}" : "${RD}关闭${R}";
} else {
    if ($default_action eq 'ACCEPT') {
        $sec{fw_status} = "${RD}全部放行 (INPUT $default_action)${R}";
    } elsif ($default_action eq 'DROP') {
        $sec{fw_status} = "${G}严格限制 (INPUT $default_action)${R}";
    } else {
        $sec{fw_status} = "${Y}INPUT $default_action${R}";
    }
    my $has_docker = grep { $_->{name} =~ /^DOCKER/ } @chains;
    $sec{fw_name} .= $has_docker ? ' + Docker' : '';
}

my @port_report;
for my $lp (@listening) {
    my ($rule_action, $chain_name) = check_port_in_rules($lp->{port}, \@chains);
    my $status;
    if ($rule_action eq 'ACCEPT') {
        $status = "${G}放行${R}";
    } elsif ($rule_action eq 'DROP') {
        $status = "${RD}拒绝${R}";
    } elsif ($default_action eq 'ACCEPT') {
        $status = "${Y}默认放行${R}";
    } elsif ($default_action eq 'DROP') {
        $status = "${Y}默认拒绝(未豁免)${R}";
    } else {
        $status = "${Y}未匹配规则${R}";
    }
    push @port_report, {
        port   => $lp->{port},
        addr   => $lp->{addr},
        proc   => $lp->{proc} || '-',
        status => $status,
        rule   => $rule_action || $default_action,
    };
}

# ──── 安全子系统和内核加固（精简） ────
$sec{selinux_mode}   = _cmd('getenforce');
$sec{selinux_mode}   = '未安装/未启用' if $sec{selinux_mode} eq '';
$sec{selinux_config} = _cmd("grep '^SELINUX=' /etc/selinux/config 2>/dev/null | cut -d= -f2") || '无配置';
$sec{selinux_type}   = _cmd("grep '^SELINUXTYPE=' /etc/selinux/config 2>/dev/null | cut -d= -f2") || '-';
$sec{apparmor} = "${RD}未安装${R}";
$sec{aa_info}  = 'N/A';
if (_svc_active('apparmor') || -r '/sys/module/apparmor/parameters/enabled') {
    my $aa = _readfile('/sys/module/apparmor/parameters/enabled');
    $sec{apparmor} = $aa && $aa eq 'Y' ? "${G}启用${R}" : "${RD}禁用${R}";
    $sec{aa_info}  = _cmd("aa-status 2>/dev/null | head -3 | tr '\n' ' '") || '?';
}
$sec{evm} = -r '/sys/kernel/security/evm/evm-enabled' ? "${G}启用${R}" : "${RD}不支持${R}";
$sec{ima} = -r '/sys/kernel/security/ima/ima_policy'   ? "${G}启用${R}" : "${RD}未启用${R}";
my $aslr = _firstline('/proc/sys/kernel/randomize_va_space');
$sec{aslr} = $aslr eq '2' ? "${G}完全随机化 (2)${R}" : $aslr eq '1' ? "${Y}部分随机化 (1)${R}" : "${RD}关闭 (0)${R}";
my $yama = _firstline('/proc/sys/kernel/yama/ptrace_scope') || 'N/A';
$sec{kptr_restrict}  = _firstline('/proc/sys/kernel/kptr_restrict')  || 'N/A';
$sec{dmesg_restrict} = _firstline('/proc/sys/kernel/dmesg_restrict') || 'N/A';
my $core_pat = _firstline('/proc/sys/kernel/core_pattern');
$sec{core_pipe} = ($core_pat =~ /^\|/) ? "${G}管道捕获${R}" : "${Y}文件落盘${R}";
$sec{kexec_disabled}   = _firstline('/proc/sys/kernel/kexec_load_disabled') eq '1' ? "${G}禁用${R}" : "${RD}未禁用${R}";
$sec{modules_disabled} = _firstline('/proc/sys/kernel/modules_disabled') eq '1'     ? "${G}禁用${R}" : "${RD}未禁用${R}";
$sec{protect_hardlinks}= _firstline('/proc/sys/fs/protected_hardlinks') eq '1'     ? "${G}开启${R}" : "${RD}关闭${R}";
$sec{protect_symlinks} = _firstline('/proc/sys/fs/protected_symlinks') eq '1'      ? "${G}开启${R}" : "${RD}关闭${R}";
$sec{tcp_syncookies}  = _firstline('/proc/sys/net/ipv4/tcp_syncookies') eq '1' ? "${G}开启${R}" : "${RD}关闭${R}";
$sec{rp_filter}       = _firstline('/proc/sys/net/ipv4/conf/all/rp_filter') eq '1' ? "${G}开启${R}" : "${RD}关闭${R}";
$sec{ip_forward_flag} = _firstline('/proc/sys/net/ipv4/ip_forward') eq '1' ? "${Y}开启${R}" : "${G}关闭${R}";
$sec{icmp_ignore}     = _firstline('/proc/sys/net/ipv4/icmp_echo_ignore_broadcasts') eq '1' ? "${G}开启${R}" : "${RD}关闭${R}";
my $ssh_cfg = '/etc/ssh/sshd_config';
my $ssh_root = 'prohibit-password';
my $ssh_pw = 'yes';
if (-r $ssh_cfg) {
    $ssh_root = _cmd("grep -i '^PermitRootLogin' $ssh_cfg | awk '{print \$2}' | tail -1") || 'prohibit-password';
    $ssh_pw   = _cmd("grep -i '^PasswordAuthentication' $ssh_cfg | awk '{print \$2}' | tail -1") || 'yes';
}
$sec{auditd}  = _svc_active('auditd') ? "${G}运行中${R}" : "${RD}未运行${R}";
$sec{lockdown}= _readfile('/sys/kernel/security/lockdown') || 'none';
$sec{lockdown}=~ s/^\s+|\s+$//g;

# ───── 参数 ─────
my $FOLLOW = 0;
my $EXPORT = '';
my $EXPORTING = 0;

# ──────────────────── 渲染 ────────────────────
sub run {
    printf "\e[H\e[2J" unless $EXPORT;
    my $W  = get_cols();
    my $CW = int(($W - 7) / 4);
    my $TW = 7 + 4 * $CW;           # 表格实际宽度，与标题栏统一
    my $time = strftime('%Y-%m-%d %H:%M:%S', localtime);
    header("系统安全策略检查  $time", $TW);

    # ════════ 板块一：安全子系统概览 ════════
    top_border($CW, $CW, $CW, $CW);
    table_row("安全子系统", $CW, "状态", $CW,
              "安全子系统", $CW, "状态", $CW);
    mid_border($CW, $CW, $CW, $CW);
    table_row(" SELinux",          $CW, " $sec{selinux_mode}",    $CW,
              " SELinux(配置)",    $CW, " $sec{selinux_config} / $sec{selinux_type}", $CW);
    table_row(" AppArmor",         $CW, " $sec{apparmor}",        $CW,
              " AppArmor(详情)",   $CW, " $sec{aa_info}",         $CW);
    table_row(" EVM",              $CW, " $sec{evm}",             $CW,
              " IMA",              $CW, " $sec{ima}",             $CW);
    table_row(" Auditd",           $CW, " $sec{auditd}",          $CW,
              " 系统锁定",         $CW, " $sec{lockdown}",        $CW);
    table_row(" ASLR",             $CW, " $sec{aslr}",            $CW,
              " Yama ptrace",      $CW, " $yama",                 $CW);
    table_row(" kptr_restrict",    $CW, " $sec{kptr_restrict}",   $CW,
              " dmesg_restrict",   $CW, " $sec{dmesg_restrict}",  $CW);
    table_row(" Core dump",        $CW, " $sec{core_pipe}",       $CW,
              " kexec / modules",  $CW, " $sec{kexec_disabled} / $sec{modules_disabled}", $CW);
    table_row(" 硬/软链接保护",    $CW, " $sec{protect_hardlinks} / $sec{protect_symlinks}", $CW,
              " TCP Syncookies",   $CW, " $sec{tcp_syncookies}",  $CW);
    table_row(" rp_filter",        $CW, " $sec{rp_filter}",       $CW,
              " IP 转发",          $CW, " $sec{ip_forward_flag}", $CW);
    table_row(" ICMP 广播忽略",    $CW, " $sec{icmp_ignore}",     $CW,
              " Root SSH 登录",    $CW, " $ssh_root",             $CW);
    bot_border($CW, $CW, $CW, $CW);
    print "\n";

    # ════════ 板块二：防火墙默认策略 ════════
    my $PCW = int(($TW - 7) / 3);
    top_border($PCW, $PCW, $PCW);
    table_row("链",                    $PCW,
              "默认策略",              $PCW,
              "说明",                  $PCW);
    mid_border($PCW, $PCW, $PCW);
    for my $ch (@chains) {
        next unless $ch->{name} =~ /^(INPUT|FORWARD|OUTPUT)$/;
        my $pol = $ch->{policy};
        my $pol_color = $pol eq 'ACCEPT' ? "$pol" : "$pol";
        my $desc = $pol eq 'ACCEPT' ? "全部放行" :
                   $pol eq 'DROP'   ? "全部拒绝" :
                   $pol eq 'REJECT' ? "全部拒绝" : $pol;
        my $rc = scalar @{$ch->{rules}};
        table_row(" $ch->{name}",     $PCW,
                  " $pol_color",      $PCW,
                  " $desc (${rc}条规则)", $PCW);
    }
    bot_border($PCW, $PCW, $PCW);
    print "\n";

    # ════════ 板块三：防火墙端口规则明细 ════════
    my $RCW = int(($TW - 7) / 2);
    top_border($RCW, $RCW);
    table_row("防火墙规则 (INPUT链)", $RCW, "规则内容", $RCW);
    mid_border($RCW, $RCW);
    my $input_chain;
    for my $ch (@chains) {
        if ($ch->{name} eq 'INPUT') { $input_chain = $ch; last }
    }
    if ($input_chain && @{$input_chain->{rules}}) {
        for my $rule (@{$input_chain->{rules}}) {
            my $target_color = $rule->{target} eq 'ACCEPT' ? "$rule->{target}" :
                               $rule->{target} eq 'DROP'   ? "$rule->{target}" :
                               $rule->{target} eq 'REJECT' ? "$rule->{target}" : "$rule->{target}";
            my $desc = '';
            if ($rule->{dport}) {
                $desc = $rule->{prot} . " dpt:$rule->{dport}";
            } elsif ($rule->{ctstate}) {
                $desc = "state $rule->{ctstate}";
            } else {
                $desc = $rule->{raw};
            }
            table_row(" $target_color", $RCW, " $desc", $RCW);
        }
    } else {
        my $msg = $default_action eq 'ACCEPT' ? "无规则约束，所有入站默认放行" :
                  $default_action eq 'DROP'   ? "无规则豁免，所有入站默认拒绝" :
                  "INPUT 链无自定义规则";
        table_row(" $msg", $RCW, '', $RCW);
    }
    for my $ch (@chains) {
        next unless $ch->{name} =~ /^DOCKER/;
        mid_border($RCW, $RCW);
        table_row("$ch->{name} (policy $ch->{policy})", $RCW, '', $RCW);
        for my $rule (@{$ch->{rules}}) {
            my $target_color = $rule->{target} eq 'ACCEPT' ? "$rule->{target}" :
                               $rule->{target} eq 'DROP'   ? "$rule->{target}" : "$rule->{target}";
            my $desc = '';
            if ($rule->{dport}) {
                $desc = $rule->{prot} . " -> dpt:$rule->{dport}  (in:$rule->{in} out:$rule->{out})";
            } elsif ($rule->{ctstate}) {
                $desc = "state $rule->{ctstate}  (in:$rule->{in} out:$rule->{out})";
            } else {
                $desc = "in:$rule->{in} out:$rule->{out} $rule->{src}";
            }
            table_row(" $target_color", $RCW, " $desc", $RCW);
        }
    }
    bot_border($RCW, $RCW);
    print "\n";

    # ════════ 板块四：监听端口 vs 防火墙状态 ════════
    if (@port_report) {
        my $PW = int(($TW - 7) / 4);
        top_border($PW, $PW, $PW, $PW);
        table_row("端口",      $PW,
                  "监听地址",  $PW,
                  "进程",      $PW,
                  "防火墙状态",$PW);
        mid_border($PW, $PW, $PW, $PW);
        for my $p (sort { $a->{port} <=> $b->{port} } @port_report) {
            my $addr_color = $p->{addr} eq '0.0.0.0' ? "$p->{addr}" :
                             $p->{addr} eq '127.0.0.1' ? "$p->{addr}" : $p->{addr};
            table_row(" $p->{port}/tcp", $PW,
                      " $addr_color",   $PW,
                      " $p->{proc}",     $PW,
                      " $p->{status}",   $PW);
        }
        bot_border($PW, $PW, $PW, $PW);
        print "\n";
    }

    # ════════ 板块五：防火墙规则汇总 ════════
    {
        my $SW = int(($TW - 7) / 2);
        top_border($SW, $SW);
        table_row("规则汇总", $SW, "统计", $SW);
        mid_border($SW, $SW);
        my $total_rules = 0;
        my $accept_rules = 0;
        my $drop_rules = 0;
        for my $ch (@chains) {
            $total_rules += scalar @{$ch->{rules}};
            for my $r (@{$ch->{rules}}) {
                $accept_rules++ if $r->{target} eq 'ACCEPT' || $r->{target} eq 'RETURN';
                $drop_rules++   if $r->{target} eq 'DROP'   || $r->{target} eq 'REJECT';
            }
        }
        table_row(" iptables 链总数",   $SW, " " . scalar(@chains), $SW);
        table_row(" 规则总条数",         $SW, " $total_rules",       $SW);
        table_row("   放行 (ACCEPT)",    $SW, " $accept_rules",      $SW);
        table_row("   拒绝 (DROP)",      $SW, " $drop_rules",        $SW);
        table_row(" 监听端口数",         $SW, " " . scalar(@port_report), $SW);
        table_row(" 防火墙类型",         $SW, " $sec{fw_name}",     $SW);
        my $total_allow = grep { $_->{rule} eq 'ACCEPT' || $_->{rule} eq $default_action } @port_report;
        my $total_deny  = grep { $_->{rule} eq 'DROP' } @port_report;
        table_row(" 端口开放数",         $SW, " $total_allow",       $SW);
        table_row(" 端口拒绝数",         $SW, " $total_deny",        $SW);
        bot_border($SW, $SW);
        print "\n";
    }

    # 导出
    if ($EXPORT && !$EXPORTING) {
        $EXPORTING = 1;
        `mkdir -p "$EXPORT"`;
        my $fname = "$EXPORT/security-" . strftime('%Y%m%d-%H%M%S', localtime) . ".txt";
        my $plain = `perl $0 2>/dev/null | perl -pe 's/\\e\\[[0-9;]*m//g'`;
        if ($plain) {
            open my $efh, '>:utf8', $fname or warn "导出失败: $!";
            if ($efh) { print $efh $plain; close $efh }
        }
        $EXPORTING = 0;
        print "  已导出: $fname\n";
    }
}

# ──────────────────── 入口 ────────────────────
while (@ARGV) {
    my $a = shift @ARGV;
    if ($a eq '-f')       { $FOLLOW = 1 }
    if ($a eq '-export')  { $EXPORT = shift @ARGV || '.' }
}
if ($FOLLOW) {
    while (1) { run(); print " 每5秒刷新 | Ctrl+C 退出\n"; sleep 5 }
} else { run() }
exit 0;
