use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::checks::{CheckResult, CheckStatus, Item, Section};
use crate::dashboard::SystemInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyFinding {
    pub rule_id: String,
    pub level: String,
    pub category: String,
    pub title: String,
    pub target: String,
    pub summary: String,
    pub evidence: Vec<String>,
    pub suggestion: String,
    pub commands: Vec<String>,
}

impl AnomalyFinding {
    fn new(
        rule_id: impl Into<String>,
        level: impl Into<String>,
        category: impl Into<String>,
        title: impl Into<String>,
        target: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            level: level.into(),
            category: category.into(),
            title: title.into(),
            target: target.into(),
            summary: summary.into(),
            evidence: Vec::new(),
            suggestion: String::new(),
            commands: Vec::new(),
        }
    }

    fn evidence(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.evidence = values.into_iter().collect();
        self
    }

    fn suggestion(mut self, value: impl Into<String>) -> Self {
        self.suggestion = value.into();
        self
    }

    fn commands(mut self, values: impl IntoIterator<Item = String>) -> Self {
        self.commands = values.into_iter().collect();
        self
    }
}

pub fn finding_item(finding: &AnomalyFinding) -> Item {
    Item::Finding {
        rule_id: finding.rule_id.clone(),
        level: finding.level.clone(),
        category: finding.category.clone(),
        title: finding.title.clone(),
        target: finding.target.clone(),
        summary: finding.summary.clone(),
        evidence: finding.evidence.clone(),
        suggestion: finding.suggestion.clone(),
        commands: finding.commands.clone(),
    }
}

pub fn findings_section(findings: &[AnomalyFinding]) -> Option<Section> {
    if findings.is_empty() {
        return None;
    }
    Some(Section {
        title: "异常明细".to_string(),
        icon: Some("ALERT".to_string()),
        description: Some(format!(
            "共 {} 条异常/警告，全部由规则引擎生成，可逐条定位和处理",
            findings.len()
        )),
        items: findings.iter().map(finding_item).collect(),
    })
}

