use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::net::IpAddr;

use crate::config::Config;
use crate::db::{AlertRecord, Database, ExecRecord};
use crate::script::metadata::{ScriptMetadata, ScriptParam};
use crate::script::{self, Script};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

const MAX_ID_LEN: usize = 128;
const MAX_TITLE_LEN: usize = 500;
const MAX_DESC_LEN: usize = 5000;
const RULE_OVERRIDE_KEYS: &[&str] = &[
    "enabled",
    "level",
    "title",
    "summary",
    "suggestion",
    "commands",
];
const RULE_CREATE_KEYS: &[&str] = &[
    "enabled",
    "level",
    "title",
    "summary",
    "suggestion",
    "commands",
    "category",
    "target",
    "condition",
    "description",
];

fn validate_id(id: &str) -> Result<(), StatusCode> {
    if id.is_empty()
        || id.len() > MAX_ID_LEN
        || !id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn validate_non_empty(s: &str, max_len: usize) -> Result<(), StatusCode> {
    if s.trim().is_empty() || s.len() > max_len {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn validate_rule_id_string(id: &str) -> Result<(), StatusCode> {
    if id.is_empty()
        || id.len() > 160
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn sanitize_service_name(name: &str) -> Result<String, StatusCode> {
    let value = name.trim();
    if value.is_empty() || value.len() > 128 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | ':'))
    {
        Ok(value.to_string())
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

#[derive(Debug, Clone)]
struct RuleImportPlan {
    imported: Vec<(String, serde_json::Value)>,
    skipped: Vec<String>,
    errors: Vec<String>,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub alert_cache: Arc<RwLock<AlertCache>>,
    pub health_tasks: Arc<RwLock<HashMap<String, HealthTaskState>>>,
    pub java_cancel_tokens: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    pub java_cancelled_keys: Arc<RwLock<HashSet<String>>>,
    pub alert_refreshing: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Default)]
pub struct AlertCache {
    pub scans: Vec<serde_json::Value>,
    pub timestamp: String,
    pub checked: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthTaskState {
    pub id: String,
    pub status: String,
    pub percent: u8,
    pub current_step: String,
    pub current_check_id: String,
    pub total: usize,
    pub completed: usize,
    pub warnings: usize,
    pub errors: usize,
    pub logs: Vec<String>,
    pub started_at: String,
    pub updated_at: String,
    pub result: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct RunRequest {
    #[serde(default)]
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub args: Vec<String>,
}

fn run_request_params_value(req: &RunRequest) -> serde_json::Value {
    serde_json::to_value(&req.params).unwrap_or_else(|_| serde_json::json!({}))
}

fn valid_script_param_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn validate_script_params(params: &[ScriptParam]) -> Result<(), StatusCode> {
    if params.len() > 32 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut names = HashSet::new();
    for param in params {
        let name = param.name.trim();
        if name.len() > 64 || !valid_script_param_name(name) || !names.insert(name.to_string()) {
            return Err(StatusCode::BAD_REQUEST);
        }
        if !matches!(
            param.param_type.as_str(),
            "string" | "number" | "boolean" | "select"
        ) {
            return Err(StatusCode::BAD_REQUEST);
        }
        if param.description.len() > 512 {
            return Err(StatusCode::BAD_REQUEST);
        }
        if param
            .default
            .as_ref()
            .is_some_and(|value| value.len() > 256)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    Ok(())
}

fn parse_script_params_json(text: &str) -> Result<Vec<ScriptParam>, StatusCode> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let params: Vec<ScriptParam> =
        serde_json::from_str(text).map_err(|_| StatusCode::BAD_REQUEST)?;
    validate_script_params(&params)?;
    Ok(params)
}

const SUPPORTED_SCRIPT_EXTENSIONS: &[&str] = &[
    "sh", "bash", "zsh", "ksh", "py", "python", "js", "mjs", "pl", "perl", "rb", "lua", "php",
    "awk", "expect", "exp", "run", "bin",
];

fn script_extension_from_filename(filename: &str) -> Result<String, StatusCode> {
    let path = std::path::Path::new(filename);
    if path.components().count() != 1 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "sh".to_string());
    if SUPPORTED_SCRIPT_EXTENSIONS.contains(&ext.as_str()) {
        Ok(ext)
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

fn set_executable_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

fn sync_log_path_cache_from_config(state: &AppState, check_id: &str, value: &serde_json::Value) {
    let Some(path) = value
        .get("log_path")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return;
    };
    let _ = state
        .db
        .save_service_log_cache(check_id, path, "imported-check-config");
    if let Some(prefix) = value
        .get("service_prefix")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        for name in prefix.split(',').map(str::trim).filter(|v| !v.is_empty()) {
            let _ = state
                .db
                .save_service_log_cache(name, path, "imported-check-config");
        }
    }
}

#[derive(Serialize)]
pub struct ScriptListResponse {
    pub scripts: Vec<Script>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct DashboardStats {
    pub total_scripts: usize,
    pub total_executions: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub recent_execs: Vec<ExecRecord>,
    pub categories: HashMap<String, usize>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub search: Option<String>,
    pub category: Option<String>,
}

#[derive(Serialize)]
pub struct ScriptSourceResponse {
    pub id: String,
    pub path: String,
    pub content: String,
    pub line_count: usize,
    pub size_bytes: u64,
}

#[derive(Debug, Deserialize)]
pub struct DuplicateRequest {
    pub new_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateScriptRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub feature: String,
    #[serde(default)]
    pub example: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub params: Option<Vec<ScriptParam>>,
}

#[derive(Serialize)]
pub struct ScriptStatsResponse {
    pub script_id: String,
    pub total_executions: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub avg_duration_ms: Option<f64>,
    pub last_execution: Option<crate::db::ExecRecord>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    pub script_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
pub struct HistoryResponse {
    pub total: usize,
    pub returned: usize,
    pub records: Vec<ExecRecord>,
}

#[derive(Debug, Deserialize)]
pub struct AlertsQuery {
    pub history: Option<bool>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct MetricsQuery {
    pub minutes: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceLogsQuery {
    pub pid: Option<u32>,
    pub path: Option<String>,
    pub category: Option<String>,
    pub process: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServiceHealthQuery {
    pub pid: Option<u32>,
    pub path: Option<String>,
    pub category: Option<String>,
    pub process: Option<String>,
    pub ports: Option<String>,
}

const HEALTH_CHECK_IDS: &[&str] = &[
    "system",
    "resource",
    "service",
    "service-manage",
    "network",
    "security",
    "middleware",
    "elasticsearch",
    "redis",
    "nginx",
    "keepalived",
    "mysql",
    "kafka",
    "java-service",
];

pub async fn list_scripts(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<ScriptListResponse>, StatusCode> {
    let dirs = crate::config::all_script_dirs(&state.config);
    let scripts = script::discover_scripts(&dirs).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filtered: Vec<Script> = scripts
        .into_iter()
        .map(|mut s| {
            s.user_managed = is_user_script(&state.config, &s);
            s
        })
        .filter(|s| {
            if let Some(ref search) = query.search {
                let q = search.to_lowercase();
                s.name.to_lowercase().contains(&q)
                    || s.description.to_lowercase().contains(&q)
                    || s.id.to_lowercase().contains(&q)
                    || s.feature.to_lowercase().contains(&q)
            } else {
                true
            }
        })
        .filter(|s| {
            if let Some(ref cat) = query.category {
                s.category == *cat
            } else {
                true
            }
        })
        .collect();

    let total = filtered.len();
    Ok(Json(ScriptListResponse {
        scripts: filtered,
        total,
    }))
}

pub async fn get_script(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Script>, StatusCode> {
    validate_id(&id)?;
    let dirs = crate::config::all_script_dirs(&state.config);
    script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(|mut s| {
            s.user_managed = is_user_script(&state.config, &s);
            Json(s)
        })
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn get_script_source(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ScriptSourceResponse>, StatusCode> {
    validate_id(&id)?;
    let dirs = crate::config::all_script_dirs(&state.config);
    let script = script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let content =
        std::fs::read_to_string(&script.path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let meta = std::fs::metadata(&script.path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ScriptSourceResponse {
        id: script.id,
        path: script.path.display().to_string(),
        line_count: content.lines().count(),
        size_bytes: meta.len(),
        content,
    }))
}

pub async fn get_script_stats(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ScriptStatsResponse>, StatusCode> {
    validate_id(&id)?;
    let (total, success, failure, avg_dur) = state.db.get_script_stats(&id);
    let last_execution = state.db.get_history(Some(&id), 1).into_iter().next();
    Ok(Json(ScriptStatsResponse {
        script_id: id,
        total_executions: total,
        success_count: success,
        failure_count: failure,
        avg_duration_ms: avg_dur,
        last_execution,
    }))
}

pub async fn duplicate_script(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<DuplicateRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let dirs = crate::config::all_script_dirs(&state.config);
    let script = script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let src_dir = script
        .path
        .parent()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let new_id = req.new_id.trim().to_string();
    if new_id.is_empty()
        || new_id.len() > 64
        || !new_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let dst_dir = state.config.user_scripts_dir.join(&new_id);
    if dst_dir.exists() {
        return Err(StatusCode::CONFLICT);
    }

    std::fs::create_dir_all(&dst_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    for entry in std::fs::read_dir(src_dir).into_iter().flatten() {
        if let Ok(e) = entry {
            let src = e.path();
            if src.is_file() {
                let _ = std::fs::copy(&src, dst_dir.join(e.file_name()));
            }
        }
    }

    let toml_path = dst_dir.join(".dm.toml");
    if toml_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&toml_path) {
            let updated: String = content
                .lines()
                .map(|line| {
                    if line.starts_with("name") && line.contains('=') {
                        format!("name = \"{}\"", new_id)
                    } else {
                        line.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let _ = std::fs::write(&toml_path, updated);
        }
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "new_id": new_id,
        "message": format!("脚本已复制为 {}", new_id)
    })))
}

pub async fn run_script(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<RunRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let dirs = crate::config::all_script_dirs(&state.config);
    let script = script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    state.db.insert_exec_with_inputs(
        &script.id,
        &script.name,
        None,
        None,
        0,
        &run_request_params_value(&req),
        &req.args,
    );

    Ok(Json(serde_json::json!({
        "status": "started",
        "script_id": id,
        "message": "脚本已启动，请通过 WebSocket 获取实时日志"
    })))
}

pub async fn dashboard_stats(
    State(state): State<AppState>,
) -> Result<Json<DashboardStats>, StatusCode> {
    let dirs = crate::config::all_script_dirs(&state.config);
    let scripts = script::discover_scripts(&dirs).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (total_executions, success_count, failure_count) = state.db.get_stats();
    let recent_execs = state.db.get_history(None, 20);

    let mut categories = HashMap::new();
    for s in &scripts {
        *categories.entry(s.category.clone()).or_insert(0) += 1;
    }

    Ok(Json(DashboardStats {
        total_scripts: scripts.len(),
        total_executions,
        success_count,
        failure_count,
        recent_execs,
        categories,
    }))
}

pub fn record_metric_sample(db: &Database, sys: &crate::dashboard::SystemInfo) {
    let totals = sys
        .networks
        .iter()
        .filter(|n| {
            n.name != "lo"
                && !n.name.contains("docker")
                && !n.name.contains("br-")
                && !n.name.contains("veth")
        })
        .fold((0i64, 0i64), |acc, n| {
            (
                acc.0.saturating_add(n.received_bytes as i64),
                acc.1.saturating_add(n.transmitted_bytes as i64),
            )
        });
    let load_ratio = if sys.cpu_count > 0 {
        (sys.load_avg.one / sys.cpu_count as f64) * 100.0
    } else {
        0.0
    };
    db.insert_metric_point(
        sys.cpu_usage as f64,
        sys.memory_usage as f64,
        sys.load_avg.one,
        load_ratio.clamp(0.0, 300.0),
        totals.0,
        totals.1,
    );
}

pub async fn dashboard_metrics(
    State(state): State<AppState>,
    Query(query): Query<MetricsQuery>,
) -> Json<serde_json::Value> {
    state.db.cleanup_metric_history();
    let minutes = query.minutes.unwrap_or(30).clamp(3, 120);
    let mut points = state.db.get_metric_history(minutes);
    if points.is_empty() {
        let sys = tokio::task::spawn_blocking(crate::dashboard::get_system_info)
            .await
            .unwrap_or_else(|_| crate::dashboard::get_system_info());
        record_metric_sample(&state.db, &sys);
        points = state.db.get_metric_history(minutes);
    }
    Json(serde_json::json!({
        "minutes": minutes,
        "retention_minutes": 120,
        "points": points,
        "total": points.len(),
    }))
}

pub async fn system_info(State(state): State<AppState>) -> Json<crate::dashboard::SystemInfo> {
    let sys = tokio::task::spawn_blocking(crate::dashboard::get_system_info)
        .await
        .unwrap_or_else(|_| crate::dashboard::get_system_info());
    record_metric_sample(&state.db, &sys);
    Json(sys)
}

pub async fn traffic_interfaces(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let sys = tokio::task::spawn_blocking(crate::dashboard::get_system_info)
        .await
        .unwrap_or_else(|_| crate::dashboard::get_system_info());
    let mut interfaces: Vec<serde_json::Value> = sys
        .networks
        .iter()
        .map(|n| {
            let class = classify_network_interface(&n.name, &n.ip);
            serde_json::json!({
                "name": n.name,
                "ip": n.ip,
                "mac": n.mac,
                "received_bytes": n.received_bytes,
                "transmitted_bytes": n.transmitted_bytes,
                "kind": class.kind,
                "kind_label": class.label,
                "priority": class.priority,
                "is_public": class.is_public,
                "is_physical": class.is_physical,
                "is_virtual": class.is_virtual,
            })
        })
        .collect();

    interfaces.sort_by(|a, b| {
        let a_name = a["name"].as_str().unwrap_or_default();
        let b_name = b["name"].as_str().unwrap_or_default();
        let a_priority = a["priority"].as_i64().unwrap_or(99);
        let b_priority = b["priority"].as_i64().unwrap_or(99);
        a_priority
            .cmp(&b_priority)
            .then_with(|| {
                b["ip"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty()
                    .cmp(&a["ip"].as_str().unwrap_or_default().is_empty())
            })
            .then_with(|| a_name.cmp(b_name))
    });

    Json(serde_json::json!({
        "interfaces": interfaces,
        "capture_supported": cfg!(target_os = "linux"),
        "platform": std::env::consts::OS,
    }))
}

#[derive(Debug, Clone, Copy)]
struct InterfaceClass {
    kind: &'static str,
    label: &'static str,
    priority: i64,
    is_public: bool,
    is_physical: bool,
    is_virtual: bool,
}

fn classify_network_interface(name: &str, ip: &str) -> InterfaceClass {
    let lower = name.to_ascii_lowercase();
    let is_loopback = is_loopback_interface(&lower, ip);
    let is_virtual = !is_loopback && is_virtual_interface(&lower);
    let is_physical = !is_virtual && is_physical_interface(&lower);
    let is_public = is_public_ip(ip);
    if is_public {
        InterfaceClass {
            kind: "public",
            label: "公网",
            priority: 0,
            is_public,
            is_physical,
            is_virtual,
        }
    } else if is_physical {
        InterfaceClass {
            kind: "physical",
            label: "物理",
            priority: 1,
            is_public,
            is_physical,
            is_virtual,
        }
    } else if is_loopback {
        InterfaceClass {
            kind: "loopback",
            label: "本地回环",
            priority: 2,
            is_public,
            is_physical: false,
            is_virtual: false,
        }
    } else if is_virtual {
        InterfaceClass {
            kind: "virtual",
            label: "虚拟",
            priority: 4,
            is_public,
            is_physical,
            is_virtual,
        }
    } else {
        InterfaceClass {
            kind: "other",
            label: "其它",
            priority: 3,
            is_public,
            is_physical,
            is_virtual,
        }
    }
}

fn is_loopback_interface(lower: &str, ip: &str) -> bool {
    if matches!(lower, "lo" | "lo0" | "loopback") || lower.starts_with("lo:") {
        return true;
    }
    let primary = ip
        .split(|c| matches!(c, '/' | ',' | ' '))
        .find(|part| !part.trim().is_empty())
        .unwrap_or("")
        .trim();
    matches!(
        primary.parse::<IpAddr>(),
        Ok(IpAddr::V4(v4)) if v4.is_loopback()
    ) || matches!(
        primary.parse::<IpAddr>(),
        Ok(IpAddr::V6(v6)) if v6.is_loopback()
    )
}

fn is_virtual_interface(lower: &str) -> bool {
    lower.starts_with("docker")
        || lower.starts_with("br-")
        || lower.starts_with("veth")
        || lower.starts_with("virbr")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
        || lower.starts_with("tailscale")
        || lower.starts_with("zt")
        || lower.starts_with("kube")
        || lower.starts_with("cni")
        || lower.starts_with("flannel")
        || lower.starts_with("calico")
        || lower.starts_with("vmnet")
        || lower.contains("virtual")
}

fn is_physical_interface(lower: &str) -> bool {
    lower.starts_with("eth")
        || lower.starts_with("en")
        || lower.starts_with("ens")
        || lower.starts_with("eno")
        || lower.starts_with("enp")
        || lower.starts_with("em")
        || lower.starts_with("bond")
        || lower.starts_with("team")
        || lower.starts_with("wlan")
        || lower.starts_with("wl")
}

fn is_public_ip(ip: &str) -> bool {
    let primary = ip
        .split(|c| matches!(c, '/' | ',' | ' '))
        .find(|part| !part.trim().is_empty())
        .unwrap_or("")
        .trim();
    match primary.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => {
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified())
        }
        Ok(IpAddr::V6(v6)) => {
            !(v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || ((v6.segments()[0] & 0xfe00) == 0xfc00)
                || ((v6.segments()[0] & 0xffc0) == 0xfe80))
        }
        Err(_) => false,
    }
}

#[derive(Debug, Deserialize)]
pub struct JavaProcessQuery {
    pub q: Option<String>,
}

pub async fn java_processes(Query(query): Query<JavaProcessQuery>) -> Json<serde_json::Value> {
    let q = query.q.clone();
    let processes = tokio::task::spawn_blocking(move || {
        crate::java_analyzer::list_java_processes(q.as_deref())
    })
    .await
    .unwrap_or_default();
    Json(serde_json::json!({
        "processes": processes,
        "total": processes.len(),
    }))
}

pub async fn java_analyze(
    State(state): State<AppState>,
    Json(req): Json<crate::java_analyzer::JavaAnalyzeRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let cancel_key = req.cancel_key.clone();
    let cancel = register_java_cancel_token(&state, cancel_key.as_deref());
    let cleanup_key = cancel_key.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::java_analyzer::analyze_java_with_cancel(req, cancel)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    unregister_java_cancel_token(&state, cleanup_key.as_deref());
    let analysis = result.map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(analysis).unwrap_or_default()))
}

pub async fn java_scan(
    State(state): State<AppState>,
    Json(req): Json<crate::java_analyzer::JavaFleetScanRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let cancel_key = req.cancel_key.clone();
    let cancel = register_java_cancel_token(&state, cancel_key.as_deref());
    let cleanup_key = cancel_key.clone();
    let result = tokio::task::spawn_blocking(move || {
        crate::java_analyzer::scan_java_process_rules(req, cancel)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    unregister_java_cancel_token(&state, cleanup_key.as_deref());
    let scan = result.map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(serde_json::to_value(scan).unwrap_or_default()))
}

pub async fn java_hprof(
    State(state): State<AppState>,
    Json(req): Json<crate::java_analyzer::JavaHeapDumpRequest>,
) -> Result<Response<Body>, StatusCode> {
    let cancel_key = req.cancel_key.clone();
    let cancel = register_java_cancel_token(&state, cancel_key.as_deref());
    let cleanup_key = cancel_key.clone();
    let result =
        tokio::task::spawn_blocking(move || crate::java_analyzer::dump_java_hprof(req, cancel))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    unregister_java_cancel_token(&state, cleanup_key.as_deref());
    let dump = result.map_err(|_| StatusCode::BAD_REQUEST)?;
    let bytes = std::fs::read(&dump.path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let _ = std::fs::remove_file(&dump.path);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", dump.filename),
        )
        .header("x-dm-hprof-bytes", dump.bytes.to_string())
        .header(
            "x-dm-hprof-message",
            dump.message.replace(['\r', '\n'], " "),
        )
        .body(Body::from(bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn java_cancel(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Json<serde_json::Value> {
    let before_active = state
        .java_cancel_tokens
        .read()
        .ok()
        .map(|tokens| tokens.len())
        .unwrap_or(0);
    let cancelled = state
        .java_cancel_tokens
        .write()
        .ok()
        .and_then(|mut tokens| tokens.remove(&key))
        .map(|token| {
            token.store(true, Ordering::SeqCst);
            true
        })
        .unwrap_or(false);
    if !cancelled {
        if let Ok(mut keys) = state.java_cancelled_keys.write() {
            keys.insert(key.clone());
        }
    }
    let active = state
        .java_cancel_tokens
        .read()
        .ok()
        .map(|tokens| tokens.len())
        .unwrap_or(0);
    let pending = state
        .java_cancelled_keys
        .read()
        .ok()
        .map(|keys| keys.len())
        .unwrap_or(0);
    Json(serde_json::json!({
        "status": "ok",
        "cancelled": cancelled,
        "pending": !cancelled,
        "active_before": before_active,
        "active": active,
        "pending_count": pending,
        "key": key,
    }))
}

fn register_java_cancel_token(state: &AppState, key: Option<&str>) -> Option<Arc<AtomicBool>> {
    let key = key?.trim();
    if key.is_empty() || key.len() > 160 {
        return None;
    }
    let token = Arc::new(AtomicBool::new(false));
    if state
        .java_cancelled_keys
        .write()
        .ok()
        .is_some_and(|mut keys| keys.remove(key))
    {
        token.store(true, Ordering::SeqCst);
    }
    if let Ok(mut tokens) = state.java_cancel_tokens.write() {
        tokens.insert(key.to_string(), token.clone());
    }
    Some(token)
}

fn unregister_java_cancel_token(state: &AppState, key: Option<&str>) {
    let Some(key) = key else {
        return;
    };
    if let Ok(mut tokens) = state.java_cancel_tokens.write() {
        tokens.remove(key);
    }
}

pub async fn clear_history(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state.db.clear_history();
    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "执行历史已清空"
    })))
}

pub async fn get_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, StatusCode> {
    let limit = query.limit.unwrap_or(100).min(1000);
    let records = state.db.get_history(query.script_id.as_deref(), limit);
    let total = records.len();
    Ok(Json(HistoryResponse {
        total,
        returned: records.len(),
        records,
    }))
}

pub async fn all_scripts_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let dirs = crate::config::all_script_dirs(&state.config);
    let scripts = script::discover_scripts(&dirs).unwrap_or_default();

    let mut stats = serde_json::Map::new();
    for s in &scripts {
        let (total, success, failure, avg_dur) = state.db.get_script_stats(&s.id);
        let last_exec = state.db.get_history(Some(&s.id), 1).into_iter().next();
        stats.insert(
            s.id.clone(),
            serde_json::json!({
                "total_executions": total,
                "success_count": success,
                "failure_count": failure,
                "avg_duration_ms": avg_dur,
                "last_execution": last_exec
            }),
        );
    }

    Json(serde_json::json!({ "stats": stats }))
}

pub async fn upload_script(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut script_id = String::new();
    let mut title = String::new();
    let mut description = String::new();
    let mut feature = String::new();
    let mut category = "维护脚本".to_string();
    let mut author = "user".to_string();
    let mut filename = String::new();
    let mut bytes: Option<Vec<u8>> = None;
    let mut params: Vec<ScriptParam> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field
                .file_name()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "script.sh".to_string());
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| StatusCode::BAD_REQUEST)?
                    .to_vec(),
            );
            continue;
        }
        let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
        match name.as_str() {
            "id" => script_id = text,
            "title" | "name" => title = text,
            "description" => description = text,
            "feature" => feature = text,
            "category" => category = text,
            "author" => author = text,
            "params" => params = parse_script_params_json(&text)?,
            _ => {}
        }
    }

    let bytes = bytes.ok_or(StatusCode::BAD_REQUEST)?;
    if script_id.trim().is_empty() {
        script_id = filename
            .split('.')
            .next()
            .unwrap_or("script")
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>();
    }
    script_id = script_id.trim_matches('-').to_lowercase();
    validate_id(&script_id)?;
    if title.trim().is_empty() {
        title = script_id.clone();
    }
    if description.trim().is_empty() {
        description = "用户上传维护脚本".to_string();
    }
    if feature.trim().is_empty() {
        feature = description.clone();
    }

    let script_dir = state.config.user_scripts_dir.join(&script_id);
    if script_dir.exists() {
        return Err(StatusCode::CONFLICT);
    }
    std::fs::create_dir_all(&script_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ext = script_extension_from_filename(&filename)?;
    let script_file = script_dir.join(format!("{}.{}", script_id, ext));
    std::fs::write(&script_file, &bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    set_executable_permissions(&script_file);
    write_script_metadata(
        &script_dir,
        &ScriptMetadata {
            name: title.clone(),
            description: description.clone(),
            feature,
            example: format!("dm run {}", script_id),
            version: "1.0.0".to_string(),
            author,
            category,
            params,
        },
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "status": "ok",
        "id": script_id,
        "filename": filename,
        "path": script_file.display().to_string(),
        "message": "脚本上传成功"
    })))
}

pub async fn replace_script_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let dirs = crate::config::all_script_dirs(&state.config);
    let script = script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if !is_user_script(&state.config, &script) {
        return Err(StatusCode::FORBIDDEN);
    }

    let mut filename = String::new();
    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        if field.name().unwrap_or("") != "file" {
            continue;
        }
        filename = field
            .file_name()
            .map(|v| v.to_string())
            .unwrap_or_else(|| format!("{}.sh", id));
        bytes = Some(
            field
                .bytes()
                .await
                .map_err(|_| StatusCode::BAD_REQUEST)?
                .to_vec(),
        );
    }

    let bytes = bytes.ok_or(StatusCode::BAD_REQUEST)?;
    let script_dir = script
        .path
        .parent()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let ext = script_extension_from_filename(&filename)?;
    let next_path = script_dir.join(format!("{}.{}", id, ext));
    if next_path != script.path && script.path.exists() {
        std::fs::remove_file(&script.path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    std::fs::write(&next_path, bytes).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    set_executable_permissions(&next_path);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "id": id,
        "filename": filename,
        "path": next_path.display().to_string(),
        "message": "脚本文件已更新"
    })))
}

