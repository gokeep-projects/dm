#!/usr/bin/perl
use strict;
use warnings;
use POSIX qw(strftime);
use File::Basename;

# ═══════════════════════════════════════════════════════════════════════
# 日志异常聚合分析 v3.0
# 优先级：Java → 中间件 → 系统
# 仅显示 ERROR/WARN 级别，堆栈仅保留关键行
# 末尾输出进程维度统计表
# ═══════════════════════════════════════════════════════════════════════

my ($R, $B, $D, $CY, $G, $Y, $RD, $BL) = (
    "\e[0m", "\e[1m", "\e[2m", "\e[36m",
    "\e[32m", "\e[33m", "\e[31m", "\e[34m"
);

# ─── 表格 ───
sub _dw {
    my ($s) = @_;
    $s =~ s/\e\[[0-9;]*m//g;
    my $len = length($s);
    my $w = 0;
    for my $i (0 .. $len - 1) {
        my $b = ord(substr($s, $i, 1));
        $b < 0x80 ? $w++ : ($b >= 0xC0 ? $w += 2 : ());
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
    my $s = "  ┌"; my $i = 0;
    for my $w (@_) { $s .= "─" x $w; $s .= ($i++ < $#_) ? "┬" : "┐"; }
    print "$s\n";
}
sub mid_border {
    my $s = "  ├"; my $i = 0;
    for my $w (@_) { $s .= "─" x $w; $s .= ($i++ < $#_) ? "┼" : "┤"; }
    print "$s\n";
}
sub bot_border {
    my $s = "  └"; my $i = 0;
    for my $w (@_) { $s .= "─" x $w; $s .= ($i++ < $#_) ? "┴" : "┘"; }
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
    my $pad = $w - 2 - $tdw; $pad = 0 if $pad < 0;
    my $l = int($pad / 2); my $r = $pad - $l;
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

# ─── 工具 ───
sub _cmd {
    my ($cmd) = @_;
    my $out = `$cmd 2>/dev/null`; chomp $out; return $out;
}

# ─── 日志扫描 ───
sub _find_files {
    my $glob = shift;
    my @files;
    for my $f (glob($glob)) { next unless -r $f && -f $f; push @files, $f }
    return @files;
}
sub _journalctl {
    my ($unit, $since) = @_;
    $since ||= '24 hours ago';
    my $out = _cmd("journalctl -u $unit --since \"$since\" --no-pager -n 2000 2>/dev/null");
    return split("\n", $out);
}

sub _parse_level {
    my ($line) = @_;
    return 'ERROR' if $line =~ /\b(ERROR|FATAL|SEVERE|CRITICAL|ALERT|CRIT|PANIC)\b/i;
    return 'WARN'  if $line =~ /\b(WARN|WARNING)\b/i;
    return 'ERROR' if $line =~ /\b(Exception|Error|Throwable|Failed|Failure)\b/;
    return 'WARN'  if $line =~ /\b(warn|warning|timeout|refused|rejected|denied|unreachable)\b/i;
    return '';
}

sub _parse_process {
    my ($line, $source) = @_;
    # Caddy JSON logs: "logger":"http.log.error"
    if ($line =~ /"logger":"([^"]+)"/) {
        my $l = $1;
        return 'caddy' if $l =~ /^http/ || $l =~ /^tls/;
        return $l;
    }
    # syslog: "hostname process[pid]:" or "hostname process:"
    if ($line =~ /^\S+\s+\S+\s+\d+\s+\d+:\d+:\d+\s+\S+\s+(\S+?)(?:\[\d+\])?:\s+/) {
        my $proc = $1;
        $proc =~ s/[^a-zA-Z0-9_\.\-\+]//g;
        # 过滤掉非进程名
        return '' if $proc =~ /^(message\s+repeated|error|warn|info)$/i;
        return $proc if $proc;
    }
    # journalctl
    if ($line =~ /\s+(\S+?)(?:\[\d+\])?:\s*(error|warning|fail)/i) {
        my $proc = $1;
        $proc =~ s/[^a-zA-Z0-9_\.\-\+]//g;
        return '' if $proc =~ /^(error|warn|info)$/i;
        return $proc if $proc;
    }
    return '';
}

sub _resolve_service_path {
    my ($process) = @_;
    # Try systemd service
    my $svc_path = _cmd("systemctl show -p FragmentPath $process.service 2>/dev/null | cut -d= -f2");
    return $svc_path if $svc_path;
    # Try .mount unit
    $svc_path = _cmd("systemctl show -p FragmentPath $process 2>/dev/null | cut -d= -f2");
    return $svc_path if $svc_path;
    # Try which
    my $bin = _cmd("which $process 2>/dev/null");
    return $bin if $bin;
    # Try common paths
    for my $d ("/usr/sbin/$process", "/usr/bin/$process", "/usr/local/bin/$process",
               "/sbin/$process", "/bin/$process", "/opt/$process/$process") {
        return $d if -x $d;
    }
    return '';
}

sub _resolve_service_name {
    my ($process) = @_;
    my $svc = _cmd("systemctl show -p Id $process.service 2>/dev/null | cut -d= -f2");
    return $svc if $svc;
    # Check if a service with this name exists
    my $found = _cmd("systemctl list-units --type=service --all 2>/dev/null | grep -i \"$process\\.service\" | awk '{print \$1}' | head -1");
    return $found if $found;
    return "$process.service";
}

sub _is_stack_line {
    my ($line) = @_;
    return $line =~ /^\s+at\s/ || $line =~ /^\s+\.\.\.\s+\d+\s+more/ ||
           $line =~ /^\s+Caused by:/ || $line =~ /^\s+Suppressed:/;
}

sub _collapse_java_stack {
    my ($lines_ref) = @_;
    my @filtered;
    my $in_stack = 0;
    for my $item (@$lines_ref) {
        my $text = $item->{text};
        if (_is_stack_line($text)) {
            unless ($in_stack) {
                $in_stack = 1;
                if ($text =~ /at\s+(\S+)\((\S+)\)/) {
                    push @filtered, { level => $item->{level}, text => "  ╰─ $1($2)", line_num => $item->{line_num} };
                } elsif ($text =~ /Caused by:\s+(\S+)/) {
                    push @filtered, { level => 'ERROR', text => "  ╰─ Caused by: $1", line_num => $item->{line_num} };
                }
            }
            next;
        }
        $in_stack = 0;
        push @filtered, $item;
    }
    return @filtered;
}

# ─── 路径扫描 ───
sub scan_system_logs {
    my @results;
    my @paths = (
        '/var/log/syslog',        '/var/log/syslog.1',
        '/var/log/messages',      '/var/log/messages.1',
        '/var/log/kern.log',      '/var/log/kern.log.1',
        '/var/log/auth.log',      '/var/log/auth.log.1',
        '/var/log/dmesg',
        '/var/log/bootstrap.log',
        '/var/log/daemon.log',    '/var/log/daemon.log.1',
        '/var/log/debug',         '/var/log/debug.1',
    );
    for my $p (@paths) { next unless -r $p; push @results, $p }
    return @results;
}
sub scan_middleware_logs {
    my @results;
    push @results, _find_files('/var/log/caddy/*.log');
    push @results, _find_files('/var/log/nginx/*.log');
    push @results, _find_files('/var/log/mysql/*.log');
    push @results, _find_files('/var/log/mysql/*.err');
    push @results, _find_files('/var/log/mariadb/*.log');
    push @results, _find_files('/var/log/postgresql/*.log');
    push @results, _find_files('/var/log/redis/*.log');
    push @results, _find_files('/var/log/elasticsearch/*.log');
    push @results, _find_files('/var/log/logstash/*.log');
    push @results, _find_files('/var/log/kibana/*.log');
    push @results, _find_files('/var/log/kafka/*.log');
    push @results, _find_files('/opt/kafka/logs/*.log');
    push @results, _find_files('/var/log/zookeeper/*.log');
    push @results, _find_files('/var/log/jenkins/*.log');
    push @results, _find_files('/var/log/mongodb/*.log');
    push @results, _find_files('/var/log/rabbitmq/*.log');
    push @results, _find_files('/var/log/docker/*.log');
    return @results;
}
sub scan_java_logs {
    my @results;
    for my $dir (glob('/var/log/tomcat*'), glob('/var/log/jetty*'), glob('/var/log/spring*'),
                 glob('/var/log/java*'), glob('/var/log/app*'), glob('/opt/*/logs'),
                 glob('/usr/local/*/logs'), glob('/data/*/logs'), glob('/app/*/logs')) {
        next unless -d $dir;
        for my $f (glob("$dir/*.log"), glob("$dir/*.out"), glob("$dir/*.txt")) {
            next unless -r $f && -f $f; push @results, $f;
        }
    }
    for my $f (glob('/var/log/gc*.log'), glob('/opt/*/gc*.log'), glob('/*/gc.log')) {
        next unless -r $f && -f $f; push @results, $f;
    }
    for my $f (glob('/var/log/tomcat*/catalina.out'), glob('/opt/tomcat*/logs/catalina.out')) {
        next unless -r $f && -f $f; push @results, $f;
    }
    return @results;
}

sub categorize_path {
    my ($path) = @_;
    return 'Java' if $path =~ /tomcat|jetty|spring|java|gc\.log|catalina|app\.log/i;
    return 'Elasticsearch' if $path =~ /elasticsearch/;
    return 'Caddy' if $path =~ /caddy/i;
    return 'Nginx' if $path =~ /nginx/i;
    return 'MySQL' if $path =~ /mysql|mariadb/i;
    return 'PostgreSQL' if $path =~ /postgres/i;
    return 'Redis' if $path =~ /redis/i;
    return 'Kafka' if $path =~ /kafka/i;
    return 'Zookeeper' if $path =~ /zookeeper/i;
    return 'Docker' if $path =~ /docker/i;
    return 'Jenkins' if $path =~ /jenkins/i;
    return 'MongoDB' if $path =~ /mongo/i;
    return 'RabbitMQ' if $path =~ /rabbit/i;
    return 'Logstash' if $path =~ /logstash/i;
    return 'Kibana' if $path =~ /kibana/i;
    return '系统';
}

sub analyze_file {
    my ($path, $max_lines) = @_;
    $max_lines ||= 5000;
    return undef unless -r $path;
    open my $fh, '<:raw', $path or return undef;
    my @lines;
    while (<$fh>) { chomp; push @lines, $_; shift @lines if @lines > $max_lines }
    close $fh;
    return \@lines;
}

# ─── 处理单个日志文件 ───
# 返回: { path, label, total, errors, warns, items, category, process_stats }
sub process_log_file {
    my ($path, $label) = @_;
    $label ||= $path;
    my $lines = analyze_file($path, 5000);
    return undef unless $lines && @$lines;

    my $total   = @$lines;
    my @matched;
    my $errors  = 0;
    my $warns   = 0;
    my %proc_count;    # process_name => error_count

    for my $i (0 .. $#$lines) {
        my $line = $lines->[$i];
        my $level = _parse_level($line);
        next unless $level eq 'ERROR' || $level eq 'WARN';
        $errors++ if $level eq 'ERROR';
        $warns++   if $level eq 'WARN';
        push @matched, { level => $level, text => $line, line_num => $i + 1 };

        # 提取进程名并计数
        my $proc = _parse_process($line, $path);
        if ($proc) {
            $proc_count{$proc}++;
        }
    }

    @matched = _collapse_java_stack(\@matched);
    my $show = $#matched > 99 ? 100 : $#matched + 1;
    splice @matched, 0, $#matched - $show + 1 if $#matched >= $show;

    return {
        path          => $path,
        label         => $label,
        total         => $total,
        errors        => $errors,
        warns         => $warns,
        items         => \@matched,
        proc_count    => \%proc_count,
    };
}

# ─── 主流程 ───
sub run {
    printf "\e[H\e[2J";
    my $W  = get_cols();
    my $CW = int(($W - 7) / 2);
    my $TW = 7 + 2 * $CW;
    my $time = strftime('%Y-%m-%d %H:%M:%S', localtime);
    header("日志异常聚合分析  $time", $TW);

    # ── 1. 扫描 ──
    my @java_paths      = scan_java_logs();
    my @middleware_paths = scan_middleware_logs();
    my @system_paths    = scan_system_logs();

    # ── 2. 处理 ──
    my @all_results;
    my $seen = {};
    # Java
    for my $p (@java_paths) {
        next if $seen->{$p}++;
        my $r = process_log_file($p);
        next unless $r && $r->{errors} + $r->{warns} > 0;
        $r->{category} = 'Java'; push @all_results, $r;
    }
    # 中间件
    for my $p (@middleware_paths) {
        next if $seen->{$p}++;
        my $r = process_log_file($p);
        next unless $r && $r->{errors} + $r->{warns} > 0;
        $r->{category} = categorize_path($p); push @all_results, $r;
    }
    # 系统
    for my $p (@system_paths) {
        next if $seen->{$p}++;
        my $r = process_log_file($p);
        next unless $r && $r->{errors} + $r->{warns} > 0;
        $r->{category} = '系统'; push @all_results, $r;
    }

    # ── 3. 概览（3列严格对齐）──
    {
        my $cw_total = $TW - 7;
        my @cws;
        $cws[0] = int($cw_total * 0.25);
        $cws[1] = int($cw_total * 0.25);
        $cws[2] = $cw_total - $cws[0] - $cws[1];

        top_border(@cws);
        table_row("分类", $cws[0], "文件", $cws[1], "错误/警告", $cws[2]);
        mid_border(@cws);
        my %cat_stats;
        for my $r (@all_results) {
            $cat_stats{$r->{category}}{total}  += $r->{total};
            $cat_stats{$r->{category}}{errors} += $r->{errors};
            $cat_stats{$r->{category}}{warns}  += $r->{warns};
            $cat_stats{$r->{category}}{files}++;
        }
        for my $cat (sort { $a cmp $b } keys %cat_stats) {
            my $s = $cat_stats{$cat};
            my $cat_color = $cat eq 'Java' ? "${RD}$cat${R}" :
                            $cat eq '系统' ? "${D}$cat${R}" : "${CY}$cat${R}";
            my $err_color = $s->{errors} > 0 ? "${RD}$s->{errors}${R}" : $s->{errors};
            my $warn_color = $s->{warns} > 0 ? "${Y}$s->{warns}${R}" : $s->{warns};
            table_row(" $cat_color", $cws[0], " $s->{files} 个文件", $cws[1],
                      " ${err_color}错误 / ${warn_color}警告", $cws[2]);
        }
        if (!%cat_stats) { table_row(" ${G}未发现异常日志${R}", $cws[0], '', $cws[1], '', $cws[2]) }
        bot_border(@cws); print "\n";
    }

    # ── 5. 进程维度统计表（3列可换行，严格对齐） ──
    {
        my %global_proc;
        for my $r (@all_results) {
            while (my ($proc, $cnt) = each %{$r->{proc_count}}) {
                $global_proc{$proc}{errors} += $cnt;
                push @{$global_proc{$proc}{sources}}, $r->{path}
                    unless grep { $_ eq $r->{path} } @{$global_proc{$proc}{sources}};
            }
        }

        # 采样：从每个进程的日志文件中提取消息概要
        for my $r (@all_results) {
            next unless $r->{items} && @{$r->{items}};
            for my $item (@{$r->{items}}) {
                my $proc = '';
                if ($item->{text} =~ /\S+\s+(\S+?)(?:\[\d+\])?:\s+/) {
                    $proc = $1; $proc =~ s/[^a-zA-Z0-9_\.\-\+]//g;
                } elsif ($item->{text} =~ /"logger":"([^"]+)"/) {
                    $proc = $1; $proc = 'caddy' if $proc =~ /^http\./ || $proc =~ /^tls/;
                }
                next unless $proc && $global_proc{$proc};
                next if ($global_proc{$proc}{samples} || 0) >= 2;
                my $t = $item->{text};
                # Caddy JSON → 提取关键字段
                if ($t =~ /"msg":"([^"]+)"/)         { $t = $1 }
                elsif ($t =~ /"message":"([^"]+)"/)  { $t = $1 }
                $t =~ s/^\d{4}[-\/]\d{2}[-\/]\d{2}[T ]\d{2}:\d{2}:\d{2}[.,]\d{3,6}Z?\s*//;
                $t =~ s/^\w{3}\s+\w{3}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}\s+\S+\s+\S+\[\d+\]:\s*//;
                $t =~ s/^"//; $t =~ s/"$//;
                next if $t =~ /^\s*$/;
                push @{$global_proc{$proc}{summary}}, $t;
                $global_proc{$proc}{samples}++;
            }
        }

        if (%global_proc) {
            my @sorted = sort { $global_proc{$b}{errors} <=> $global_proc{$a}{errors} } keys %global_proc;

            # 列宽分配：总内容宽度 = TW - 7
            my $cw_total = $TW - 7;
            # 进程列 22% / 日志路径列 30% / 概要列 48%
            my @cws;
            $cws[0] = int($cw_total * 0.22);
            $cws[1] = int($cw_total * 0.30);
            $cws[2] = $cw_total - $cws[0] - $cws[1];

            # _wrap: 按显示宽度换行，返回行列表
            sub _wrap {
                my ($text, $maxw) = @_;
                return ('') unless defined $text && $text ne '';
                my @lines;
                local $_ = $text;
                while (_dw($_) > $maxw) {
                    my $cw = 0; my $pos = 0; my $blen = length($_);
                    while ($pos < $blen && $cw < $maxw) {
                        my $b = ord(substr($_, $pos, 1));
                        if ($b < 0x80)       { $cw++; $pos++ }
                        elsif ($b >= 0xF0)   { $cw += 2; $pos += 4 }
                        elsif ($b >= 0xE0)   { $cw += 2; $pos += 3 }
                        elsif ($b >= 0xC0)   { $cw += 2; $pos += 2 }
                        else                 { $pos++ }
                    }
                    my $cut = rindex(substr($_, 0, $pos), ' ');
                    $cut = $pos if $cut < 0;
                    push @lines, substr($_, 0, $cut);
                    $_ = substr($_, $cut + 1);
                    s/^\s+//;
                }
                push @lines, $_ if $_ ne '';
                return @lines;
            }

            top_border(@cws);
            table_row("${B}进程/服务${R}", $cws[0], "${B}日志路径${R}", $cws[1], "${B}概要${R}", $cws[2]);
            mid_border(@cws);

            my $pcount = 0;
            for my $proc (@sorted) {
                last if $pcount >= 12;
                $pcount++;
                my $s   = $global_proc{$proc};
                my $svc = _resolve_service_name($proc);
                $svc =~ s/\.service$//;

                # 进程/服务列
                my @proc_cell = _wrap($proc, $cws[0] - 2);
                if ($svc && $svc ne $proc) {
                    push @proc_cell, "($svc)";
                }

                # 日志路径列
                my @path_cell;
                for my $sp (@{$s->{sources}}) {
                    push @path_cell, _wrap($sp, $cws[1] - 2);
                }

                # 概要列——完整显示消息，不截断
                my @summary_cell;
                my $ec = $s->{errors};
                my $ec_color = $ec > 100 ? "${RD}${ec}${R}" : $ec > 10 ? "${Y}${ec}${R}" : $ec;
                push @summary_cell, "${ec_color} 次异常";
                if ($s->{summary} && @{$s->{summary}}) {
                    for my $sm (@{$s->{summary}}) {
                        my @wrapped = _wrap("  $sm", $cws[2] - 2);
                        push @summary_cell, @wrapped;
                        last if @summary_cell >= 6;
                    }
                }

                # 统一行数
                my $max_rows = 0;
                $max_rows = @proc_cell    if @proc_cell > $max_rows;
                $max_rows = @path_cell    if @path_cell > $max_rows;
                $max_rows = @summary_cell if @summary_cell > $max_rows;

                for my $ri (0 .. $max_rows - 1) {
                    my $pc = $proc_cell[$ri]    || '';
                    my $pp = $path_cell[$ri]    || '';
                    my $ps = $summary_cell[$ri] || '';
                    table_row(" $pc", $cws[0], " $pp", $cws[1], " $ps", $cws[2]);
                }

                mid_border(@cws);
            }

            if (@sorted > 12) {
                my $rest = @sorted - 12;
                table_row(" ${D}... 还有 $rest 个进程${R}", $cws[0], '', $cws[1], '', $cws[2]);
                bot_border(@cws);
            } else {
                print "\e[1A\e[K";
                bot_border(@cws);
            }
            print "\n";
        }
    }
}

# ─── 入口 ───
run();
exit 0;