pub fn rule_catalog() -> Vec<serde_json::Value> {
    const RULES: &[(
        &str,
        &str,
        &str,
        &str,
        &str,
        &str,
        &str,
        &str,
        &[&str],
        &[&str],
    )] = &[
        (
            "resource.cpu.critical",
            "资源水位",
            "error",
            "CPU 使用率严重过高",
            "system.cpu",
            "CPU >= 90%",
            "90",
            "系统 CPU 长时间处于高风险区间，可能导致业务响应抖动或超时。",
            &["system_info.cpu_usage", "top_processes.cpu_usage"],
            &[
                "top -o %CPU",
                "ps -eo pid,ppid,comm,%cpu,%mem --sort=-%cpu | head -20",
            ],
        ),
        (
            "resource.cpu.warning",
            "资源水位",
            "warn",
            "CPU 使用率偏高",
            "system.cpu",
            "CPU >= 80%",
            "80",
            "CPU 使用率偏高，需要结合负载趋势和进程分布判断是否持续异常。",
            &["system_info.cpu_usage"],
            &["vmstat 1 10", "pidstat 1 5"],
        ),
        (
            "resource.memory.critical",
            "资源水位",
            "error",
            "内存使用率严重过高",
            "system.memory",
            "内存 >= 90%",
            "90",
            "物理内存进入危险区间，可能触发 OOM、频繁回收或业务进程退出。",
            &[
                "system_info.memory_usage",
                "swap_usage",
                "top_processes.memory_bytes",
            ],
            &[
                "free -h",
                "ps -eo pid,comm,rss,%mem --sort=-rss | head -20",
                "journalctl -k -p warning --since '2 hours ago' | grep -i oom",
            ],
        ),
        (
            "resource.memory.warning",
            "资源水位",
            "warn",
            "内存使用率偏高",
            "system.memory",
            "内存 >= 80%",
            "80",
            "内存占用偏高，需要区分缓存增长、业务峰值和泄漏趋势。",
            &["system_info.memory_usage"],
            &["free -h", "ps -eo pid,comm,rss,%mem --sort=-rss | head -20"],
        ),
        (
            "resource.swap.warning",
            "资源水位",
            "warn",
            "Swap 使用偏高",
            "system.swap",
            "Swap 使用率 >= 30%",
            "30",
            "Swap 持续使用会显著影响响应，通常说明内存压力或进程异常增长。",
            &["system_info.swap_usage"],
            &["free -h", "vmstat 1 10"],
        ),
        (
            "resource.disk.critical",
            "资源水位",
            "error",
            "磁盘使用率严重过高",
            "system.disk",
            "磁盘 >= 90%",
            "90",
            "整体磁盘空间不足会导致写入失败、日志丢失和服务异常。",
            &["system_info.disk_usage", "disk_total", "disk_used"],
            &["df -h", "du -xh /var/log 2>/dev/null | sort -h | tail -20"],
        ),
        (
            "resource.disk.warning",
            "资源水位",
            "warn",
            "磁盘使用率偏高",
            "system.disk",
            "磁盘 >= 80%",
            "80",
            "磁盘空间接近高水位，应提前定位增长目录和清理策略。",
            &["system_info.disk_usage"],
            &["df -h", "du -xh / 2>/dev/null | sort -h | tail -20"],
        ),
        (
            "resource.load.critical",
            "资源水位",
            "error",
            "系统负载严重过高",
            "system.loadavg",
            "1 分钟负载 / CPU 核心数 >= 2",
            "2",
            "负载显著超过 CPU 核心数，需要区分 CPU 饱和、IO 等待和不可中断进程。",
            &["load_avg.one", "cpu_count"],
            &[
                "uptime",
                "vmstat 1 10",
                "ps -eo stat,pid,comm,%cpu,%mem --sort=-%cpu | head -20",
            ],
        ),
        (
            "resource.load.warning",
            "资源水位",
            "warn",
            "系统负载偏高",
            "system.loadavg",
            "1 分钟负载 / CPU 核心数 >= 1",
            "1",
            "短期负载超过核心数，需要观察 5/15 分钟趋势判断持续性。",
            &["load_avg.one", "load_avg.five", "load_avg.fifteen"],
            &["uptime", "vmstat 1 5"],
        ),
        (
            "resource.mount.critical",
            "磁盘分区",
            "error",
            "分区空间严重不足",
            "mount",
            "分区使用率 >= 90%",
            "90",
            "单个关键挂载点空间不足会直接影响对应数据目录、日志目录或备份目录。",
            &["disk.mount_point", "disk.usage", "disk.fs_type"],
            &[
                "df -h <mount>",
                "du -xh <mount> 2>/dev/null | sort -h | tail -20",
            ],
        ),
        (
            "resource.mount.warning",
            "磁盘分区",
            "warn",
            "分区空间偏高",
            "mount",
            "分区使用率 >= 80%",
            "80",
            "单个挂载点达到预警水位，需要检查增长来源并提前扩容或清理。",
            &["disk.mount_point", "disk.usage"],
            &["df -h <mount>"],
        ),
        (
            "resource.inode.warning",
            "磁盘分区",
            "warn",
            "inode 使用率偏高",
            "mount.inode",
            "inode 使用率 >= 80%",
            "80",
            "大量小文件可能耗尽 inode，即使磁盘容量未满也会造成写入失败。",
            &["df -i", "mount_point"],
            &[
                "df -i",
                "find <mount> -xdev -type f | cut -d/ -f2 | sort | uniq -c | sort -n | tail",
            ],
        ),
        (
            "process.cpu.warning",
            "进程异常",
            "warn",
            "进程 CPU 占用过高",
            "process",
            "进程 CPU >= 85%",
            "85",
            "单进程 CPU 异常通常来自热点请求、死循环、GC 抖动或批处理任务。",
            &["top_processes.cpu_usage", "process.pid"],
            &[
                "ps -p <pid> -o pid,ppid,comm,%cpu,%mem,etime,args",
                "top -Hp <pid>",
            ],
        ),
        (
            "process.memory.warning",
            "进程异常",
            "warn",
            "进程内存占用过高",
            "process",
            "单进程 RSS 占系统内存 >= 25%",
            "25",
            "单进程内存过高需要确认是否为缓存、堆增长、连接堆积或泄漏。",
            &["top_processes.memory_bytes", "memory_total"],
            &[
                "ps -p <pid> -o pid,ppid,comm,rss,%mem,etime,args",
                "pmap -x <pid> | tail -20",
            ],
        ),
        (
            "process.zombie.warning",
            "进程异常",
            "warn",
            "僵尸进程数量异常",
            "process.stat",
            "Z 状态进程数量 >= 1",
            "1",
            "僵尸进程说明父进程未正确回收子进程，长期累积会影响进程管理。",
            &["ps stat=Z"],
            &["ps -eo stat,pid,ppid,comm,args | awk '$1 ~ /Z/'"],
        ),
        (
            "process.fd.warning",
            "进程异常",
            "warn",
            "进程文件句柄接近上限",
            "process.fd",
            "打开 FD / limit >= 80%",
            "80",
            "文件句柄耗尽会导致网络连接、日志写入或文件打开失败。",
            &["/proc/<pid>/fd", "limits"],
            &["ls /proc/<pid>/fd | wc -l", "cat /proc/<pid>/limits"],
        ),
        (
            "service.failed.error",
            "服务管理",
            "error",
            "systemd 服务失败",
            "systemd.service",
            "服务处于 failed 状态",
            "",
            "核心服务进入 failed 状态，需要读取 unit 状态、最近日志和依赖关系后再处理。",
            &["systemctl list-units --state=failed"],
            &[
                "systemctl status <service> --no-pager",
                "journalctl -u <service> -n 120 --no-pager",
            ],
        ),
        (
            "service.restart.warning",
            "服务管理",
            "warn",
            "服务近期频繁重启",
            "systemd.service",
            "短时间重启次数 >= 3",
            "3",
            "频繁重启通常代表配置错误、依赖不可用、端口冲突或业务启动异常。",
            &["systemctl show NRestarts", "journalctl"],
            &[
                "systemctl show <service> -p NRestarts",
                "journalctl -u <service> --since '2 hours ago'",
            ],
        ),
        (
            "service.port.error",
            "服务管理",
            "error",
            "服务监听端口缺失",
            "service.port",
            "期望监听端口未出现",
            "",
            "服务进程存在但端口未监听，常见于启动半成功、配置加载失败或端口被占用。",
            &["ss -ltnp", "service config"],
            &["ss -ltnp", "systemctl status <service> --no-pager"],
        ),
        (
            "service.permission.error",
            "服务管理",
            "error",
            "服务权限或用户异常",
            "service.user",
            "运行用户不存在或目录无权限",
            "",
            "权限异常会导致服务无法读取配置、写日志或访问数据目录。",
            &["systemd user", "path permissions"],
            &["id <user>", "namei -l <path>"],
        ),
        (
            "log.error.burst",
            "日志异常",
            "warn",
            "日志异常关键字突增",
            "log",
            "最近日志 error/failed/panic/oom >= 5",
            "5",
            "日志异常突增需要结合故障时间点查看上下文，避免只看最后一行。",
            &["tail -n 100", "keyword count"],
            &[
                "tail -n 200 <log>",
                "grep -Ei 'error|critical|panic|failed|oom' <log> | tail -50",
            ],
        ),
        (
            "log.oom.error",
            "日志异常",
            "error",
            "系统出现 OOM 记录",
            "kernel.log",
            "kernel/journal 出现 OOM kill",
            "",
            "OOM 表示内存已经影响进程存活，需要定位被杀进程和内存增长来源。",
            &["journalctl -k", "dmesg"],
            &[
                "journalctl -k --since '2 hours ago' | grep -i oom",
                "dmesg -T | grep -i oom | tail -20",
            ],
        ),
        (
            "log.auth.warning",
            "日志异常",
            "warn",
            "认证失败异常增多",
            "auth.log",
            "认证失败/拒绝次数 >= 10",
            "10",
            "认证失败异常可能来自密码错误、暴力尝试、权限配置或自动化任务异常。",
            &["/var/log/auth.log", "journal sshd"],
            &[
                "grep -Ei 'failed|denied|invalid' /var/log/auth.log | tail -50",
                "journalctl -u sshd --since '2 hours ago'",
            ],
        ),
        (
            "log.permission.warning",
            "日志异常",
            "warn",
            "权限拒绝异常",
            "log",
            "日志出现 permission denied/access denied",
            "",
            "权限拒绝需要定位访问主体、目标路径和最近权限变更。",
            &["application logs", "journalctl"],
            &["grep -R \"permission denied\\|access denied\" /var/log 2>/dev/null | tail -50"],
        ),
        (
            "database.mysql.down.error",
            "数据库",
            "error",
            "MySQL 服务不可用",
            "mysql",
            "连接失败或服务未运行",
            "",
            "MySQL 不可用会直接影响依赖业务，需要先区分进程、端口、认证和存储问题。",
            &["mysqladmin ping", "systemctl mysql"],
            &[
                "mysqladmin ping",
                "systemctl status mysql mysqld --no-pager",
            ],
        ),
        (
            "database.mysql.replication.error",
            "数据库",
            "error",
            "MySQL 主从复制异常",
            "mysql.replication",
            "Slave_IO_Running/Slave_SQL_Running != Yes",
            "",
            "复制异常会导致数据延迟或不一致，需要定位 IO、SQL 线程错误。",
            &["SHOW SLAVE STATUS", "SHOW REPLICA STATUS"],
            &[
                "mysql -e 'SHOW SLAVE STATUS\\G'",
                "mysql -e 'SHOW REPLICA STATUS\\G'",
            ],
        ),
        (
            "database.mysql.slow.warning",
            "数据库",
            "warn",
            "MySQL 慢查询异常",
            "mysql.slowlog",
            "慢查询数量或耗时异常",
            "",
            "慢查询可能造成连接堆积和接口超时，需要结合索引和执行计划处理。",
            &["slow_query_log", "performance_schema"],
            &[
                "mysql -e \"SHOW GLOBAL STATUS LIKE 'Slow_queries'\"",
                "mysql -e 'SHOW PROCESSLIST'",
            ],
        ),
        (
            "database.redis.down.error",
            "数据库",
            "error",
            "Redis 服务不可用",
            "redis",
            "PING 失败或端口未监听",
            "",
            "Redis 不可用会影响缓存、队列或会话，需要区分进程、端口、密码和持久化错误。",
            &["redis-cli PING", "ss -ltnp"],
            &[
                "redis-cli ping",
                "systemctl status redis redis-server --no-pager",
            ],
        ),
        (
            "database.redis.memory.warning",
            "数据库",
            "warn",
            "Redis 内存接近上限",
            "redis.memory",
            "used_memory / maxmemory >= 80%",
            "80",
            "Redis 内存接近上限可能触发淘汰、写入失败或延迟升高。",
            &["INFO memory", "maxmemory"],
            &["redis-cli info memory", "redis-cli config get maxmemory"],
        ),
        (
            "database.redis.aof.error",
            "数据库",
            "error",
            "Redis AOF 文件损坏",
            "redis.aof",
            "日志出现 Bad file format/AOF corrupted",
            "",
            "AOF 损坏会导致 Redis 无法启动或数据恢复失败，需要备份后执行修复。",
            &["redis log", "appendonly.aof"],
            &[
                "redis-check-aof --fix appendonly.aof",
                "cp appendonly.aof appendonly.aof.bak",
            ],
        ),
        (
            "middleware.elasticsearch.red.error",
            "中间件",
            "error",
            "Elasticsearch 集群 red",
            "elasticsearch.health",
            "cluster status == red",
            "",
            "ES red 表示存在未分配主分片，数据读写可能不完整。",
            &["_cluster/health", "_cat/shards"],
            &[
                "curl -s localhost:9200/_cluster/health?pretty",
                "curl -s localhost:9200/_cat/shards?v",
            ],
        ),
        (
            "middleware.elasticsearch.yellow.warning",
            "中间件",
            "warn",
            "Elasticsearch 集群 yellow",
            "elasticsearch.health",
            "cluster status == yellow",
            "",
            "ES yellow 表示副本分片未分配，容灾能力下降。",
            &["_cluster/health", "_cat/allocation"],
            &[
                "curl -s localhost:9200/_cluster/health?pretty",
                "curl -s localhost:9200/_cat/allocation?v",
            ],
        ),
        (
            "middleware.elasticsearch.disk.error",
            "中间件",
            "error",
            "ES 磁盘水位过高",
            "elasticsearch.disk",
            "节点磁盘达到 flood stage",
            "",
            "ES 磁盘水位过高会导致索引只读或分片迁移失败。",
            &["_cat/allocation", "cluster settings"],
            &[
                "curl -s localhost:9200/_cat/allocation?v",
                "curl -s localhost:9200/_cluster/settings?pretty",
            ],
        ),
        (
            "web.nginx.config.error",
            "Web网关",
            "error",
            "Nginx 配置检测失败",
            "nginx.config",
            "nginx -t 返回失败",
            "",
            "Nginx 配置错误会导致 reload 失败或网关不可用。",
            &["nginx -t", "nginx.conf"],
            &["nginx -t", "nginx -T | head -120"],
        ),
        (
            "web.nginx.upstream.error",
            "Web网关",
            "error",
            "Nginx upstream 异常",
            "nginx.upstream",
            "error.log 出现 upstream failed/refused/timed out",
            "",
            "反向代理后端异常会导致 502/504，需要定位 upstream 地址、端口和后端服务状态。",
            &["nginx error.log", "upstream config"],
            &[
                "tail -n 100 /var/log/nginx/error.log",
                "nginx -T | grep -n upstream -A20",
            ],
        ),
        (
            "web.nginx.security.warning",
            "Web网关",
            "warn",
            "Nginx 安全配置不足",
            "nginx.security",
            "缺少 server_tokens off / TLS 安全项",
            "",
            "网关安全配置不足会暴露版本信息或弱 TLS 配置。",
            &["nginx -T", "server_tokens", "ssl_protocols"],
            &["nginx -T | grep -Ei 'server_tokens|ssl_protocols|add_header'"],
        ),
        (
            "web.caddy.down.error",
            "Web网关",
            "error",
            "Caddy 服务不可用",
            "caddy",
            "caddy 进程缺失或服务失败",
            "",
            "Caddy 作为入口网关不可用会直接导致站点访问失败，需要优先确认 systemd、端口和配置。",
            &["systemctl caddy", "ss -ltnp", "Caddyfile"],
            &[
                "systemctl status caddy --no-pager",
                "journalctl -u caddy -n 160 --no-pager",
                "caddy validate --config /etc/caddy/Caddyfile",
            ],
        ),
        (
            "web.caddy.config.error",
            "Web网关",
            "error",
            "Caddy 配置检测失败",
            "caddy.config",
            "caddy validate 返回失败",
            "",
            "Caddyfile 配置错误会导致 reload 或启动失败，常见于反向代理地址、证书和语法错误。",
            &["caddy validate", "Caddyfile"],
            &[
                "caddy validate --config /etc/caddy/Caddyfile",
                "caddy fmt --diff /etc/caddy/Caddyfile",
            ],
        ),
        (
            "web.caddy.upstream.error",
            "Web网关",
            "error",
            "Caddy upstream 异常",
            "caddy.upstream",
            "Caddy 日志出现 reverse_proxy dial/refused/timeout",
            "",
            "Caddy 反向代理后端异常通常来自后端端口未监听、连接拒绝、DNS 或超时。",
            &["caddy log", "reverse_proxy"],
            &[
                "journalctl -u caddy --since '2 hours ago' --no-pager",
                "grep -RniE 'dial tcp|connection refused|timeout|502|503|504' /var/log/caddy 2>/dev/null | tail -80",
            ],
        ),
        (
            "web.http.5xx.error",
            "异常类型",
            "error",
            "HTTP 5xx 错误增多",
            "http.5xx",
            "访问日志中 5xx 数量异常",
            "",
            "HTTP 5xx 表明网关或后端服务已影响请求成功率，需要关联 upstream、应用日志和发布时间。",
            &["access.log", "status>=500"],
            &[
                "awk '$9 ~ /^5/ {print}' /var/log/nginx/access.log 2>/dev/null | tail -80",
                "grep -RniE ' 5[0-9][0-9] ' /var/log/caddy /var/log/nginx 2>/dev/null | tail -80",
            ],
        ),
        (
            "web.http.4xx.warning",
            "异常类型",
            "warn",
            "HTTP 4xx 异常增多",
            "http.4xx",
            "访问日志中 4xx 数量异常",
            "",
            "HTTP 4xx 异常可能来自认证失败、路径变化、爬虫扫描或网关规则误配置。",
            &["access.log", "status>=400"],
            &[
                "awk '$9 ~ /^4/ {print}' /var/log/nginx/access.log 2>/dev/null | tail -80",
                "grep -RniE ' 4[0-9][0-9] ' /var/log/caddy /var/log/nginx 2>/dev/null | tail -80",
            ],
        ),
        (
            "web.tls.cert.warning",
            "配置风险",
            "warn",
            "TLS 证书即将过期",
            "tls.certificate",
            "证书剩余有效期 <= 15 天",
            "15",
            "证书过期会导致 HTTPS 访问失败，需要提前确认自动续签和证书链。",
            &["openssl x509", "cert notAfter"],
            &[
                "openssl x509 -in <cert> -noout -dates",
                "systemctl status certbot.timer caddy --no-pager",
            ],
        ),
        (
            "network.connection.exhaustion.error",
            "网络链路",
            "error",
            "连接数或 TIME_WAIT 异常",
            "network.connections",
            "连接数/TIME_WAIT/SYN_RECV 超过阈值",
            "",
            "连接耗尽会造成新请求失败，常见于流量突增、连接泄漏、后端慢响应或攻击流量。",
            &["ss -s", "SYN_RECV", "TIME_WAIT"],
            &[
                "ss -s",
                "ss -ant state syn-recv | wc -l",
                "ss -ant state time-wait | wc -l",
            ],
        ),
        (
            "network.packet.loss.warning",
            "网络链路",
            "warn",
            "网络丢包或重传异常",
            "network.loss",
            "网卡统计出现丢包/错误/重传异常",
            "",
            "丢包和重传会导致请求延迟升高，需要确认网卡、交换机、链路质量和突发流量。",
            &["ip -s link", "netstat -s"],
            &["ip -s link", "netstat -s | grep -Ei 'retrans|drop|error'"],
        ),
        (
            "network.port.conflict.error",
            "网络链路",
            "error",
            "端口冲突",
            "network.port",
            "多个进程争用或期望端口被占用",
            "",
            "端口冲突会导致服务启动失败或监听错误进程，需要确认占用者和启动顺序。",
            &["ss -ltnp", "service config"],
            &["ss -ltnp | grep ':<port>'", "lsof -i :<port>"],
        ),
        (
            "kernel.io.error",
            "内核异常",
            "error",
            "内核 IO 错误",
            "kernel.io",
            "dmesg/journal 出现 I/O error、EXT4-fs error、XFS error",
            "",
            "内核 IO 错误可能意味着磁盘、文件系统或底层存储异常，需要优先保护数据。",
            &["dmesg", "journalctl -k", "filesystem error"],
            &[
                "dmesg -T | grep -Ei 'I/O error|EXT4-fs error|XFS.*error|blk_update_request' | tail -80",
                "journalctl -k --since '2 hours ago' --no-pager",
            ],
        ),
        (
            "kernel.panic.error",
            "内核异常",
            "error",
            "内核严重异常",
            "kernel.panic",
            "日志出现 panic、BUG、soft lockup、hung task",
            "",
            "内核级异常可能导致系统卡顿或重启，需要保留日志并确认驱动、内核和硬件状态。",
            &["dmesg", "journalctl -k"],
            &[
                "dmesg -T | grep -Ei 'panic|BUG:|soft lockup|hung task' | tail -80",
                "journalctl -k -p warning --since '6 hours ago' --no-pager",
            ],
        ),
        (
            "runtime.container.restart.warning",
            "容器运行时",
            "warn",
            "容器频繁重启",
            "container.restart",
            "容器 RestartCount 短时间升高",
            "",
            "容器频繁重启通常来自探针失败、配置错误、依赖不可用或 OOM。",
            &["docker ps", "kubernetes restartCount"],
            &[
                "docker ps --format 'table {{.Names}}\t{{.Status}}'",
                "kubectl get pods -A --sort-by=.status.containerStatuses[0].restartCount",
            ],
        ),
        (
            "runtime.container.oom.error",
            "容器运行时",
            "error",
            "容器 OOM",
            "container.oom",
            "容器被 OOMKilled",
            "",
            "容器 OOM 会直接导致服务重启或请求失败，需要分析内存限制、堆大小和流量峰值。",
            &["docker inspect OOMKilled", "kubectl describe pod"],
            &[
                "docker inspect <container> --format '{{.State.OOMKilled}}'",
                "kubectl describe pod <pod> -n <namespace> | grep -i oom -C3",
            ],
        ),
        (
            "exception.stacktrace.error",
            "异常类型",
            "error",
            "应用堆栈异常",
            "application.stacktrace",
            "日志出现 Exception/Traceback/panic stack",
            "",
            "堆栈异常需要按首次异常、业务请求和发布时间定位根因，而不是只看最后一行。",
            &["application log", "stacktrace"],
            &[
                "grep -RniE 'Exception|Traceback|panic:|stack trace|Caused by' <log_dir> | tail -100",
                "tail -n 300 <app.log>",
            ],
        ),
        (
            "exception.timeout.warning",
            "异常类型",
            "warn",
            "调用超时异常",
            "application.timeout",
            "日志出现 timeout/deadline exceeded/read timed out",
            "",
            "超时异常通常来自下游慢、连接池耗尽、网络抖动或线程阻塞，需要结合上下游日志。",
            &["application log", "timeout keywords"],
            &[
                "grep -RniE 'timeout|deadline exceeded|read timed out|connect timed out' <log_dir> | tail -100",
                "ss -antp | head -80",
            ],
        ),
        (
            "exception.connection-refused.error",
            "异常类型",
            "error",
            "连接拒绝异常",
            "application.connection",
            "日志出现 connection refused / connect ECONNREFUSED",
            "",
            "连接拒绝说明目标服务端口不可达或未监听，需要确认目标地址、端口和防火墙。",
            &["application log", "connection refused"],
            &[
                "grep -RniE 'connection refused|ECONNREFUSED|No route to host' <log_dir> | tail -100",
                "ss -ltnp",
            ],
        ),
        (
            "exception.deadlock.error",
            "异常类型",
            "error",
            "死锁异常",
            "application.deadlock",
            "日志出现 deadlock 或数据库死锁",
            "",
            "死锁会导致事务回滚或请求超时，需要定位 SQL、锁等待和业务并发路径。",
            &["application log", "database log", "deadlock"],
            &[
                "grep -RniE 'deadlock|Lock wait timeout' <log_dir> | tail -100",
                "mysql -e 'SHOW ENGINE INNODB STATUS\\G'",
            ],
        ),
        (
            "storage.io-latency.warning",
            "存储异常",
            "warn",
            "磁盘 IO 延迟偏高",
            "storage.latency",
            "await/util 持续偏高",
            "",
            "磁盘 IO 延迟会影响数据库、日志写入和服务响应，需要确认热点盘和写入来源。",
            &["iostat await util", "pidstat -d"],
            &["iostat -x 1 5", "pidstat -d 1 5"],
        ),
        (
            "java.service.down.error",
            "Java服务",
            "error",
            "Java 服务进程缺失",
            "java.process",
            "按服务前缀未匹配到 Java 进程",
            "",
            "Java 进程缺失需要确认启动脚本、systemd 状态、端口和最近日志。",
            &["ps args", "service prefix"],
            &[
                "ps -ef | grep java",
                "systemctl status <service> --no-pager",
            ],
        ),
        (
            "java.gc.warning",
            "Java服务",
            "warn",
            "Java GC 异常频繁",
            "java.gc",
            "GC 日志出现频繁 Full GC/OOM",
            "",
            "频繁 Full GC 或 OOM 会造成接口停顿，需要分析堆、线程和对象增长。",
            &["gc.log", "jstat"],
            &[
                "jstat -gcutil <pid> 1000 10",
                "grep -Ei 'full gc|outofmemory|java heap' <log> | tail -50",
            ],
        ),
        (
            "java.thread.warning",
            "Java服务",
            "warn",
            "Java 线程数异常",
            "java.thread",
            "线程数超过阈值或大量 BLOCKED",
            "",
            "线程异常可能来自连接池耗尽、锁竞争或请求堆积。",
            &["jstack", "thread count"],
            &[
                "jstack <pid> | head -200",
                "ps -eLf | awk '$2==<pid>{print}' | wc -l",
            ],
        ),
        (
            "java.port.error",
            "Java服务",
            "error",
            "Java 服务端口未监听",
            "java.port",
            "Java 进程存在但端口未监听",
            "",
            "端口缺失通常表示应用启动失败、绑定地址错误或配置未加载。",
            &["ss -ltnp", "java pid"],
            &["ss -ltnp | grep <pid>", "tail -n 200 <app.log>"],
        ),
        (
            "config.file.missing.error",
            "配置风险",
            "error",
            "关键配置文件缺失",
            "config.file",
            "配置文件不存在或不可读",
            "",
            "关键配置缺失会造成服务启动失败或使用默认不安全配置。",
            &["configured path", "filesystem"],
            &["ls -l <config>", "namei -l <config>"],
        ),
        (
            "config.secret.warning",
            "配置风险",
            "warn",
            "配置中存在明文敏感信息",
            "config.secret",
            "配置文件匹配 password/token/secret",
            "",
            "明文敏感信息需要确认权限、脱敏和密钥管理策略。",
            &["config content", "file permissions"],
            &[
                "grep -RniE 'password|token|secret' <config_dir>",
                "stat <config>",
            ],
        ),
        (
            "config.listen.warning",
            "配置风险",
            "warn",
            "服务监听地址风险",
            "config.listen",
            "敏感服务监听 0.0.0.0",
            "",
            "敏感服务开放全部网卡可能扩大暴露面，需要结合现场访问策略确认。",
            &["ss -ltnp", "service config"],
            &["ss -ltnp", "grep -R \"0.0.0.0\" <config_dir>"],
        ),
        (
            "backup.missing.warning",
            "备份恢复",
            "warn",
            "近期未发现备份文件",
            "backup",
            "备份目录最近 N 天无新文件",
            "",
            "缺少近期备份会降低故障恢复能力，需要核对备份任务和备份介质。",
            &["backup directory", "mtime"],
            &["find <backup_dir> -type f -mtime -2 -ls | tail"],
        ),
        (
            "backup.failed.error",
            "备份恢复",
            "error",
            "备份任务失败",
            "backup.job",
            "备份日志/任务返回失败",
            "",
            "备份失败需要优先确认存储空间、权限、远端连接和脚本退出码。",
            &["backup log", "cron/systemd timer"],
            &[
                "systemctl list-timers",
                "grep -Ei 'backup.*failed|error' <backup_log> | tail -50",
            ],
        ),
        (
            "backup.restore.warning",
            "备份恢复",
            "warn",
            "缺少恢复验证记录",
            "restore.validation",
            "未发现恢复演练或校验记录",
            "",
            "只有备份没有恢复验证仍存在不可恢复风险，需要定期做恢复演练。",
            &["restore history", "checksum"],
            &["ls -l <backup_dir>", "sha256sum <backup_file>"],
        ),
        (
            "database.mysql.connection.warning",
            "数据库",
            "warn",
            "MySQL 连接数接近上限",
            "mysql.connections",
            "Threads_connected / max_connections >= 80%",
            "80",
            "连接数接近上限会造成新请求获取连接失败，需要确认连接池、慢 SQL 和泄漏连接。",
            &["SHOW GLOBAL STATUS Threads_connected", "max_connections"],
            &[
                "mysql -e \"SHOW GLOBAL STATUS LIKE 'Threads_connected'\"",
                "mysql -e \"SHOW VARIABLES LIKE 'max_connections'\"",
                "mysql -e 'SHOW FULL PROCESSLIST'",
            ],
        ),
        (
            "database.mysql.disk.error",
            "数据库",
            "error",
            "MySQL 数据目录磁盘不足",
            "mysql.datadir",
            "datadir 所在分区使用率 >= 90%",
            "90",
            "MySQL 数据目录空间不足会导致写入失败、binlog 无法落盘或实例异常退出。",
            &["datadir", "df -h", "binlog"],
            &[
                "mysql -e \"SHOW VARIABLES LIKE 'datadir'\"",
                "df -h <datadir>",
                "du -xh <datadir> 2>/dev/null | sort -h | tail -30",
            ],
        ),
        (
            "database.mysql.lock.warning",
            "数据库",
            "warn",
            "MySQL 锁等待异常",
            "mysql.locks",
            "锁等待/死锁/长事务异常",
            "",
            "锁等待会造成接口卡顿和连接堆积，需要定位阻塞事务、SQL 和业务并发路径。",
            &["INNODB_TRX", "SHOW ENGINE INNODB STATUS", "Lock wait"],
            &[
                "mysql -e 'SHOW ENGINE INNODB STATUS\\G'",
                "mysql -e 'SELECT * FROM information_schema.INNODB_TRX\\G'",
            ],
        ),
        (
            "database.redis.eviction.warning",
            "数据库",
            "warn",
            "Redis 淘汰或拒绝连接异常",
            "redis.stats",
            "evicted_keys/rejected_connections 增长",
            "",
            "Redis 出现淘汰或拒绝连接说明容量、连接池或客户端行为已经影响业务稳定性。",
            &["INFO stats evicted_keys", "rejected_connections"],
            &[
                "redis-cli info stats | grep -E 'evicted_keys|rejected_connections'",
                "redis-cli info clients",
                "redis-cli info memory",
            ],
        ),
        (
            "database.redis.persistence.warning",
            "数据库",
            "warn",
            "Redis 持久化失败",
            "redis.persistence",
            "rdb_last_bgsave_status/aof_last_bgrewrite_status != ok",
            "",
            "Redis 持久化失败会影响故障恢复，需要确认磁盘、权限、AOF/RDB 配置和后台保存日志。",
            &["INFO persistence", "redis log"],
            &[
                "redis-cli info persistence",
                "grep -RniE 'background saving error|AOF|RDB' /var/log/redis* 2>/dev/null | tail -80",
            ],
        ),
        (
            "middleware.kafka.underreplicated.error",
            "中间件",
            "error",
            "Kafka 副本不足",
            "kafka.replication",
            "UnderReplicatedPartitions > 0",
            "0",
            "Kafka 副本不足会降低可用性并可能影响生产消费稳定性，需要确认 broker、磁盘和 ISR。",
            &["UnderReplicatedPartitions", "kafka-topics --describe"],
            &[
                "kafka-topics.sh --bootstrap-server <broker> --describe --under-replicated-partitions",
                "grep -RniE 'UnderReplicated|ISR|broker.*down' <kafka_log_dir> | tail -80",
            ],
        ),
        (
            "middleware.kafka.controller.warning",
            "中间件",
            "warn",
            "Kafka 控制器频繁切换",
            "kafka.controller",
            "ActiveControllerCount 异常或 controller 日志频繁变更",
            "",
            "控制器频繁切换通常来自 broker 抖动、Zookeeper/KRaft 不稳或网络延迟。",
            &["ActiveControllerCount", "controller.log"],
            &[
                "grep -RniE 'Controller moved|controller epoch|ActiveController' <kafka_log_dir> | tail -80",
                "kafka-broker-api-versions.sh --bootstrap-server <broker>",
            ],
        ),
        (
            "middleware.rabbitmq.queue.warning",
            "中间件",
            "warn",
            "RabbitMQ 队列堆积",
            "rabbitmq.queue",
            "messages_ready/messages_unacknowledged 持续升高",
            "",
            "队列堆积会造成业务延迟，需要定位消费者状态、死信、限流和下游处理能力。",
            &["rabbitmqctl list_queues", "messages_ready"],
            &[
                "rabbitmqctl list_queues name messages_ready messages_unacknowledged consumers",
                "rabbitmqctl list_connections",
            ],
        ),
        (
            "middleware.zookeeper.down.error",
            "中间件",
            "error",
            "Zookeeper 节点不可用",
            "zookeeper",
            "ruok 未返回 imok 或服务失败",
            "",
            "Zookeeper 不可用会影响依赖的注册、选主或 Kafka 老集群稳定性。",
            &["ruok", "mntr", "systemctl zookeeper"],
            &[
                "echo ruok | nc 127.0.0.1 2181",
                "echo mntr | nc 127.0.0.1 2181",
                "systemctl status zookeeper --no-pager",
            ],
        ),
        (
            "web.gateway.cert.error",
            "Web网关",
            "error",
            "TLS 证书已过期",
            "tls.certificate",
            "证书已过期或证书链不可用",
            "",
            "证书过期会直接导致 HTTPS 访问失败，需要立即更新证书并确认自动续签链路。",
            &["openssl x509 notAfter", "curl TLS"],
            &[
                "openssl x509 -in <cert> -noout -dates",
                "curl -Iv https://<domain> 2>&1 | head -60",
            ],
        ),
        (
            "security.ssh.bruteforce.warning",
            "安全风险",
            "warn",
            "SSH 暴力尝试异常",
            "ssh.auth",
            "短时间认证失败次数异常",
            "",
            "SSH 暴力尝试可能带来账户风险，需要确认来源 IP、登录策略和防护规则。",
            &["auth.log Failed password", "journal sshd"],
            &[
                "journalctl -u sshd --since '2 hours ago' --no-pager | grep -Ei 'Failed|Invalid' | tail -100",
                "grep -Ei 'Failed password|Invalid user' /var/log/auth.log 2>/dev/null | tail -100",
            ],
        ),
        (
            "security.firewall.disabled.warning",
            "安全风险",
            "warn",
            "防火墙未启用",
            "firewall",
            "firewalld/ufw/iptables 规则为空或未运行",
            "",
            "防火墙未启用会扩大服务暴露面，需要结合现场网络边界确认是否符合要求。",
            &["systemctl firewalld", "ufw status", "iptables -S"],
            &[
                "systemctl status firewalld ufw --no-pager",
                "iptables -S | head -80",
                "nft list ruleset 2>/dev/null | head -120",
            ],
        ),
        (
            "security.selinux.disabled.info",
            "安全风险",
            "info",
            "SELinux 未启用",
            "selinux",
            "SELinux disabled/permissive",
            "",
            "SELinux 未启用不一定是故障，但需要与现场安全基线一致并记录原因。",
            &["getenforce", "/etc/selinux/config"],
            &["getenforce", "grep -n '^SELINUX=' /etc/selinux/config 2>/dev/null"],
        ),
        (
            "config.drift.warning",
            "配置风险",
            "warn",
            "配置疑似漂移",
            "config.drift",
            "运行配置与磁盘配置、模板或预期值不一致",
            "",
            "配置漂移会导致重启后行为变化，需要确认运行态、文件态和发布记录是否一致。",
            &["runtime config", "file checksum", "template"],
            &["sha256sum <config>", "systemctl cat <service>", "diff -u <expected> <config>"],
        ),
        (
            "runtime.docker.daemon.error",
            "容器运行时",
            "error",
            "Docker daemon 异常",
            "docker.daemon",
            "docker info 失败或 daemon 日志异常",
            "",
            "Docker daemon 异常会影响容器启动、网络和镜像操作，需要先确认 daemon、存储驱动和日志。",
            &["docker info", "dockerd journal"],
            &[
                "docker info",
                "systemctl status docker --no-pager",
                "journalctl -u docker --since '2 hours ago' --no-pager",
            ],
        ),
        (
            "runtime.kubernetes.notready.error",
            "容器运行时",
            "error",
            "Kubernetes 节点 NotReady",
            "kubernetes.node",
            "Node Ready != True",
            "",
            "节点 NotReady 会影响 Pod 调度和服务可用性，需要确认 kubelet、容器运行时、磁盘和网络插件。",
            &["kubectl get nodes", "kubelet journal"],
            &[
                "kubectl get nodes -o wide",
                "kubectl describe node <node>",
                "journalctl -u kubelet --since '2 hours ago' --no-pager",
            ],
        ),
        (
            "storage.readonly.error",
            "存储异常",
            "error",
            "文件系统只读",
            "filesystem.readonly",
            "mount 显示 ro 或日志出现 remount read-only",
            "",
            "文件系统只读通常来自底层 IO 或文件系统错误，会导致数据库、日志和业务写入失败。",
            &["mount ro", "EXT4-fs remount", "XFS error"],
            &[
                "mount | grep ' ro,'",
                "dmesg -T | grep -Ei 'read-only|remount|EXT4-fs error|XFS.*error' | tail -100",
            ],
        ),
        (
            "storage.nfs.stale.error",
            "存储异常",
            "error",
            "NFS 挂载异常",
            "nfs.mount",
            "stale file handle / nfs timeout / 挂载不可访问",
            "",
            "NFS 异常会造成进程卡死、IO 等待和文件访问失败，需要确认服务端、网络和挂载参数。",
            &["nfs mount", "stale file handle", "dmesg"],
            &[
                "mount | grep -i nfs",
                "dmesg -T | grep -Ei 'nfs|stale file handle|server not responding' | tail -100",
            ],
        ),
        (
            "network.dns.error",
            "网络链路",
            "error",
            "DNS 解析异常",
            "dns",
            "关键域名解析失败或耗时异常",
            "",
            "DNS 解析异常会造成下游连接失败、接口超时和服务注册异常。",
            &["resolv.conf", "dig/nslookup", "systemd-resolved"],
            &[
                "cat /etc/resolv.conf",
                "dig <domain>",
                "resolvectl status 2>/dev/null | head -120",
            ],
        ),
        (
            "network.firewall.drop.warning",
            "网络链路",
            "warn",
            "防火墙丢包或拒绝异常",
            "network.firewall",
            "iptables/nftables/journal 出现 DROP/REJECT 异常",
            "",
            "防火墙丢弃可能导致端口偶发不可达，需要关联策略、来源地址和业务访问路径。",
            &["iptables counters", "nft ruleset", "kernel DROP"],
            &[
                "iptables -vnL | head -120",
                "nft list ruleset 2>/dev/null | head -160",
                "journalctl -k --since '2 hours ago' --no-pager | grep -Ei 'DROP|REJECT' | tail -80",
            ],
        ),
        (
            "exception.null-pointer.error",
            "异常类型",
            "error",
            "空指针异常",
            "application.null_pointer",
            "日志出现 NullPointerException/NoneType/nil pointer",
            "",
            "空指针异常通常来自代码缺陷、脏数据或配置缺失，需要定位首次异常和请求上下文。",
            &["application log", "NullPointerException", "NoneType"],
            &[
                "grep -RniE 'NullPointerException|NoneType|nil pointer|null reference' <log_dir> | tail -100",
                "tail -n 300 <app.log>",
            ],
        ),
        (
            "exception.out-of-memory.error",
            "异常类型",
            "error",
            "应用内存溢出",
            "application.oom",
            "日志出现 OutOfMemoryError / memory allocation failed",
            "",
            "应用内存溢出会导致进程退出或请求失败，需要确认堆大小、对象增长和最近流量变化。",
            &["application log", "OutOfMemoryError", "oom"],
            &[
                "grep -RniE 'OutOfMemoryError|memory allocation failed|Cannot allocate memory' <log_dir> | tail -100",
                "ps -eo pid,comm,rss,%mem --sort=-rss | head -20",
            ],
        ),
        (
            "exception.database-pool.warning",
            "异常类型",
            "warn",
            "数据库连接池耗尽",
            "application.db_pool",
            "日志出现 connection pool exhausted / timeout waiting for connection",
            "",
            "连接池耗尽会造成接口超时，需要确认慢 SQL、连接泄漏、池配置和数据库连接上限。",
            &["application log", "connection pool", "timeout"],
            &[
                "grep -RniE 'connection pool|timeout waiting for connection|HikariPool|Too many connections' <log_dir> | tail -100",
                "mysql -e 'SHOW FULL PROCESSLIST'",
            ],
        ),
        (
            "schedule.cron.failed.warning",
            "调度任务",
            "warn",
            "定时任务执行失败",
            "cron.job",
            "cron/systemd timer 日志出现失败或退出码非 0",
            "",
            "定时任务失败会影响备份、清理和巡检类维护动作，需要定位任务脚本、权限和最近输出。",
            &["cron log", "systemd timer", "exit code"],
            &[
                "systemctl list-timers --all",
                "journalctl --since '24 hours ago' --no-pager | grep -Ei 'cron|timer|failed|exit status' | tail -120",
            ],
        ),
        (
            "check.generic.warning",
            "检查结果",
            "warn",
            "检查项警告",
            "check",
            "结构化检查返回 warn",
            "",
            "常规检查输出警告项，规则引擎会保留原始证据并给出处理建议。",
            &["CheckResult.sections.items.status=warn"],
            &["dm check <id>"],
        ),
        (
            "check.generic.error",
            "检查结果",
            "error",
            "检查项异常",
            "check",
            "结构化检查返回 error",
            "",
            "常规检查输出异常项，规则引擎会同步到系统告警并保留详情。",
            &["CheckResult.sections.items.status=error"],
            &["dm check <id>"],
        ),
        (
            "check.timeout.warning",
            "检查结果",
            "warn",
            "检查项执行超时",
            "check.timeout",
            "检查执行超过后台体检超时",
            "",
            "检查超时通常表示连接、命令或权限异常，需要进入单项检查定位。",
            &["health task timeout", "duration"],
            &["dm check <id>"],
        ),
        (
            "script.failure.warning",
            "脚本执行",
            "warn",
            "近期脚本执行失败",
            "script.history",
            "最近执行历史中失败次数 >= 2",
            "2",
            "维护脚本连续失败通常代表现场维护动作未完成、参数错误、权限不足或依赖命令不可用。",
            &["exec_history.exit_code", "script_id", "script_name"],
            &["dm logs <script_id>", "dm stats <script_id>"],
        ),
    ];
    RULES
        .iter()
        .map(
            |(
                id,
                category,
                level,
                title,
                target,
                condition,
                threshold,
                description,
                signals,
                commands,
            )| {
                rule_def(
                    id,
                    category,
                    level,
                    title,
                    target,
                    condition,
                    threshold,
                    description,
                    signals,
                    commands,
                )
            },
        )
        .collect()
}

