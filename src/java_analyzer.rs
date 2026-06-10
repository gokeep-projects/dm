use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct JavaProcess {
    pub pid: u32,
    pub name: String,
    pub service_name: String,
    pub display_name: String,
    pub exe: String,
    pub cwd: String,
    pub cmd: String,
    pub main: String,
    pub jar: String,
    pub user: String,
    pub state: String,
    pub threads: usize,
    pub cpu_ticks: u64,
    pub memory_bytes: u64,
    pub heap_flags: Vec<String>,
    pub ports: Vec<String>,
    pub start_time: String,
    pub uptime: String,
    pub score: u32,
    pub attach_available: bool,
    pub attach_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadFrame {
    pub class_method: String,
    pub file_line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadInfo {
    pub name: String,
    pub nid: Option<u32>,
    pub java_tid: Option<String>,
    pub state: String,
    pub top_frame: String,
    pub frames: Vec<ThreadFrame>,
    pub raw_header: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotFrame {
    pub method: String,
    pub count: usize,
    pub weight: usize,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadSample {
    pub timestamp: String,
    pub states: BTreeMap<String, usize>,
    pub runnable: usize,
    pub blocked: usize,
    pub waiting: usize,
    pub timed_waiting: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaFinding {
    pub level: String,
    pub title: String,
    pub detail: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaAnalysisLog {
    pub timestamp: String,
    pub level: String,
    pub stage: String,
    pub message: String,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaAnalysis {
    pub process: JavaProcess,
    pub progress: u8,
    pub runtime_logs: Vec<JavaAnalysisLog>,
    pub attach_ok: bool,
    pub attach_error: String,
    pub thread_dump: String,
    pub heap_info: String,
    pub class_histogram: String,
    pub threads: Vec<ThreadInfo>,
    pub hot_frames: Vec<HotFrame>,
    pub samples: Vec<ThreadSample>,
    pub findings: Vec<JavaFinding>,
    pub summary: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaAnalyzeRequest {
    pub pid: u32,
    pub samples: Option<u8>,
    pub interval_ms: Option<u64>,
    pub include_histogram: Option<bool>,
    pub cancel_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaFleetScanRequest {
    pub samples: Option<u8>,
    pub interval_ms: Option<u64>,
    pub include_histogram: Option<bool>,
    pub max_processes: Option<usize>,
    pub cancel_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JavaHeapDumpRequest {
    pub pid: u32,
    pub live: Option<bool>,
    pub cancel_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JavaHeapDump {
    pub path: PathBuf,
    pub filename: String,
    pub bytes: u64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaFleetFinding {
    pub pid: u32,
    pub service_name: String,
    pub level: String,
    pub title: String,
    pub detail: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaProcessRuleReport {
    pub process: JavaProcess,
    pub attach_ok: bool,
    pub error: String,
    pub thread_count: usize,
    pub hot_method: String,
    pub findings: Vec<JavaFinding>,
    pub summary: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JavaFleetScan {
    pub task_key: String,
    pub progress: u8,
    pub runtime_logs: Vec<JavaAnalysisLog>,
    pub total: usize,
    pub analyzed: usize,
    pub warnings: usize,
    pub errors: usize,
    pub reports: Vec<JavaProcessRuleReport>,
    pub findings: Vec<JavaFleetFinding>,
}

pub fn list_java_processes(query: Option<&str>) -> Vec<JavaProcess> {
    let ports = listen_port_index();
    let needle = query.unwrap_or_default().trim().to_lowercase();
    let mut processes: Vec<JavaProcess> = fs::read_dir("/proc")
        .ok()
        .into_iter()
        .flat_map(|entries| entries.flatten())
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            java_process_from_pid(pid, &ports)
        })
        .filter(|p| {
            if needle.is_empty() {
                return true;
            }
            format!(
                "{} {} {} {} {} {}",
                p.pid, p.name, p.exe, p.cwd, p.cmd, p.main
            )
            .to_lowercase()
            .contains(&needle)
        })
        .collect();
    processes.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.pid.cmp(&b.pid)));
    processes
}

pub fn analyze_java(req: JavaAnalyzeRequest) -> Result<JavaAnalysis> {
    analyze_java_with_cancel(req, None)
}

pub fn analyze_java_with_cancel(
    req: JavaAnalyzeRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<JavaAnalysis> {
    let mut runtime_logs = Vec::new();
    check_cancelled(&cancel)?;
    push_log(
        &mut runtime_logs,
        "info",
        "discover",
        "开始定位 Java 进程",
        3,
    );
    let ports = listen_port_index();
    let process = java_process_from_pid(req.pid, &ports)
        .ok_or_else(|| anyhow!("未找到 Java 进程 {}", req.pid))?;
    check_cancelled(&cancel)?;
    push_log(
        &mut runtime_logs,
        "ok",
        "discover",
        &format!(
            "已锁定 {} / PID {} / {}",
            process.display_name, process.pid, process.user
        ),
        8,
    );
    let samples = req.samples.unwrap_or(3).clamp(1, 12);
    let interval = Duration::from_millis(req.interval_ms.unwrap_or(900).clamp(200, 5000));
    let include_histogram = req.include_histogram.unwrap_or(true);

    let mut thread_dump = String::new();
    let mut heap_info = String::new();
    let mut class_histogram = String::new();
    let mut attach_ok = false;
    let mut attach_error = String::new();

    push_log(
        &mut runtime_logs,
        "info",
        "attach",
        "正在通过 HotSpot Attach 协议连接 JVM",
        14,
    );
    check_cancelled(&cancel)?;
    match attach_jcmd(req.pid, "Thread.print -l") {
        Ok(out) => {
            attach_ok = true;
            thread_dump = out;
            push_log(
                &mut runtime_logs,
                "ok",
                "attach",
                "JVM Attach 连接成功，已获取首轮线程转储",
                26,
            );
        }
        Err(err) => {
            attach_error = err.to_string();
            push_log(
                &mut runtime_logs,
                "error",
                "attach",
                &format!("JVM Attach 失败: {attach_error}"),
                26,
            );
        }
    }
    if attach_ok {
        check_cancelled(&cancel)?;
        push_log(
            &mut runtime_logs,
            "info",
            "heap",
            "读取 GC.heap_info 堆概要",
            34,
        );
        heap_info = attach_jcmd(req.pid, "GC.heap_info")
            .unwrap_or_else(|e| format!("GC.heap_info 失败: {e}"));
        push_log(
            &mut runtime_logs,
            if heap_info.contains("失败") {
                "warn"
            } else {
                "ok"
            },
            "heap",
            "堆概要读取完成",
            42,
        );
        if include_histogram {
            check_cancelled(&cancel)?;
            push_log(
                &mut runtime_logs,
                "info",
                "histogram",
                "读取 GC.class_histogram 类直方图",
                48,
            );
            class_histogram = attach_jcmd(req.pid, "GC.class_histogram")
                .unwrap_or_else(|e| format!("GC.class_histogram 失败: {e}"));
            push_log(
                &mut runtime_logs,
                if class_histogram.contains("失败") {
                    "warn"
                } else {
                    "ok"
                },
                "histogram",
                "类直方图读取完成",
                56,
            );
        }
    }

    let mut sample_dumps = Vec::new();
    let mut thread_samples = Vec::new();
    if !thread_dump.trim().is_empty() {
        sample_dumps.push(thread_dump.clone());
        thread_samples.push(sample_from_dump(&thread_dump));
    }
    for _ in thread_samples.len()..samples as usize {
        check_cancelled(&cancel)?;
        let idx = thread_samples.len() + 1;
        let percent = 58 + ((idx as u8).saturating_mul(24) / samples.max(1));
        push_log(
            &mut runtime_logs,
            "info",
            "sampling",
            &format!("执行第 {idx}/{samples} 次线程采样"),
            percent.min(84),
        );
        sleep_cancellable(interval, &cancel)?;
        if let Ok(out) = attach_jcmd(req.pid, "Thread.print -l") {
            sample_dumps.push(out.clone());
            thread_samples.push(sample_from_dump(&out));
            if thread_dump.trim().is_empty() {
                thread_dump = out;
            }
        } else {
            push_log(
                &mut runtime_logs,
                "warn",
                "sampling",
                "后续采样失败，使用已获取的采样结果继续分析",
                percent.min(84),
            );
            break;
        }
    }

    push_log(
        &mut runtime_logs,
        "info",
        "parse",
        "解析线程状态、热点方法和诊断结论",
        88,
    );
    check_cancelled(&cancel)?;
    let threads = parse_thread_dump(&thread_dump);
    let hot_frames = aggregate_hot_frames_from_dumps(&sample_dumps);
    let findings = build_findings(
        &process,
        &threads,
        &hot_frames,
        &heap_info,
        &class_histogram,
        attach_ok,
        &attach_error,
    );
    let mut summary = BTreeMap::new();
    summary.insert("线程数".to_string(), threads.len().to_string());
    summary.insert(
        "RUNNABLE".to_string(),
        count_state(&threads, "RUNNABLE").to_string(),
    );
    summary.insert(
        "BLOCKED".to_string(),
        count_state(&threads, "BLOCKED").to_string(),
    );
    summary.insert(
        "WAITING".to_string(),
        count_state(&threads, "WAITING").to_string(),
    );
    summary.insert(
        "TIMED_WAITING".to_string(),
        count_state(&threads, "TIMED_WAITING").to_string(),
    );
    summary.insert(
        "热点方法".to_string(),
        hot_frames
            .first()
            .map(|f| f.method.clone())
            .unwrap_or_else(|| "-".to_string()),
    );
    summary.insert(
        "Attach".to_string(),
        if attach_ok { "可用" } else { "不可用" }.to_string(),
    );
    summary.insert("服务名".to_string(), process.service_name.clone());
    summary.insert("监听端口".to_string(), process.ports.join(", "));
    push_log(
        &mut runtime_logs,
        "ok",
        "done",
        &format!(
            "分析完成: {} 个线程，{} 个热点方法，{} 条诊断结论",
            threads.len(),
            hot_frames.len(),
            findings.len()
        ),
        100,
    );

    Ok(JavaAnalysis {
        process,
        progress: 100,
        runtime_logs,
        attach_ok,
        attach_error,
        thread_dump,
        heap_info,
        class_histogram,
        threads,
        hot_frames,
        samples: thread_samples,
        findings,
        summary,
    })
}

pub fn scan_java_process_rules(
    req: JavaFleetScanRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<JavaFleetScan> {
    let mut runtime_logs = Vec::new();
    push_log(
        &mut runtime_logs,
        "info",
        "discover",
        "开始扫描所有运行中的 Java 进程",
        4,
    );
    check_cancelled(&cancel)?;
    let mut processes = list_java_processes(None);
    let max_processes = req.max_processes.unwrap_or(64).clamp(1, 256);
    if processes.len() > max_processes {
        processes.truncate(max_processes);
        push_log(
            &mut runtime_logs,
            "warn",
            "discover",
            &format!("Java 进程数量较多，本轮限制扫描前 {max_processes} 个高优先级进程"),
            8,
        );
    }
    let total = processes.len();
    let mut reports = Vec::new();
    let mut findings = Vec::new();
    let mut warnings = 0usize;
    let mut errors = 0usize;

    for (idx, process) in processes.into_iter().enumerate() {
        check_cancelled(&cancel)?;
        let percent = if total == 0 {
            100
        } else {
            10 + (((idx + 1) * 84) / total).min(84) as u8
        };
        push_log(
            &mut runtime_logs,
            "info",
            "rules",
            &format!(
                "规则引擎扫描 {}/{}: {} / PID {}",
                idx + 1,
                total,
                process.service_name,
                process.pid
            ),
            percent,
        );
        let analysis = analyze_java_with_cancel(
            JavaAnalyzeRequest {
                pid: process.pid,
                samples: Some(req.samples.unwrap_or(2).clamp(1, 4)),
                interval_ms: Some(req.interval_ms.unwrap_or(240).clamp(180, 800)),
                include_histogram: Some(req.include_histogram.unwrap_or(true)),
                cancel_key: req.cancel_key.clone(),
            },
            cancel.clone(),
        );
        match analysis {
            Ok(analysis) => {
                let report_findings = analysis.findings.clone();
                for finding in &report_findings {
                    if finding.level == "error" {
                        errors += 1;
                    } else if finding.level == "warn" {
                        warnings += 1;
                    }
                    findings.push(JavaFleetFinding {
                        pid: analysis.process.pid,
                        service_name: analysis.process.service_name.clone(),
                        level: finding.level.clone(),
                        title: finding.title.clone(),
                        detail: finding.detail.clone(),
                        suggestion: finding.suggestion.clone(),
                    });
                }
                reports.push(JavaProcessRuleReport {
                    process: analysis.process,
                    attach_ok: analysis.attach_ok,
                    error: analysis.attach_error,
                    thread_count: analysis.threads.len(),
                    hot_method: analysis
                        .hot_frames
                        .first()
                        .map(|f| f.method.clone())
                        .unwrap_or_default(),
                    findings: report_findings,
                    summary: analysis.summary,
                });
            }
            Err(err) => {
                errors += 1;
                findings.push(JavaFleetFinding {
                    pid: process.pid,
                    service_name: process.service_name.clone(),
                    level: "error".into(),
                    title: "Java 进程规则扫描失败".into(),
                    detail: err.to_string(),
                    suggestion: "确认进程仍在运行、权限一致、Attach 未被禁用，并重新扫描。".into(),
                });
                reports.push(JavaProcessRuleReport {
                    process,
                    attach_ok: false,
                    error: err.to_string(),
                    thread_count: 0,
                    hot_method: String::new(),
                    findings: Vec::new(),
                    summary: BTreeMap::new(),
                });
            }
        }
    }
    push_log(
        &mut runtime_logs,
        "ok",
        "done",
        &format!(
            "规则扫描完成: {} 个 Java 进程，{} 条警告，{} 条错误",
            total, warnings, errors
        ),
        100,
    );
    Ok(JavaFleetScan {
        task_key: req.cancel_key.unwrap_or_default(),
        progress: 100,
        runtime_logs,
        total,
        analyzed: reports.len(),
        warnings,
        errors,
        reports,
        findings,
    })
}

pub fn dump_java_hprof(
    req: JavaHeapDumpRequest,
    cancel: Option<Arc<AtomicBool>>,
) -> Result<JavaHeapDump> {
    check_cancelled(&cancel)?;
    let ports = listen_port_index();
    let process = java_process_from_pid(req.pid, &ports)
        .ok_or_else(|| anyhow!("未找到 Java 进程 {}", req.pid))?;
    check_cancelled(&cancel)?;
    let ts = chrono::Local::now().format("%Y%m%d%H%M%S");
    let filename = format!(
        "dm-java-heap-{}-{}-{ts}.hprof",
        process.service_name.replace(['/', '\\', ' ', ':'], "_"),
        process.pid
    );
    let path = std::env::temp_dir().join(&filename);
    let _ = fs::remove_file(&path);
    let command = if req.live.unwrap_or(true) {
        format!("GC.heap_dump {}", path.display())
    } else {
        format!("GC.heap_dump -all {}", path.display())
    };
    let message = attach_jcmd_timeout(req.pid, &command, Duration::from_secs(180))
        .with_context(|| format!("生成 HPROF 快照失败: {}", process.display_name))?;
    check_cancelled(&cancel)?;
    let meta =
        fs::metadata(&path).with_context(|| format!("HPROF 文件未生成: {}", path.display()))?;
    if meta.len() == 0 {
        return Err(anyhow!("HPROF 文件为空: {}", path.display()));
    }
    Ok(JavaHeapDump {
        path,
        filename,
        bytes: meta.len(),
        message,
    })
}

fn check_cancelled(cancel: &Option<Arc<AtomicBool>>) -> Result<()> {
    if cancel
        .as_ref()
        .is_some_and(|flag| flag.load(Ordering::SeqCst))
    {
        return Err(anyhow!("Java 分析任务已取消"));
    }
    Ok(())
}

fn sleep_cancellable(duration: Duration, cancel: &Option<Arc<AtomicBool>>) -> Result<()> {
    let start = Instant::now();
    while start.elapsed() < duration {
        check_cancelled(cancel)?;
        let remaining = duration.saturating_sub(start.elapsed());
        std::thread::sleep(remaining.min(Duration::from_millis(80)));
    }
    check_cancelled(cancel)
}

fn java_process_from_pid(pid: u32, ports: &HashMap<u32, Vec<String>>) -> Option<JavaProcess> {
    let base = PathBuf::from(format!("/proc/{pid}"));
    let cmd = read_cmdline(pid)?;
    let name = fs::read_to_string(base.join("comm"))
        .unwrap_or_default()
        .trim()
        .to_string();
    if !is_java_process(&name, &cmd) {
        return None;
    }
    let exe = fs::read_link(base.join("exe"))
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let cwd = fs::read_link(base.join("cwd"))
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let status = fs::read_to_string(base.join("status")).unwrap_or_default();
    let memory_bytes = status_value_kb(&status, "VmRSS:")
        .unwrap_or(0)
        .saturating_mul(1024);
    let threads = status_value_kb(&status, "Threads:").unwrap_or(0) as usize;
    let state = status_line_value(&status, "State:").unwrap_or_default();
    let user = uid_to_user(
        status_line_value(&status, "Uid:")
            .unwrap_or_default()
            .split_whitespace()
            .next()
            .unwrap_or_default(),
    );
    let (cpu_ticks, _) = proc_stat_ticks(pid).unwrap_or((0, 0));
    let (start_time, uptime) = proc_start_time(pid).unwrap_or_else(|| ("-".into(), "-".into()));
    let heap_flags = cmd
        .split_whitespace()
        .filter(|t| {
            t.starts_with("-Xmx")
                || t.starts_with("-Xms")
                || t.starts_with("-XX:")
                || t.starts_with("-Dspring.profiles.active")
                || t.starts_with("-Dserver.port")
        })
        .take(16)
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let jar = cmd
        .split_whitespace()
        .find(|t| t.ends_with(".jar"))
        .unwrap_or_default()
        .to_string();
    let main = infer_main(&cmd, &jar);
    let service_name = infer_service_name(&cmd, &main, &jar, pid);
    let display_name = if !service_name.is_empty() {
        service_name.clone()
    } else if !main.is_empty() {
        main.clone()
    } else if !jar.is_empty() {
        jar.clone()
    } else {
        name.clone()
    };
    let attach_path = attach_socket_path(pid)
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let attach_available = !attach_path.is_empty();
    let mut score = 20;
    if !jar.is_empty() {
        score += 30;
    }
    if !ports.get(&pid).cloned().unwrap_or_default().is_empty() {
        score += 20;
    }
    if cmd.contains("spring") || cmd.contains("tomcat") || cmd.contains("catalina") {
        score += 20;
    }
    if memory_bytes > 1024 * 1024 * 1024 {
        score += 10;
    }
    Some(JavaProcess {
        pid,
        name,
        service_name,
        display_name,
        exe,
        cwd,
        cmd,
        main,
        jar,
        user,
        state: state.to_string(),
        threads,
        cpu_ticks,
        memory_bytes,
        heap_flags,
        ports: ports.get(&pid).cloned().unwrap_or_default(),
        start_time,
        uptime,
        score,
        attach_available,
        attach_path,
    })
}

fn attach_jcmd(pid: u32, command: &str) -> Result<String> {
    attach_jcmd_timeout(pid, command, Duration::from_secs(10))
}

fn attach_jcmd_timeout(pid: u32, command: &str, read_timeout: Duration) -> Result<String> {
    ensure_attach_socket(pid)?;
    let socket = attach_socket_path(pid).ok_or_else(|| anyhow!("未找到 JVM attach socket"))?;
    let mut stream = UnixStream::connect(&socket)
        .with_context(|| format!("连接 attach socket 失败: {}", socket.display()))?;
    stream.set_read_timeout(Some(read_timeout)).ok();
    stream.set_write_timeout(Some(Duration::from_secs(3))).ok();
    let payload = format!("1\0jcmd\0{}\0\0\0", command);
    stream.write_all(payload.as_bytes())?;
    let mut out = String::new();
    stream.read_to_string(&mut out)?;
    Ok(out)
}

fn ensure_attach_socket(pid: u32) -> Result<()> {
    if attach_socket_path(pid).is_some() {
        return Ok(());
    }
    let marker = PathBuf::from(format!("/proc/{pid}/cwd/.attach_pid{pid}"));
    fs::write(&marker, "").or_else(|_| fs::write(format!("/tmp/.attach_pid{pid}"), ""))?;
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGQUIT);
    }
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(6) {
        if attach_socket_path(pid).is_some() {
            let _ = fs::remove_file(&marker);
            let _ = fs::remove_file(format!("/tmp/.attach_pid{pid}"));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    Err(anyhow!(
        "JVM 未创建 attach socket，可能禁用了 Attach、权限不足或不是 HotSpot JVM"
    ))
}

fn attach_socket_path(pid: u32) -> Option<PathBuf> {
    let candidates = [
        PathBuf::from(format!("/tmp/.java_pid{pid}")),
        PathBuf::from(format!("/proc/{pid}/root/tmp/.java_pid{pid}")),
    ];
    candidates.into_iter().find(|p| p.exists())
}

fn parse_thread_dump(text: &str) -> Vec<ThreadInfo> {
    let mut threads = Vec::new();
    let mut current: Option<ThreadInfo> = None;
    for line in text.lines() {
        if line.starts_with('"') {
            if let Some(t) = current.take() {
                threads.push(t);
            }
            current = Some(ThreadInfo {
                name: line.split('"').nth(1).unwrap_or("unknown").to_string(),
                nid: parse_nid(line),
                java_tid: token_after(line, "tid=").map(str::to_string),
                state: "UNKNOWN".to_string(),
                top_frame: String::new(),
                frames: Vec::new(),
                raw_header: line.to_string(),
            });
        } else if let Some(t) = current.as_mut() {
            let trimmed = line.trim();
            if let Some(state) = trimmed.strip_prefix("java.lang.Thread.State:") {
                t.state = state
                    .trim()
                    .split_whitespace()
                    .next()
                    .unwrap_or("UNKNOWN")
                    .to_string();
            } else if let Some(frame) = trimmed.strip_prefix("at ") {
                let (method, file_line) = split_frame(frame);
                if t.top_frame.is_empty() {
                    t.top_frame = method.clone();
                }
                t.frames.push(ThreadFrame {
                    class_method: method,
                    file_line,
                });
            }
        }
    }
    if let Some(t) = current {
        threads.push(t);
    }
    threads
}

fn aggregate_hot_frames_from_dumps(dumps: &[String]) -> Vec<HotFrame> {
    let mut counts: HashMap<String, (usize, usize)> = HashMap::new();
    for dump in dumps {
        for thread in parse_thread_dump(dump) {
            for (idx, frame) in thread.frames.iter().take(18).enumerate() {
                if is_noise_frame(&frame.class_method) {
                    continue;
                }
                let base: usize = if thread.state == "RUNNABLE" { 10 } else { 4 };
                let weight = base.saturating_sub(idx.min(8)).max(1);
                let entry = counts.entry(frame.class_method.clone()).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += weight;
            }
        }
    }
    let mut frames = counts
        .into_iter()
        .map(|(method, (count, weight))| HotFrame {
            category: method_category(&method),
            method,
            count,
            weight,
        })
        .collect::<Vec<_>>();
    frames.sort_by(|a, b| b.weight.cmp(&a.weight).then_with(|| b.count.cmp(&a.count)));
    frames.truncate(40);
    frames
}

fn sample_from_dump(text: &str) -> ThreadSample {
    let threads = parse_thread_dump(text);
    let mut states = BTreeMap::new();
    for t in &threads {
        *states.entry(t.state.clone()).or_insert(0) += 1;
    }
    ThreadSample {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        runnable: *states.get("RUNNABLE").unwrap_or(&0),
        blocked: *states.get("BLOCKED").unwrap_or(&0),
        waiting: *states.get("WAITING").unwrap_or(&0),
        timed_waiting: *states.get("TIMED_WAITING").unwrap_or(&0),
        states,
    }
}

fn push_log(logs: &mut Vec<JavaAnalysisLog>, level: &str, stage: &str, message: &str, percent: u8) {
    logs.push(JavaAnalysisLog {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        level: level.to_string(),
        stage: stage.to_string(),
        message: message.to_string(),
        percent: percent.min(100),
    });
}

fn build_findings(
    process: &JavaProcess,
    threads: &[ThreadInfo],
    hot: &[HotFrame],
    heap: &str,
    _histogram: &str,
    attach_ok: bool,
    attach_error: &str,
) -> Vec<JavaFinding> {
    let mut findings = Vec::new();
    if !attach_ok {
        findings.push(JavaFinding { level: "error".into(), title: "无法读取 JVM 运行时".into(), detail: attach_error.into(), suggestion: "确认运行用户和 Java 进程用户一致，JVM 未设置 -XX:+DisableAttachMechanism，容器 /tmp 可见。".into() });
        return findings;
    }
    let blocked = count_state(threads, "BLOCKED");
    if blocked > 0 {
        findings.push(JavaFinding { level: "warn".into(), title: "存在 BLOCKED 线程".into(), detail: format!("{blocked} 个线程正在等待锁，可能存在锁竞争或同步块耗时。"), suggestion: "查看 BLOCKED 线程顶部栈，重点检查 synchronized、ReentrantLock、连接池借还和缓存锁。".into() });
    }
    let runnable = count_state(threads, "RUNNABLE");
    if runnable >= 16 {
        findings.push(JavaFinding {
            level: "warn".into(),
            title: "RUNNABLE 线程偏多".into(),
            detail: format!("{runnable} 个线程处于 RUNNABLE，可能是 CPU 热点、忙等或高并发请求。"),
            suggestion: "优先查看热点方法列表，连续采样对比顶部方法是否稳定出现。".into(),
        });
    }
    if let Some(top) = hot.first() {
        findings.push(JavaFinding {
            level: "info".into(),
            title: "最可疑耗时方法".into(),
            detail: format!("{} 出现 {} 次，权重 {}", top.method, top.count, top.weight),
            suggestion:
                "如果该方法连续多次位于顶部，优先检查接口实现、SQL/HTTP 调用、锁等待或循环逻辑。"
                    .into(),
        });
    }
    if let Some(deepest) = threads.iter().max_by_key(|thread| thread.frames.len()) {
        if deepest.frames.len() >= 80 {
            findings.push(JavaFinding {
                level: "warn".into(),
                title: "调用栈深度异常".into(),
                detail: format!(
                    "线程 {} 当前调用栈深度 {} 层，顶部方法 {}。",
                    deepest.name,
                    deepest.frames.len(),
                    deepest.top_frame
                ),
                suggestion: "检查递归、AOP/代理链、序列化/反序列化嵌套、模板渲染或复杂业务编排是否导致栈过深。"
                    .into(),
            });
        }
    }
    let external_hot = hot.iter().find(|frame| {
        matches!(
            frame.category.as_str(),
            "外部调用" | "数据库" | "Redis" | "搜索引擎"
        )
    });
    if let Some(frame) = external_hot {
        findings.push(JavaFinding {
            level: "warn".into(),
            title: "外部依赖调用热点".into(),
            detail: format!(
                "{} 在采样中出现 {} 次，分类为 {}，权重 {}。",
                frame.method, frame.count, frame.category, frame.weight
            ),
            suggestion:
                "优先检查慢 SQL、HTTP/Redis/ES 超时、连接池等待、网络抖动、批量大小和重试风暴。"
                    .into(),
        });
    }
    let repeated_top = hot
        .first()
        .filter(|frame| frame.count >= 3 && frame.weight >= 20);
    if let Some(frame) = repeated_top {
        findings.push(JavaFinding {
            level: "warn".into(),
            title: "热点方法持续聚集".into(),
            detail: format!(
                "{} 在多轮采样中稳定出现，命中 {} 次，权重 {}。",
                frame.method, frame.count, frame.weight
            ),
            suggestion: "开启实时跟踪观察该方法是否持续居首；若稳定出现，建议定位对应接口、任务或消费线程的耗时来源。"
                .into(),
        });
    }
    let dump_text = threads
        .iter()
        .map(|thread| thread.raw_header.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    if dump_text.contains("deadlock") {
        findings.push(JavaFinding {
            level: "error".into(),
            title: "疑似死锁线索".into(),
            detail: "线程转储中出现 deadlock 关键字。".into(),
            suggestion: "立即导出完整线程转储，定位互相持有的 monitor/lock，并按锁顺序、超时获取或无锁结构改造。".into(),
        });
    }
    if !process.heap_flags.iter().any(|f| {
        f.to_lowercase().starts_with("-xmx") || f.to_lowercase().contains("maxrampercentage")
    }) {
        findings.push(JavaFinding {
            level: "warn".into(),
            title: "未识别到最大堆限制".into(),
            detail: "启动参数中未发现 -Xmx 或 MaxRAMPercentage。".into(),
            suggestion: "生产进程建议明确堆上限，避免容器/宿主机内存压力下被 OOM Killer 终止。"
                .into(),
        });
    }
    let lower_heap = heap.to_lowercase();
    if lower_heap.contains("outofmemory") || lower_heap.contains("java.lang.outofmemoryerror") {
        findings.push(JavaFinding { level: "error".into(), title: "发现 OOM 痕迹".into(), detail: "JVM 诊断输出中包含 OutOfMemoryError。".into(), suggestion: "结合类直方图排名检查 byte[]、char[]、集合、缓存对象和业务实体是否异常增长。".into() });
    }
    if process.memory_bytes > 0 && process.memory_bytes > 8 * 1024 * 1024 * 1024 {
        findings.push(JavaFinding { level: "warn".into(), title: "RSS 内存较高".into(), detail: format!("当前 RSS 约 {:.1} GB。", process.memory_bytes as f64 / 1024.0 / 1024.0 / 1024.0), suggestion: "比较堆信息与 RSS 差异，若 RSS 明显大于堆，检查 DirectBuffer、线程栈、JNI/native 内存。".into() });
    }
    findings
}

fn count_state(threads: &[ThreadInfo], state: &str) -> usize {
    threads.iter().filter(|t| t.state == state).count()
}

fn read_cmdline(pid: u32) -> Option<String> {
    let bytes = fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let cmd = bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Some(cmd)
}

fn is_java_process(name: &str, cmd: &str) -> bool {
    let lower_name = name.to_lowercase();
    if matches!(
        lower_name.as_str(),
        "java" | "javaw" | "jsvc" | "catalina" | "tomcat"
    ) {
        return true;
    }
    if lower_name.ends_with(".jar") {
        return true;
    }
    let tokens = cmd.split_whitespace().collect::<Vec<_>>();
    let first_base = tokens
        .first()
        .map(|token| {
            Path::new(token)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or(token)
                .to_lowercase()
        })
        .unwrap_or_default();
    if matches!(first_base.as_str(), "java" | "javaw" | "jsvc") {
        return true;
    }
    tokens.iter().enumerate().any(|(idx, token)| {
        let lower = token.to_lowercase();
        (lower.ends_with(".jar")
            && idx > 0
            && tokens
                .get(idx.saturating_sub(1))
                .is_some_and(|prev| prev.eq_ignore_ascii_case("-jar")))
            || lower.contains("/tomcat/")
            || lower.contains("/catalina.")
            || (idx > 0 && lower == "org.apache.catalina.startup.bootstrap")
    })
}

fn infer_main(cmd: &str, jar: &str) -> String {
    if !jar.is_empty() {
        return Path::new(jar)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(jar)
            .to_string();
    }
    let tokens = cmd.split_whitespace().collect::<Vec<_>>();
    let mut seen_java = false;
    let mut skip_next = false;
    for token in tokens {
        let base = Path::new(token)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(token)
            .to_ascii_lowercase();
        if !seen_java {
            if base == "java" || base == "javaw" || token.contains("/java") {
                seen_java = true;
            }
            continue;
        }
        if skip_next {
            skip_next = false;
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if matches!(
            lower.as_str(),
            "-cp" | "-classpath" | "--class-path" | "--module-path" | "-p"
        ) {
            skip_next = true;
            continue;
        }
        if lower == "-jar" {
            skip_next = true;
            continue;
        }
        if token.starts_with("-") {
            continue;
        }
        return token.to_string();
    }
    String::new()
}

fn infer_service_name(cmd: &str, main: &str, jar: &str, pid: u32) -> String {
    let tokens = cmd.split_whitespace().collect::<Vec<_>>();
    for key in [
        "-Dspring.application.name=",
        "-Dapp.name=",
        "-Dservice.name=",
        "-Dproject.name=",
        "--spring.application.name=",
        "--server.servlet.context-path=",
    ] {
        if let Some(value) = tokens
            .iter()
            .find_map(|token| token.strip_prefix(key).map(str::trim))
            .filter(|value| !value.is_empty())
        {
            return value.trim_matches('"').trim_start_matches('/').to_string();
        }
    }
    if !jar.is_empty() {
        return Path::new(jar)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(jar)
            .to_string();
    }
    if !main.is_empty() {
        return main.rsplit('.').next().unwrap_or(main).to_string();
    }
    format!("java-{pid}")
}

fn status_value_kb(status: &str, key: &str) -> Option<u64> {
    status.lines().find_map(|line| {
        line.strip_prefix(key)?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}

fn status_line_value<'a>(status: &'a str, key: &str) -> Option<&'a str> {
    status
        .lines()
        .find_map(|line| line.strip_prefix(key).map(str::trim))
}

fn uid_to_user(uid: &str) -> String {
    let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
    passwd
        .lines()
        .find_map(|line| {
            let parts = line.split(':').collect::<Vec<_>>();
            if parts.len() > 2 && parts[2] == uid {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| uid.to_string())
}

fn proc_stat_ticks(pid: u32) -> Option<(u64, u64)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(") ")?.1;
    let fields = after.split_whitespace().collect::<Vec<_>>();
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    Some((utime + stime, 0))
}

fn proc_start_time(pid: u32) -> Option<(String, String)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = stat.rsplit_once(") ")?.1;
    let fields = after.split_whitespace().collect::<Vec<_>>();
    let start_ticks = fields.get(19)?.parse::<u64>().ok()?;
    let boot_secs = fs::read_to_string("/proc/stat")
        .ok()?
        .lines()
        .find_map(|line| line.strip_prefix("btime "))?
        .trim()
        .parse::<u64>()
        .ok()?;
    let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as u64;
    let ticks_per_second = ticks_per_second.max(1);
    let start_secs = boot_secs.saturating_add(start_ticks / ticks_per_second);
    let now = chrono::Local::now().timestamp().max(0) as u64;
    let uptime_secs = now.saturating_sub(start_secs);
    let dt = chrono::DateTime::<chrono::Local>::from(
        std::time::UNIX_EPOCH + Duration::from_secs(start_secs),
    );
    Some((
        dt.format("%Y-%m-%d %H:%M:%S").to_string(),
        format_duration(uptime_secs),
    ))
}

fn format_duration(total_secs: u64) -> String {
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    if days > 0 {
        format!("{days}天 {hours}时 {minutes}分")
    } else if hours > 0 {
        format!("{hours}时 {minutes}分")
    } else {
        format!("{minutes}分")
    }
}

fn listen_port_index() -> HashMap<u32, Vec<String>> {
    let socket_ports = socket_inode_ports();
    let mut map: HashMap<u32, Vec<String>> = HashMap::new();
    if socket_ports.is_empty() {
        return map;
    }
    for entry in fs::read_dir("/proc").ok().into_iter().flatten().flatten() {
        let Some(pid) = entry.file_name().to_string_lossy().parse::<u32>().ok() else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let mut ports = HashSet::new();
        for fd in fs::read_dir(fd_dir).ok().into_iter().flatten().flatten() {
            let Ok(target) = fs::read_link(fd.path()) else {
                continue;
            };
            let target = target.to_string_lossy();
            let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|v| v.strip_suffix(']'))
            else {
                continue;
            };
            if let Some(port) = socket_ports.get(inode) {
                ports.insert(port.clone());
            }
        }
        if !ports.is_empty() {
            map.insert(pid, ports.into_iter().collect());
        }
    }
    for ports in map.values_mut() {
        ports.sort();
        ports.dedup();
    }
    map
}

fn socket_inode_ports() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for line in text.lines().skip(1) {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            let Some(local) = fields.get(1) else {
                continue;
            };
            let Some(state) = fields.get(3) else {
                continue;
            };
            if *state != "0A" {
                continue;
            }
            let Some(inode) = fields.get(9) else {
                continue;
            };
            let Some(port_hex) = local.rsplit_once(':').map(|(_, port)| port) else {
                continue;
            };
            if let Ok(port) = u16::from_str_radix(port_hex, 16) {
                map.insert((*inode).to_string(), port.to_string());
            }
        }
    }
    map
}

fn parse_nid(line: &str) -> Option<u32> {
    let raw = token_after(line, "nid=")?;
    u32::from_str_radix(raw.trim_start_matches("0x"), 16)
        .ok()
        .or_else(|| raw.parse().ok())
}

fn token_after<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(key)? + key.len();
    Some(line[start..].split_whitespace().next()?.trim_matches('"'))
}

fn split_frame(frame: &str) -> (String, String) {
    if let Some((method, rest)) = frame.split_once('(') {
        (method.to_string(), rest.trim_end_matches(')').to_string())
    } else {
        (frame.to_string(), String::new())
    }
}

fn is_noise_frame(method: &str) -> bool {
    method.starts_with("java.lang.Thread.")
        || method.starts_with("jdk.internal.")
        || method.starts_with("sun.nio.")
        || method.starts_with("java.util.concurrent.locks.LockSupport")
        || method.starts_with("java.net.Socket")
}

fn method_category(method: &str) -> String {
    let lower = method.to_lowercase();
    if lower.contains("controller") || lower.contains("servlet") || lower.contains("filter") {
        "接口入口".into()
    } else if lower.contains("jdbc")
        || lower.contains("mybatis")
        || lower.contains("hibernate")
        || lower.contains("repository")
        || lower.contains("dao")
    {
        "数据库".into()
    } else if lower.contains("redis")
        || lower.contains("jedis")
        || lower.contains("lettuce")
        || lower.contains("redisson")
    {
        "Redis".into()
    } else if lower.contains("elasticsearch")
        || lower.contains("opensearch")
        || lower.contains("resthighlevel")
        || lower.contains("elastic")
    {
        "搜索引擎".into()
    } else if lower.contains("http")
        || lower.contains("okhttp")
        || lower.contains("feign")
        || lower.contains("resttemplate")
    {
        "外部调用".into()
    } else if lower.contains("lock") || lower.contains("sync") {
        "锁竞争".into()
    } else {
        "业务/框架".into()
    }
}

#[cfg(test)]
mod tests {
    use super::{aggregate_hot_frames_from_dumps, build_findings, parse_thread_dump, JavaProcess};

    #[test]
    fn parses_thread_dump_and_hot_frames() {
        let dump = r#""http-nio-8080-exec-1" #31 nid=0x2f runnable
   java.lang.Thread.State: RUNNABLE
        at com.demo.OrderController.query(OrderController.java:42)
        at com.demo.OrderService.load(OrderService.java:88)

"worker" #32 nid=0x30 waiting
   java.lang.Thread.State: WAITING
        at java.util.concurrent.locks.LockSupport.park(LockSupport.java:1)
        at com.demo.Queue.take(Queue.java:18)
"#;
        let threads = parse_thread_dump(dump);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].state, "RUNNABLE");
        let hot = aggregate_hot_frames_from_dumps(&[dump.to_string()]);
        assert_eq!(hot[0].method, "com.demo.OrderController.query");
    }

    #[test]
    fn java_process_matcher_rejects_java_word_in_url() {
        assert!(!super::is_java_process(
            "curl",
            "curl -s http://127.0.0.1:3400/api/java/processes"
        ));
        assert!(!super::is_java_process(
            "bash",
            "/bin/bash -c curl http://localhost/api/java/analyze"
        ));
        assert!(!super::is_java_process(
            "rg",
            "rg -n Java 堆栈实时分析|未发现 Java 进程"
        ));
        assert!(!super::is_java_process(
            "chrome",
            "chrome --dump-dom http://127.0.0.1:3400/#/java-analyzer"
        ));
        assert!(super::is_java_process(
            "java",
            "/usr/bin/java -Xmx1g -jar /opt/app/service.jar"
        ));
        assert!(super::is_java_process(
            "service",
            "/usr/lib/jvm/bin/java -cp app.jar com.demo.Main"
        ));
    }

    #[test]
    fn rule_engine_reports_stack_depth_and_external_dependency_hotspots() {
        let mut deep_frames = String::new();
        for i in 0..85 {
            deep_frames.push_str(&format!(
                "        at com.demo.deep.Layer{i}.invoke(Layer{i}.java:{i})\n"
            ));
        }
        let dump = format!(
            r#""http-nio-8080-exec-9" #41 nid=0x41 runnable
   java.lang.Thread.State: RUNNABLE
        at okhttp3.internal.connection.RealCall.execute(RealCall.kt:153)
        at com.demo.RemoteClient.query(RemoteClient.java:27)
{deep_frames}

"blocked-worker" #42 nid=0x42 waiting
   java.lang.Thread.State: BLOCKED
        at com.demo.LockService.update(LockService.java:19)
"#
        );
        let threads = parse_thread_dump(&dump);
        let hot = aggregate_hot_frames_from_dumps(&[dump]);
        let process = fake_process();

        let findings = build_findings(&process, &threads, &hot, "", "", true, "");
        let titles = findings
            .iter()
            .map(|finding| finding.title.as_str())
            .collect::<Vec<_>>();

        assert!(titles.contains(&"调用栈深度异常"));
        assert!(titles.contains(&"外部依赖调用热点"));
    }

    fn fake_process() -> JavaProcess {
        JavaProcess {
            pid: 42,
            name: "java".into(),
            service_name: "demo".into(),
            display_name: "demo".into(),
            exe: "/usr/bin/java".into(),
            cwd: "/opt/demo".into(),
            cmd: "java -Xmx512m -jar demo.jar".into(),
            main: "demo.jar".into(),
            jar: "demo.jar".into(),
            user: "root".into(),
            state: "S".into(),
            threads: 2,
            cpu_ticks: 0,
            memory_bytes: 512 * 1024 * 1024,
            heap_flags: vec!["-Xmx512m".into()],
            ports: vec!["8080".into()],
            start_time: "2026-06-10 00:00:00".into(),
            uptime: "1分".into(),
            score: 80,
            attach_available: true,
            attach_path: "/tmp/.java_pid42".into(),
        }
    }

    #[test]
    fn infers_main_class_after_classpath_option() {
        assert_eq!(
            super::infer_main("java -cp /tmp DmStackTarget", ""),
            "DmStackTarget"
        );
        assert_eq!(
            super::infer_main(
                "/usr/lib/jvm/jdk-21.0.11-oracle-x64/bin/java -Xmx1g -classpath lib/* com.demo.Main --port 8080",
                ""
            ),
            "com.demo.Main"
        );
    }

    #[test]
    fn infers_service_name_from_java_arguments() {
        assert_eq!(
            super::infer_service_name(
                "java -Dspring.application.name=order-api -jar /opt/app/order.jar",
                "order.jar",
                "/opt/app/order.jar",
                42
            ),
            "order-api"
        );
        assert_eq!(
            super::infer_service_name(
                "java -cp /tmp com.demo.StackDemo",
                "com.demo.StackDemo",
                "",
                42
            ),
            "StackDemo"
        );
        assert_eq!(
            super::infer_service_name(
                "java -jar /opt/app/payment-service.jar",
                "payment-service.jar",
                "/opt/app/payment-service.jar",
                42
            ),
            "payment-service"
        );
    }

    #[test]
    fn formats_process_uptime() {
        assert_eq!(super::format_duration(65), "1分");
        assert_eq!(super::format_duration(3_900), "1时 5分");
        assert_eq!(super::format_duration(90_300), "1天 1时 5分");
    }
}