fn is_user_script(config: &Config, script: &Script) -> bool {
    script.path.starts_with(&config.user_scripts_dir)
}

fn write_script_metadata(
    dir: &std::path::Path,
    meta: &ScriptMetadata,
) -> Result<(), std::io::Error> {
    let content = toml::to_string_pretty(meta).unwrap_or_default();
    std::fs::write(dir.join(".dm.toml"), content)
}

pub async fn update_script(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateScriptRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let dirs = crate::config::all_script_dirs(&state.config);
    let script = script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let is_user = is_user_script(&state.config, &script);
    let script_dir = script
        .path
        .parent()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    if let Some(content) = req.content {
        if !is_user {
            return Err(StatusCode::FORBIDDEN);
        }
        std::fs::write(&script.path, content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let current = script.metadata.unwrap_or(ScriptMetadata {
        name: id.clone(),
        description: String::new(),
        feature: String::new(),
        example: format!("dm run {}", id),
        version: "1.0.0".to_string(),
        author: String::new(),
        category: "维护脚本".to_string(),
        params: Vec::new(),
    });
    let params = if let Some(params) = req.params {
        validate_script_params(&params)?;
        params
    } else {
        current.params.clone()
    };
    let meta = ScriptMetadata {
        name: if req.name.trim().is_empty() {
            current.name
        } else {
            req.name
        },
        description: if req.description.trim().is_empty() {
            current.description
        } else {
            req.description
        },
        feature: if req.feature.trim().is_empty() {
            current.feature
        } else {
            req.feature
        },
        example: if req.example.trim().is_empty() {
            current.example
        } else {
            req.example
        },
        version: if req.version.trim().is_empty() {
            current.version
        } else {
            req.version
        },
        author: if req.author.trim().is_empty() {
            current.author
        } else {
            req.author
        },
        category: if req.category.trim().is_empty() {
            current.category
        } else {
            req.category
        },
        params,
    };
    write_script_metadata(script_dir, &meta).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({ "status": "ok", "id": id, "message": "脚本已更新" }),
    ))
}

pub async fn delete_script(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let dirs = crate::config::all_script_dirs(&state.config);
    let script = script::find_script(&dirs, &id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if !is_user_script(&state.config, &script) {
        return Err(StatusCode::FORBIDDEN);
    }
    let script_dir = script
        .path
        .parent()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    std::fs::remove_dir_all(script_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({ "status": "ok", "id": id, "message": "脚本已删除" }),
    ))
}

pub async fn list_checks(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let checks = crate::checks::list_checks();
    Json(serde_json::json!({
        "checks": checks,
        "total": checks.len()
    }))
}

pub async fn run_check(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let check_id = id.clone();
    let result = tokio::task::spawn_blocking(move || crate::checks::run_check(&check_id))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match result {
        Some(mut result) => {
            let overrides = overrides_from_db(&state);
            apply_rule_overrides_to_check_result(&mut result, &overrides);
            Ok(Json(serde_json::to_value(result).unwrap_or_default()))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn get_check_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let value = crate::check_config_store::load_config_value(&state.config, &state.db, &id);
    let record = state.db.get_check_config(&id);
    Ok(Json(serde_json::json!({
        "check_id": id,
        "value": value.unwrap_or_else(|| serde_json::json!({})),
        "updated_at": record.map(|r| r.updated_at).unwrap_or_default(),
        "config_file": crate::check_config_store::connection_config_path(&state.config).display().to_string(),
    })))
}

pub async fn update_check_config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    if !req.is_object() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if serde_json::to_string(&req).unwrap_or_default().len() > 64 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }

    if req
        .get("password")
        .and_then(|v| v.as_str())
        .map(|v| v.trim().is_empty())
        .unwrap_or(false)
    {
        if let Some(existing) = state.db.get_check_config(&id) {
            if let Some(password) = existing.value.get("password").and_then(|v| v.as_str()) {
                if !password.is_empty() {
                    if let Some(obj) = req.as_object_mut() {
                        obj.insert("password".to_string(), serde_json::json!(password));
                    }
                }
            }
        }
    }

    if !crate::check_config_store::upsert_and_sync(&state.config, &state.db, &id, &req) {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    sync_log_path_cache_from_config(&state, &id, &req);
    let record = state.db.get_check_config(&id);
    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "检查配置已保存，并已同步到连接配置文件",
        "check_id": id,
        "value": record.as_ref().map(|r| r.value.clone()).unwrap_or(req),
        "updated_at": record.map(|r| r.updated_at).unwrap_or_default(),
        "config_file": crate::check_config_store::connection_config_path(&state.config).display().to_string(),
    })))
}

fn check_config_template_value() -> serde_json::Value {
    crate::check_config_store::template_value()
}

fn normalize_check_config_import_payload(
    payload: &serde_json::Value,
) -> (Vec<(String, serde_json::Value)>, Vec<String>, Vec<String>) {
    let source = payload
        .get("configs")
        .and_then(|v| v.as_object())
        .or_else(|| payload.as_object());

    let Some(source) = source else {
        return (
            Vec::new(),
            Vec::new(),
            vec!["导入文件必须是 JSON 对象，或包含 configs 对象".to_string()],
        );
    };

    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();
    for (check_id, value) in source {
        if check_id == "version" || check_id == "description" {
            continue;
        }
        if !crate::check_config_store::is_configurable_check_id(check_id) {
            skipped.push(check_id.to_string());
            continue;
        }
        if !value.is_object() {
            errors.push(format!("{} 配置必须是 JSON 对象", check_id));
            continue;
        }
        if serde_json::to_string(value).unwrap_or_default().len() > 64 * 1024 {
            errors.push(format!("{} 配置过大，已拒绝", check_id));
            continue;
        }
        imported.push((check_id.to_string(), value.clone()));
    }

    (imported, skipped, errors)
}

pub async fn check_config_template() -> Json<serde_json::Value> {
    Json(check_config_template_value())
}

pub async fn export_check_configs(State(state): State<AppState>) -> Json<serde_json::Value> {
    let value = crate::check_config_store::export_value(&state.config, &state.db);
    let _ = crate::check_config_store::sync_db_to_file(&state.config, &state.db);
    Json(value)
}