fn rule_def(
    id: &str,
    category: &str,
    level: &str,
    title: &str,
    target: &str,
    condition: &str,
    threshold: &str,
    description: &str,
    signals: &[&str],
    commands: &[&str],
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "category": category,
        "level": level,
        "title": title,
        "target": target,
        "condition": condition,
        "threshold": threshold,
        "enabled": true,
        "source": "builtin",
        "description": description,
        "signals": signals,
        "commands": commands,
    })
}

pub fn apply_overrides(
    mut findings: Vec<AnomalyFinding>,
    overrides: &HashMap<String, serde_json::Value>,
) -> Vec<AnomalyFinding> {
    findings.retain(|finding| {
        let override_value = overrides
            .get(&finding.rule_id)
            .or_else(|| overrides.get(finding.rule_id.rsplit_once('.').map(|v| v.0).unwrap_or("")));
        !matches!(
            override_value
                .and_then(|v| v.get("enabled"))
                .and_then(|v| v.as_bool()),
            Some(false)
        )
    });
    for finding in &mut findings {
        if let Some(value) = overrides
            .get(&finding.rule_id)
            .or_else(|| overrides.get(finding.rule_id.rsplit_once('.').map(|v| v.0).unwrap_or("")))
        {
            if let Some(level) = value
                .get("level")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
            {
                finding.level = level.to_string();
            }
            if let Some(title) = value
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
            {
                finding.title = title.to_string();
            }
            if let Some(summary) = value
                .get("summary")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
            {
                finding.summary = summary.to_string();
            }
            if let Some(suggestion) = value
                .get("suggestion")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
            {
                finding.suggestion = suggestion.to_string();
            }
            if let Some(commands) = value.get("commands").and_then(|v| v.as_array()) {
                finding.commands = commands
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
            }
        }
    }
    findings
}

pub fn evaluate_system_info(sys: &SystemInfo) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let load_ratio = if sys.cpu_count > 0 {
        sys.load_avg.one / sys.cpu_count as f64
    } else {
        0.0
    };

    if sys.cpu_usage >= 90.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.cpu.critical",
                "error",
                "资源水位",
                "CPU 使用率严重过高",
                "system.cpu",
                format!("CPU 使用率 {:.1}%，已进入高风险区间", sys.cpu_usage),
            )
            .evidence([
                format!("CPU 使用率: {:.1}%", sys.cpu_usage),
                format!("CPU 核心数: {}", sys.cpu_count),
            ])
            .suggestion("优先定位 CPU Top 进程，确认是否存在死循环、突发任务或异常流量。")
            .commands([
                "top -o %CPU".to_string(),
                "ps -eo pid,ppid,comm,%cpu,%mem --sort=-%cpu | head -20".to_string(),
            ]),
        );
    } else if sys.cpu_usage >= 80.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.cpu.warning",
                "warn",
                "资源水位",
                "CPU 使用率偏高",
                "system.cpu",
                format!("CPU 使用率 {:.1}%，需要持续观察", sys.cpu_usage),
            )
            .evidence([format!("CPU 使用率: {:.1}%", sys.cpu_usage)])
            .suggestion("结合 5/15 分钟负载和 Top 进程判断是否为持续异常。")
            .commands(["vmstat 1 10".to_string(), "pidstat 1 5".to_string()]),
        );
    }

    if sys.memory_usage >= 90.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.memory.critical",
                "error",
                "资源水位",
                "内存使用率严重过高",
                "system.memory",
                format!(
                    "内存使用率 {:.1}%，可能触发 OOM 或频繁回收",
                    sys.memory_usage
                ),
            )
            .evidence([
                format!("内存使用率: {:.1}%", sys.memory_usage),
                format!("内存用量: {} / {} bytes", sys.memory_used, sys.memory_total),
                format!("Swap 用量: {} / {} bytes", sys.swap_used, sys.swap_total),
            ])
            .suggestion("定位 RSS 最高进程，检查 OOM 记录和业务堆内存增长趋势。")
            .commands([
                "free -h".to_string(),
                "ps -eo pid,comm,rss,%mem --sort=-rss | head -20".to_string(),
                "journalctl -k -p warning --since '2 hours ago' | grep -i oom".to_string(),
            ]),
        );
    } else if sys.memory_usage >= 80.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.memory.warning",
                "warn",
                "资源水位",
                "内存使用率偏高",
                "system.memory",
                format!(
                    "内存使用率 {:.1}%，需要关注缓存和进程增长",
                    sys.memory_usage
                ),
            )
            .evidence([
                format!("内存使用率: {:.1}%", sys.memory_usage),
                format!("内存用量: {} / {} bytes", sys.memory_used, sys.memory_total),
            ])
            .suggestion("对比历史趋势，确认是否为业务峰值、缓存增长或内存泄漏。")
            .commands([
                "free -h".to_string(),
                "ps -eo pid,comm,rss,%mem --sort=-rss | head -20".to_string(),
            ]),
        );
    }

    if sys.disk_usage >= 90.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.disk.critical",
                "error",
                "资源水位",
                "磁盘使用率严重过高",
                "system.disk",
                format!("整体磁盘使用率 {:.1}%，存在写入失败风险", sys.disk_usage),
            )
            .evidence([
                format!("磁盘使用率: {:.1}%", sys.disk_usage),
                format!("磁盘用量: {} / {} bytes", sys.disk_used, sys.disk_total),
            ])
            .suggestion("优先清理日志、临时文件、备份文件，并确认业务数据目录是否异常增长。")
            .commands([
                "df -h".to_string(),
                "du -xh /var/log 2>/dev/null | sort -h | tail -20".to_string(),
            ]),
        );
    } else if sys.disk_usage >= 80.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.disk.warning",
                "warn",
                "资源水位",
                "磁盘使用率偏高",
                "system.disk",
                format!("整体磁盘使用率 {:.1}%，建议提前处理", sys.disk_usage),
            )
            .evidence([
                format!("磁盘使用率: {:.1}%", sys.disk_usage),
                format!("磁盘用量: {} / {} bytes", sys.disk_used, sys.disk_total),
            ])
            .suggestion("检查增长最快的目录，避免到达 90% 后影响服务写入。")
            .commands([
                "df -h".to_string(),
                "du -xh / 2>/dev/null | sort -h | tail -20".to_string(),
            ]),
        );
    }

    if load_ratio >= 2.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.load.critical",
                "error",
                "资源水位",
                "系统负载严重过高",
                "system.loadavg",
                format!(
                    "1 分钟负载 {:.2}，约为 CPU 核心数的 {:.1} 倍",
                    sys.load_avg.one, load_ratio
                ),
            )
            .evidence([
                format!(
                    "loadavg: {:.2}, {:.2}, {:.2}",
                    sys.load_avg.one, sys.load_avg.five, sys.load_avg.fifteen
                ),
                format!("CPU 核心数: {}", sys.cpu_count),
            ])
            .suggestion("区分 CPU 饱和、IO 等待和不可中断进程，若 5/15 分钟也高说明不是瞬时抖动。")
            .commands([
                "uptime".to_string(),
                "vmstat 1 10".to_string(),
                "ps -eo stat,pid,comm,%cpu,%mem --sort=-%cpu | head -20".to_string(),
            ]),
        );
    } else if load_ratio >= 1.0 {
        findings.push(
            AnomalyFinding::new(
                "resource.load.warning",
                "warn",
                "资源水位",
                "系统负载偏高",
                "system.loadavg",
                format!("1 分钟负载 {:.2}，已超过 CPU 核心数", sys.load_avg.one),
            )
            .evidence([format!(
                "loadavg: {:.2}, {:.2}, {:.2}",
                sys.load_avg.one, sys.load_avg.five, sys.load_avg.fifteen
            )])
            .suggestion("继续观察 5 分钟和 15 分钟负载，结合 vmstat 判断瓶颈。")
            .commands(["uptime".to_string(), "vmstat 1 5".to_string()]),
        );
    }

    for disk in &sys.disks {
        if is_noise_mount(&disk.mount_point, &disk.fs_type) {
            continue;
        }
        if disk.usage >= 90.0 {
            findings.push(
                AnomalyFinding::new(
                    format!(
                        "resource.mount.critical.{}",
                        disk.mount_point.replace('/', "_")
                    ),
                    "error",
                    "磁盘分区",
                    format!("分区 {} 空间严重不足", disk.mount_point),
                    disk.mount_point.clone(),
                    format!(
                        "{} 使用率 {:.1}%，需要立即处理",
                        disk.mount_point, disk.usage
                    ),
                )
                .evidence([
                    format!("挂载点: {}", disk.mount_point),
                    format!("文件系统: {}", disk.fs_type),
                    format!("已用: {} / {} bytes", disk.used, disk.total),
                ])
                .suggestion("优先处理该挂载点下的日志、临时文件、备份文件，必要时扩容。")
                .commands([
                    format!("df -h {}", disk.mount_point),
                    format!(
                        "du -xh {} 2>/dev/null | sort -h | tail -20",
                        disk.mount_point
                    ),
                ]),
            );
        } else if disk.usage >= 80.0 {
            findings.push(
                AnomalyFinding::new(
                    format!(
                        "resource.mount.warning.{}",
                        disk.mount_point.replace('/', "_")
                    ),
                    "warn",
                    "磁盘分区",
                    format!("分区 {} 空间偏高", disk.mount_point),
                    disk.mount_point.clone(),
                    format!(
                        "{} 使用率 {:.1}%，建议跟进增长来源",
                        disk.mount_point, disk.usage
                    ),
                )
                .evidence([
                    format!("挂载点: {}", disk.mount_point),
                    format!("已用: {} / {} bytes", disk.used, disk.total),
                ])
                .suggestion("确认是否为可预期增长，必要时扩容或清理。")
                .commands([format!("df -h {}", disk.mount_point)]),
            );
        }
    }

    for p in &sys.top_processes {
        if p.cpu_usage >= 85.0 {
            findings.push(
                AnomalyFinding::new(
                    format!("process.cpu.warning.{}", p.pid),
                    "warn",
                    "进程异常",
                    format!("进程 {} CPU 占用过高", p.name),
                    format!("pid:{}", p.pid),
                    format!("PID {} 当前 CPU {:.1}%", p.pid, p.cpu_usage),
                )
                .evidence([
                    format!("PID: {}", p.pid),
                    format!("进程: {}", p.name),
                    format!("CPU: {:.1}%", p.cpu_usage),
                ])
                .suggestion("确认进程类型和业务角色，结合线程、日志判断是否异常请求或循环任务。")
                .commands([
                    format!("ps -p {} -o pid,ppid,comm,%cpu,%mem,etime,args", p.pid),
                    format!("top -Hp {}", p.pid),
                ]),
            );
        }
    }

    findings
}