pub async fn import_check_configs(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (mut imported, skipped, errors) = normalize_check_config_import_payload(&payload);
    if imported.is_empty() && !errors.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "imported": 0,
            "skipped": skipped,
            "errors": errors,
            "message": "未导入任何配置",
        })));
    }

    let mut to_save = Vec::new();
    for (check_id, mut value) in imported.drain(..) {
        if value
            .get("password")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().is_empty())
            .unwrap_or(false)
        {
            if let Some(existing) = state.db.get_check_config(&check_id) {
                if let Some(password) = existing.value.get("password").and_then(|v| v.as_str()) {
                    if !password.is_empty() {
                        if let Some(obj) = value.as_object_mut() {
                            obj.insert("password".to_string(), serde_json::json!(password));
                        }
                    }
                }
            }
        }
        sync_log_path_cache_from_config(&state, &check_id, &value);
        to_save.push((check_id, value));
    }
    let saved =
        crate::check_config_store::import_configs_and_sync(&state.config, &state.db, to_save);

    Ok(Json(serde_json::json!({
        "status": "ok",
        "imported": saved.len(),
        "check_ids": saved,
        "skipped": skipped,
        "errors": errors,
        "config_file": crate::check_config_store::connection_config_path(&state.config).display().to_string(),
        "message": format!("已导入 {} 个检查连接配置，并已同步到连接配置文件", saved.len()),
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateDocRequest {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub category: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDocDirRequest {
    pub name: String,
}

pub async fn list_docs(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let docs = crate::docs::list_docs(None);
    let dirs = crate::docs::list_doc_dirs();
    Json(serde_json::json!({ "docs": docs, "dirs": dirs, "total": docs.len() }))
}

pub async fn get_doc_api(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    match crate::docs::get_doc(&id) {
        Some(doc) => Ok(Json(serde_json::to_value(doc).unwrap_or_default())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_doc_api(
    State(_state): State<AppState>,
    Json(req): Json<CreateDocRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&req.id)?;
    validate_non_empty(&req.title, MAX_TITLE_LEN)?;
    let cat = if req.category.is_empty() {
        "通用"
    } else {
        &req.category
    };
    match crate::docs::create_doc(&req.id, &req.title, cat, "") {
        Ok(meta) => Ok(Json(serde_json::to_value(meta).unwrap_or_default())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_doc_dir_api(
    State(_state): State<AppState>,
    Json(req): Json<CreateDocDirRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&req.name, 80)?;
    match crate::docs::create_doc_dir(&req.name) {
        Ok(dirs) => Ok(Json(serde_json::json!({ "status": "ok", "dirs": dirs }))),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

pub async fn import_doc_api(
    State(_state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut filename = String::new();
    let mut title = String::new();
    let mut category = String::new();
    let mut content = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field.file_name().unwrap_or("uploaded-doc.md").to_string();
            let bytes = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            content = String::from_utf8_lossy(&bytes).to_string();
        } else {
            let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            match name.as_str() {
                "title" => title = text,
                "category" => category = text,
                _ => {}
            }
        }
    }
    if content.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let parsed = parse_import_text(&content, &filename);
    let title = first_non_empty(&[title.as_str(), parsed.title.as_str(), "导入文档"]);
    let category = first_non_empty(&[category.as_str(), parsed.category.as_str(), "导入文档"]);
    let id = slug_id_from(&filename, &title);
    validate_id(&id)?;
    let body = parsed.body;
    let meta = if crate::docs::get_doc(&id).is_some() {
        crate::docs::update_doc(&id, Some(&title), Some(&category), None, Some(&body))
    } else {
        crate::docs::create_doc(&id, &title, &category, &body)
    }
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "status": "ok",
        "doc": meta,
        "message": "文档已导入并自动解析标题/分类"
    })))
}

pub async fn delete_doc_api(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match crate::docs::delete_doc(&id) {
        Ok(_) => Ok(Json(serde_json::json!({ "status": "ok" }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn update_doc_api(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateDocRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    let title_opt = if req.title.is_empty() {
        None
    } else {
        Some(req.title.as_str())
    };
    let cat_opt = if req.category.is_empty() {
        None
    } else {
        Some(req.category.as_str())
    };
    let content_opt = req.content.as_deref();
    match crate::docs::update_doc(&id, title_opt, cat_opt, None, content_opt) {
        Ok(meta) => Ok(Json(serde_json::to_value(meta).unwrap_or_default())),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn upload_doc_file(
    State(_state): State<AppState>,
    Path((doc_id, filename)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match crate::docs::upload_file(&doc_id, &filename, &body) {
        Ok(path) => Ok(Json(serde_json::json!({ "status": "ok", "path": path }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn download_doc_file(
    State(_state): State<AppState>,
    Path((doc_id, filename)): Path<(String, String)>,
) -> Result<axum::response::Response, StatusCode> {
    match crate::docs::download_file(&doc_id, &filename) {
        Ok(data) => {
            let mime = mime_guess::from_path(&filename)
                .first_or_octet_stream()
                .to_string();
            Ok(axum::response::Response::builder()
                .status(200)
                .header("content-type", mime)
                .header(
                    "content-disposition",
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(axum::body::Body::from(data))
                .unwrap())
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn list_doc_attachments(
    State(_state): State<AppState>,
    Path(doc_id): Path<String>,
) -> Json<serde_json::Value> {
    let files = crate::docs::list_attachments(&doc_id);
    Json(serde_json::json!({ "files": files, "total": files.len() }))
}

#[derive(Debug, Deserialize)]
pub struct DirQuery {
    pub path: Option<String>,
}

pub async fn list_directory(
    Query(query): Query<DirQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let dir_path = query.path.unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".dm")
            .join("scripts")
            .display()
            .to_string()
    });
    match crate::docs::list_dir_files(&dir_path) {
        Ok(files) => Ok(Json(
            serde_json::json!({ "path": dir_path, "files": files, "total": files.len() }),
        )),
        Err(_) => Err(StatusCode::BAD_REQUEST),
    }
}

pub async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }))
}

/// 极轻量心跳: 不读 db, 不写日志, 不带业务逻辑, 仅用于前端可达性探测 + 渲染节拍
#[inline]
pub async fn ping() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ping": "pong" }))
}

#[derive(Debug, Deserialize)]
pub struct CreateMaintenanceRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_maint_category")]
    pub category: String,
    #[serde(default = "default_maint_operator")]
    pub operator: String,
}

fn default_maint_category() -> String {
    "常规维护".to_string()
}
fn default_maint_operator() -> String {
    "system".to_string()
}

#[derive(Debug, Deserialize)]
pub struct CompleteMaintenanceRequest {
    pub result: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMaintenanceRequest {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub operator: String,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub status: String,
}

pub async fn list_maintenance(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let records = crate::maintenance::list_records(None);
    Json(serde_json::json!({ "records": records, "total": records.len() }))
}

pub async fn get_maintenance(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    match crate::maintenance::get_record(&id) {
        Some(record) => Ok(Json(serde_json::to_value(record).unwrap_or_default())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn create_maintenance(
    State(_state): State<AppState>,
    Json(req): Json<CreateMaintenanceRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&req.title, MAX_TITLE_LEN)?;
    match crate::maintenance::create_record(
        &req.title,
        &req.description,
        &req.category,
        &req.operator,
    ) {
        Ok(record) => Ok(Json(serde_json::to_value(record).unwrap_or_default())),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn import_maintenance_api(
    State(_state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut filename = String::new();
    let mut title = String::new();
    let mut category = String::new();
    let mut operator = String::new();
    let mut content = String::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            filename = field.file_name().unwrap_or("maintenance.txt").to_string();
            let bytes = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            content = String::from_utf8_lossy(&bytes).to_string();
        } else {
            let text = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            match name.as_str() {
                "title" => title = text,
                "category" => category = text,
                "operator" => operator = text,
                _ => {}
            }
        }
    }
    if content.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let parsed = parse_import_text(&content, &filename);
    let title = first_non_empty(&[title.as_str(), parsed.title.as_str(), "导入维护记录"]);
    let category = first_non_empty(&[category.as_str(), parsed.category.as_str(), "导入记录"]);
    let operator = first_non_empty(&[operator.as_str(), parsed.operator.as_str(), "system"]);
    let record = crate::maintenance::create_record(&title, &parsed.body, &category, &operator)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "status": "ok",
        "record": record,
        "message": "维护记录已导入并自动解析标题/分类/操作人"
    })))
}

struct ParsedImportText {
    title: String,
    category: String,
    operator: String,
    body: String,
}

fn parse_import_text(raw: &str, filename: &str) -> ParsedImportText {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        if value.is_object() {
            return ParsedImportText {
                title: value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                category: value
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                operator: value
                    .get("operator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string(),
                body: value
                    .get("content")
                    .or_else(|| value.get("description"))
                    .or_else(|| value.get("body"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(raw)
                    .trim()
                    .to_string(),
            };
        }
    }
    let mut title = String::new();
    let mut category = String::new();
    let mut operator = String::new();
    let mut body_lines = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if title.is_empty() && trimmed.starts_with("# ") {
            title = trimmed.trim_start_matches("# ").trim().to_string();
            continue;
        }
        if let Some(value) = trimmed
            .strip_prefix("category:")
            .or_else(|| trimmed.strip_prefix("分类:"))
        {
            category = value.trim().to_string();
            continue;
        }
        if let Some(value) = trimmed
            .strip_prefix("operator:")
            .or_else(|| trimmed.strip_prefix("操作人:"))
        {
            operator = value.trim().to_string();
            continue;
        }
        body_lines.push(line);
    }
    if title.is_empty() {
        title = raw
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or(filename)
            .trim_matches('#')
            .trim()
            .chars()
            .take(80)
            .collect();
    }
    ParsedImportText {
        title,
        category,
        operator,
        body: body_lines.join("\n").trim().to_string(),
    }
}

fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .map(|v| v.trim())
        .find(|v| !v.is_empty())
        .unwrap_or("")
        .to_string()
}

fn slug_id_from(filename: &str, title: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or(title);
    let mut id: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    while id.contains("--") {
        id = id.replace("--", "-");
    }
    id = id.trim_matches('-').chars().take(96).collect();
    if id.is_empty() {
        format!("import-{}", chrono::Local::now().timestamp())
    } else {
        id
    }
}

pub async fn update_maintenance(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateMaintenanceRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    if !req.title.is_empty() {
        validate_non_empty(&req.title, MAX_TITLE_LEN)?;
    }
    let result = crate::maintenance::update_record(
        &id,
        (!req.title.is_empty()).then_some(req.title.as_str()),
        Some(req.description.as_str()),
        (!req.category.is_empty()).then_some(req.category.as_str()),
        (!req.operator.is_empty()).then_some(req.operator.as_str()),
        Some(req.result.as_str()),
        (!req.status.is_empty()).then_some(req.status.as_str()),
    );
    match result {
        Ok(record) => Ok(Json(serde_json::to_value(record).unwrap_or_default())),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn complete_maintenance(
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<CompleteMaintenanceRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    validate_non_empty(&req.result, MAX_DESC_LEN)?;
    match crate::maintenance::complete_record(&id, &req.result) {
        Ok(_) => Ok(Json(serde_json::json!({ "status": "ok" }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn delete_maintenance(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_id(&id)?;
    match crate::maintenance::delete_record(&id) {
        Ok(_) => Ok(Json(serde_json::json!({ "status": "ok" }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(Debug, Deserialize)]
pub struct ServiceActionRequest {
    pub action: String,
}

pub async fn service_action(
    State(_state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ServiceActionRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&name, 128)?;
    let allowed = ["start", "stop", "restart", "status"];
    if !allowed.contains(&req.action.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let safe_name = sanitize_service_name(&name)?;

    let output = std::process::Command::new("systemctl")
        .args([&req.action, &safe_name])
        .output()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "service": safe_name,
        "action": req.action,
        "success": output.status.success(),
        "stdout": String::from_utf8_lossy(&output.stdout).trim(),
        "stderr": String::from_utf8_lossy(&output.stderr).trim(),
    })))
}

pub async fn full_health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    let overrides = overrides_from_db(&state);
    let mut payload =
        tokio::task::spawn_blocking(move || build_full_health_check_with_overrides(&overrides))
            .await
            .unwrap_or_else(|_| {
                serde_json::json!({
                    "overall_status": "error",
                    "total_checks": 0,
                    "total_warnings": 0,
                    "total_errors": 1,
                    "checks": [],
                    "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    "message": "健康体检后台任务执行失败",
                })
            });
    apply_rule_overrides_to_payload(&state, &mut payload);
    let alerts = payload["alerts"].as_array().cloned().unwrap_or_default();
    if !alerts.is_empty() {
        state.db.upsert_active_alerts(&alerts);
    }
    Json(payload)
}

pub async fn export_checks(State(state): State<AppState>) -> Json<serde_json::Value> {
    let overrides = overrides_from_db(&state);
    let mut payload =
        tokio::task::spawn_blocking(move || build_checks_export_with_overrides(&overrides))
            .await
            .unwrap_or_else(|_| {
                serde_json::json!({
                    "exported_at": alert_timestamp(),
                    "total": 0,
                    "summary": {
                        "ok": 0,
                        "warn": 0,
                        "error": 1,
                        "warnings": 0,
                        "errors": 1
                    },
                    "checks": [],
                    "alerts": [{
                        "id": "export-failed",
                        "type": "check",
                        "level": "error",
                        "title": "检查信息导出失败",
                        "message": "后台导出任务执行失败",
                        "summary": "后台导出任务执行失败",
                        "timestamp": alert_timestamp()
                    }]
                })
            });
    apply_rule_overrides_to_payload(&state, &mut payload);
    let alerts = payload["alerts"].as_array().cloned().unwrap_or_default();
    if !alerts.is_empty() {
        state.db.upsert_active_alerts(&alerts);
    }
    Json(payload)
}

fn build_checks_export_with_overrides(
    overrides: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    let started = std::time::Instant::now();
    let mut checks = Vec::new();
    let mut alerts = Vec::new();
    let mut ok_count = 0usize;
    let mut warn_status_count = 0usize;
    let mut error_status_count = 0usize;
    let mut total_warnings = 0usize;
    let mut total_errors = 0usize;

    for id in HEALTH_CHECK_IDS {
        let started_at = alert_timestamp();
        match run_health_check_item(id, std::time::Duration::from_secs(12)) {
            Some(mut result) => {
                apply_rule_overrides_to_check_result(&mut result, overrides);
                let counts = count_check_findings(&result);
                total_warnings += counts.0;
                total_errors += counts.1;
                alerts.extend(check_result_alerts(&result));
                match &result.status {
                    crate::checks::CheckStatus::Ok => ok_count += 1,
                    crate::checks::CheckStatus::Info => ok_count += 1,
                    crate::checks::CheckStatus::Warn => warn_status_count += 1,
                    crate::checks::CheckStatus::Error => error_status_count += 1,
                }
                checks.push(serde_json::json!({
                    "id": result.id,
                    "name": result.name,
                    "description": result.description,
                    "category": result.category,
                    "version": result.version,
                    "status": result.status,
                    "timestamp": result.timestamp,
                    "started_at": started_at,
                    "duration_ms": result.duration_ms,
                    "warning_count": counts.0,
                    "error_count": counts.1,
                    "section_count": result.sections.len(),
                    "sections": result.sections,
                }));
            }
            None => {
                warn_status_count += 1;
                total_warnings += 1;
                let alert = alert_value(
                    format!("export-check-missing-{}", id),
                    "check",
                    "warn",
                    format!("{} 无结果返回", check_display_name(id)),
                    "检查项没有返回结构化结果，已在导出中记录为警告",
                    vec![
                        format!("检查ID: {}", id),
                        format!("开始时间: {}", started_at),
                    ],
                    vec!["进入对应检查项单独执行，确认命令、权限或配置是否正常".to_string()],
                    vec![format!("dm check {}", id)],
                );
                alerts.push(alert.clone());
                checks.push(serde_json::json!({
                    "id": id,
                    "name": check_display_name(id),
                    "status": "warn",
                    "timestamp": alert_timestamp(),
                    "started_at": started_at,
                    "duration_ms": 0,
                    "warning_count": 1,
                    "error_count": 0,
                    "section_count": 1,
                    "sections": [{
                        "title": "检查无结果",
                        "icon": "WARN",
                        "description": "该检查项没有返回可渲染结果",
                        "items": [{
                            "type": "warning",
                            "text": "检查项没有返回结构化结果，请单独执行定位原因"
                        }]
                    }],
                }));
            }
        }
    }

    let overall = if total_errors > 0 || error_status_count > 0 {
        "error"
    } else if total_warnings > 0 || warn_status_count > 0 {
        "warn"
    } else {
        "ok"
    };

    let result_context = serde_json::json!(checks);
    alerts.extend(custom_rule_alerts(
        overrides,
        &alerts,
        Some(&result_context),
    ));
    let alerts = dedupe_alerts(alerts);
    serde_json::json!({
        "exported_at": alert_timestamp(),
        "duration_ms": started.elapsed().as_millis() as u64,
        "overall_status": overall,
        "total": checks.len(),
        "scope": {
            "type": "core_health_checks",
            "check_ids": HEALTH_CHECK_IDS,
            "note": "导出核心常规检查的完整结构化数据；外部插件请使用 dm check <id> 单独执行"
        },
        "summary": {
            "ok": ok_count,
            "warn": warn_status_count,
            "error": error_status_count,
            "warnings": total_warnings,
            "errors": total_errors,
            "alerts": alerts.len(),
        },
        "checks": checks,
        "alerts": alerts,
    })
}

fn build_full_health_check_with_overrides(
    overrides: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    let mut results = Vec::new();
    let mut total_warn = 0;
    let mut total_error = 0;
    let mut alerts = Vec::new();

    for id in HEALTH_CHECK_IDS {
        if let Some(mut result) = run_health_check_item(id, std::time::Duration::from_secs(12)) {
            apply_rule_overrides_to_check_result(&mut result, overrides);
            let counts = count_check_findings(&result);
            total_warn += counts.0;
            total_error += counts.1;
            alerts.extend(check_result_alerts(&result));
            results.push(serde_json::json!({
                "id": result.id,
                "name": result.name,
                "description": result.description,
                "category": result.category,
                "timestamp": result.timestamp,
                "status": result.status,
                "duration_ms": result.duration_ms,
                "section_count": result.sections.len(),
                "sections": result.sections,
                "warnings": counts.0,
                "errors": counts.1,
            }));
        }
    }

    let overall = if total_error > 0 {
        "error"
    } else if total_warn > 0 {
        "warn"
    } else {
        "ok"
    };

    let result_context = serde_json::json!(results);
    alerts.extend(custom_rule_alerts(
        overrides,
        &alerts,
        Some(&result_context),
    ));

    serde_json::json!({
        "overall_status": overall,
        "total_checks": results.len(),
        "total_warnings": total_warn,
        "total_errors": total_error,
        "checks": results,
        "alerts": dedupe_alerts(alerts),
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

pub async fn start_full_health_check(State(state): State<AppState>) -> Json<serde_json::Value> {
    let task_id = format!(
        "health-{}-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .unwrap_or_default(),
        std::process::id()
    );
    let now = alert_timestamp();
    let task = HealthTaskState {
        id: task_id.clone(),
        status: "running".to_string(),
        percent: 0,
        current_step: "准备体检任务".to_string(),
        current_check_id: String::new(),
        total: HEALTH_CHECK_IDS.len(),
        completed: 0,
        warnings: 0,
        errors: 0,
        logs: vec![format!(
            "{} 准备执行 {} 个检查项",
            now,
            HEALTH_CHECK_IDS.len()
        )],
        started_at: now.clone(),
        updated_at: now,
        result: None,
    };
    if let Ok(mut tasks) = state.health_tasks.write() {
        tasks.insert(task_id.clone(), task);
    }

    let worker_state = state.clone();
    let worker_task_id = task_id.clone();
    tokio::task::spawn_blocking(move || {
        run_full_health_task(worker_state, worker_task_id);
    });

    Json(serde_json::json!({
        "task_id": task_id,
        "status": "running",
        "total": HEALTH_CHECK_IDS.len(),
    }))
}

pub async fn get_full_health_progress(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&id, 160)?;
    let task = state
        .health_tasks
        .read()
        .ok()
        .and_then(|tasks| tasks.get(&id).cloned())
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::to_value(task).unwrap_or_default()))
}

fn run_full_health_task(state: AppState, task_id: String) {
    let mut results = Vec::new();
    let mut alerts = Vec::new();
    let mut total_warn = 0usize;
    let mut total_error = 0usize;
    let total = HEALTH_CHECK_IDS.len();
    let overrides = overrides_from_db(&state);

    for (idx, id) in HEALTH_CHECK_IDS.iter().enumerate() {
        update_health_task(&state, &task_id, |task| {
            task.status = "running".to_string();
            task.current_check_id = (*id).to_string();
            task.current_step = format!("正在执行 {}", check_display_name(id));
            task.percent = ((idx as f64 / total as f64) * 100.0).round() as u8;
            task.updated_at = alert_timestamp();
            task.logs.push(format!(
                "{} 开始 {} ({}/{})",
                task.updated_at,
                check_display_name(id),
                idx + 1,
                total
            ));
        });

        match run_health_check_item(id, std::time::Duration::from_secs(12)) {
            Some(mut result) => {
                apply_rule_overrides_to_check_result(&mut result, &overrides);
                let counts = count_check_findings(&result);
                total_warn += counts.0;
                total_error += counts.1;
                alerts.extend(check_result_alerts(&result));
                update_health_task(&state, &task_id, |task| {
                    task.completed = idx + 1;
                    task.percent = (((idx + 1) as f64 / total as f64) * 100.0).round() as u8;
                    task.warnings = total_warn;
                    task.errors = total_error;
                    task.updated_at = alert_timestamp();
                    task.logs.push(format!(
                        "{} 完成 {}，警告 {}，错误 {}，耗时 {}ms",
                        task.updated_at, result.name, counts.0, counts.1, result.duration_ms
                    ));
                    task.logs = tail_task_logs(&task.logs);
                });
                results.push(serde_json::json!({
                    "id": result.id,
                    "name": result.name,
                    "description": result.description,
                    "category": result.category,
                    "version": result.version,
                    "status": result.status,
                    "timestamp": result.timestamp,
                    "duration_ms": result.duration_ms,
                    "section_count": result.sections.len(),
                    "sections": result.sections,
                    "warnings": counts.0,
                    "errors": counts.1,
                }));
            }
            None => {
                total_warn += 1;
                update_health_task(&state, &task_id, |task| {
                    task.completed = idx + 1;
                    task.percent = (((idx + 1) as f64 / total as f64) * 100.0).round() as u8;
                    task.warnings = total_warn;
                    task.updated_at = alert_timestamp();
                    task.logs.push(format!(
                        "{} {} 无结果返回，已记录为警告",
                        task.updated_at,
                        check_display_name(id)
                    ));
                    task.logs = tail_task_logs(&task.logs);
                });
                results.push(serde_json::json!({
                    "id": id,
                    "name": check_display_name(id),
                    "status": "warn",
                    "timestamp": alert_timestamp(),
                    "duration_ms": 0,
                    "section_count": 1,
                    "sections": [{
                        "title": "检查无结果",
                        "icon": "WARN",
                        "description": "该检查项没有返回可渲染结果",
                        "items": [{
                            "type": "warning",
                            "text": "检查项没有返回结构化结果，请单独执行定位原因",
                            "details": format!("检查ID: {}\n建议命令: dm check {}", id, id)
                        }]
                    }],
                    "warnings": 1,
                    "errors": 0,
                }));
            }
        }
    }

    let overall = if total_error > 0 {
        "error"
    } else if total_warn > 0 {
        "warn"
    } else {
        "ok"
    };
    let result_context = serde_json::json!(results);
    alerts.extend(custom_rule_alerts(
        &overrides,
        &alerts,
        Some(&result_context),
    ));
    let alerts = apply_rule_overrides_to_alerts(dedupe_alerts(alerts), &overrides);
    if !alerts.is_empty() {
        state.db.upsert_active_alerts(&alerts);
    }
    let result = serde_json::json!({
        "overall_status": overall,
        "total_checks": results.len(),
        "total_warnings": total_warn,
        "total_errors": total_error,
        "checks": results,
        "alerts": alerts,
        "timestamp": alert_timestamp(),
    });

    update_health_task(&state, &task_id, |task| {
        task.status = "done".to_string();
        task.percent = 100;
        task.current_step = "体检完成".to_string();
        task.current_check_id.clear();
        task.completed = total;
        task.warnings = total_warn;
        task.errors = total_error;
        task.updated_at = alert_timestamp();
        task.logs.push(format!(
            "{} 体检完成：{} 个检查项，{} 条警告，{} 条错误",
            task.updated_at, total, total_warn, total_error
        ));
        task.logs = tail_task_logs(&task.logs);
        task.result = Some(result);
    });

    if let Ok(mut cache) = state.alert_cache.write() {
        cache.timestamp = alert_timestamp();
        cache.scans.push(serde_json::json!({
            "name": "系统体检",
            "status": "checked",
            "summary": format!("最新体检同步 {} 条告警到告警库", total_warn + total_error),
        }));
        cache.checked = cache
            .scans
            .iter()
            .filter(|s| s["status"] == "checked")
            .count();
        cache.skipped = cache
            .scans
            .iter()
            .filter(|s| s["status"] == "skipped")
            .count();
        if cache.scans.len() > 20 {
            cache.scans = cache.scans[cache.scans.len() - 20..].to_vec();
        }
    }
}

fn update_health_task(state: &AppState, task_id: &str, f: impl FnOnce(&mut HealthTaskState)) {
    if let Ok(mut tasks) = state.health_tasks.write() {
        if let Some(task) = tasks.get_mut(task_id) {
            f(task);
        }
    }
}

fn tail_task_logs(logs: &[String]) -> Vec<String> {
    let keep = 120usize;
    if logs.len() <= keep {
        logs.to_vec()
    } else {
        logs[logs.len() - keep..].to_vec()
    }
}

fn check_display_name(id: &str) -> String {
    crate::checks::list_checks()
        .into_iter()
        .find(|c| c.id == id)
        .map(|c| c.name)
        .unwrap_or_else(|| id.to_string())
}

fn run_health_check_item(
    id: &str,
    timeout: std::time::Duration,
) -> Option<crate::checks::CheckResult> {
    let id = id.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    let check_id = id.clone();
    let _ = std::thread::Builder::new()
        .name(format!("dm-health-{}", id))
        .spawn(move || {
            let _ = tx.send(crate::checks::run_check(&check_id));
        });
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Some(crate::checks::CheckResult {
            id,
            name: "检查超时".to_string(),
            description: "该检查超过健康体检单项超时时间，已跳过以保护服务可用性".to_string(),
            category: "常规检查".to_string(),
            version: "1.0.0".to_string(),
            timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            duration_ms: timeout.as_millis() as u64,
            status: crate::checks::CheckStatus::Warn,
            sections: vec![crate::checks::Section {
                title: "检查超时".to_string(),
                icon: Some("TIMEOUT".to_string()),
                description: None,
                items: vec![crate::checks::Item::Warning {
                    text: "该检查执行时间过长，健康体检已跳过它；请进入对应检查详情单独排查连接、命令或权限问题。".to_string(),
                }],
            }],
        }),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => None,
    }
}

fn alert_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn evidence_value(evidence: &[String], prefix: &str) -> Option<String> {
    evidence
        .iter()
        .find_map(|line| line.strip_prefix(prefix).map(|v| v.trim().to_string()))
}

fn infer_pid(message: &str, evidence: &[String]) -> Option<String> {
    if let Some(pid) = evidence_value(evidence, "PID:") {
        return Some(pid);
    }
    let mut parts = message.split_whitespace();
    while let Some(part) = parts.next() {
        if part == "PID" {
            return parts.next().map(|p| p.trim_matches(',').to_string());
        }
    }
    None
}

fn infer_service_name(target: &str, evidence: &[String]) -> Option<String> {
    if let Some(process) = evidence_value(evidence, "进程:") {
        return Some(process);
    }
    if target.starts_with("pid:") || target.starts_with("system.") || target.starts_with('/') {
        return None;
    }
    if target.trim().is_empty() {
        None
    } else {
        Some(target.to_string())
    }
}

fn infer_log_path(id: &str, commands: &[String]) -> Option<String> {
    for cmd in commands {
        if let Some(rest) = cmd.strip_prefix("tail -n 200 ") {
            return Some(rest.to_string());
        }
        if let Some(rest) = cmd.strip_prefix("grep -Ei 'error|critical|panic|failed|oom' ") {
            return rest.split(" | ").next().map(|s| s.to_string());
        }
    }
    id.strip_prefix("log-").and_then(|rest| {
        let path = rest.replace('-', "/");
        if path.starts_with('/') {
            Some(path)
        } else {
            None
        }
    })
}

fn alert_value(
    id: impl Into<String>,
    alert_type: &str,
    level: &str,
    title: impl Into<String>,
    message: impl Into<String>,
    evidence: Vec<String>,
    suggestions: Vec<String>,
    commands: Vec<String>,
) -> serde_json::Value {
    let id = id.into();
    let title = title.into();
    let message = message.into();
    let service_name = if alert_type == "service" {
        id.strip_prefix("service-").map(|s| s.to_string())
    } else {
        evidence_value(&evidence, "进程:")
    };
    let pid = infer_pid(&message, &evidence);
    let log_path = infer_log_path(&id, &commands);
    let handling = suggestions.first().cloned().unwrap_or_default();

    serde_json::json!({
        "id": id,
        "type": alert_type,
        "level": level,
        "title": title,
        "message": message,
        "summary": message,
        "service_name": service_name,
        "pid": pid,
        "log_path": log_path,
        "handling": handling,
        "evidence": evidence,
        "suggestions": suggestions,
        "commands": commands,
        "timestamp": alert_timestamp(),
    })
}

fn finding_alert_value(finding: &crate::anomaly::AnomalyFinding) -> serde_json::Value {
    let pid = infer_pid(&finding.summary, &finding.evidence);
    let log_path = infer_log_path(&finding.rule_id, &finding.commands);
    serde_json::json!({
        "id": finding.rule_id,
        "type": finding.category,
        "level": finding.level,
        "title": finding.title,
        "message": finding.summary,
        "summary": finding.summary,
        "service_name": infer_service_name(&finding.target, &finding.evidence),
        "pid": pid,
        "log_path": log_path,
        "handling": finding.suggestion,
        "evidence": finding.evidence,
        "suggestions": [finding.suggestion.clone()],
        "commands": finding.commands,
        "rule_id": finding.rule_id,
        "target": finding.target,
        "timestamp": alert_timestamp(),
    })
}

fn count_check_findings(result: &crate::checks::CheckResult) -> (usize, usize) {
    let mut warn = 0usize;
    let mut error = 0usize;
    for section in &result.sections {
        for item in &section.items {
            match item {
                crate::checks::Item::Label {
                    status: Some(s), ..
                }
                | crate::checks::Item::Bar {
                    status: Some(s), ..
                }
                | crate::checks::Item::Table {
                    status: Some(s), ..
                }
                | crate::checks::Item::Sparkline {
                    status: Some(s), ..
                } => {
                    if s == "error" {
                        error += 1;
                    } else if s == "warn" {
                        warn += 1;
                    }
                }
                crate::checks::Item::Warning { .. } => warn += 1,
                crate::checks::Item::Error { .. } => error += 1,
                crate::checks::Item::Finding { level, .. } => {
                    if level == "error" {
                        error += 1;
                    } else if level == "warn" {
                        warn += 1;
                    }
                }
                _ => {}
            }
        }
    }
    (warn, error)
}

fn hash_alert_key(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn check_result_alerts(result: &crate::checks::CheckResult) -> Vec<serde_json::Value> {
    let mut alerts = Vec::new();
    for section in &result.sections {
        for item in &section.items {
            match item {
                crate::checks::Item::Finding {
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
                    let finding = crate::anomaly::AnomalyFinding {
                        rule_id: rule_id.clone(),
                        level: level.clone(),
                        category: category.clone(),
                        title: title.clone(),
                        target: target.clone(),
                        summary: summary.clone(),
                        evidence: evidence.clone(),
                        suggestion: suggestion.clone(),
                        commands: commands.clone(),
                    };
                    alerts.push(finding_alert_value(&finding));
                }
                crate::checks::Item::Warning { text } => {
                    alerts.push(check_item_alert(result, section, "warn", "检查警告", text));
                }
                crate::checks::Item::Error { text } => {
                    alerts.push(check_item_alert(result, section, "error", "检查错误", text));
                }
                crate::checks::Item::Label {
                    key,
                    value,
                    status: Some(status),
                } if status == "warn" || status == "error" => {
                    alerts.push(check_item_alert(
                        result,
                        section,
                        status,
                        key,
                        &format!("{}: {}", key, value),
                    ));
                }
                crate::checks::Item::Bar {
                    key,
                    value,
                    unit,
                    status: Some(status),
                    ..
                } if status == "warn" || status == "error" => {
                    alerts.push(check_item_alert(
                        result,
                        section,
                        status,
                        key,
                        &format!("{}: {:.1}{}", key, value, unit),
                    ));
                }
                crate::checks::Item::Table {
                    headers,
                    rows,
                    status: Some(status),
                } if status == "warn" || status == "error" => {
                    let sample = rows
                        .iter()
                        .take(8)
                        .map(|row| row.join(" | "))
                        .collect::<Vec<_>>();
                    alerts.push(check_item_alert_with_evidence(
                        result,
                        section,
                        status,
                        "表格检查异常",
                        &format!("{} 存在 {} 行异常数据", section.title, rows.len()),
                        std::iter::once(format!("表头: {}", headers.join(" | ")))
                            .chain(sample)
                            .collect(),
                    ));
                }
                _ => {}
            }
        }
    }
    dedupe_alerts(alerts)
}

fn check_item_alert(
    result: &crate::checks::CheckResult,
    section: &crate::checks::Section,
    level: &str,
    title: &str,
    message: &str,
) -> serde_json::Value {
    check_item_alert_with_evidence(
        result,
        section,
        level,
        title,
        message,
        vec![
            format!("检查项: {}", result.name),
            format!("检查ID: {}", result.id),
            format!("分区: {}", section.title),
            message.to_string(),
        ],
    )
}

fn check_item_alert_with_evidence(
    result: &crate::checks::CheckResult,
    section: &crate::checks::Section,
    level: &str,
    title: &str,
    message: &str,
    evidence: Vec<String>,
) -> serde_json::Value {
    let key = format!(
        "{}|{}|{}|{}|{}",
        result.id, section.title, level, title, message
    );
    alert_value(
        format!("check-{}-{}", result.id, hash_alert_key(&key)),
        "check",
        level,
        format!("{} / {}", result.name, title),
        message.to_string(),
        evidence,
        vec![
            "进入对应检查详情页查看完整结构化结果".to_string(),
            "按异常证据定位配置、进程、端口、日志或中间件状态".to_string(),
        ],
        vec![format!("dm check {}", result.id)],
    )
}

fn overrides_from_db(state: &AppState) -> HashMap<String, serde_json::Value> {
    state
        .db
        .get_rule_overrides()
        .into_iter()
        .map(|r| (r.rule_id, r.value))
        .collect()
}

fn custom_rules_from_overrides(
    overrides: &HashMap<String, serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut rules = Vec::new();
    for (id, value) in overrides {
        if value.get("custom").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        rules.push(serde_json::json!({
            "id": id,
            "enabled": value.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
            "level": value.get("level").and_then(|v| v.as_str()).unwrap_or("warn"),
            "title": value.get("title").and_then(|v| v.as_str()).unwrap_or(id),
            "summary": value.get("summary").and_then(|v| v.as_str()).unwrap_or(""),
            "suggestion": value.get("suggestion").and_then(|v| v.as_str()).unwrap_or(""),
            "commands": value.get("commands").and_then(|v| v.as_array()).cloned().unwrap_or_default(),
            "category": value.get("category").and_then(|v| v.as_str()).unwrap_or("自定义规则"),
            "target": value.get("target").and_then(|v| v.as_str()).unwrap_or("system"),
            "condition": value.get("condition").and_then(|v| v.as_str()).unwrap_or(""),
            "description": value.get("description").and_then(|v| v.as_str()).unwrap_or("用户新增规则，保存后实时参与告警刷新。"),
            "signals": value.get("signals").and_then(|v| v.as_array()).cloned().unwrap_or_else(|| {
                vec![serde_json::json!(value.get("condition").and_then(|v| v.as_str()).unwrap_or(""))]
            }),
            "custom": true,
            "override": value,
        }));
    }
    rules.sort_by(|a, b| {
        a["id"]
            .as_str()
            .unwrap_or_default()
            .cmp(b["id"].as_str().unwrap_or_default())
    });
    rules
}

fn normalize_custom_rule_payload(
    payload: &serde_json::Value,
    existing_ids: &HashSet<String>,
) -> Result<(String, serde_json::Value), String> {
    let id = payload
        .get("id")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "规则 ID 不能为空".to_string())?;
    validate_rule_id_string(id)
        .map_err(|_| "规则 ID 只能包含字母、数字、点、横线和下划线".to_string())?;
    if existing_ids.contains(id) {
        return Err("规则 ID 已存在，请换一个 ID".to_string());
    }
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| "规则标题不能为空".to_string())?;
    if title.len() > MAX_TITLE_LEN {
        return Err("规则标题过长".to_string());
    }
    let level = payload
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("warn")
        .trim();
    if !["info", "warn", "error"].contains(&level) {
        return Err("级别只能是 info/warn/error".to_string());
    }
    let mut value = serde_json::json!({
        "custom": true,
        "enabled": payload.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        "level": level,
        "title": title,
        "category": payload.get("category").and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty()).unwrap_or("自定义规则"),
        "target": payload.get("target").and_then(|v| v.as_str()).map(str::trim).filter(|v| !v.is_empty()).unwrap_or("system"),
        "condition": payload.get("condition").and_then(|v| v.as_str()).map(str::trim).unwrap_or(""),
        "summary": payload.get("summary").and_then(|v| v.as_str()).map(str::trim).unwrap_or(""),
        "suggestion": payload.get("suggestion").and_then(|v| v.as_str()).map(str::trim).unwrap_or(""),
        "description": payload.get("description").and_then(|v| v.as_str()).map(str::trim).unwrap_or("用户新增规则"),
        "updated_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    });
    if let Some(commands) = payload.get("commands") {
        if !commands
            .as_array()
            .map(|items| items.iter().all(|v| v.as_str().is_some()))
            .unwrap_or(false)
        {
            return Err("commands 必须是字符串数组".to_string());
        }
        value["commands"] = commands.clone();
    } else {
        value["commands"] = serde_json::json!([]);
    }
    value["signals"] = serde_json::json!(value["condition"]
        .as_str()
        .unwrap_or("")
        .split([',', '\n', '|'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>());
    Ok((id.to_string(), value))
}

fn custom_rule_condition_matches(condition: &str, context: &str) -> bool {
    let tokens: Vec<String> = condition
        .split([',', '\n', '|'])
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect();
    !tokens.is_empty() && tokens.iter().all(|token| context.contains(token))
}

fn custom_rule_alerts(
    overrides: &HashMap<String, serde_json::Value>,
    existing_alerts: &[serde_json::Value],
    extra_context: Option<&serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut context = serde_json::to_string(existing_alerts)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if let Some(extra) = extra_context {
        context.push_str(
            &serde_json::to_string(extra)
                .unwrap_or_default()
                .to_ascii_lowercase(),
        );
    }
    let mut alerts = Vec::new();
    for (id, rule) in overrides {
        if rule.get("custom").and_then(|v| v.as_bool()) != Some(true)
            || matches!(rule.get("enabled").and_then(|v| v.as_bool()), Some(false))
        {
            continue;
        }
        let condition = rule.get("condition").and_then(|v| v.as_str()).unwrap_or("");
        if !custom_rule_condition_matches(condition, &context) {
            continue;
        }
        let title = rule
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(id)
            .to_string();
        let summary = rule
            .get("summary")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("自定义规则条件已命中")
            .to_string();
        let suggestion = rule
            .get("suggestion")
            .and_then(|v| v.as_str())
            .filter(|v| !v.trim().is_empty())
            .unwrap_or("查看本次检查结果与告警证据，按自定义规则建议处理")
            .to_string();
        let commands = rule
            .get("commands")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        alerts.push(alert_value(
            format!("custom-rule-{}", id),
            rule.get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("custom"),
            rule.get("level").and_then(|v| v.as_str()).unwrap_or("warn"),
            title,
            summary,
            vec![
                format!("规则ID: {}", id),
                format!("条件: {}", condition),
                "自定义规则在当前检查/告警上下文中命中".to_string(),
            ],
            vec![suggestion],
            commands,
        ));
        if let Some(last) = alerts.last_mut().and_then(|v| v.as_object_mut()) {
            last.insert("rule_id".to_string(), serde_json::json!(id));
            last.insert(
                "target".to_string(),
                rule.get("target")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!("system")),
            );
        }
    }
    alerts
}

fn infer_alert_rule_id(alert: &serde_json::Value) -> String {
    if let Some(rule_id) = alert.get("rule_id").and_then(|v| v.as_str()) {
        return rule_id.to_string();
    }
    let id = alert.get("id").and_then(|v| v.as_str()).unwrap_or_default();
    if crate::anomaly::rule_catalog()
        .iter()
        .any(|r| r.get("id").and_then(|v| v.as_str()) == Some(id))
    {
        return id.to_string();
    }
    let level = alert
        .get("level")
        .and_then(|v| v.as_str())
        .unwrap_or("warn");
    if id.starts_with("check-") || id.starts_with("export-check-missing-") {
        return if level == "error" {
            "check.generic.error"
        } else if id.contains("timeout") {
            "check.timeout.warning"
        } else {
            "check.generic.warning"
        }
        .to_string();
    }
    if id.starts_with("service-") {
        return "service.failed.error".to_string();
    }
    if id.starts_with("log-") || id == "journal-recent-warnings" {
        return "log.error.burst".to_string();
    }
    if id == "script-recent-failures" {
        return "script.failure.warning".to_string();
    }
    String::new()
}

fn rule_override_for<'a>(
    rule_id: &str,
    overrides: &'a HashMap<String, serde_json::Value>,
) -> Option<&'a serde_json::Value> {
    overrides
        .get(rule_id)
        .or_else(|| overrides.get(rule_id.rsplit_once('.').map(|v| v.0).unwrap_or("")))
}

fn apply_rule_overrides_to_alerts(
    alerts: Vec<serde_json::Value>,
    overrides: &HashMap<String, serde_json::Value>,
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for mut alert in alerts {
        let rule_id = infer_alert_rule_id(&alert);
        if !rule_id.is_empty() {
            if let Some(obj) = alert.as_object_mut() {
                obj.insert("rule_id".to_string(), serde_json::json!(rule_id));
            }
        }
        let Some(value) = rule_override_for(
            alert.get("rule_id").and_then(|v| v.as_str()).unwrap_or(""),
            overrides,
        ) else {
            out.push(alert);
            continue;
        };
        if matches!(value.get("enabled").and_then(|v| v.as_bool()), Some(false)) {
            continue;
        }
        if let Some(obj) = alert.as_object_mut() {
            if let Some(level) = value
                .get("level")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                obj.insert("level".to_string(), serde_json::json!(level));
            }
            if let Some(title) = value
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                obj.insert("title".to_string(), serde_json::json!(title));
            }
            if let Some(summary) = value
                .get("summary")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                obj.insert("summary".to_string(), serde_json::json!(summary));
                obj.insert("message".to_string(), serde_json::json!(summary));
            }
            if let Some(suggestion) = value
                .get("suggestion")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                obj.insert("handling".to_string(), serde_json::json!(suggestion));
                obj.insert(
                    "suggestions".to_string(),
                    serde_json::json!([suggestion.to_string()]),
                );
            }
            if let Some(commands) = value.get("commands").and_then(|v| v.as_array()) {
                obj.insert("commands".to_string(), serde_json::json!(commands));
            }
        }
        out.push(alert);
    }
    dedupe_alerts(out)
}

fn apply_rule_overrides_to_payload(state: &AppState, payload: &mut serde_json::Value) {
    let Some(alerts) = payload.get("alerts").and_then(|v| v.as_array()).cloned() else {
        return;
    };
    let overrides = overrides_from_db(state);
    let alerts = apply_rule_overrides_to_alerts(alerts, &overrides);
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("alerts".to_string(), serde_json::json!(alerts));
        if let Some(summary) = obj.get_mut("summary").and_then(|v| v.as_object_mut()) {
            summary.insert("alerts".to_string(), serde_json::json!(alerts.len()));
        }
    }
}

fn apply_rule_overrides_to_check_result(
    result: &mut crate::checks::CheckResult,
    overrides: &HashMap<String, serde_json::Value>,
) {
    for section in &mut result.sections {
        section.items.retain_mut(|item| {
            let crate::checks::Item::Finding {
                rule_id,
                level,
                title,
                summary,
                suggestion,
                commands,
                ..
            } = item
            else {
                return true;
            };
            let Some(value) = rule_override_for(rule_id.as_str(), overrides) else {
                return true;
            };
            if matches!(value.get("enabled").and_then(|v| v.as_bool()), Some(false)) {
                return false;
            }
            if let Some(v) = value
                .get("level")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                *level = v.to_string();
            }
            if let Some(v) = value
                .get("title")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                *title = v.to_string();
            }
            if let Some(v) = value
                .get("summary")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                *summary = v.to_string();
            }
            if let Some(v) = value
                .get("suggestion")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                *suggestion = v.to_string();
            }
            if let Some(v) = value.get("commands").and_then(|v| v.as_array()) {
                *commands = v
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect();
            }
            true
        });
    }

    let (warnings, errors) = count_check_findings(result);
    result.status = if errors > 0 {
        crate::checks::CheckStatus::Error
    } else if warnings > 0 {
        crate::checks::CheckStatus::Warn
    } else {
        crate::checks::CheckStatus::Ok
    };
}

fn dedupe_alerts(alerts: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for alert in alerts {
        let id = alert["id"].as_str().unwrap_or("unknown").to_string();
        if seen.insert(id) {
            out.push(alert);
        }
    }
    out
}

fn compact_rule_category(rule_id: &str) -> String {
    for suffix in [".critical", ".warning", ".error", ".warn", ".info"] {
        if let Some(base) = rule_id.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    rule_id.to_string()
}

fn sanitize_group_value(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn alert_group_program(alert: &serde_json::Value) -> Option<String> {
    alert
        .get("service_name")
        .and_then(|v| v.as_str())
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            alert.get("target").and_then(|v| v.as_str()).filter(|v| {
                let value = v.trim();
                !value.is_empty() && !value.starts_with("system.") && !value.starts_with('/')
            })
        })
        .or_else(|| {
            alert
                .get("log_path")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
        })
        .map(sanitize_group_value)
        .filter(|v| !v.is_empty())
}

fn alert_group_category(alert: &serde_json::Value) -> String {
    if let Some(rule_id) = alert.get("rule_id").and_then(|v| v.as_str()) {
        return compact_rule_category(rule_id);
    }
    let alert_type = alert
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("other");
    let title = alert
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    format!("{}:{}", alert_type, title)
}

fn normalize_alert_identity(mut alert: serde_json::Value) -> serde_json::Value {
    let Some(program) = alert_group_program(&alert) else {
        return alert;
    };
    let category = alert_group_category(&alert);
    let raw = format!("{}|{}", program, category);
    let canonical_id = format!("agg-{}", hash_alert_key(&raw));
    if let Some(obj) = alert.as_object_mut() {
        obj.insert("id".to_string(), serde_json::json!(canonical_id));
        obj.insert("group_program".to_string(), serde_json::json!(program));
        obj.insert("group_category".to_string(), serde_json::json!(category));
    }
    alert
}

fn normalize_alert_identities(alerts: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    alerts.into_iter().map(normalize_alert_identity).collect()
}

fn analyze_alerts(state: &AppState) -> serde_json::Value {
    let mut alerts = Vec::new();
    let mut scans = Vec::new();
    let sys = crate::dashboard::get_system_info();
    let overrides: HashMap<String, serde_json::Value> = state
        .db
        .get_rule_overrides()
        .into_iter()
        .map(|r| (r.rule_id, r.value))
        .collect();
    let engine_findings =
        crate::anomaly::apply_overrides(crate::anomaly::evaluate_system_info(&sys), &overrides);
    alerts.extend(engine_findings.iter().map(finding_alert_value));

    scans.push(serde_json::json!({
        "name": "资源水位",
        "status": "checked",
        "summary": format!(
            "规则引擎发现 {} 条资源/进程异常；CPU {:.1}%, 内存 {:.1}%, 磁盘 {:.1}%, 1分钟负载 {:.2}/{}核",
            engine_findings.len(),
            sys.cpu_usage,
            sys.memory_usage,
            sys.disk_usage,
            sys.load_avg.one,
            sys.cpu_count
        ),
    }));

    let mut check_alerts = Vec::new();
    let mut checked_names = Vec::new();
    for id in HEALTH_CHECK_IDS {
        if let Some(mut result) = run_health_check_item(id, std::time::Duration::from_secs(8)) {
            apply_rule_overrides_to_check_result(&mut result, &overrides);
            let counts = count_check_findings(&result);
            if counts.0 + counts.1 > 0 {
                check_alerts.extend(check_result_alerts(&result));
            }
            checked_names.push(format!("{}({}/{})", result.name, counts.0, counts.1));
        }
    }
    let check_alert_count = check_alerts.len();
    alerts.extend(check_alerts);
    scans.push(serde_json::json!({
        "name": "常规检查规则",
        "status": "checked",
        "summary": format!(
            "已执行 {} 个核心检查并生成 {} 条规则命中告警：{}",
            checked_names.len(),
            check_alert_count,
            checked_names.join(", ")
        ),
    }));

    /*
    if sys.cpu_usage > 90.0 {
        alerts.push(alert_value(
            "cpu-high",
            "resource",
            "error",
            "CPU 使用率过高",
            format!(
                "CPU 使用率 {:.1}%，已达到影响业务响应的风险区间",
                sys.cpu_usage
            ),
            vec![
                format!("CPU: {:.1}%", sys.cpu_usage),
                format!("核心数: {}", sys.cpu_count),
            ],
            vec![
                "优先确认是否存在突发任务、死循环或高并发请求".to_string(),
                "结合 Top 进程定位 CPU 消耗最高的服务".to_string(),
            ],
            vec![
                "top -o %CPU".to_string(),
                "ps -eo pid,ppid,comm,%cpu,%mem --sort=-%cpu | head -20".to_string(),
            ],
        ));
    } else if sys.cpu_usage > 80.0 {
        alerts.push(alert_value(
            "cpu-warn",
            "resource",
            "warn",
            "CPU 使用率较高",
            format!("CPU 使用率 {:.1}%，建议持续观察", sys.cpu_usage),
            vec![format!("CPU: {:.1}%", sys.cpu_usage)],
            vec!["观察是否持续升高，并核对最近是否有发布、批处理或备份任务".to_string()],
            vec!["vmstat 1 10".to_string(), "pidstat 1 5".to_string()],
        ));
    }

    if sys.memory_usage > 90.0 {
        alerts.push(alert_value(
            "mem-high",
            "resource",
            "error",
            "内存使用率过高",
            format!(
                "内存使用率 {:.1}%，可能引发频繁回收或 OOM",
                sys.memory_usage
            ),
            vec![
                format!("内存: {} / {} bytes", sys.memory_used, sys.memory_total),
                format!("Swap: {} / {} bytes", sys.swap_used, sys.swap_total),
            ],
            vec![
                "确认是否有单进程内存异常增长".to_string(),
                "检查系统日志中的 OOM kill 记录".to_string(),
            ],
            vec![
                "free -h".to_string(),
                "ps -eo pid,comm,rss,%mem --sort=-rss | head -20".to_string(),
                "journalctl -k -p warning --since '2 hours ago' | grep -i oom".to_string(),
            ],
        ));
    } else if sys.memory_usage > 80.0 {
        alerts.push(alert_value(
            "mem-warn",
            "resource",
            "warn",
            "内存使用率较高",
            format!(
                "内存使用率 {:.1}%，需要关注缓存、堆内存和常驻进程",
                sys.memory_usage
            ),
            vec![format!(
                "内存: {} / {} bytes",
                sys.memory_used, sys.memory_total
            )],
            vec!["比对历史趋势，确认是否为业务峰值或泄漏趋势".to_string()],
            vec![
                "free -h".to_string(),
                "ps -eo pid,comm,rss,%mem --sort=-rss | head -20".to_string(),
            ],
        ));
    }

    if sys.disk_usage > 90.0 {
        alerts.push(alert_value(
            "disk-high",
            "resource",
            "error",
            "磁盘使用率过高",
            format!(
                "整体磁盘使用率 {:.1}%，存在写入失败和服务异常风险",
                sys.disk_usage
            ),
            vec![format!(
                "磁盘: {} / {} bytes",
                sys.disk_used, sys.disk_total
            )],
            vec![
                "优先清理日志、临时文件和历史备份".to_string(),
                "确认业务数据目录是否存在异常增长".to_string(),
            ],
            vec![
                "df -h".to_string(),
                "du -xh /var/log 2>/dev/null | sort -h | tail -20".to_string(),
            ],
        ));
    } else if sys.disk_usage > 80.0 {
        alerts.push(alert_value(
            "disk-warn",
            "resource",
            "warn",
            "磁盘使用率较高",
            format!("整体磁盘使用率 {:.1}%，建议提前处置", sys.disk_usage),
            vec![format!(
                "磁盘: {} / {} bytes",
                sys.disk_used, sys.disk_total
            )],
            vec!["检查增长最快的目录，避免到达 90% 后被动处理".to_string()],
            vec![
                "df -h".to_string(),
                "du -xh / 2>/dev/null | sort -h | tail -20".to_string(),
            ],
        ));
    }

    for disk in &sys.disks {
        if disk.usage > 90.0 {
            alerts.push(alert_value(
                format!("disk-mount-high-{}", disk.mount_point.replace('/', "-")),
                "resource",
                "error",
                format!("分区 {} 空间严重不足", disk.mount_point),
                format!(
                    "{} 使用率 {:.1}%，需要立即处理",
                    disk.mount_point, disk.usage
                ),
                vec![
                    format!("挂载点: {}", disk.mount_point),
                    format!("文件系统: {}", disk.fs_type),
                    format!("已用: {} / {} bytes", disk.used, disk.total),
                ],
                vec!["优先处理该挂载点下的日志、临时文件、备份文件".to_string()],
                vec![
                    format!("df -h {}", disk.mount_point),
                    format!(
                        "du -xh {} 2>/dev/null | sort -h | tail -20",
                        disk.mount_point
                    ),
                ],
            ));
        } else if disk.usage > 80.0 {
            alerts.push(alert_value(
                format!("disk-mount-warn-{}", disk.mount_point.replace('/', "-")),
                "resource",
                "warn",
                format!("分区 {} 空间偏高", disk.mount_point),
                format!(
                    "{} 使用率 {:.1}%，建议跟进增长来源",
                    disk.mount_point, disk.usage
                ),
                vec![
                    format!("挂载点: {}", disk.mount_point),
                    format!("已用: {} / {} bytes", disk.used, disk.total),
                ],
                vec!["确认是否为可预期增长，必要时扩容或清理".to_string()],
                vec![format!("df -h {}", disk.mount_point)],
            ));
        }
    }

    if sys.load_avg.one > 0.0 {
        let cores = sys.cpu_count as f64;
        if cores > 0.0 && sys.load_avg.one / cores > 2.0 {
            alerts.push(alert_value(
                "load-high",
                "resource",
                "error",
                "系统负载过高",
                format!(
                    "1分钟负载 {:.2}，约为 CPU 核心数的 {:.1} 倍",
                    sys.load_avg.one,
                    sys.load_avg.one / cores
                ),
                vec![
                    format!(
                        "loadavg: {:.2}, {:.2}, {:.2}",
                        sys.load_avg.one, sys.load_avg.five, sys.load_avg.fifteen
                    ),
                    format!("CPU核心数: {}", sys.cpu_count),
                ],
                vec![
                    "区分 CPU 饱和、IO 等待和不可中断进程".to_string(),
                    "如果 5/15 分钟负载也偏高，说明不是瞬时抖动".to_string(),
                ],
                vec![
                    "uptime".to_string(),
                    "vmstat 1 10".to_string(),
                    "ps -eo stat,pid,comm,%cpu,%mem --sort=-%cpu | head -20".to_string(),
                ],
            ));
        } else if cores > 0.0 && sys.load_avg.one / cores > 1.0 {
            alerts.push(alert_value(
                "load-warn",
                "resource",
                "warn",
                "系统负载偏高",
                format!("1分钟负载 {:.2}，超过 CPU 核心数", sys.load_avg.one),
                vec![format!(
                    "loadavg: {:.2}, {:.2}, {:.2}",
                    sys.load_avg.one, sys.load_avg.five, sys.load_avg.fifteen
                )],
                vec!["继续观察 5 分钟和 15 分钟负载，判断是否持续".to_string()],
                vec!["uptime".to_string(), "vmstat 1 5".to_string()],
            ));
        }
    }

    for p in &sys.top_processes {
        let mem_pct = if sys.memory_total > 0 {
            (p.memory_bytes as f64 / sys.memory_total as f64) * 100.0
        } else {
            0.0
        };
        if p.cpu_usage > 85.0 {
            alerts.push(alert_value(
                format!("process-cpu-{}", p.pid),
                "process",
                "warn",
                format!("进程 {} CPU 占用过高", p.name),
                format!("PID {} 当前 CPU {:.1}%", p.pid, p.cpu_usage),
                vec![
                    format!("PID: {}", p.pid),
                    format!("进程: {}", p.name),
                    format!("CPU: {:.1}%", p.cpu_usage),
                ],
                vec![
                    "确认进程是否为核心业务服务，结合日志判断是否有异常请求或循环任务".to_string(),
                ],
                vec![
                    format!("ps -p {} -o pid,ppid,comm,%cpu,%mem,etime,args", p.pid),
                    format!("top -Hp {}", p.pid),
                ],
            ));
        }
        if mem_pct > 25.0 {
            alerts.push(alert_value(
                format!("process-mem-{}", p.pid),
                "process",
                "warn",
                format!("进程 {} 内存占用较高", p.name),
                format!("PID {} 当前内存约占 {:.1}%", p.pid, mem_pct),
                vec![
                    format!("PID: {}", p.pid),
                    format!("进程: {}", p.name),
                    format!("内存: {} bytes", p.memory_bytes),
                ],
                vec!["检查是否为预期缓存或堆内存增长，必要时抓取进程内存快照".to_string()],
                vec![
                    format!("ps -p {} -o pid,ppid,comm,rss,%mem,etime,args", p.pid),
                    format!("pmap -x {} | tail -20", p.pid),
                ],
            ));
        }
    }
    */

    let services_output = std::process::Command::new("systemctl")
        .args([
            "list-units",
            "--type=service",
            "--state=failed",
            "--no-legend",
            "--no-pager",
            "--plain",
        ])
        .output();
    if let Ok(output) = services_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut failed = 0usize;
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.first().is_some_and(|p| p.ends_with(".service")) {
                failed += 1;
                let service = parts[0];
                alerts.push(alert_value(
                    format!("service-{}", service),
                    "service",
                    "error",
                    "服务异常",
                    format!("服务 {} 处于 failed 状态", service),
                    vec![line.to_string()],
                    vec![
                        "查看服务最近日志，确认失败原因后再重启".to_string(),
                        "如果服务依赖端口或配置文件，先验证依赖状态".to_string(),
                    ],
                    vec![
                        format!("systemctl status {} --no-pager", service),
                        format!("journalctl -u {} -n 120 --no-pager", service),
                    ],
                ));
            }
        }
        scans.push(serde_json::json!({
            "name": "失败服务",
            "status": "checked",
            "summary": format!("systemctl failed services: {}", failed),
        }));
    } else {
        scans.push(serde_json::json!({
            "name": "失败服务",
            "status": "skipped",
            "summary": "systemctl 不可用或当前环境无法访问",
        }));
    }

    let mut log_scan_count = 0usize;
    let log_paths = vec!["/var/log/syslog", "/var/log/messages", "/var/log/auth.log"];
    for log_path in log_paths {
        if std::path::Path::new(log_path).exists() {
            if let Ok(output) = std::process::Command::new("tail")
                .args(["-n", "100", log_path])
                .output()
            {
                let stdout = String::from_utf8_lossy(&output.stdout);
                log_scan_count += 1;
                let matched: Vec<String> = stdout
                    .lines()
                    .filter(|l| {
                        let lower = l.to_lowercase();
                        lower.contains("error")
                            || lower.contains("critical")
                            || lower.contains("panic")
                            || lower.contains("failed")
                            || lower.contains("oom")
                    })
                    .take(12)
                    .map(|l| l.to_string())
                    .collect();
                let error_count = matched.len();
                if error_count > 5 {
                    alerts.push(alert_value(
                        format!("log-{}", log_path.replace('/', "-")),
                        "log",
                        "warn",
                        "日志异常",
                        format!("{} 最近 100 行发现多条异常关键字", log_path),
                        matched,
                        vec!["结合告警时间点查看上下文日志，确认是否持续出现".to_string()],
                        vec![
                            format!("tail -n 200 {}", log_path),
                            format!(
                                "grep -Ei 'error|critical|panic|failed|oom' {} | tail -50",
                                log_path
                            ),
                        ],
                    ));
                }
            }
        }
    }
    scans.push(serde_json::json!({
        "name": "系统日志",
        "status": "checked",
        "summary": format!("已扫描 {} 个日志文件的最近 100 行", log_scan_count),
    }));

    if let Ok(output) = std::process::Command::new("journalctl")
        .args([
            "-p",
            "warning",
            "--since",
            "2 hours ago",
            "-n",
            "80",
            "--no-pager",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<String> = stdout
            .lines()
            .filter(|l| {
                let lower = l.to_lowercase();
                lower.contains("error")
                    || lower.contains("failed")
                    || lower.contains("critical")
                    || lower.contains("oom")
                    || lower.contains("denied")
            })
            .take(12)
            .map(|l| l.to_string())
            .collect();
        scans.push(serde_json::json!({
            "name": "Journal",
            "status": "checked",
            "summary": format!("最近 2 小时匹配异常关键字 {} 条", lines.len()),
        }));
        if lines.len() >= 3 {
            alerts.push(alert_value(
                "journal-recent-warnings",
                "log",
                "warn",
                "Journal 最近存在异常记录",
                format!("最近 2 小时匹配到 {} 条 warning/error 级别异常线索", lines.len()),
                lines,
                vec!["按时间排序对照业务故障时间点，优先处理 repeated failed、OOM、permission denied 等记录".to_string()],
                vec!["journalctl -p warning --since '2 hours ago' --no-pager".to_string()],
            ));
        }
    } else {
        scans.push(serde_json::json!({
            "name": "Journal",
            "status": "skipped",
            "summary": "journalctl 不可用或权限不足",
        }));
    }

    let recent = state.db.get_history(None, 50);
    let failed_recent: Vec<_> = recent
        .iter()
        .filter(|r| r.exit_code.is_some_and(|code| code != 0))
        .take(10)
        .collect();
    scans.push(serde_json::json!({
        "name": "脚本执行历史",
        "status": "checked",
        "summary": format!("最近 {} 次执行中失败 {} 次", recent.len(), failed_recent.len()),
    }));
    if failed_recent.len() >= 2 {
        alerts.push(alert_value(
            "script-recent-failures",
            "script",
            "warn",
            "近期脚本执行存在多次失败",
            format!(
                "最近 {} 次执行中有 {} 次失败，需要核对维护动作是否未完成",
                recent.len(),
                failed_recent.len()
            ),
            failed_recent
                .iter()
                .map(|r| {
                    format!(
                        "{} / {} / exit={:?} / {}",
                        r.timestamp, r.script_id, r.exit_code, r.script_name
                    )
                })
                .collect(),
            vec![
                "打开对应脚本详情页查看最新结构化结果".to_string(),
                "优先复查连续失败或与当前故障相关的脚本".to_string(),
            ],
            vec![
                "dm logs <script_id>".to_string(),
                "dm stats <script_id>".to_string(),
            ],
        ));
    }

    let sys_context = serde_json::to_value(&sys).unwrap_or_default();
    alerts.extend(custom_rule_alerts(&overrides, &alerts, Some(&sys_context)));
    alerts = normalize_alert_identities(apply_rule_overrides_to_alerts(
        dedupe_alerts(alerts),
        &overrides,
    ));
    let error_count = alerts.iter().filter(|a| a["level"] == "error").count();
    let warn_count = alerts.iter().filter(|a| a["level"] == "warn").count();
    let total = alerts.len();
    let checked = scans.iter().filter(|s| s["status"] == "checked").count();
    let skipped = scans.iter().filter(|s| s["status"] == "skipped").count();

    serde_json::json!({
        "alerts": alerts,
        "total": total,
        "error_count": error_count,
        "warn_count": warn_count,
        "scans": scans,
        "summary": {
            "checked": checked,
            "skipped": skipped,
        },
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })
}

fn parse_json_array(raw: &str) -> Vec<serde_json::Value> {
    serde_json::from_str::<Vec<serde_json::Value>>(raw).unwrap_or_default()
}

fn alert_record_to_value(record: AlertRecord) -> serde_json::Value {
    serde_json::json!({
        "id": record.id,
        "type": record.alert_type,
        "level": record.level,
        "title": record.title,
        "message": record.message,
        "service_name": record.service_name,
        "pid": record.pid,
        "log_path": record.log_path,
        "summary": record.summary,
        "handling": record.handling,
        "evidence": parse_json_array(&record.evidence_json),
        "suggestions": parse_json_array(&record.suggestions_json),
        "commands": parse_json_array(&record.commands_json),
        "first_seen": record.first_seen,
        "last_seen": record.last_seen,
        "timestamp": record.last_seen,
        "occurrence_count": record.occurrence_count,
        "active": record.active,
    })
}

pub fn refresh_alert_cache(state: &AppState) -> serde_json::Value {
    let snapshot = analyze_alerts(state);
    let alerts = snapshot["alerts"].as_array().cloned().unwrap_or_default();
    state.db.upsert_active_alerts(&alerts);

    let scans = snapshot["scans"].as_array().cloned().unwrap_or_default();
    let timestamp = snapshot["timestamp"].as_str().unwrap_or("").to_string();
    let checked = scans.iter().filter(|s| s["status"] == "checked").count();
    let skipped = scans.iter().filter(|s| s["status"] == "skipped").count();
    if let Ok(mut cache) = state.alert_cache.write() {
        *cache = AlertCache {
            scans,
            timestamp,
            checked,
            skipped,
        };
    }

    build_alert_response(state, false, 500)
}

fn build_alert_response(
    state: &AppState,
    include_history: bool,
    limit: usize,
) -> serde_json::Value {
    let alerts: Vec<serde_json::Value> = state
        .db
        .get_alerts(!include_history, limit)
        .into_iter()
        .map(alert_record_to_value)
        .collect();
    let error_count = alerts.iter().filter(|a| a["level"] == "error").count();
    let warn_count = alerts.iter().filter(|a| a["level"] == "warn").count();
    let cache = state
        .alert_cache
        .read()
        .ok()
        .map(|c| c.clone())
        .unwrap_or_default();

    serde_json::json!({
        "alerts": alerts,
        "total": alerts.len(),
        "error_count": error_count,
        "warn_count": warn_count,
        "scans": cache.scans,
        "summary": {
            "checked": cache.checked,
            "skipped": cache.skipped,
        },
        "history": include_history,
        "timestamp": cache.timestamp,
    })
}

pub async fn get_alerts(
    State(state): State<AppState>,
    Query(query): Query<AlertsQuery>,
) -> Json<serde_json::Value> {
    if alert_cache_stale(&state) {
        let state_for_refresh = state.clone();
        if state_for_refresh
            .alert_refreshing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            tokio::task::spawn_blocking(move || {
                refresh_alert_cache(&state_for_refresh);
                state_for_refresh
                    .alert_refreshing
                    .store(false, Ordering::SeqCst);
            });
        }
    }
    Json(build_alert_response(
        &state,
        query.history.unwrap_or(false),
        query.limit.unwrap_or(500).clamp(1, 1000),
    ))
}

pub async fn clear_alerts(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if !state.db.clear_alerts() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    if let Ok(mut cache) = state.alert_cache.write() {
        *cache = AlertCache {
            scans: Vec::new(),
            timestamp: alert_timestamp(),
            checked: 0,
            skipped: 0,
        };
    }
    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "系统告警已全部清理",
        "total": 0,
        "error_count": 0,
        "warn_count": 0,
        "timestamp": alert_timestamp(),
    })))
}

fn alert_cache_stale(state: &AppState) -> bool {
    let timestamp = state
        .alert_cache
        .read()
        .ok()
        .map(|c| c.timestamp.clone())
        .unwrap_or_default();
    if timestamp.trim().is_empty() {
        return true;
    }
    let Ok(ts) = chrono::NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S") else {
        return true;
    };
    let now = chrono::Local::now().naive_local();
    now.signed_duration_since(ts).num_seconds() >= 30
}

pub async fn list_rules(State(state): State<AppState>) -> Json<serde_json::Value> {
    let overrides: HashMap<String, serde_json::Value> = state
        .db
        .get_rule_overrides()
        .into_iter()
        .map(|r| (r.rule_id, r.value))
        .collect();
    let mut rules = crate::anomaly::rule_catalog();
    rules.extend(custom_rules_from_overrides(&overrides));
    for rule in &mut rules {
        if let Some(id) = rule
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
        {
            if let Some(override_value) = overrides.get(&id) {
                if let Some(obj) = rule.as_object_mut() {
                    apply_rule_override_fields(obj, override_value);
                }
            }
        }
    }
    let categories = rules
        .iter()
        .filter_map(|r| {
            r.get("category")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect::<HashSet<_>>();
    Json(serde_json::json!({
        "rules": rules,
        "total": rules.len(),
        "categories": categories,
        "updated_at": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    }))
}

fn apply_rule_override_fields(
    rule: &mut serde_json::Map<String, serde_json::Value>,
    override_value: &serde_json::Value,
) {
    rule.insert("override".to_string(), override_value.clone());
    for key in [
        "enabled",
        "level",
        "title",
        "summary",
        "suggestion",
        "commands",
        "category",
        "target",
        "condition",
        "description",
    ] {
        if let Some(value) = override_value.get(key) {
            rule.insert(key.to_string(), value.clone());
        }
    }
}

pub async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&id, 160)?;
    validate_rule_id_string(&id)?;
    let overrides = overrides_from_db(&state);
    let exists = crate::anomaly::rule_catalog()
        .iter()
        .any(|r| r.get("id").and_then(|v| v.as_str()) == Some(id.as_str()))
        || overrides
            .get(&id)
            .is_some_and(|v| v.get("custom").and_then(|v| v.as_bool()) == Some(true));
    if !exists {
        return Err(StatusCode::NOT_FOUND);
    }
    let mut value = overrides
        .get(&id)
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    for key in RULE_CREATE_KEYS {
        if let Some(v) = req.get(key) {
            value[*key] = v.clone();
        }
    }
    if let Some(level) = value.get("level").and_then(|v| v.as_str()) {
        if !["info", "warn", "error"].contains(&level) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    if let Some(commands) = value.get("commands") {
        if !commands
            .as_array()
            .map(|items| items.iter().all(|v| v.as_str().is_some()))
            .unwrap_or(false)
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    value["updated_at"] =
        serde_json::json!(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
    if !state.db.save_rule_override(&id, &value) {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let refreshed = refresh_alert_cache(&state);
    Ok(Json(serde_json::json!({
        "status": "ok",
        "rule_id": id,
        "rule": value,
        "alerts": refreshed["total"],
        "message": "规则已保存并实时生效",
    })))
}

pub async fn create_rule(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if serde_json::to_string(&req).unwrap_or_default().len() > 128 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let overrides = overrides_from_db(&state);
    let mut existing_ids: HashSet<String> = crate::anomaly::rule_catalog()
        .iter()
        .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    existing_ids.extend(overrides.keys().cloned());
    let (id, value) =
        normalize_custom_rule_payload(&req, &existing_ids).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !state.db.save_rule_override(&id, &value) {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let refreshed = refresh_alert_cache(&state);
    Ok(Json(serde_json::json!({
        "status": "ok",
        "rule_id": id,
        "rule": value,
        "alerts": refreshed["total"],
        "message": "自定义规则已新增并实时生效",
    })))
}

fn normalize_rule_import_payload(
    payload: &serde_json::Value,
    catalog: &[serde_json::Value],
) -> RuleImportPlan {
    let catalog_ids: HashSet<String> = catalog
        .iter()
        .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    let rules = if let Some(rules) = payload.get("rules").and_then(|v| v.as_array()) {
        rules.clone()
    } else if let Some(rules) = payload.as_array() {
        rules.clone()
    } else {
        return RuleImportPlan {
            imported: Vec::new(),
            skipped: Vec::new(),
            errors: vec!["导入文件必须是 {\"rules\": [...]} 或规则数组".to_string()],
        };
    };

    let mut imported = Vec::new();
    let mut skipped = Vec::new();
    let mut errors = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        let Some(id) = rule.get("id").and_then(|v| v.as_str()).map(str::trim) else {
            errors.push(format!("第 {} 条缺少 id", index + 1));
            continue;
        };
        if id.is_empty() {
            errors.push(format!("第 {} 条 id 为空", index + 1));
            continue;
        }
        if !catalog_ids.contains(id) {
            skipped.push(id.to_string());
            continue;
        }

        let mut value = serde_json::json!({});
        for key in RULE_OVERRIDE_KEYS {
            if let Some(v) = rule.get(*key) {
                value[*key] = v.clone();
            }
        }
        if value.as_object().map(|o| o.is_empty()).unwrap_or(true) {
            skipped.push(format!("{}: 无可导入覆盖字段", id));
            continue;
        }
        if let Some(level) = value.get("level").and_then(|v| v.as_str()) {
            if !["info", "warn", "error"].contains(&level) {
                errors.push(format!("{} 的 level 不合法: {}", id, level));
                continue;
            }
        }
        if let Some(commands) = value.get("commands") {
            if !commands
                .as_array()
                .map(|items| items.iter().all(|v| v.as_str().is_some()))
                .unwrap_or(false)
            {
                errors.push(format!("{} 的 commands 必须是字符串数组", id));
                continue;
            }
        }
        if let Some(enabled) = value.get("enabled") {
            if !enabled.is_boolean() {
                errors.push(format!("{} 的 enabled 必须是布尔值", id));
                continue;
            }
        }
        value["updated_at"] =
            serde_json::json!(chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string());
        imported.push((id.to_string(), value));
    }
    RuleImportPlan {
        imported,
        skipped,
        errors,
    }
}

pub async fn import_rules(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if serde_json::to_string(&req).unwrap_or_default().len() > 512 * 1024 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let catalog = crate::anomaly::rule_catalog();
    let plan = normalize_rule_import_payload(&req, &catalog);
    if !plan.errors.is_empty() && plan.imported.is_empty() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "imported": 0,
            "skipped": plan.skipped,
            "errors": plan.errors,
            "message": "规则导入失败，请按模板修正 JSON 后重试",
        })));
    }

    let mut saved = Vec::new();
    for (id, value) in &plan.imported {
        if state.db.save_rule_override(id, value) {
            saved.push(id.clone());
        } else {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    let refreshed = refresh_alert_cache(&state);
    Ok(Json(serde_json::json!({
        "status": "ok",
        "imported": saved.len(),
        "rule_ids": saved,
        "skipped": plan.skipped,
        "errors": plan.errors,
        "alerts": refreshed["total"],
        "message": format!("已导入 {} 条规则覆盖并实时生效", plan.imported.len()),
    })))
}

pub async fn get_service_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ServiceLogsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&name, 128)?;
    let safe_name = sanitize_service_name(&name)?;
    let process_name = query
        .process
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| service_process_candidate(&safe_name));
    let mut unit_candidates = vec![safe_name.clone()];
    if !safe_name.ends_with(".service")
        && !safe_name.ends_with(".scope")
        && !safe_name.ends_with(".socket")
        && !safe_name.ends_with(".timer")
    {
        unit_candidates.push(format!("{}.service", safe_name));
    }
    let systemd_contexts = collect_systemd_log_contexts(&unit_candidates);
    let effective_pid = query
        .pid
        .or_else(|| systemd_contexts.iter().find_map(|ctx| ctx.main_pid));

    let mut parts = Vec::new();
    let mut sources = Vec::new();
    let mut tried = Vec::new();

    for unit in &unit_candidates {
        let label = format!("systemd-unit:{}", unit);
        tried.push(label.clone());
        if let Ok(output) = std::process::Command::new("journalctl")
            .args(["-u", unit, "-n", "160", "--no-pager"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() && !text.contains("No journal files were found") {
                sources.push(label.clone());
                parts.push(format!("===== {} =====\n{}", label, text));
            }
        }
    }

    for (label, args) in [
        (
            "journal-comm",
            vec!["_COMM", process_name.as_str(), "-n", "120", "--no-pager"],
        ),
        (
            "journal-tag",
            vec!["-t", process_name.as_str(), "-n", "120", "--no-pager"],
        ),
    ] {
        tried.push(format!("{}:{}", label, process_name));
        if let Ok(output) = std::process::Command::new("journalctl").args(args).output() {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() && !text.contains("No journal files were found") {
                sources.push(format!("{}:{}", label, process_name));
                parts.push(format!("===== {}:{} =====\n{}", label, process_name, text));
            }
        }
    }
    if let Some(pid) = effective_pid {
        tried.push(format!("journal-pid:{}", pid));
        if let Ok(output) = std::process::Command::new("journalctl")
            .args(["_PID", &pid.to_string(), "-n", "160", "--no-pager"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() && !text.contains("No journal files were found") {
                sources.push(format!("journal-pid:{}", pid));
                parts.push(format!("===== journal-pid:{} =====\n{}", pid, text));
            }
        }
    }

    let mut preferred_paths = Vec::new();
    if let Some(config) = state.db.get_check_config(&safe_name) {
        if let Some(path) = config.value.get("log_path").and_then(|v| v.as_str()) {
            push_log_candidate(&mut preferred_paths, path);
        }
    }
    if let Some((path, source, updated_at)) = state.db.get_service_log_cache(&safe_name) {
        tried.push(format!("cache:{}:{}", source, updated_at));
        push_log_candidate(&mut preferred_paths, path);
    }
    for ctx in &systemd_contexts {
        for source in &ctx.sources {
            tried.push(format!("systemd-show:{}:{}", ctx.unit, source));
        }
        for path in &ctx.log_paths {
            push_log_candidate(&mut preferred_paths, path);
        }
        if let Some(exec_start) = ctx.exec_start.as_deref() {
            for path in infer_service_log_paths(
                &safe_name,
                ctx.main_pid.or(effective_pid),
                Some(exec_start),
                query.category.as_deref(),
                Some(&process_name),
            ) {
                push_log_candidate(&mut preferred_paths, path);
            }
        }
    }
    let inferred_paths = infer_service_log_paths(
        &safe_name,
        effective_pid,
        query.path.as_deref(),
        query.category.as_deref(),
        Some(&process_name),
    );
    for path in inferred_paths {
        push_log_candidate(&mut preferred_paths, path);
    }
    for path in preferred_paths {
        tried.push(path.clone());
        let p = std::path::PathBuf::from(&path);
        if p.exists() && p.is_file() {
            if let Ok(output) = std::process::Command::new("tail")
                .args(["-n", "160", &path])
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !text.is_empty() {
                    sources.push(path.clone());
                    let _ = state.db.save_service_log_cache(
                        &safe_name,
                        &path,
                        "inferred-readable-path",
                    );
                    parts.push(format!("===== {} =====\n{}", path, text));
                }
            }
        }
    }
    let logs = if parts.is_empty() {
        format!(
            "未读取到日志。已尝试 systemd unit、journal _COMM/tag/_PID、命令行日志参数、程序目录 logs 以及常见 /var/log、/opt、/data 日志路径。\n\n尝试来源:\n{}",
            tried.join("\n")
        )
    } else {
        parts.join("\n\n")
    };
    Ok(Json(serde_json::json!({
        "service": safe_name,
        "logs": logs,
        "sources": sources,
        "tried": tried,
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    })))
}

fn push_log_candidate(paths: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into();
    if value.trim().is_empty() || paths.contains(&value) {
        return;
    }
    paths.push(value);
}

fn service_process_candidate(name: &str) -> String {
    let base = name
        .trim()
        .trim_end_matches(".service")
        .trim_end_matches(".scope")
        .trim_end_matches(".socket")
        .trim_end_matches(".timer");
    if base.contains('@') {
        base.split('@').next().unwrap_or(base).to_string()
    } else {
        base.to_string()
    }
}

#[derive(Debug, Default)]
struct SystemdLogContext {
    unit: String,
    main_pid: Option<u32>,
    exec_start: Option<String>,
    log_paths: Vec<String>,
    sources: Vec<String>,
}

fn collect_systemd_log_contexts(units: &[String]) -> Vec<SystemdLogContext> {
    units
        .iter()
        .filter_map(|unit| {
            let output = std::process::Command::new("systemctl")
                .args([
                    "show",
                    unit,
                    "--property=MainPID,ExecStart,FragmentPath,LogsDirectory,StandardOutput,StandardError",
                    "--no-pager",
                ])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            let mut ctx = SystemdLogContext {
                unit: unit.clone(),
                ..Default::default()
            };
            for line in text.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let value = value.trim();
                if value.is_empty() || value == "0" {
                    continue;
                }
                match key {
                    "MainPID" => {
                        ctx.main_pid = value.parse::<u32>().ok();
                        if ctx.main_pid.is_some() {
                            ctx.sources.push(format!("MainPID={}", value));
                        }
                    }
                    "ExecStart" => {
                        ctx.exec_start = Some(value.to_string());
                        ctx.sources.push("ExecStart".to_string());
                    }
                    "FragmentPath" => {
                        ctx.sources.push(format!("FragmentPath={}", value));
                    }
                    "LogsDirectory" => {
                        for item in value.split_whitespace() {
                            let dir = if item.starts_with('/') {
                                item.to_string()
                            } else {
                                format!("/var/log/{}", item.trim_matches('/'))
                            };
                            push_log_candidate(&mut ctx.log_paths, format!("{}/{}.log", dir, unit));
                            push_log_candidate(&mut ctx.log_paths, format!("{}/error.log", dir));
                            push_log_candidate(&mut ctx.log_paths, format!("{}/access.log", dir));
                        }
                        ctx.sources.push(format!("LogsDirectory={}", value));
                    }
                    "StandardOutput" | "StandardError" => {
                        if let Some(path) = value.strip_prefix("file:") {
                            push_log_candidate(&mut ctx.log_paths, path);
                            ctx.sources.push(format!("{}=file", key));
                        } else if let Some(path) = value.strip_prefix("append:") {
                            push_log_candidate(&mut ctx.log_paths, path);
                            ctx.sources.push(format!("{}=append", key));
                        }
                    }
                    _ => {}
                }
            }
            if ctx.main_pid.is_some()
                || ctx.exec_start.is_some()
                || !ctx.log_paths.is_empty()
                || !ctx.sources.is_empty()
            {
                Some(ctx)
            } else {
                None
            }
        })
        .collect()
}

fn infer_service_log_paths(
    name: &str,
    pid: Option<u32>,
    cmd: Option<&str>,
    category: Option<&str>,
    process_name: Option<&str>,
) -> Vec<String> {
    let mut paths = Vec::new();
    let name = service_process_candidate(name);
    let process_name = process_name
        .map(service_process_candidate)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| name.clone());
    let category_lower = category.unwrap_or_default().to_lowercase();
    if name != process_name {
        push_log_candidate(&mut paths, format!("/var/log/{}.log", process_name));
        push_log_candidate(
            &mut paths,
            format!("/var/log/{}/{}.log", process_name, process_name),
        );
        push_log_candidate(&mut paths, format!("/var/log/{}/error.log", process_name));
    }
    for path in [
        format!("/var/log/{}.log", name),
        format!("/var/log/{}/{}.log", name, name),
        format!("/var/log/{}/error.log", name),
        format!("/var/log/{}/access.log", name),
        format!("/opt/{}/logs/{}.log", name, name),
        format!("/opt/{}/logs/error.log", name),
        format!("/data/{}/logs/{}.log", name, name),
        format!("/data/{}/logs/error.log", name),
    ] {
        push_log_candidate(&mut paths, path);
    }
    if category_lower.contains("nginx") {
        push_log_candidate(&mut paths, "/var/log/nginx/error.log");
        push_log_candidate(&mut paths, "/usr/local/nginx/logs/error.log");
    } else if category_lower.contains("redis") {
        push_log_candidate(&mut paths, "/var/log/redis/redis-server.log");
        push_log_candidate(&mut paths, "/var/log/redis/redis.log");
    } else if category_lower.contains("mysql") {
        push_log_candidate(&mut paths, "/var/log/mysql/error.log");
        push_log_candidate(&mut paths, "/var/log/mysqld.log");
    } else if category_lower.contains("caddy") {
        push_log_candidate(&mut paths, "/var/log/caddy/access.log");
        push_log_candidate(&mut paths, "/var/log/caddy/error.log");
        push_log_candidate(&mut paths, "/var/log/caddy/caddy.log");
    }

    if let Some(cmd) = cmd {
        let mut prev = "";
        for raw in cmd.split_whitespace() {
            let token = raw
                .trim_matches('"')
                .trim_matches('\'')
                .trim_matches(';')
                .trim_matches(']');
            if token.starts_with('/') {
                add_executable_nearby_log_candidates(&mut paths, token, &name, &process_name);
            }
            if token.ends_with(".log") {
                push_log_candidate(&mut paths, token);
            }
            if matches!(
                prev,
                "-log"
                    | "--log"
                    | "--log-file"
                    | "--logfile"
                    | "--log.path"
                    | "--log.dir"
                    | "--log-directory"
                    | "--logging.path"
                    | "--logging.file.name"
                    | "--logging.file.path"
                    | "-Dlogging.file"
                    | "-Dlogging.path"
            ) {
                add_log_path_or_dir_candidate(&mut paths, token, &name, &process_name);
            }
            if let Some(value) = token
                .strip_prefix("--log-file=")
                .or_else(|| token.strip_prefix("--logfile="))
                .or_else(|| token.strip_prefix("--log.path="))
                .or_else(|| token.strip_prefix("--logging.file.name="))
                .or_else(|| token.strip_prefix("-Dlogging.file="))
                .or_else(|| token.strip_prefix("-Dlogging.file.name="))
            {
                push_log_candidate(&mut paths, value);
            }
            if let Some(dir) = token
                .strip_prefix("--log-dir=")
                .or_else(|| token.strip_prefix("--log.dir="))
                .or_else(|| token.strip_prefix("--log-directory="))
                .or_else(|| token.strip_prefix("--logging.path="))
                .or_else(|| token.strip_prefix("--logging.file.path="))
                .or_else(|| token.strip_prefix("-Dlogging.file.path="))
                .or_else(|| token.strip_prefix("-Dlogging.path="))
                .or_else(|| token.strip_prefix("-Dserver.tomcat.accesslog.directory="))
            {
                add_log_path_or_dir_candidate(&mut paths, dir, &name, &process_name);
            }
            if token.contains("/logs/") {
                if token.ends_with(".log") {
                    push_log_candidate(&mut paths, token);
                } else {
                    push_log_candidate(
                        &mut paths,
                        format!("{}/{}.log", token.trim_end_matches('/'), name),
                    );
                    push_log_candidate(
                        &mut paths,
                        format!("{}/error.log", token.trim_end_matches('/')),
                    );
                }
            }
            prev = token;
        }
    }

    if let Some(pid) = pid {
        if let Ok(link) = std::fs::read_link(format!("/proc/{}/cwd", pid)) {
            let cwd = link.display().to_string();
            push_log_candidate(&mut paths, format!("{}/logs/{}.log", cwd, name));
            push_log_candidate(&mut paths, format!("{}/logs/{}.log", cwd, process_name));
            push_log_candidate(&mut paths, format!("{}/logs/error.log", cwd));
        }
    }

    if let Ok(output) = std::process::Command::new("find")
        .args([
            "/var/log",
            "/opt",
            "/data",
            "-maxdepth",
            "4",
            "-type",
            "f",
            "(",
            "-iname",
            &format!("*{}*.log", name),
            "-o",
            "-path",
            &format!("*{}*/logs/*", name),
            ")",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines().take(16) {
            push_log_candidate(&mut paths, line);
        }
    }
    paths
}

fn add_log_path_or_dir_candidate(
    paths: &mut Vec<String>,
    value: &str,
    name: &str,
    process_name: &str,
) {
    if value.ends_with(".log") {
        push_log_candidate(paths, value);
        return;
    }
    let dir = value.trim_end_matches('/');
    push_log_candidate(paths, format!("{}/{}.log", dir, name));
    push_log_candidate(paths, format!("{}/{}.log", dir, process_name));
    push_log_candidate(paths, format!("{}/error.log", dir));
    push_log_candidate(paths, format!("{}/access.log", dir));
}

fn add_executable_nearby_log_candidates(
    paths: &mut Vec<String>,
    value: &str,
    name: &str,
    process_name: &str,
) {
    if value.ends_with(".log") || value.contains("/logs/") {
        add_log_path_or_dir_candidate(paths, value, name, process_name);
        return;
    }
    let path = std::path::Path::new(value);
    let Some(dir) = path.parent() else {
        return;
    };
    for candidate_dir in [
        dir.join("logs"),
        dir.parent()
            .map(|parent| parent.join("logs"))
            .unwrap_or_else(|| dir.join("logs")),
        dir.join("../logs"),
    ] {
        let candidate_dir = candidate_dir.display().to_string();
        add_log_path_or_dir_candidate(paths, &candidate_dir, name, process_name);
    }
}

fn normalize_process_status(stat: &str) -> String {
    let code = stat.chars().next();
    let label = match code {
        Some('R') => "运行",
        Some('S') => "睡眠",
        Some('D') => "等待IO",
        Some('T') | Some('t') => "停止",
        Some('Z') => "僵尸",
        Some('I') => "空闲",
        Some('W') => "分页",
        Some('X') | Some('x') => "结束中",
        Some('K') => "唤醒中",
        Some('P') => "暂停",
        _ => "未知",
    };
    if stat.trim().is_empty() {
        label.to_string()
    } else {
        format!("{}({})", label, stat)
    }
}

fn infer_process_path(pid: u32, cmd: &str, name: &str) -> String {
    if let Ok(path) = std::fs::read_link(format!("/proc/{}/exe", pid)) {
        let path = path.display().to_string();
        if !path.trim().is_empty() {
            return path;
        }
    }
    let first = cmd.split_whitespace().next().unwrap_or_default();
    if first.starts_with('/') {
        return first
            .trim_matches('"')
            .trim_matches('\'')
            .trim_end_matches(":")
            .to_string();
    }
    if !cmd.trim().is_empty() {
        cmd.trim().to_string()
    } else if !name.trim().is_empty() {
        name.trim().to_string()
    } else {
        "unknown".to_string()
    }
}

pub async fn check_service_health(
    State(_state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<ServiceHealthQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    validate_non_empty(&name, 128)?;
    let safe_name = sanitize_service_name(&name)?;
    let process_name = query
        .process
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .map(|v| service_process_candidate(v.trim()))
        .unwrap_or_else(|| service_process_candidate(&safe_name));

    let mut health_info = serde_json::json!({
        "service": safe_name,
        "process": process_name.clone(),
        "query_context": {
            "pid": query.pid,
            "path": query.path.clone(),
            "category": query.category.clone(),
            "ports": query.ports.clone(),
        },
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    });

    let unit_name = if safe_name.ends_with(".service") {
        safe_name.clone()
    } else {
        format!("{}.service", safe_name)
    };
    let systemctl_output = std::process::Command::new("systemctl")
        .args(["is-active", &unit_name])
        .output();

    if let Ok(output) = systemctl_output {
        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
        health_info["systemd_status"] = serde_json::json!(status);
        health_info["is_active"] = serde_json::json!(status == "active");
    }

    if let Ok(output) = std::process::Command::new("systemctl")
        .args([
            "show",
            &unit_name,
            "--property=Id,LoadState,ActiveState,SubState,MainPID,ExecMainStatus,RestartUSec,FragmentPath",
            "--no-pager",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let mut props = serde_json::Map::new();
        let mut systemd_pids = Vec::new();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            props.insert(key.to_string(), serde_json::json!(value));
            if matches!(key, "MainPID" | "ControlPID") {
                if value != "0" && !value.trim().is_empty() {
                    systemd_pids.push(value.to_string());
                }
            }
        }
        if !props.is_empty() {
            health_info["systemd_properties"] = serde_json::json!(props);
        }
        if !systemd_pids.is_empty() {
            health_info["systemd_pids"] = serde_json::json!(systemd_pids);
        }
    }

    let mut pid_values: Vec<String> = health_info["systemd_pids"]
        .as_array()
        .map(|pids| {
            pids.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let has_query_pid = query.pid.is_some_and(|pid| pid > 0);
    if let Some(pid) = query.pid {
        if pid > 0 {
            pid_values.push(pid.to_string());
        }
    }
    let pid_output = if has_query_pid {
        None
    } else {
        std::process::Command::new("pgrep")
            .args(["-f", &process_name])
            .output()
            .ok()
    };

    if let Some(output) = pid_output {
        let pids = String::from_utf8_lossy(&output.stdout).trim().to_string();
        pid_values.extend(
            pids.lines()
                .filter(|l| !l.is_empty())
                .map(|v| v.to_string()),
        );
        pid_values.sort();
        pid_values.dedup();
        pid_values.retain(|pid| {
            pid.parse::<u32>()
                .ok()
                .is_some_and(|value| std::path::Path::new(&format!("/proc/{value}")).exists())
        });
        health_info["pids"] = serde_json::json!(pid_values);
        health_info["process_count"] = serde_json::json!(pid_values.len());
        health_info["is_running"] = serde_json::json!(!pid_values.is_empty());

        let mut process_rows = Vec::new();
        for pid in &pid_values {
            if let Ok(ps) = std::process::Command::new("ps")
                .args([
                    "-p",
                    pid,
                    "-o",
                    "pid=,ppid=,%cpu=,%mem=,rss=,etime=,stat=,comm=,args=",
                ])
                .output()
            {
                let line = String::from_utf8_lossy(&ps.stdout).trim().to_string();
                if !line.is_empty() {
                    let mut cols = line.split_whitespace();
                    let pid_col = cols.next().unwrap_or("");
                    let ppid = cols.next().unwrap_or("");
                    let cpu = cols.next().unwrap_or("");
                    let mem = cols.next().unwrap_or("");
                    let rss = cols.next().unwrap_or("");
                    let etime = cols.next().unwrap_or("");
                    let stat = cols.next().unwrap_or("");
                    let comm = cols.next().unwrap_or("");
                    let args = cols.collect::<Vec<_>>().join(" ");
                    process_rows.push(serde_json::json!({
                        "pid": pid_col,
                        "ppid": ppid,
                        "cpu": cpu,
                        "memory": mem,
                        "rss_kb": rss,
                        "etime": etime,
                        "stat": stat,
                        "command": comm,
                        "args": args,
                    }));
                }
            }
        }
        health_info["processes"] = serde_json::json!(process_rows);
    } else if pid_values.is_empty() {
        health_info["pids"] = serde_json::json!([]);
        health_info["process_count"] = serde_json::json!(0);
        health_info["is_running"] = serde_json::json!(false);
    } else {
        pid_values.sort();
        pid_values.dedup();
        pid_values.retain(|pid| {
            pid.parse::<u32>()
                .ok()
                .is_some_and(|value| std::path::Path::new(&format!("/proc/{value}")).exists())
        });
        health_info["pids"] = serde_json::json!(pid_values);
        health_info["process_count"] = serde_json::json!(pid_values.len());
        health_info["is_running"] = serde_json::json!(!pid_values.is_empty());

        let mut process_rows = Vec::new();
        for pid in &pid_values {
            if let Ok(ps) = std::process::Command::new("ps")
                .args([
                    "-p",
                    pid,
                    "-o",
                    "pid=,ppid=,%cpu=,%mem=,rss=,etime=,stat=,comm=,args=",
                ])
                .output()
            {
                let line = String::from_utf8_lossy(&ps.stdout).trim().to_string();
                if !line.is_empty() {
                    let mut cols = line.split_whitespace();
                    let pid_col = cols.next().unwrap_or("");
                    let ppid = cols.next().unwrap_or("");
                    let cpu = cols.next().unwrap_or("");
                    let mem = cols.next().unwrap_or("");
                    let rss = cols.next().unwrap_or("");
                    let etime = cols.next().unwrap_or("");
                    let stat = cols.next().unwrap_or("");
                    let comm = cols.next().unwrap_or("");
                    let args = cols.collect::<Vec<_>>().join(" ");
                    process_rows.push(serde_json::json!({
                        "pid": pid_col,
                        "ppid": ppid,
                        "cpu": cpu,
                        "memory": mem,
                        "rss_kb": rss,
                        "etime": etime,
                        "stat": stat,
                        "command": comm,
                        "args": args,
                    }));
                }
            }
        }
        health_info["processes"] = serde_json::json!(process_rows);
    }

    let port_output = std::process::Command::new("ss").args(["-tulnp"]).output();
    if let Ok(output) = port_output {
        let ss_info = String::from_utf8_lossy(&output.stdout);
        let ports: Vec<String> = ss_info
            .lines()
            .filter(|l| {
                l.contains(&safe_name)
                    || l.contains(&process_name)
                    || health_info["pids"].as_array().is_some_and(|pids| {
                        pids.iter()
                            .any(|p| p.as_str().is_some_and(|pid| l.contains(pid)))
                    })
            })
            .map(|line| line.to_string())
            .collect();
        health_info["listening_ports"] = serde_json::json!(ports);
    } else {
        health_info["listening_ports"] = serde_json::json!([]);
    }

    if let Some(path) = query.path.as_deref().filter(|v| !v.trim().is_empty()) {
        let executable = path
            .split('|')
            .find_map(|part| part.trim().strip_prefix("exe=").map(str::trim))
            .unwrap_or(path.trim());
        let exists = std::path::Path::new(executable).exists();
        health_info["executable_path"] = serde_json::json!(executable);
        health_info["executable_exists"] = serde_json::json!(exists);
    }

    let is_running = health_info["is_running"].as_bool().unwrap_or(false);
    let is_active = health_info["is_active"].as_bool().unwrap_or(false);
    let systemd_load_state = health_info["systemd_properties"]["LoadState"]
        .as_str()
        .unwrap_or_default();
    let has_systemd_unit = !systemd_load_state.is_empty() && systemd_load_state != "not-found";
    let list_context_running = query.pid.filter(|pid| *pid > 0).is_some()
        || query
            .ports
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty() && v.trim() != "-")
        || query
            .path
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty() && v.trim() != "-")
        || query
            .category
            .as_deref()
            .is_some_and(|v| v.contains("进程") || v.contains("应用") || v.contains("端口"));
    let mut issues = Vec::new();
    if has_systemd_unit && !is_active && !is_running {
        issues.push("systemd 未处于 active 状态".to_string());
    }
    if !is_running {
        issues.push("未发现关联运行进程".to_string());
    }
    let missing_port = health_info["listening_ports"]
        .as_array()
        .map(|v| v.is_empty())
        .unwrap_or(true);
    if missing_port
        && query
            .ports
            .as_deref()
            .is_some_and(|v| !v.trim().is_empty() && v.trim() != "-")
    {
        issues.push("未发现关联监听端口".to_string());
    }
    if let Ok(output) = std::process::Command::new("journalctl")
        .args([
            "-u",
            &unit_name,
            "-n",
            "80",
            "--no-pager",
            "-p",
            "warning..alert",
        ])
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let log_issues: Vec<String> = text
            .lines()
            .filter(|line| {
                let lower = line.to_lowercase();
                lower.contains("error")
                    || lower.contains("exception")
                    || lower.contains("failed")
                    || lower.contains("panic")
                    || lower.contains("fatal")
                    || lower.contains("oom")
                    || lower.contains("timeout")
            })
            .take(8)
            .map(|line| line.to_string())
            .collect();
        if !log_issues.is_empty() {
            issues.push(format!("最近日志存在 {} 条异常关键字", log_issues.len()));
        }
        health_info["recent_log_issues"] = serde_json::json!(log_issues);
    }
    health_info["issues"] = serde_json::json!(issues);
    health_info["recommendations"] = serde_json::json!([
        format!("systemctl status {}", unit_name),
        format!("journalctl -u {} -n 200 --no-pager", unit_name),
        format!(
            "ss -tulnp | grep -E '{}|{}'",
            safe_name,
            health_info["pids"]
                .as_array()
                .map(|pids| pids
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("|"))
                .unwrap_or_default()
        ),
    ]);

    let effective_status = if is_running && (!has_systemd_unit || is_active || list_context_running)
    {
        "running"
    } else if is_running {
        "degraded"
    } else {
        "stopped"
    };
    let status_source = if has_systemd_unit && is_active {
        "systemd"
    } else if is_running {
        "process"
    } else {
        "unknown"
    };
    health_info["effective_status"] = serde_json::json!(effective_status);
    health_info["status_source"] = serde_json::json!(status_source);
    health_info["systemd_unit_found"] = serde_json::json!(has_systemd_unit);

    if effective_status == "running" {
        health_info["status"] = serde_json::json!("ok");
        let source_label = if status_source == "systemd" {
            "systemd active"
        } else {
            "进程/PID/端口"
        };
        health_info["message"] = serde_json::json!(format!(
            "服务 {} 运行正常，状态来源: {}",
            safe_name, source_label
        ));
    } else if effective_status == "degraded" {
        health_info["status"] = serde_json::json!("warn");
        health_info["message"] = serde_json::json!(format!(
            "服务 {} 进程存在，但 systemd 状态不一致",
            safe_name
        ));
    } else {
        health_info["status"] = serde_json::json!("error");
        health_info["message"] = serde_json::json!(format!("服务 {} 未运行", safe_name));
    }

    Ok(Json(health_info))
}

pub async fn get_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "port": state.config.port,
        "bind": state.config.bind,
        "allow_remote": state.config.bind == "0.0.0.0",
        "log_dir": state.config.log_dir.display().to_string(),
        "data_dir": state.config.data_dir.display().to_string(),
        "scripts_dir": state.config.user_scripts_dir.display().to_string(),
        "theme": state.config.theme,
        "language": state.config.language,
        "about": {
            "version": env!("CARGO_PKG_VERSION"),
            "author": "xuning",
            "email": "gokeeps@qq.com",
            "license": "MIT"
        }
    }))
}