fn is_noise_mount(mount_point: &str, fs_type: &str) -> bool {
    fs_type == "overlay"
        || mount_point.starts_with("/var/lib/docker/")
        || mount_point.starts_with("/var/lib/containerd/")
        || mount_point.starts_with("/run/containerd/")
        || mount_point.starts_with("/snap/")
}

pub fn evaluate_check_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    for section in &result.sections {
        if section.title == "异常明细" {
            continue;
        }
        for item in &section.items {
            match item {
                Item::Label {
                    key,
                    value,
                    status: Some(status),
                } if status == "warn" || status == "error" => {
                    findings.push(
                        AnomalyFinding::new(
                            format!("check.{}.{}.{}", result.id, section.title, key),
                            status.clone(),
                            section.title.clone(),
                            format!(
                                "{} 状态{}",
                                key,
                                if status == "error" {
                                    "异常"
                                } else {
                                    "警告"
                                }
                            ),
                            key.clone(),
                            format!("{}: {}", key, value),
                        )
                        .evidence([
                            format!("检查项: {}", key),
                            format!("当前值: {}", value),
                            format!("来源模块: {}", section.title),
                        ])
                        .suggestion(default_suggestion(&result.id, &section.title, key, status))
                        .commands(default_commands(&result.id, key)),
                    );
                }
                Item::Bar {
                    key,
                    value,
                    max,
                    unit,
                    status: Some(status),
                } if status == "warn" || status == "error" => {
                    if section.title == "磁盘挂载" && is_noise_mount(key, key) {
                        continue;
                    }
                    findings.push(
                        AnomalyFinding::new(
                            format!("check.{}.{}.{}", result.id, section.title, key),
                            status.clone(),
                            section.title.clone(),
                            format!(
                                "{} 状态{}",
                                key,
                                if status == "error" {
                                    "异常"
                                } else {
                                    "警告"
                                }
                            ),
                            key.clone(),
                            format!("{} 当前 {:.1}{}，检查状态为 {}", key, value, unit, status),
                        )
                        .evidence([
                            format!("当前值: {:.2}{}", value, unit),
                            format!("容量/基准: {:.2}{}", max, unit),
                            format!("检查状态: {}", status),
                            format!("来源模块: {}", section.title),
                        ])
                        .suggestion(default_suggestion(&result.id, &section.title, key, status))
                        .commands(default_commands(&result.id, key)),
                    );
                }
                Item::Warning { text } | Item::Error { text } => {
                    let status = if matches!(item, Item::Error { .. }) {
                        "error"
                    } else {
                        "warn"
                    };
                    findings.push(
                        AnomalyFinding::new(
                            format!("check.{}.{}.message", result.id, section.title),
                            status,
                            section.title.clone(),
                            if status == "error" {
                                "检查返回异常"
                            } else {
                                "检查返回警告"
                            },
                            section.title.clone(),
                            text.clone(),
                        )
                        .evidence([text.clone(), format!("来源模块: {}", section.title)])
                        .suggestion(default_suggestion(
                            &result.id,
                            &section.title,
                            &section.title,
                            status,
                        ))
                        .commands(default_commands(&result.id, &section.title)),
                    );
                }
                _ => {}
            }
        }
    }
    findings.extend(evaluate_domain_check_result(result));
    findings
}

fn evaluate_domain_check_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    match result.id.as_str() {
        "service-manage" => evaluate_service_manage_result(result),
        "nginx" => evaluate_nginx_result(result),
        "redis" => evaluate_redis_result(result),
        "elasticsearch" => evaluate_es_result(result),
        "keepalived" => evaluate_keepalived_result(result),
        "kafka" => evaluate_kafka_result(result),
        "java-service" => evaluate_java_service_result(result),
        _ => Vec::new(),
    }
}

fn label_value<'a>(result: &'a CheckResult, section_title: &str, key: &str) -> Option<&'a str> {
    result
        .sections
        .iter()
        .find(|s| s.title == section_title)
        .and_then(|s| {
            s.items.iter().find_map(|item| match item {
                Item::Label { key: k, value, .. } if k == key => Some(value.as_str()),
                _ => None,
            })
        })
}

fn first_table_rows<'a>(result: &'a CheckResult, section_title: &str) -> Vec<&'a Vec<String>> {
    result
        .sections
        .iter()
        .find(|s| s.title == section_title)
        .and_then(|s| {
            s.items.iter().find_map(|item| match item {
                Item::Table { rows, .. } => Some(rows.iter().collect()),
                _ => None,
            })
        })
        .unwrap_or_default()
}

fn evaluate_service_manage_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    for row in first_table_rows(result, "服务列表") {
        if row.len() < 12 {
            continue;
        }
        let pid = row[1].trim();
        let name = row[2].trim();
        let process = row[3].trim();
        let path = row[4].trim();
        let ports = row[5].trim();
        let status = row[6].trim();
        let cpu = row[7].trim();
        let memory = row[8].trim();
        let category = row[9].trim();
        let unit_state = row[10].trim();
        let log_anomaly = row[11].trim();
        if !unit_state.contains("/active/")
            && unit_state != "process-only"
            && unit_state != "unknown"
        {
            findings.push(
                AnomalyFinding::new(
                    format!("service.systemctl.state.{}", name),
                    "error",
                    "服务管理",
                    format!("服务 {} systemctl 状态异常", name),
                    name.to_string(),
                    format!("systemctl 状态为 {}", unit_state),
                )
                .evidence([
                    format!("PID: {}", pid),
                    format!("进程: {}", process),
                    format!("状态: {}", status),
                    format!("Systemd: {}", unit_state),
                    format!("端口: {}", ports),
                    format!("路径: {}", path),
                ])
                .suggestion("先查看 systemctl status 和 journalctl，确认是否为 failed、activating 卡住或反复重启。")
                .commands([
                    format!("systemctl status {}", name),
                    format!("journalctl -u {} -n 160 --no-pager", name),
                ]),
            );
        }
        if log_anomaly != "-" && !log_anomaly.is_empty() {
            findings.push(
                AnomalyFinding::new(
                    format!("service.systemctl.log_anomaly.{}", name),
                    "error",
                    "服务管理",
                    format!("服务 {} 最近日志存在异常", name),
                    name.to_string(),
                    format!("最近 systemd 日志发现异常关键字: {}", log_anomaly),
                )
                .evidence([
                    format!("PID: {}", pid),
                    format!("进程: {}", process),
                    format!("类型: {}", category),
                    format!("CPU: {}", cpu),
                    format!("内存: {}", memory),
                    format!("日志摘要: {}", log_anomaly),
                ])
                .suggestion("保留现场日志上下文，确认异常是否持续发生；必要时结合服务健康、端口和最近变更定位。")
                .commands([
                    format!("journalctl -u {} -n 200 --no-pager -p warning..alert", name),
                    format!("systemctl status {}", name),
                ]),
            );
        }
    }
    findings
}

fn evaluate_nginx_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let running = label_value(result, "连接与程序信息", "运行状态").unwrap_or("");
    if running.contains("未发现") {
        findings.push(
            AnomalyFinding::new(
                "middleware.nginx.not_running",
                "warn",
                "Web网关",
                "Nginx 未运行",
                "nginx",
                "未发现 Nginx master/worker 进程",
            )
            .evidence([format!("运行状态: {}", running)])
            .suggestion("确认该机器是否应承载 Nginx；若应运行，先检查配置再启动服务。")
            .commands(["systemctl status nginx".to_string(), "nginx -t".to_string()]),
        );
    }
    let config_test = label_value(result, "连接与程序信息", "配置检测").unwrap_or("");
    if !config_test.is_empty()
        && !config_test.contains("successful")
        && !config_test.contains("ok")
        && !config_test.contains("未安装")
    {
        findings.push(
            AnomalyFinding::new(
                "middleware.nginx.config_invalid",
                "error",
                "Web网关",
                "Nginx 配置检测失败",
                "nginx.conf",
                "nginx -t 未通过，重载或启动可能失败",
            )
            .evidence([crate::checks::common::truncate(config_test, 360)])
            .suggestion("先修复 nginx -t 输出中的配置错误，再执行 reload/start。")
            .commands([
                "nginx -t".to_string(),
                "journalctl -u nginx -n 80 --no-pager".to_string(),
            ]),
        );
    }
    findings
}

fn evaluate_redis_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let ping = label_value(result, "连接信息", "PING").unwrap_or("");
    if ping.contains("无响应") {
        findings.push(
            AnomalyFinding::new(
                "middleware.redis.ping_failed",
                "warn",
                "中间件",
                "Redis PING 无响应",
                label_value(result, "连接信息", "地址").unwrap_or("redis"),
                "Redis 无法响应 PING，可能未运行、认证错误或端口不可达",
            )
            .evidence([format!("PING: {}", ping)])
            .suggestion("确认 Redis 进程、监听端口和密码配置，避免误判为业务缓存故障。")
            .commands([
                "systemctl status redis redis-server".to_string(),
                "redis-cli ping".to_string(),
            ]),
        );
    }
    for row in first_table_rows(result, "运行状态") {
        if row.len() >= 2 {
            let key = &row[0];
            let value = &row[1];
            if (key == "blocked_clients" && value.parse::<u64>().unwrap_or(0) > 0)
                || (key == "rdb_last_bgsave_status" && value != "ok" && value != "-")
                || (key == "aof_last_write_status" && value != "ok" && value != "-")
            {
                findings.push(
                    AnomalyFinding::new(
                        format!("middleware.redis.{}", key),
                        "warn",
                        "中间件",
                        format!("Redis {} 异常", key),
                        key.clone(),
                        format!("{} = {}", key, value),
                    )
                    .evidence([format!("{}: {}", key, value)])
                    .suggestion("检查 Redis INFO、慢日志、持久化日志和磁盘空间。")
                    .commands([
                        "redis-cli info".to_string(),
                        "redis-cli slowlog get 10".to_string(),
                    ]),
                );
            }
        }
    }
    findings
}

fn evaluate_es_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let state = label_value(result, "集群健康", "集群状态").unwrap_or("");
    if state == "yellow" || state == "red" || state == "unknown" {
        findings.push(
            AnomalyFinding::new(
                "middleware.elasticsearch.cluster_health",
                if state == "red" { "error" } else { "warn" },
                "中间件",
                "Elasticsearch 集群健康异常",
                "cluster",
                format!("集群状态: {}", state),
            )
            .evidence([
                format!("集群状态: {}", state),
                format!(
                    "未分配分片: {}",
                    label_value(result, "集群健康", "未分配分片").unwrap_or("-")
                ),
            ])
            .suggestion("优先处理 red/yellow、未分配分片和磁盘水位，再观察 shard recovery。")
            .commands([
                "curl -s localhost:9200/_cluster/health?pretty".to_string(),
                "curl -s localhost:9200/_cat/shards?v".to_string(),
            ]),
        );
    }
    findings
}

fn evaluate_keepalived_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let running = label_value(result, "运行状态", "运行状态").unwrap_or("");
    if running.contains("未发现") {
        findings.push(
            AnomalyFinding::new(
                "middleware.keepalived.not_running",
                "warn",
                "中间件",
                "Keepalived 未运行",
                "keepalived",
                "未发现 Keepalived 进程，VIP 漂移能力可能失效",
            )
            .evidence([format!("运行状态: {}", running)])
            .suggestion("确认该节点是否应参与 VRRP；若应参与，检查配置、网卡、VRID 和日志。")
            .commands([
                "systemctl status keepalived".to_string(),
                "ip addr show".to_string(),
                "journalctl -u keepalived -n 100 --no-pager".to_string(),
            ]),
        );
    }
    findings
}

fn evaluate_kafka_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let running = label_value(result, "运行与连接", "运行状态").unwrap_or("");
    if running.contains("未发现") || running.contains("未安装") {
        findings.push(
            AnomalyFinding::new(
                "middleware.kafka.not_running",
                "warn",
                "中间件",
                "Kafka 未运行",
                "kafka",
                "未发现 Kafka broker 进程或常见监听端口",
            )
            .evidence([format!("运行状态: {}", running)])
            .suggestion("确认该机器是否应承载 Kafka；若应运行，检查 systemd、server.properties、磁盘和 broker 日志。")
            .commands([
                "systemctl status kafka".to_string(),
                "ss -ltnp | grep -E ':9092|:9093'".to_string(),
            ]),
        );
    }
    findings
}