fn migrate_dir(old: &std::path::Path, new: &std::path::Path) -> Result<(), std::io::Error> {
    if old == new {
        std::fs::create_dir_all(new)?;
        return Ok(());
    }
    std::fs::create_dir_all(new)?;
    if !old.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(old)? {
        let entry = entry?;
        let src = entry.path();
        let dst = new.join(entry.file_name());
        if dst.exists() {
            continue;
        }
        if src.is_dir() {
            copy_dir_recursive(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if !target.exists() {
            std::fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

pub async fn update_config(
    State(state): State<AppState>,
    Json(req): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut config = state.config.clone();
    if let Some(port) = req.get("port").and_then(|v| v.as_u64()) {
        if port == 0 || port > u16::MAX as u64 {
            return Err(StatusCode::BAD_REQUEST);
        }
        config.port = port as u16;
    }
    let allow_remote = req
        .get("allow_remote")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            req.get("bind")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0.0")
                != "127.0.0.1"
        });
    config.bind = if allow_remote { "0.0.0.0" } else { "127.0.0.1" }.to_string();
    if let Some(theme) = req.get("theme").and_then(|v| v.as_str()) {
        config.theme = theme.to_string();
    }
    if let Some(lang) = req.get("language").and_then(|v| v.as_str()) {
        config.language = lang.to_string();
    }
    if let Some(path) = req.get("log_dir").and_then(|v| v.as_str()) {
        if !path.trim().is_empty() {
            config.log_dir = std::path::PathBuf::from(path);
        }
    }
    if let Some(path) = req.get("scripts_dir").and_then(|v| v.as_str()) {
        if !path.trim().is_empty() {
            config.user_scripts_dir = std::path::PathBuf::from(path);
        }
    }
    if let Some(path) = req.get("data_dir").and_then(|v| v.as_str()) {
        if !path.trim().is_empty() {
            config.data_dir = std::path::PathBuf::from(path);
        }
    }

    migrate_dir(&state.config.log_dir, &config.log_dir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    migrate_dir(&state.config.user_scripts_dir, &config.user_scripts_dir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    migrate_dir(&state.config.data_dir, &config.data_dir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let legacy_db = state.config.log_dir.join("dm.db");
    let next_db = config.data_dir.join("dm.db");
    if legacy_db.exists() && !next_db.exists() {
        std::fs::create_dir_all(&config.data_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        std::fs::copy(&legacy_db, &next_db).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    match config.save() {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "ok",
            "message": "配置已保存，目录数据已同步迁移",
            "config": {
                "port": config.port,
                "bind": config.bind,
                "allow_remote": config.bind == "0.0.0.0",
                "log_dir": config.log_dir.display().to_string(),
                "data_dir": config.data_dir.display().to_string(),
                "scripts_dir": config.user_scripts_dir.display().to_string(),
                "theme": config.theme,
                "language": config.language
            }
        }))),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_all_processes(State(_state): State<AppState>) -> Json<serde_json::Value> {
    let output = tokio::task::spawn_blocking(|| {
        std::process::Command::new("ps")
            .args(["-eo", "pid=,comm=,%cpu=,rss=,stat=,args="])
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let mut processes: Vec<serde_json::Value> = output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pid = parts.next()?.parse::<u32>().ok()?;
            let name = parts.next()?.to_string();
            let cpu_usage = parts
                .next()
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0);
            let rss_kb = parts
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            let stat = parts.next().unwrap_or_default();
            let cmd = parts.collect::<Vec<_>>().join(" ");
            let status = normalize_process_status(stat);
            let path = infer_process_path(pid, &cmd, &name);
            Some(serde_json::json!({
                "pid": pid,
                "name": name,
                "cmd": cmd,
                "path": path,
                "cpu_usage": cpu_usage,
                "memory_bytes": rss_kb.saturating_mul(1024),
                "status": status,
            }))
        })
        .collect();

    processes.sort_by(|a, b| {
        let a_cpu = a["cpu_usage"].as_f64().unwrap_or(0.0);
        let b_cpu = b["cpu_usage"].as_f64().unwrap_or(0.0);
        let a_mem = a["memory_bytes"].as_u64().unwrap_or(0);
        let b_mem = b["memory_bytes"].as_u64().unwrap_or(0);
        let score_a = a_cpu * 0.6 + (a_mem as f64 / 1024.0 / 1024.0) * 0.4;
        let score_b = b_cpu * 0.6 + (b_mem as f64 / 1024.0 / 1024.0) * 0.4;
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Json(serde_json::json!({
        "processes": processes,
        "total": processes.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_interface_classification_prefers_public_then_physical_then_virtual() {
        let public = classify_network_interface("eth0", "8.8.8.8");
        let physical = classify_network_interface("ens192", "10.0.0.12");
        let other = classify_network_interface("ib0", "172.16.0.4");
        let virtual_iface = classify_network_interface("docker0", "172.17.0.1");

        assert_eq!(public.kind, "public");
        assert_eq!(physical.kind, "physical");
        assert_eq!(other.kind, "other");
        assert_eq!(virtual_iface.kind, "virtual");
        assert!(public.priority < physical.priority);
        assert!(physical.priority < other.priority);
        assert!(other.priority < virtual_iface.priority);
    }

    #[test]
    fn private_or_loopback_addresses_are_not_public() {
        assert!(!is_public_ip("10.0.0.1"));
        assert!(!is_public_ip("172.16.1.10"));
        assert!(!is_public_ip("192.168.1.10"));
        assert!(!is_public_ip("127.0.0.1"));
        assert!(is_public_ip("1.1.1.1/24"));
    }

    #[test]
    fn loopback_interface_is_local_loopback_not_virtual() {
        let loopback = classify_network_interface("lo", "127.0.0.1");

        assert_eq!(loopback.kind, "loopback");
        assert_eq!(loopback.label, "本地回环");
        assert!(!loopback.is_virtual);
        assert!(!loopback.is_public);
    }

    #[test]
    fn rule_import_accepts_template_shape_and_skips_unknown_rules() {
        let catalog = crate::anomaly::rule_catalog();
        let payload = serde_json::json!({
            "rules": [
                {
                    "id": "resource.cpu.warning",
                    "enabled": false,
                    "level": "error",
                    "title": "CPU 自定义标题",
                    "commands": ["top -o %CPU"]
                },
                {
                    "id": "missing.rule",
                    "enabled": true
                }
            ]
        });
        let plan = normalize_rule_import_payload(&payload, &catalog);
        assert_eq!(plan.imported.len(), 1);
        assert_eq!(plan.imported[0].0, "resource.cpu.warning");
        assert_eq!(plan.imported[0].1["enabled"], false);
        assert_eq!(plan.imported[0].1["level"], "error");
        assert_eq!(plan.skipped, vec!["missing.rule".to_string()]);
        assert!(plan.errors.is_empty());
    }

    #[test]
    fn rule_import_rejects_invalid_level_and_commands() {
        let catalog = crate::anomaly::rule_catalog();
        let payload = serde_json::json!({
            "rules": [
                { "id": "resource.cpu.warning", "level": "fatal" },
                { "id": "resource.memory.warning", "commands": ["free -h", 42] }
            ]
        });
        let plan = normalize_rule_import_payload(&payload, &catalog);
        assert!(plan.imported.is_empty());
        assert_eq!(plan.errors.len(), 2);
    }

    #[test]
    fn custom_rule_payload_creates_enabled_rule() {
        let existing = HashSet::new();
        let payload = serde_json::json!({
            "id": "custom.nginx.5xx",
            "title": "Nginx 5xx 激增",
            "level": "error",
            "category": "nginx",
            "target": "nginx",
            "condition": "nginx, 5xx",
            "summary": "Nginx 5xx 异常",
            "suggestion": "查看 access/error 日志",
            "commands": ["tail -n 200 /var/log/nginx/error.log"]
        });
        let (id, value) = normalize_custom_rule_payload(&payload, &existing).unwrap();
        assert_eq!(id, "custom.nginx.5xx");
        assert_eq!(value["custom"], true);
        assert_eq!(value["enabled"], true);
        assert_eq!(value["level"], "error");
        assert_eq!(value["signals"][0], "nginx");
        assert_eq!(value["signals"][1], "5xx");
    }

    #[test]
    fn custom_rule_payload_rejects_duplicate_id() {
        let mut existing = HashSet::new();
        existing.insert("custom.nginx.5xx".to_string());
        let payload = serde_json::json!({
            "id": "custom.nginx.5xx",
            "title": "重复规则"
        });
        assert!(normalize_custom_rule_payload(&payload, &existing).is_err());
    }

    #[test]
    fn custom_rule_alert_matches_context_tokens() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "custom.redis.memory".to_string(),
            serde_json::json!({
                "custom": true,
                "enabled": true,
                "level": "warn",
                "title": "Redis 内存异常",
                "category": "redis",
                "condition": "redis,memory",
                "summary": "Redis memory 告警",
                "suggestion": "检查 Redis maxmemory",
                "commands": ["redis-cli info memory"]
            }),
        );
        let alerts = vec![serde_json::json!({
            "id": "redis-memory-source",
            "title": "redis memory high",
            "message": "redis memory usage is high"
        })];
        let matched = custom_rule_alerts(&overrides, &alerts, None);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0]["rule_id"], "custom.redis.memory");
        assert_eq!(matched[0]["level"], "warn");
    }

    #[test]
    fn rule_override_merges_editable_metadata_fields() {
        let mut rule = serde_json::json!({
            "id": "resource.cpu.warning",
            "title": "CPU 默认",
            "category": "resource",
            "target": "cpu",
            "condition": "cpu > 80",
            "description": "默认描述"
        })
        .as_object()
        .cloned()
        .unwrap();
        let override_value = serde_json::json!({
            "title": "CPU 自定义",
            "category": "runtime",
            "target": "java",
            "condition": "cpu > 70 && thread blocked",
            "description": "刷新后必须继续显示的编辑内容"
        });

        apply_rule_override_fields(&mut rule, &override_value);

        assert_eq!(rule["title"], "CPU 自定义");
        assert_eq!(rule["category"], "runtime");
        assert_eq!(rule["target"], "java");
        assert_eq!(rule["condition"], "cpu > 70 && thread blocked");
        assert_eq!(rule["description"], "刷新后必须继续显示的编辑内容");
        assert_eq!(rule["override"]["target"], "java");
    }

    #[test]
    fn process_status_keeps_real_ps_state_codes() {
        assert_eq!(normalize_process_status("Rsl"), "运行(Rsl)");
        assert_eq!(normalize_process_status("Ss"), "睡眠(Ss)");
        assert_eq!(normalize_process_status("D"), "等待IO(D)");
        assert_eq!(normalize_process_status("Z+"), "僵尸(Z+)");
        assert_eq!(normalize_process_status(""), "未知");
    }

    #[test]
    fn service_log_inference_uses_command_flags_and_executable_directory() {
        let paths = infer_service_log_paths(
            "demo.service",
            None,
            Some("/opt/demo/bin/demo --log.dir=/data/demo/logs --logging.file.name=/tmp/demo/app.log -Dserver.tomcat.accesslog.directory=/var/log/demo"),
            Some("java"),
            Some("demo"),
        );

        assert!(paths.contains(&"/data/demo/logs/demo.log".to_string()));
        assert!(paths.contains(&"/data/demo/logs/error.log".to_string()));
        assert!(paths.contains(&"/tmp/demo/app.log".to_string()));
        assert!(paths.contains(&"/var/log/demo/access.log".to_string()));
        assert!(paths.contains(&"/opt/demo/bin/logs/demo.log".to_string()));
        assert!(paths.contains(&"/opt/demo/logs/demo.log".to_string()));
    }

    #[test]
    fn check_config_import_accepts_configs_object_and_skips_unknown() {
        let payload = serde_json::json!({
            "version": 1,
            "configs": {
                "elasticsearch": {
                    "url": "http://10.0.0.10:9200",
                    "username": "elastic"
                },
                "redis": {
                    "host": "10.0.0.11",
                    "port": "6379"
                },
                "kafka": {
                    "host": "10.0.0.12",
                    "port": "9092",
                    "config_path": "/etc/kafka/server.properties"
                },
                "unknown": {
                    "host": "127.0.0.1"
                }
            }
        });
        let (imported, skipped, errors) = normalize_check_config_import_payload(&payload);
        assert_eq!(imported.len(), 3);
        let ids: std::collections::HashSet<_> =
            imported.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains("elasticsearch"));
        assert!(ids.contains("redis"));
        assert!(ids.contains("kafka"));
        assert_eq!(skipped, vec!["unknown".to_string()]);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_request_params_are_preserved_for_history() {
        let mut params = HashMap::new();
        params.insert("target".to_string(), "nginx".to_string());
        params.insert("dry_run".to_string(), "true".to_string());
        let req = RunRequest {
            params,
            args: vec!["--verbose".to_string()],
        };
        let value = run_request_params_value(&req);
        assert_eq!(value["target"], "nginx");
        assert_eq!(value["dry_run"], "true");
        assert_eq!(req.args, vec!["--verbose"]);
    }

    #[test]
    fn update_script_request_accepts_param_definitions() {
        let req: UpdateScriptRequest = serde_json::from_value(serde_json::json!({
            "name": "参数脚本",
            "params": [
                {
                    "name": "target",
                    "description": "目标服务",
                    "type": "string",
                    "default": "nginx",
                    "required": true
                }
            ]
        }))
        .expect("request should deserialize");
        let params = req.params.expect("params provided");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "target");
        assert_eq!(params[0].param_type, "string");
        assert_eq!(params[0].default.as_deref(), Some("nginx"));
        assert!(params[0].required);
    }

    #[test]
    fn upload_param_json_parser_accepts_valid_array() {
        let params = parse_script_params_json(
            r#"[{"name":"target","description":"目标服务","type":"string","default":"nginx","required":true}]"#,
        )
        .expect("valid upload params should parse");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "target");
        assert_eq!(params[0].param_type, "string");
        assert_eq!(params[0].default.as_deref(), Some("nginx"));
        assert!(params[0].required);
        assert!(parse_script_params_json("").unwrap().is_empty());
        assert!(parse_script_params_json(r#"{"name":"target"}"#).is_err());
    }

    #[test]
    fn script_extension_parser_keeps_supported_executable_types_safe() {
        assert_eq!(
            script_extension_from_filename("repair.redis.py").unwrap(),
            "py"
        );
        assert_eq!(
            script_extension_from_filename("cleanup.PERL").unwrap(),
            "perl"
        );
        assert_eq!(script_extension_from_filename("restart").unwrap(), "sh");
        assert!(script_extension_from_filename("../bad.sh").is_err());
        assert!(script_extension_from_filename("bad.txt").is_err());
    }

    #[test]
    fn script_param_validation_rejects_invalid_definitions() {
        let invalid_type = vec![ScriptParam {
            name: "target".to_string(),
            description: String::new(),
            param_type: "object".to_string(),
            default: None,
            required: false,
        }];
        assert!(validate_script_params(&invalid_type).is_err());

        let duplicate = vec![
            ScriptParam {
                name: "target".to_string(),
                description: String::new(),
                param_type: "string".to_string(),
                default: None,
                required: false,
            },
            ScriptParam {
                name: "target".to_string(),
                description: String::new(),
                param_type: "boolean".to_string(),
                default: Some("false".to_string()),
                required: false,
            },
        ];
        assert!(validate_script_params(&duplicate).is_err());

        let valid = vec![ScriptParam {
            name: "target_service".to_string(),
            description: "目标服务".to_string(),
            param_type: "string".to_string(),
            default: Some("nginx".to_string()),
            required: true,
        }];
        assert!(validate_script_params(&valid).is_ok());
    }

    #[test]
    fn service_name_sanitizer_keeps_systemd_unit_chars() {
        assert_eq!(
            sanitize_service_name("redis@6379.service").unwrap(),
            "redis@6379.service"
        );
        assert!(sanitize_service_name("../redis").is_err());
        assert!(sanitize_service_name("redis;rm").is_err());
    }

    #[test]
    fn rule_overrides_filter_and_rewrite_alerts() {
        let mut overrides = HashMap::new();
        overrides.insert(
            "service.failed.error".to_string(),
            serde_json::json!({ "enabled": false }),
        );
        overrides.insert(
            "check.generic.warning".to_string(),
            serde_json::json!({
                "level": "error",
                "title": "检查警告升级",
                "summary": "检查存在需要立即处理的问题",
                "suggestion": "进入检查详情确认证据",
                "commands": ["dm check resource"]
            }),
        );
        let alerts = vec![
            alert_value(
                "service-nginx.service",
                "service",
                "error",
                "服务异常",
                "nginx failed",
                vec![],
                vec![],
                vec![],
            ),
            alert_value(
                "check-resource-abc",
                "check",
                "warn",
                "检查警告",
                "资源检查警告",
                vec![],
                vec![],
                vec![],
            ),
        ];
        let rewritten = apply_rule_overrides_to_alerts(alerts, &overrides);
        assert_eq!(rewritten.len(), 1);
        assert_eq!(rewritten[0]["rule_id"], "check.generic.warning");
        assert_eq!(rewritten[0]["level"], "error");
        assert_eq!(rewritten[0]["title"], "检查警告升级");
        assert_eq!(rewritten[0]["handling"], "进入检查详情确认证据");
    }

    #[test]
    fn alert_identity_groups_same_program_and_category() {
        let first = normalize_alert_identity(serde_json::json!({
            "id": "raw-1",
            "type": "日志异常",
            "level": "error",
            "title": "连接拒绝异常",
            "message": "order-service connection refused",
            "service_name": "order-service",
            "rule_id": "exception.connection-refused.error"
        }));
        let second = normalize_alert_identity(serde_json::json!({
            "id": "raw-2",
            "type": "日志异常",
            "level": "error",
            "title": "连接拒绝异常",
            "message": "order-service connection refused again",
            "service_name": "order-service",
            "rule_id": "exception.connection-refused.error"
        }));
        let other_category = normalize_alert_identity(serde_json::json!({
            "id": "raw-3",
            "type": "日志异常",
            "level": "warn",
            "title": "调用超时异常",
            "message": "order-service timeout",
            "service_name": "order-service",
            "rule_id": "exception.timeout.warning"
        }));

        assert_eq!(first["id"], second["id"]);
        assert_ne!(first["id"], other_category["id"]);
        assert_eq!(first["group_program"], "order-service");
        assert_eq!(first["group_category"], "exception.connection-refused");
    }
}