fn evaluate_java_service_result(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let matched = label_value(result, "匹配配置", "匹配结果").unwrap_or("");
    if matched.contains("未发现") {
        findings.push(
            AnomalyFinding::new(
                "java.service.not_found",
                "warn",
                "Java服务",
                "未发现匹配 Java 服务",
                "java-service",
                "未发现符合服务前缀或 Java/Tomcat 特征的进程",
            )
            .evidence([format!("匹配结果: {}", matched)])
            .suggestion("确认服务前缀配置是否过窄，或目标 Java/Tomcat 服务是否已启动。")
            .commands(["ps -eo pid,comm,args | grep -E 'java|tomcat|catalina'".to_string()]),
        );
    }
    for row in first_table_rows(result, "Java 服务列表") {
        if row.len() < 7 {
            continue;
        }
        let pid = row[0].trim();
        let name = row[1].trim();
        let ports = row[5].trim();
        let cmd = row[6].trim();
        let lower_cmd = cmd.to_lowercase();
        let likely_server = lower_cmd.contains("tomcat")
            || lower_cmd.contains("catalina")
            || lower_cmd.contains("spring")
            || lower_cmd.contains("-dserver.port")
            || lower_cmd.contains(".jar");
        if likely_server
            && (ports.is_empty()
                || ports == "-"
                || ports.contains("未发现")
                || ports.contains("无监听"))
        {
            findings.push(
                AnomalyFinding::new(
                    format!("java.runtime.no_port.{}", pid),
                    "error",
                    "Java服务",
                    format!("Java 服务 {} 未监听端口", name),
                    format!("pid:{}", pid),
                    "Java/Tomcat 进程存在，但未发现关联监听端口",
                )
                .evidence([
                    format!("PID: {}", pid),
                    format!("进程: {}", name),
                    format!("监听端口: {}", if ports.is_empty() { "-" } else { ports }),
                    format!("命令: {}", cmd),
                ])
                .suggestion(
                    "确认应用是否启动完成、server.port/AJP 配置是否正确、端口是否绑定到预期地址。",
                )
                .commands([
                    format!("ss -ltnp | grep {}", pid),
                    format!(
                        "jcmd {} VM.system_properties | grep -E 'server.port|catalina|ajp'",
                        pid
                    ),
                    format!("tail -n 200 <{}-app.log>", name),
                ]),
            );
        }
    }
    for row in first_table_rows(result, "Java 运行时") {
        if row.len() < 7 {
            continue;
        }
        let pid = &row[0];
        let name = &row[1];
        let cpu = row[2].parse::<f64>().unwrap_or(0.0);
        let memory = row[3].parse::<f64>().unwrap_or(0.0);
        let threads = row[4].parse::<u64>().unwrap_or(0);
        let flags = &row[6];
        let has_heap_limit = java_has_heap_limit(flags);
        if cpu >= 85.0 || memory >= 4096.0 || threads >= 800 {
            findings.push(
                AnomalyFinding::new(
                    format!("java.runtime.{}", pid),
                    "warn",
                    "Java服务",
                    format!("Java 服务 {} 运行时压力偏高", name),
                    format!("pid:{}", pid),
                    format!("CPU {:.1}%，内存 {:.1}MB，线程 {}", cpu, memory, threads),
                )
                .evidence([
                    format!("PID: {}", pid),
                    format!("进程: {}", name),
                    format!("CPU: {:.1}%", cpu),
                    format!("内存: {:.1}MB", memory),
                    format!("线程数: {}", threads),
                    format!("参数: {}", flags),
                ])
                .suggestion(
                    "抓取线程栈和 GC/堆信息，结合业务日志判断高 CPU、线程堆积或内存增长来源。",
                )
                .commands([
                    format!("jcmd {} Thread.print | head -200", pid),
                    format!("jcmd {} GC.heap_info", pid),
                    format!("top -Hp {}", pid),
                ]),
            );
        }
        if cpu >= 90.0 {
            findings.push(java_runtime_metric_finding(
                pid,
                name,
                "cpu_high",
                "error",
                "CPU 持续高位",
                format!("CPU {:.1}% 已超过 90%", cpu),
                vec![
                    format!("top -Hp {}", pid),
                    format!("jcmd {} Thread.print | head -240", pid),
                    format!("pidstat -t -p {} 1 5", pid),
                ],
            ));
        } else if cpu >= 70.0 {
            findings.push(java_runtime_metric_finding(
                pid,
                name,
                "cpu_busy",
                "warn",
                "CPU 使用率偏高",
                format!("CPU {:.1}% 已超过 70%", cpu),
                vec![
                    format!("top -Hp {}", pid),
                    format!("jcmd {} Thread.print | head -200", pid),
                ],
            ));
        }
        if memory >= 8192.0 {
            findings.push(java_runtime_metric_finding(
                pid,
                name,
                "rss_critical",
                "error",
                "RSS 内存异常偏高",
                format!("RSS 内存 {:.1}MB 已超过 8192MB", memory),
                vec![
                    format!("jcmd {} GC.heap_info", pid),
                    format!("jcmd {} VM.native_memory summary", pid),
                    format!("jmap -histo:live {} | head -80", pid),
                ],
            ));
        } else if memory >= 4096.0 {
            findings.push(java_runtime_metric_finding(
                pid,
                name,
                "rss_high",
                "warn",
                "RSS 内存偏高",
                format!("RSS 内存 {:.1}MB 已超过 4096MB", memory),
                vec![
                    format!("jcmd {} GC.heap_info", pid),
                    format!("jmap -histo:live {} | head -60", pid),
                ],
            ));
        }
        if threads >= 800 {
            findings.push(java_runtime_metric_finding(
                pid,
                name,
                "threads_critical",
                "error",
                "线程数异常偏高",
                format!("线程数 {} 已超过 800", threads),
                vec![
                    format!("jcmd {} Thread.print | head -260", pid),
                    format!("ps -eLf | awk '$2=={}{{print}}' | wc -l", pid),
                ],
            ));
        } else if threads >= 400 {
            findings.push(java_runtime_metric_finding(
                pid,
                name,
                "threads_high",
                "warn",
                "线程数偏高",
                format!("线程数 {} 已超过 400", threads),
                vec![
                    format!("jcmd {} Thread.print | head -220", pid),
                    format!("ss -antp | grep {}", pid),
                ],
            ));
        }
        if !has_heap_limit {
            findings.push(
                AnomalyFinding::new(
                    format!("java.runtime.heap_unbounded.{}", pid),
                    "warn",
                    "Java服务",
                    format!("Java 服务 {} 未识别到堆上限", name),
                    format!("pid:{}", pid),
                    "启动参数未发现 -Xmx、MaxRAMPercentage 或 MaxHeapSize",
                )
                .evidence([
                    format!("PID: {}", pid),
                    format!("进程: {}", name),
                    format!("内存: {:.1}MB", memory),
                    format!("参数: {}", flags),
                ])
                .suggestion("为 Java 服务显式配置堆上限，并结合容器/主机内存预留 metaspace、直接内存、线程栈和 page cache。")
                .commands([
                    format!("jcmd {} VM.flags", pid),
                    format!("jcmd {} GC.heap_info", pid),
                    "grep -R \"Xmx\\|MaxRAMPercentage\\|MaxHeapSize\" /etc/systemd /opt 2>/dev/null | head -80".to_string(),
                ]),
            );
        }
    }
    findings.extend(evaluate_java_log_keywords(result));
    findings
}

fn java_has_heap_limit(flags: &str) -> bool {
    let lower = flags.to_lowercase();
    lower.contains("-xmx")
        || lower.contains("maxrampercentage")
        || lower.contains("maxheapsize")
        || lower.contains("initialrampercentage")
}

fn java_runtime_metric_finding(
    pid: &str,
    name: &str,
    suffix: &str,
    level: &str,
    title: &str,
    summary: String,
    commands: Vec<String>,
) -> AnomalyFinding {
    AnomalyFinding::new(
        format!("java.runtime.{}.{}", suffix, pid),
        level,
        "Java服务",
        format!("Java 服务 {} {}", name, title),
        format!("pid:{}", pid),
        summary.clone(),
    )
    .evidence([
        format!("PID: {}", pid),
        format!("进程: {}", name),
        format!("指标: {}", summary),
    ])
    .suggestion("结合线程栈、GC、堆对象和最近发布/流量变化定位根因，避免直接重启掩盖现场。")
    .commands(commands)
}

fn evaluate_java_log_keywords(result: &CheckResult) -> Vec<AnomalyFinding> {
    let mut findings = Vec::new();
    let blob = check_result_text(result).to_lowercase();
    let rules = [
        (
            "java.log.oom",
            "error",
            "Java OOM / 堆内存异常",
            &["outofmemoryerror", "java heap space", "gc overhead limit exceeded"][..],
            "日志出现 OOM 或堆空间不足关键词",
            &[
                "grep -RniE 'OutOfMemoryError|Java heap space|GC overhead' <log_dir> | tail -80",
                "jcmd <pid> GC.heap_info",
                "jmap -histo:live <pid> | head -80",
            ][..],
        ),
        (
            "java.log.metaspace",
            "error",
            "Java Metaspace 异常",
            &["metaspace", "compressed class space"][..],
            "日志出现 metaspace 或 class space 异常",
            &[
                "grep -RniE 'Metaspace|Compressed class space' <log_dir> | tail -80",
                "jcmd <pid> VM.native_memory summary",
            ][..],
        ),
        (
            "java.log.native_thread",
            "error",
            "Java 原生线程创建失败",
            &["unable to create new native thread"][..],
            "日志出现 unable to create new native thread",
            &[
                "ulimit -a",
                "ps -eLf | awk '$2==<pid>{print}' | wc -l",
                "jcmd <pid> Thread.print | head -240",
            ][..],
        ),
        (
            "java.log.full_gc",
            "warn",
            "Java Full GC 频繁",
            &["full gc", "allocation failure", "promotion failed"][..],
            "日志出现 Full GC 或对象晋升失败关键词",
            &[
                "grep -RniE 'Full GC|allocation failure|promotion failed' <log_dir> | tail -80",
                "jstat -gcutil <pid> 1000 10",
            ][..],
        ),
        (
            "java.log.deadlock",
            "error",
            "Java 死锁风险",
            &["deadlock", "found one java-level deadlock"][..],
            "日志或线程栈出现 deadlock 关键词",
            &[
                "jcmd <pid> Thread.print | grep -i deadlock -C8",
                "grep -Rni deadlock <log_dir> | tail -80",
            ][..],
        ),
        (
            "java.log.connection_timeout",
            "warn",
            "Java 外部依赖超时",
            &["read timed out", "connect timed out", "sockettimeoutexception"][..],
            "日志出现连接或读取超时关键词",
            &[
                "grep -RniE 'read timed out|connect timed out|SocketTimeoutException' <log_dir> | tail -80",
                "ss -antp | grep <pid> | head -80",
            ][..],
        ),
        (
            "java.log.connection_refused",
            "error",
            "Java 外部依赖连接拒绝",
            &["connection refused", "connectexception", "econnrefused"][..],
            "日志出现连接拒绝关键词",
            &[
                "grep -RniE 'Connection refused|ConnectException|ECONNREFUSED' <log_dir> | tail -80",
                "ss -ltnp",
            ][..],
        ),
    ];
    for (rule_id, level, title, keywords, summary, commands) in rules {
        if keywords.iter().any(|keyword| blob.contains(keyword)) {
            findings.push(
                AnomalyFinding::new(
                    rule_id,
                    level,
                    "Java服务",
                    title,
                    "java-log",
                    summary,
                )
                .evidence(keywords.iter().map(|keyword| format!("关键词: {}", keyword)))
                .suggestion("优先截取首次异常上下文，关联发布时间、流量峰值、下游状态和 JVM 运行时指标。")
                .commands(commands.iter().map(|cmd| (*cmd).to_string())),
            );
        }
    }
    findings
}

fn check_result_text(result: &CheckResult) -> String {
    let mut values = vec![
        result.id.clone(),
        result.name.clone(),
        result.description.clone(),
        result.category.clone(),
    ];
    for section in &result.sections {
        values.push(section.title.clone());
        if let Some(description) = &section.description {
            values.push(description.clone());
        }
        for item in &section.items {
            match item {
                Item::Label { key, value, status } => {
                    values.push(key.clone());
                    values.push(value.clone());
                    if let Some(status) = status {
                        values.push(status.clone());
                    }
                }
                Item::Table {
                    headers,
                    rows,
                    status,
                } => {
                    values.extend(headers.clone());
                    values.extend(rows.iter().flat_map(|row| row.clone()));
                    if let Some(status) = status {
                        values.push(status.clone());
                    }
                }
                Item::Bar {
                    key,
                    value,
                    max,
                    unit,
                    status,
                } => {
                    values.push(key.clone());
                    values.push(format!("{} {} {}", value, max, unit));
                    if let Some(status) = status {
                        values.push(status.clone());
                    }
                }
                Item::Sparkline {
                    key, unit, status, ..
                } => {
                    values.push(key.clone());
                    values.push(unit.clone());
                    if let Some(status) = status {
                        values.push(status.clone());
                    }
                }
                Item::Info { text }
                | Item::Warning { text }
                | Item::Error { text }
                | Item::Success { text } => values.push(text.clone()),
                Item::Finding {
                    rule_id,
                    level,
                    category,
                    title,
                    target,
                    summary,
                    evidence,
                    suggestion,
                    commands,
                } => {
                    values.extend([
                        rule_id.clone(),
                        level.clone(),
                        category.clone(),
                        title.clone(),
                        target.clone(),
                        summary.clone(),
                        suggestion.clone(),
                    ]);
                    values.extend(evidence.clone());
                    values.extend(commands.clone());
                }
                Item::Divider => {}
            }
        }
    }
    values.join("\n")
}

pub fn enrich_check_result(result: &mut CheckResult) -> Vec<AnomalyFinding> {
    let findings = evaluate_check_result(result);
    if let Some(section) = findings_section(&findings) {
        result.sections.retain(|s| s.title != "异常明细");
        result.sections.insert(0, section);
    }
    if findings.iter().any(|f| f.level == "error") {
        result.status = CheckStatus::Error;
    } else if findings.iter().any(|f| f.level == "warn") {
        result.status = CheckStatus::Warn;
    }
    findings
}

pub fn default_suggestion(check_id: &str, section: &str, key: &str, level: &str) -> String {
    match (check_id, key) {
        (_, "CPU") | (_, "使用率") => {
            "定位 CPU Top 进程，结合负载和业务时间点判断是否持续异常。".to_string()
        }
        (_, "内存") | (_, "物理内存") | (_, "Swap") => {
            "定位 RSS 最高进程，检查 OOM、堆内存、缓存和最近发布变更。".to_string()
        }
        (_, "磁盘") => "检查磁盘增长目录，优先清理日志/临时文件/备份并评估扩容。".to_string(),
        ("security", _) => "核对安全基线要求，确认是否允许该配置偏离默认加固策略。".to_string(),
        ("service", _) | ("service-manage", _) => {
            "查看服务状态、启动日志和端口占用，先定位原因再执行重启。".to_string()
        }
        _ => format!(
            "按 {} 级别处理 {} / {}，结合证据和现场业务影响确认优先级。",
            if level == "error" { "异常" } else { "警告" },
            section,
            key
        ),
    }
}

pub fn default_commands(check_id: &str, key: &str) -> Vec<String> {
    let mut commands = match key {
        "CPU" | "使用率" => vec![
            "top -o %CPU".to_string(),
            "ps -eo pid,ppid,comm,%cpu,%mem --sort=-%cpu | head -20".to_string(),
        ],
        "内存" | "物理内存" | "Swap" => vec![
            "free -h".to_string(),
            "ps -eo pid,comm,rss,%mem --sort=-rss | head -20".to_string(),
        ],
        "磁盘" => vec![
            "df -h".to_string(),
            "du -xh / 2>/dev/null | sort -h | tail -20".to_string(),
        ],
        _ => Vec::new(),
    };
    if check_id == "security" {
        commands.push(
            "sysctl -a 2>/dev/null | grep -E 'tcp_syncookies|rp_filter|randomize_va_space'"
                .to_string(),
        );
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::{DiskInfo, LoadAvg, ProcessInfo, SystemInfo};

    fn sample_system() -> SystemInfo {
        SystemInfo {
            cpu_usage: 85.0,
            memory_total: 100,
            memory_used: 85,
            memory_usage: 85.0,
            swap_total: 100,
            swap_used: 20,
            swap_usage: 20.0,
            disk_total: 100,
            disk_used: 70,
            disk_usage: 70.0,
            hostname: "host".to_string(),
            os: "linux".to_string(),
            kernel: "kernel".to_string(),
            arch: "x86_64".to_string(),
            cpu_count: 2,
            cpu_brand: "cpu".to_string(),
            uptime: 1,
            boot_time: 1,
            load_avg: LoadAvg {
                one: 3.0,
                five: 2.0,
                fifteen: 1.0,
            },
            process_count: 1,
            networks: vec![],
            disks: vec![DiskInfo {
                name: "disk".to_string(),
                mount_point: "/data".to_string(),
                total: 100,
                used: 91,
                usage: 91.0,
                fs_type: "ext4".to_string(),
            }],
            top_processes: vec![ProcessInfo {
                pid: 42,
                name: "java".to_string(),
                cpu_usage: 90.0,
                memory_bytes: 10,
            }],
        }
    }

    #[test]
    fn system_rules_return_specific_findings() {
        let findings = evaluate_system_info(&sample_system());
        assert!(findings.len() >= 4);
        assert!(findings.iter().any(|f| f.rule_id == "resource.cpu.warning"));
        assert!(findings.iter().any(|f| f.target == "/data"));
        assert!(findings.iter().all(|f| !f.summary.is_empty()));
        assert!(findings.iter().all(|f| !f.suggestion.is_empty()));
    }

    #[test]
    fn rule_catalog_covers_operational_domains() {
        let rules = rule_catalog();
        assert!(
            rules.len() >= 90,
            "规则目录太少，无法支撑现场复杂检查: {}",
            rules.len()
        );
        let mut ids = std::collections::HashSet::new();
        for rule in &rules {
            let id = rule["id"].as_str().unwrap_or("");
            assert!(ids.insert(id.to_string()), "规则 ID 重复: {}", id);
        }
        for category in [
            "资源水位",
            "磁盘分区",
            "进程异常",
            "服务管理",
            "日志异常",
            "数据库",
            "中间件",
            "Web网关",
            "异常类型",
            "网络链路",
            "内核异常",
            "容器运行时",
            "存储异常",
            "安全风险",
            "Java服务",
            "配置风险",
            "备份恢复",
            "调度任务",
            "检查结果",
        ] {
            assert!(
                rules.iter().any(|r| r["category"] == category),
                "缺少规则分类: {}",
                category
            );
        }
        assert!(rules
            .iter()
            .all(|r| r["id"].as_str().unwrap_or("").contains('.')));
        assert!(rules
            .iter()
            .all(|r| r["condition"].as_str().unwrap_or("").len() > 2));
        assert!(rules
            .iter()
            .all(|r| r["description"].as_str().unwrap_or("").len() > 10));
    }

    #[test]
    fn check_result_is_enriched_with_detail_section() {
        let mut result = CheckResult {
            id: "sample".to_string(),
            name: "sample".to_string(),
            description: String::new(),
            category: "test".to_string(),
            version: "1".to_string(),
            timestamp: String::new(),
            duration_ms: 0,
            status: CheckStatus::Ok,
            sections: vec![Section {
                title: "资源".to_string(),
                icon: None,
                description: None,
                items: vec![Item::Label {
                    key: "内存".to_string(),
                    value: "85%".to_string(),
                    status: Some("warn".to_string()),
                }],
            }],
        };
        let findings = enrich_check_result(&mut result);
        assert_eq!(findings.len(), 1);
        assert_eq!(result.sections[0].title, "异常明细");
        assert!(matches!(result.sections[0].items[0], Item::Finding { .. }));
    }

    #[test]
    fn middleware_specific_rules_create_actionable_nginx_findings() {
        let result = CheckResult {
            id: "nginx".to_string(),
            name: "nginx".to_string(),
            description: String::new(),
            category: "test".to_string(),
            version: "1".to_string(),
            timestamp: String::new(),
            duration_ms: 0,
            status: CheckStatus::Warn,
            sections: vec![Section {
                title: "连接与程序信息".to_string(),
                icon: None,
                description: None,
                items: vec![
                    Item::Label {
                        key: "运行状态".to_string(),
                        value: "运行中".to_string(),
                        status: Some("ok".to_string()),
                    },
                    Item::Label {
                        key: "配置检测".to_string(),
                        value: "nginx: [emerg] invalid number of arguments".to_string(),
                        status: Some("warn".to_string()),
                    },
                ],
            }],
        };
        let findings = evaluate_check_result(&result);
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "middleware.nginx.config_invalid"));
        assert!(findings
            .iter()
            .any(|f| f.commands.iter().any(|c| c == "nginx -t")));
    }

    #[test]
    fn java_runtime_rules_flag_hot_service() {
        let result = CheckResult {
            id: "java-service".to_string(),
            name: "java".to_string(),
            description: String::new(),
            category: "test".to_string(),
            version: "1".to_string(),
            timestamp: String::new(),
            duration_ms: 0,
            status: CheckStatus::Warn,
            sections: vec![Section {
                title: "Java 运行时".to_string(),
                icon: None,
                description: None,
                items: vec![Item::Table {
                    headers: vec![
                        "PID".to_string(),
                        "进程".to_string(),
                        "CPU%".to_string(),
                        "内存MB".to_string(),
                        "线程数".to_string(),
                        "状态".to_string(),
                        "JVM/启动参数".to_string(),
                    ],
                    rows: vec![vec![
                        "42".to_string(),
                        "order-service".to_string(),
                        "91.0".to_string(),
                        "5120.0".to_string(),
                        "900".to_string(),
                        "warn".to_string(),
                        "-Xmx4g order-service.jar".to_string(),
                    ]],
                    status: Some("warn".to_string()),
                }],
            }],
        };
        let findings = evaluate_check_result(&result);
        assert!(findings.iter().any(|f| f.rule_id == "java.runtime.42"));
        assert!(findings
            .iter()
            .any(|f| f.commands.iter().any(|c| c.contains("Thread.print"))));
    }

    #[test]
    fn java_runtime_rules_flag_heap_and_port_risks() {
        let result = CheckResult {
            id: "java-service".to_string(),
            name: "java".to_string(),
            description: String::new(),
            category: "test".to_string(),
            version: "1".to_string(),
            timestamp: String::new(),
            duration_ms: 0,
            status: CheckStatus::Warn,
            sections: vec![
                Section {
                    title: "Java 服务列表".to_string(),
                    icon: None,
                    description: None,
                    items: vec![Item::Table {
                        headers: vec![
                            "PID".to_string(),
                            "服务名".to_string(),
                            "CPU%".to_string(),
                            "内存MB".to_string(),
                            "线程数".to_string(),
                            "监听端口".to_string(),
                            "命令".to_string(),
                        ],
                        rows: vec![vec![
                            "42".to_string(),
                            "tomcat".to_string(),
                            "12.0".to_string(),
                            "1024.0".to_string(),
                            "180".to_string(),
                            "".to_string(),
                            "java -jar order-service.jar".to_string(),
                        ]],
                        status: Some("warn".to_string()),
                    }],
                },
                Section {
                    title: "Java 运行时".to_string(),
                    icon: None,
                    description: None,
                    items: vec![Item::Table {
                        headers: vec![
                            "PID".to_string(),
                            "进程".to_string(),
                            "CPU%".to_string(),
                            "内存MB".to_string(),
                            "线程数".to_string(),
                            "状态".to_string(),
                            "JVM/启动参数".to_string(),
                        ],
                        rows: vec![vec![
                            "42".to_string(),
                            "tomcat".to_string(),
                            "12.0".to_string(),
                            "1024.0".to_string(),
                            "180".to_string(),
                            "warn".to_string(),
                            "未识别到关键 JVM 参数".to_string(),
                        ]],
                        status: Some("warn".to_string()),
                    }],
                },
            ],
        };
        let findings = evaluate_check_result(&result);
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "java.runtime.heap_unbounded.42"));
        assert!(findings
            .iter()
            .any(|f| f.rule_id == "java.runtime.no_port.42"));
    }
}
