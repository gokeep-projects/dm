use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use base64::{engine::general_purpose, Engine as _};
use encoding_rs::GBK;
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{interval, Duration};

use crate::script::{self, executor};
use crate::web::api::AppState;

const WS_PING_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
pub struct WsRunRequest {
    pub action: String,
    pub params: HashMap<String, String>,
    pub args: Vec<String>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct WsLogMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub timestamp: String,
    pub line: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrafficCaptureRequest {
    pub action: String,
    #[serde(default)]
    pub interface: String,
    #[serde(default = "default_traffic_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

fn default_traffic_protocol() -> String {
    "all".to_string()
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, id, state))
}

pub async fn ws_dashboard_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_dashboard_socket(socket, state))
}

pub async fn ws_traffic_handler(
    ws: WebSocketUpgrade,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(handle_traffic_socket)
}

async fn handle_socket(socket: WebSocket, script_id: String, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    send_message(&mut sender, "connected", "已连接到脚本", None).await;

    let mut ping_timer = interval(WS_PING_INTERVAL);
    ping_timer.tick().await;

    loop {
        tokio::select! {
            _ = ping_timer.tick() => {
                if sender.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: String = text.into();
                        let req: WsRunRequest = match serde_json::from_str(&text_str) {
                            Ok(r) => r,
                            Err(e) => {
                                send_message(&mut sender, "error", &format!("请求解析错误: {}", e), None).await;
                                continue;
                            }
                        };

                        if req.action != "run" {
                            continue;
                        }

                        let dirs = crate::config::all_script_dirs(&state.config);
                        let script = match script::find_script(&dirs, &script_id) {
                            Ok(Some(s)) => s,
                            Ok(None) => {
                                send_message(&mut sender, "error", &format!("脚本不存在: {}", script_id), None).await;
                                break;
                            }
                            Err(e) => {
                                send_message(&mut sender, "error", &format!("查找脚本错误: {}", e), None).await;
                                break;
                            }
                        };

                        let mut exec_args: Vec<String> = Vec::new();
                        for (k, v) in &req.params {
                            if !v.is_empty() {
                                exec_args.push(format!("--{}={}", k, v));
                            }
                        }
                        exec_args.extend(req.args.iter().cloned());

                        let mut envs = HashMap::new();
                        envs.insert("COLUMNS".to_string(), "300".to_string());
                        state.db.insert_exec_with_inputs(
                            &script.id,
                            &script.name,
                            None,
                            None,
                            0,
                            &serde_json::to_value(&req.params).unwrap_or_else(|_| serde_json::json!({})),
                            &req.args,
                        );

                        if let Some(result) = run_structured_script(&script.id).await {
                            let elapsed = result
                                .get("duration_ms")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let status = result
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("ok");
                            let exit_code = if status == "error" { 1 } else { 0 };
                            let line_count = result
                                .get("sections")
                                .and_then(|v| v.as_array())
                                .map(|sections| sections.len())
                                .unwrap_or(0);
                            state.db.update_exec(&script_id, exit_code, elapsed, line_count);

                            let result_msg = WsLogMessage {
                                msg_type: "result".to_string(),
                                timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                                line: serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string()),
                                exit_code: Some(exit_code),
                                elapsed_ms: Some(elapsed),
                            };
                            if let Ok(json) = serde_json::to_string(&result_msg) {
                                let _ = sender.send(Message::Text(json.into())).await;
                            }
                            break;
                        }

                        match executor::execute_script(&script.path, exec_args, envs, req.timeout).await {
                            Ok((mut rx, handle)) => {
                                let mut final_exit: Option<i32> = None;
                                let mut final_elapsed: u64 = 0;
                                let mut line_count: usize = 0;
                                let mut output_lines: Vec<String> = Vec::new();

                                let log_dir = state.config.log_dir.join(&script_id);
                                let _ = std::fs::create_dir_all(&log_dir);
                                let ts = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                                let log_path = log_dir.join(format!("{}.log", ts));
                                let mut log_file = std::fs::File::create(&log_path).ok();

                                while let Some(log_msg) = rx.recv().await {
                                    if log_msg.msg_type == "done" {
                                        final_exit = log_msg.exit_code;
                                        final_elapsed = log_msg.elapsed_ms.unwrap_or(0);
                                    } else {
                                        line_count += 1;
                                        output_lines.push(log_msg.line.clone());
                                    }

                                    if let Some(ref mut f) = log_file {
                                        use std::io::Write;
                                        let _ = writeln!(f, "[{}] {}", log_msg.timestamp, log_msg.line);
                                    }

                                    if log_msg.msg_type == "done" {
                                        break;
                                    }
                                }
                                let _ = handle.await;
                                if let Some(code) = final_exit {
                                    let full_output = output_lines.join("\n");
                                    let result_json = try_parse_json_output(&full_output, code, final_elapsed);
                                    let effective_exit =
                                        effective_exit_code_from_result_json(&result_json, code);
                                    state.db.update_exec(
                                        &script_id,
                                        effective_exit,
                                        final_elapsed,
                                        line_count,
                                    );

                                    let result_msg = WsLogMessage {
                                        msg_type: "result".to_string(),
                                        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
                                        line: result_json,
                                        exit_code: Some(effective_exit),
                                        elapsed_ms: Some(final_elapsed),
                                    };

                                    if let Ok(json) = serde_json::to_string(&result_msg) {
                                        let _ = sender.send(Message::Text(json.into())).await;
                                    }
                                }
                            }
                            Err(e) => {
                                send_message(&mut sender, "error", &format!("启动脚本失败: {}", e), None).await;
                            }
                        }
                        break;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => continue,
                }
            }
        }
    }
}

async fn send_message(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    msg_type: &str,
    line: &str,
    exit_code: Option<i32>,
) {
    let msg = WsLogMessage {
        msg_type: msg_type.to_string(),
        timestamp: chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S%.3f")
            .to_string(),
        line: line.to_string(),
        exit_code,
        elapsed_ms: None,
    };
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }
}

async fn send_json_value(
    sender: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    value: serde_json::Value,
) -> bool {
    match serde_json::to_string(&value) {
        Ok(json) => sender.send(Message::Text(json.into())).await.is_ok(),
        Err(_) => false,
    }
}

async fn handle_traffic_socket(socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let _ = send_json_value(
        &mut sender,
        serde_json::json!({
            "type": "connected",
            "message": "流量分析 WebSocket 已连接"
        }),
    )
    .await;

    let mut start_req: Option<TrafficCaptureRequest> = None;
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let text_str: String = text.into();
                let req: TrafficCaptureRequest = match serde_json::from_str(&text_str) {
                    Ok(req) => req,
                    Err(e) => {
                        let _ = send_json_value(
                            &mut sender,
                            serde_json::json!({"type": "error", "message": format!("请求解析错误: {}", e)}),
                        )
                        .await;
                        continue;
                    }
                };
                if req.action == "start" {
                    start_req = Some(req);
                    break;
                }
                if req.action == "stop" {
                    let _ = send_json_value(
                        &mut sender,
                        serde_json::json!({"type": "stopped", "message": "监听已停止"}),
                    )
                    .await;
                    return;
                }
            }
            Ok(Message::Close(_)) | Err(_) => return,
            _ => {}
        }
    }

    let Some(req) = start_req else { return };
    if let Err(message) = validate_traffic_request(&req) {
        let _ = send_json_value(
            &mut sender,
            serde_json::json!({"type": "error", "message": message}),
        )
        .await;
        return;
    }

    let client_limit = req.limit.unwrap_or(500).clamp(50, 5000);
    let filter = build_traffic_filter(&req);
    let stop_flag = Arc::new(AtomicBool::new(false));
    let (record_tx, mut record_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(128);
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(16);
    spawn_linux_packet_capture(req.clone(), stop_flag.clone(), record_tx, event_tx);

    let mut ping_timer = interval(WS_PING_INTERVAL);
    ping_timer.tick().await;

    loop {
        tokio::select! {
            _ = ping_timer.tick() => {
                if sender.send(Message::Ping(vec![].into())).await.is_err() {
                    stop_flag.store(true, Ordering::SeqCst);
                    break;
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: String = text.into();
                        if serde_json::from_str::<TrafficCaptureRequest>(&text_str)
                            .map(|r| r.action == "stop")
                            .unwrap_or(false)
                        {
                            stop_flag.store(true, Ordering::SeqCst);
                            let _ = send_json_value(
                                &mut sender,
                                serde_json::json!({"type": "stopped", "message": "监听已停止"}),
                            ).await;
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => {
                        stop_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                    _ => {}
                }
            }
            event = event_rx.recv() => {
                if let Some(event) = event {
                    if event["type"] == "started" {
                        let mut started = event;
                        started["filter"] = serde_json::Value::String(filter.clone());
                        started["client_limit"] = serde_json::json!(client_limit);
                        if !send_json_value(&mut sender, started).await {
                            stop_flag.store(true, Ordering::SeqCst);
                            break;
                        }
                    } else if !send_json_value(&mut sender, event).await {
                        stop_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                } else {
                    let _ = send_json_value(
                        &mut sender,
                        serde_json::json!({"type": "stopped", "message": "监听已结束"}),
                    ).await;
                    break;
                }
            }
            record = record_rx.recv() => {
                if let Some(record) = record {
                    if !send_json_value(&mut sender, serde_json::json!({"type": "record", "record": record})).await {
                        stop_flag.store(true, Ordering::SeqCst);
                        break;
                    }
                }
            }
        }
    }
}

fn validate_traffic_request(req: &TrafficCaptureRequest) -> Result<(), String> {
    if req.interface.trim().is_empty() || !is_safe_capture_token(&req.interface) {
        return Err("网卡名称为空或包含非法字符".to_string());
    }
    if !matches!(req.protocol.as_str(), "all" | "tcp" | "udp") {
        return Err("协议只能选择 all/tcp/udp".to_string());
    }
    if !req.ip.trim().is_empty() && !is_safe_capture_token(&req.ip) {
        return Err("IP/主机过滤包含非法字符".to_string());
    }
    if !req.port.trim().is_empty() && !req.port.chars().all(|c| c.is_ascii_digit()) {
        return Err("端口必须是数字".to_string());
    }
    Ok(())
}

fn is_safe_capture_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '_' | '-' | '@' | '/'))
}

fn build_traffic_filter(req: &TrafficCaptureRequest) -> String {
    let mut filters = Vec::new();
    match req.protocol.as_str() {
        "tcp" => filters.push("tcp".to_string()),
        "udp" => filters.push("udp".to_string()),
        _ => filters.push("(tcp or udp)".to_string()),
    }
    if !req.ip.trim().is_empty() && is_safe_capture_token(&req.ip) {
        filters.push(format!("host {}", req.ip.trim()));
    }
    if !req.port.trim().is_empty() && req.port.chars().all(|c| c.is_ascii_digit()) {
        filters.push(format!("port {}", req.port.trim()));
    }
    filters.join(" and ")
}

fn spawn_linux_packet_capture(
    req: TrafficCaptureRequest,
    stop_flag: Arc<AtomicBool>,
    record_tx: tokio::sync::mpsc::Sender<serde_json::Value>,
    event_tx: tokio::sync::mpsc::Sender<serde_json::Value>,
) {
    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || {
            if let Err(e) = run_linux_packet_capture(req, stop_flag, record_tx, event_tx.clone()) {
                let _ = event_tx.blocking_send(serde_json::json!({
                    "type": "error",
                    "message": e,
                }));
            }
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = (req, stop_flag, record_tx);
        std::thread::spawn(move || {
            let _ = event_tx.blocking_send(serde_json::json!({
                "type": "error",
                "message": "当前平台不支持抓包。流量分析仅支持 Linux。",
            }));
        });
    }
}

#[cfg(target_os = "linux")]
fn run_linux_packet_capture(
    req: TrafficCaptureRequest,
    stop_flag: Arc<AtomicBool>,
    record_tx: tokio::sync::mpsc::Sender<serde_json::Value>,
    event_tx: tokio::sync::mpsc::Sender<serde_json::Value>,
) -> Result<(), String> {
    use std::ffi::CString;
    use std::mem;

    let iface =
        CString::new(req.interface.clone()).map_err(|_| "网卡名称包含非法字符".to_string())?;
    let if_index = unsafe { libc::if_nametoindex(iface.as_ptr()) };
    if if_index == 0 {
        return Err(format!("网卡不存在或不可用: {}", req.interface));
    }

    let fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW,
            (libc::ETH_P_ALL as u16).to_be() as i32,
        )
    };
    if fd < 0 {
        return Err(format!(
            "无法创建 Linux raw socket: {}。请使用 root 或 CAP_NET_RAW 权限运行。",
            std::io::Error::last_os_error()
        ));
    }
    let _guard = FdGuard(fd);

    let timeout = libc::timeval {
        tv_sec: 1,
        tv_usec: 0,
    };
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            &timeout as *const _ as *const libc::c_void,
            mem::size_of_val(&timeout) as libc::socklen_t,
        );
    }

    let mut addr: libc::sockaddr_ll = unsafe { mem::zeroed() };
    addr.sll_family = libc::AF_PACKET as u16;
    addr.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
    addr.sll_ifindex = if_index as i32;
    let bind_result = unsafe {
        libc::bind(
            fd,
            &addr as *const _ as *const libc::sockaddr,
            mem::size_of_val(&addr) as libc::socklen_t,
        )
    };
    if bind_result < 0 {
        return Err(format!(
            "绑定网卡 {} 失败: {}",
            req.interface,
            std::io::Error::last_os_error()
        ));
    }

    let _ = event_tx.blocking_send(serde_json::json!({
        "type": "started",
        "message": "Linux raw socket 抓包已启动",
        "interface": req.interface,
        "post_filter": {
            "domain": req.domain,
            "path": req.path,
        }
    }));

    let mut seq = 0usize;
    let mut tracker = TrafficExchangeTracker::new();
    let mut buf = vec![0u8; 65536];
    while !stop_flag.load(Ordering::SeqCst) {
        let size = unsafe { libc::recv(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
        if size < 0 {
            let err = std::io::Error::last_os_error();
            if matches!(
                err.kind(),
                std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
            ) {
                continue;
            }
            return Err(format!("读取网卡流量失败: {}", err));
        }
        if size == 0 {
            continue;
        }
        seq += 1;
        if let Some(mut packet) = parse_linux_packet(&buf[..size as usize], seq) {
            attach_pcap_frame(&mut packet, &buf[..size as usize]);
            if traffic_packet_matches_transport_filters(&packet, &req) {
                for record in tracker.ingest_packet(packet) {
                    if record_tx.blocking_send(record).is_err() {
                        break;
                    }
                }
            }
        }
    }

    let _ = event_tx.blocking_send(serde_json::json!({
        "type": "stopped",
        "message": "监听已停止",
    }));
    Ok(())
}

#[cfg(target_os = "linux")]
struct FdGuard(i32);

#[cfg(target_os = "linux")]
impl Drop for FdGuard {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}

fn traffic_packet_matches_transport_filters(
    packet: &serde_json::Value,
    req: &TrafficCaptureRequest,
) -> bool {
    let packet_protocol = packet["protocol"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if req.protocol == "tcp" && packet_protocol != "tcp" {
        return false;
    }
    if req.protocol == "udp" && packet_protocol != "udp" {
        return false;
    }

    let ip = req.ip.trim();
    if !ip.is_empty()
        && packet["src_ip"].as_str().unwrap_or_default() != ip
        && packet["dst_ip"].as_str().unwrap_or_default() != ip
    {
        return false;
    }

    let port = req.port.trim();
    if !port.is_empty()
        && packet["src_port"].as_str().unwrap_or_default() != port
        && packet["dst_port"].as_str().unwrap_or_default() != port
    {
        return false;
    }

    true
}

struct TrafficExchangeTracker {
    pending_requests: HashMap<String, VecDeque<serde_json::Value>>,
    raw_flows: HashMap<String, RawFlowState>,
    next_record_seq: usize,
}

struct RawFlowState {
    seq: usize,
    total_bytes: u64,
    frames: Vec<serde_json::Value>,
}

impl TrafficExchangeTracker {
    fn new() -> Self {
        Self {
            pending_requests: HashMap::new(),
            raw_flows: HashMap::new(),
            next_record_seq: 0,
        }
    }

    fn ingest_packet(&mut self, packet: serde_json::Value) -> Vec<serde_json::Value> {
        let http_kind = packet["http"]["kind"].as_str().unwrap_or_default();
        match http_kind {
            "request" => {
                self.pending_requests
                    .entry(flow_key(&packet))
                    .or_default()
                    .push_back(packet);
                Vec::new()
            }
            "response" => {
                let reverse = reverse_flow_key(&packet);
                if let Some(queue) = self.pending_requests.get_mut(&reverse) {
                    if let Some(request) = queue.pop_front() {
                        self.next_record_seq += 1;
                        return vec![build_http_exchange_record(
                            self.next_record_seq,
                            request,
                            packet,
                        )];
                    }
                }
                self.next_record_seq += 1;
                vec![build_response_only_record(self.next_record_seq, packet)]
            }
            _ => {
                if packet["length"].as_u64().unwrap_or(0) == 0 {
                    return Vec::new();
                }
                let key = flow_key(&packet);
                let packet_bytes = packet["length"].as_u64().unwrap_or(0);
                let frame = packet_frame_entry(&packet);
                let (seq, total_bytes, frames) = if let Some(state) = self.raw_flows.get_mut(&key) {
                    state.total_bytes = state.total_bytes.saturating_add(packet_bytes);
                    if let Some(frame) = frame {
                        if state.frames.len() < 128 {
                            state.frames.push(frame);
                        }
                    }
                    (state.seq, state.total_bytes, state.frames.clone())
                } else {
                    self.next_record_seq += 1;
                    let frames = frame.into_iter().collect::<Vec<_>>();
                    self.raw_flows.insert(
                        key.clone(),
                        RawFlowState {
                            seq: self.next_record_seq,
                            total_bytes: packet_bytes,
                            frames: frames.clone(),
                        },
                    );
                    (self.next_record_seq, packet_bytes, frames)
                };
                vec![build_raw_flow_record(
                    seq,
                    &key,
                    total_bytes,
                    frames,
                    packet,
                )]
            }
        }
    }
}

fn attach_pcap_frame(packet: &mut serde_json::Value, frame: &[u8]) {
    if let Some(obj) = packet.as_object_mut() {
        obj.insert(
            "pcap_frame_base64".to_string(),
            serde_json::Value::String(general_purpose::STANDARD.encode(frame)),
        );
        obj.insert("pcap_link_type".to_string(), serde_json::json!(1));
        obj.insert(
            "pcap_original_length".to_string(),
            serde_json::json!(frame.len()),
        );
    }
}

fn packet_frame_entry(packet: &serde_json::Value) -> Option<serde_json::Value> {
    let frame = packet["pcap_frame_base64"].as_str()?;
    Some(serde_json::json!({
        "timestamp": packet["timestamp"],
        "frame_base64": frame,
        "original_length": packet["pcap_original_length"],
        "link_type": packet["pcap_link_type"],
    }))
}

fn flow_key(packet: &serde_json::Value) -> String {
    format!(
        "{}:{}>{}:{}",
        packet["src_ip"].as_str().unwrap_or_default(),
        packet["src_port"].as_str().unwrap_or_default(),
        packet["dst_ip"].as_str().unwrap_or_default(),
        packet["dst_port"].as_str().unwrap_or_default()
    )
}

fn reverse_flow_key(packet: &serde_json::Value) -> String {
    format!(
        "{}:{}>{}:{}",
        packet["dst_ip"].as_str().unwrap_or_default(),
        packet["dst_port"].as_str().unwrap_or_default(),
        packet["src_ip"].as_str().unwrap_or_default(),
        packet["src_port"].as_str().unwrap_or_default()
    )
}

fn endpoint_meta(packet: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "protocol": packet["protocol"],
        "transport_protocol": packet["protocol"],
        "src_ip": packet["src_ip"],
        "src_port": packet["src_port"],
        "dst_ip": packet["dst_ip"],
        "dst_port": packet["dst_port"],
        "payload_bytes": packet["length"],
        "pcap_frame_base64": packet["pcap_frame_base64"],
        "pcap_original_length": packet["pcap_original_length"],
        "pcap_link_type": packet["pcap_link_type"],
    })
}

fn packet_uses_https_port(packet: &serde_json::Value) -> bool {
    packet["src_port"].as_str() == Some("443") || packet["dst_port"].as_str() == Some("443")
}

fn build_http_exchange_record(
    seq: usize,
    request_packet: serde_json::Value,
    response_packet: serde_json::Value,
) -> serde_json::Value {
    let method = request_packet["http"]["method"]
        .as_str()
        .unwrap_or_default();
    let path = request_packet["http"]["path"].as_str().unwrap_or_default();
    let host = request_packet["http"]["host"].as_str().unwrap_or_default();
    let status = response_packet["http"]["status"].as_i64().unwrap_or(0);
    let status_text = response_packet["http"]["status_text"]
        .as_str()
        .unwrap_or_default();
    let https_plaintext =
        packet_uses_https_port(&request_packet) || packet_uses_https_port(&response_packet);
    let display_protocol = if https_plaintext { "HTTPS" } else { "HTTP" };
    let decoded_prefix = if https_plaintext {
        "HTTPS plaintext"
    } else {
        "HTTP"
    };
    let pcap_frames = [
        packet_frame_entry(&request_packet),
        packet_frame_entry(&response_packet),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    serde_json::json!({
        "seq": seq,
        "type": "http_exchange",
        "timestamp": request_packet["timestamp"],
        "request_time": request_packet["timestamp"],
        "response_time": response_packet["timestamp"],
        "protocol": display_protocol,
        "transport_protocol": request_packet["protocol"],
        "https_plaintext": https_plaintext,
        "src_ip": request_packet["src_ip"],
        "src_port": request_packet["src_port"],
        "dst_ip": request_packet["dst_ip"],
        "dst_port": request_packet["dst_port"],
        "host": host,
        "path": path,
        "method": method,
        "status": status,
        "status_text": status_text,
        "request": {
            "method": method,
            "path": path,
            "host": host,
            "version": request_packet["http"]["version"],
            "headers": request_packet["http"]["headers"],
            "body_preview": request_packet["http"]["body_preview"],
            "payload_bytes": request_packet["length"],
            "raw_preview": request_packet["raw"],
            "meta": endpoint_meta(&request_packet),
        },
        "response": {
            "status": status,
            "status_text": status_text,
            "version": response_packet["http"]["version"],
            "headers": response_packet["http"]["headers"],
            "body_preview": response_packet["http"]["body_preview"],
            "payload_bytes": response_packet["length"],
            "raw_preview": response_packet["raw"],
            "meta": endpoint_meta(&response_packet),
        },
        "summary": format!("{} {}{} -> {} {}", method, host, path, status, status_text),
        "decoded": format!("{} {} {}{} -> {} {}", decoded_prefix, method, host, path, status, status_text),
        "pcap_frames": pcap_frames,
    })
}

fn build_response_only_record(seq: usize, packet: serde_json::Value) -> serde_json::Value {
    let pcap_frames = packet_frame_entry(&packet).into_iter().collect::<Vec<_>>();
    let https_plaintext = packet_uses_https_port(&packet);
    let display_protocol = if https_plaintext { "HTTPS" } else { "HTTP" };
    let decoded = if https_plaintext {
        format!(
            "HTTPS plaintext response {} {}",
            packet["http"]["status"].as_i64().unwrap_or(0),
            packet["http"]["status_text"].as_str().unwrap_or_default()
        )
    } else {
        packet["decoded"].as_str().unwrap_or_default().to_string()
    };
    serde_json::json!({
        "seq": seq,
        "type": "http_response",
        "timestamp": packet["timestamp"],
        "request_time": "",
        "response_time": packet["timestamp"],
        "protocol": display_protocol,
        "transport_protocol": packet["protocol"],
        "https_plaintext": https_plaintext,
        "src_ip": packet["src_ip"],
        "src_port": packet["src_port"],
        "dst_ip": packet["dst_ip"],
        "dst_port": packet["dst_port"],
        "host": "",
        "path": "",
        "method": "",
        "status": packet["http"]["status"],
        "status_text": packet["http"]["status_text"],
        "request": serde_json::json!({}),
        "response": {
            "status": packet["http"]["status"],
            "status_text": packet["http"]["status_text"],
            "version": packet["http"]["version"],
            "headers": packet["http"]["headers"],
            "body_preview": packet["http"]["body_preview"],
            "payload_bytes": packet["length"],
            "raw_preview": packet["raw"],
            "meta": endpoint_meta(&packet),
        },
        "summary": decoded,
        "decoded": decoded,
        "pcap_frames": pcap_frames,
    })
}

fn build_raw_flow_record(
    seq: usize,
    flow_id: &str,
    total_bytes: u64,
    frames: Vec<serde_json::Value>,
    packet: serde_json::Value,
) -> serde_json::Value {
    let is_tls = packet["http"]["kind"].as_str() == Some("tls");
    let protocol = if is_tls {
        serde_json::json!("HTTPS")
    } else {
        packet["protocol"].clone()
    };
    serde_json::json!({
        "seq": seq,
        "flow_id": flow_id,
        "type": "raw_flow",
        "timestamp": packet["timestamp"],
        "request_time": packet["timestamp"],
        "response_time": "",
        "protocol": protocol,
        "transport_protocol": packet["protocol"],
        "src_ip": packet["src_ip"],
        "src_port": packet["src_port"],
        "dst_ip": packet["dst_ip"],
        "dst_port": packet["dst_port"],
        "host": packet["http"]["host"].as_str().unwrap_or_default(),
        "path": "",
        "method": if is_tls { "TLS" } else { "" },
        "status": "",
        "status_text": "",
        "total_bytes": total_bytes,
        "request": {
            "headers": serde_json::json!({}),
            "body_preview": packet["raw"],
            "payload_bytes": packet["length"],
            "raw_preview": packet["raw"],
            "meta": endpoint_meta(&packet),
        },
        "response": serde_json::json!({}),
        "summary": packet["summary"],
        "decoded": packet["decoded"],
        "pcap_frames": frames,
    })
}

fn parse_linux_packet(frame: &[u8], seq: usize) -> Option<serde_json::Value> {
    let offset = find_ip_offset(frame)?;
    match frame[offset] >> 4 {
        4 => parse_ipv4_packet(&frame[offset..], seq),
        6 => parse_ipv6_packet(&frame[offset..], seq),
        _ => None,
    }
}

fn find_ip_offset(frame: &[u8]) -> Option<usize> {
    if frame.len() >= 14 {
        let ethertype = u16::from_be_bytes([frame[12], frame[13]]);
        if matches!(ethertype, 0x0800 | 0x86dd) {
            return Some(14);
        }
    }
    for offset in 0..frame.len().min(32) {
        let version = frame[offset] >> 4;
        if version == 4 && frame.len() >= offset + 20 {
            let ihl = usize::from(frame[offset] & 0x0f) * 4;
            if ihl >= 20 && frame.len() >= offset + ihl {
                return Some(offset);
            }
        }
        if version == 6 && frame.len() >= offset + 40 {
            return Some(offset);
        }
    }
    None
}

fn parse_ipv4_packet(packet: &[u8], seq: usize) -> Option<serde_json::Value> {
    if packet.len() < 20 || packet[0] >> 4 != 4 {
        return None;
    }
    let ihl = usize::from(packet[0] & 0x0f) * 4;
    if ihl < 20 || packet.len() < ihl {
        return None;
    }
    let total_len = usize::from(u16::from_be_bytes([packet[2], packet[3]])).min(packet.len());
    if total_len <= ihl {
        return None;
    }
    let proto = packet[9];
    let src_ip = format!(
        "{}.{}.{}.{}",
        packet[12], packet[13], packet[14], packet[15]
    );
    let dst_ip = format!(
        "{}.{}.{}.{}",
        packet[16], packet[17], packet[18], packet[19]
    );
    parse_transport_packet(&packet[ihl..total_len], seq, proto, src_ip, dst_ip)
}

fn parse_ipv6_packet(packet: &[u8], seq: usize) -> Option<serde_json::Value> {
    if packet.len() < 40 || packet[0] >> 4 != 6 {
        return None;
    }
    let payload_len = usize::from(u16::from_be_bytes([packet[4], packet[5]]));
    let total_len = (40 + payload_len).min(packet.len());
    let proto = packet[6];
    let src_ip = format_ipv6(&packet[8..24]);
    let dst_ip = format_ipv6(&packet[24..40]);
    parse_transport_packet(&packet[40..total_len], seq, proto, src_ip, dst_ip)
}

fn format_ipv6(bytes: &[u8]) -> String {
    bytes
        .chunks_exact(2)
        .map(|chunk| format!("{:x}", u16::from_be_bytes([chunk[0], chunk[1]])))
        .collect::<Vec<_>>()
        .join(":")
}

fn parse_transport_packet(
    segment: &[u8],
    seq: usize,
    proto: u8,
    src_ip: String,
    dst_ip: String,
) -> Option<serde_json::Value> {
    match proto {
        6 => parse_tcp_segment(segment, seq, src_ip, dst_ip),
        17 => parse_udp_datagram(segment, seq, src_ip, dst_ip),
        _ => None,
    }
}

fn parse_tcp_segment(
    segment: &[u8],
    seq: usize,
    src_ip: String,
    dst_ip: String,
) -> Option<serde_json::Value> {
    if segment.len() < 20 {
        return None;
    }
    let src_port = u16::from_be_bytes([segment[0], segment[1]]).to_string();
    let dst_port = u16::from_be_bytes([segment[2], segment[3]]).to_string();
    let header_len = usize::from(segment[12] >> 4) * 4;
    if header_len < 20 || segment.len() < header_len {
        return None;
    }
    let flags = segment[13];
    let payload = &segment[header_len..];
    let http = decode_http_payload(payload);
    let summary = format!(
        "flags {}, payload length {}",
        tcp_flags(flags),
        payload.len()
    );
    let raw = payload_preview(payload);
    Some(packet_json(
        seq,
        "TCP",
        src_ip,
        src_port,
        dst_ip,
        dst_port,
        payload.len(),
        summary,
        http,
        raw,
    ))
}

fn parse_udp_datagram(
    datagram: &[u8],
    seq: usize,
    src_ip: String,
    dst_ip: String,
) -> Option<serde_json::Value> {
    if datagram.len() < 8 {
        return None;
    }
    let src_port = u16::from_be_bytes([datagram[0], datagram[1]]).to_string();
    let dst_port = u16::from_be_bytes([datagram[2], datagram[3]]).to_string();
    let udp_len = usize::from(u16::from_be_bytes([datagram[4], datagram[5]])).min(datagram.len());
    let payload = if udp_len > 8 {
        &datagram[8..udp_len]
    } else {
        &[]
    };
    let http = decode_http_payload(payload);
    let summary = format!("udp length {}, payload length {}", udp_len, payload.len());
    let raw = payload_preview(payload);
    Some(packet_json(
        seq,
        "UDP",
        src_ip,
        src_port,
        dst_ip,
        dst_port,
        payload.len(),
        summary,
        http,
        raw,
    ))
}

fn packet_json(
    seq: usize,
    protocol: &str,
    src_ip: String,
    src_port: String,
    dst_ip: String,
    dst_port: String,
    length: usize,
    summary: String,
    http: serde_json::Value,
    raw: String,
) -> serde_json::Value {
    serde_json::json!({
        "seq": seq,
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.6f").to_string(),
        "protocol": protocol,
        "src_ip": src_ip,
        "src_port": src_port,
        "dst_ip": dst_ip,
        "dst_port": dst_port,
        "length": length,
        "summary": summary,
        "http": http,
        "raw": raw,
        "decoded": decoded_summary(protocol, &http, &summary),
    })
}

fn tcp_flags(flags: u8) -> String {
    let mut parts = Vec::new();
    if flags & 0x01 != 0 {
        parts.push("FIN");
    }
    if flags & 0x02 != 0 {
        parts.push("SYN");
    }
    if flags & 0x04 != 0 {
        parts.push("RST");
    }
    if flags & 0x08 != 0 {
        parts.push("PSH");
    }
    if flags & 0x10 != 0 {
        parts.push("ACK");
    }
    if flags & 0x20 != 0 {
        parts.push("URG");
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join("|")
    }
}

fn decode_http_payload(payload: &[u8]) -> serde_json::Value {
    if let Some(tls) = decode_tls_payload(payload) {
        return tls;
    }

    let Some((head_bytes, body_bytes)) = split_http_payload(payload) else {
        return serde_json::json!({
            "kind": "unknown",
            "method": "",
            "path": "",
            "host": "",
            "headers": serde_json::json!({}),
            "body_preview": "",
            "encrypted": false,
        });
    };

    let head = String::from_utf8_lossy(head_bytes);
    let mut lines = head.lines();
    let first = lines.next().unwrap_or_default().trim();
    let headers = parse_http_headers(lines);
    let body = body_preview_from_bytes(body_bytes, &headers);
    let methods = [
        "GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "CONNECT", "TRACE",
    ];

    for candidate in methods {
        let prefix = format!("{} ", candidate);
        if first.starts_with(&prefix) {
            let mut bits = first.split_whitespace();
            let method = bits.next().unwrap_or_default();
            let path = bits.next().unwrap_or_default();
            let version = bits.next().unwrap_or_default();
            let host = headers
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            return serde_json::json!({
                "kind": "request",
                "method": method,
                "path": path,
                "version": version,
                "host": host,
                "headers": headers,
                "body_preview": body,
                "encrypted": false,
            });
        }
    }

    if first.starts_with("HTTP/") {
        let mut bits = first.splitn(3, ' ');
        let version = bits.next().unwrap_or_default();
        let status = bits.next().and_then(|v| v.parse::<u16>().ok()).unwrap_or(0);
        let status_text = bits.next().unwrap_or_default();
        return serde_json::json!({
            "kind": "response",
            "status": status,
            "status_text": status_text,
            "version": version,
            "headers": headers,
            "body_preview": body,
            "encrypted": false,
        });
    }

    serde_json::json!({
        "kind": "unknown",
        "method": "",
        "path": "",
        "host": "",
        "headers": serde_json::json!({}),
        "body_preview": "",
        "encrypted": false,
    })
}

fn payload_preview(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "<empty payload>".to_string();
    }
    if let Some(tls) = tls_summary(payload) {
        return tls;
    }
    text_preview_from_bytes(payload)
}

fn split_http_payload(payload: &[u8]) -> Option<(&[u8], &[u8])> {
    for sep in [b"\r\n\r\n".as_slice(), b"\n\n".as_slice()] {
        if let Some(pos) = payload.windows(sep.len()).position(|w| w == sep) {
            return Some((&payload[..pos], &payload[pos + sep.len()..]));
        }
    }
    None
}

fn body_preview_from_bytes(
    body: &[u8],
    headers: &serde_json::Map<String, serde_json::Value>,
) -> String {
    let bytes = match headers
        .get("content-encoding")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "gzip" => decompress_with(GzDecoder::new(body)),
        "deflate" => decompress_deflate(body),
        _ => None,
    }
    .unwrap_or_else(|| body.to_vec());
    text_preview_from_bytes(&bytes)
}

fn decompress_deflate(body: &[u8]) -> Option<Vec<u8>> {
    decompress_with(ZlibDecoder::new(body)).or_else(|| decompress_with(DeflateDecoder::new(body)))
}

fn decompress_with<R: Read>(mut reader: R) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    reader.read_to_end(&mut out).ok()?;
    Some(out)
}

fn text_preview_from_bytes(payload: &[u8]) -> String {
    let limit = payload.len().min(4096);
    let bytes = &payload[..limit];
    if let Ok(text) = std::str::from_utf8(bytes) {
        return finalize_text_preview(text, payload.len(), limit);
    }

    let (gbk, _, gbk_had_errors) = GBK.decode(bytes);
    if !gbk_had_errors && readable_ratio(&gbk) >= 0.75 {
        return finalize_text_preview(&gbk, payload.len(), limit);
    }

    let lossy = String::from_utf8_lossy(bytes);
    if readable_ratio(&lossy) >= 0.85 && lossy.matches('\u{fffd}').count() <= 2 {
        return finalize_text_preview(&lossy, payload.len(), limit);
    }

    format!(
        "<binary payload: {} bytes, not safe text>\nHEX {}\nASCII {}{}",
        payload.len(),
        hex_preview(bytes, 96),
        ascii_preview(bytes, 96),
        if payload.len() > limit {
            format!("\n<truncated {} bytes>", payload.len() - limit)
        } else {
            String::new()
        }
    )
}

fn finalize_text_preview(text: &str, original_len: usize, limit: usize) -> String {
    let mut cleaned = text
        .trim_matches('\0')
        .chars()
        .map(|c| {
            if c == '\r' || c == '\n' || c == '\t' || !c.is_control() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>();
    if original_len > limit {
        cleaned.push_str(&format!("\n<truncated {} bytes>", original_len - limit));
    }
    cleaned
}

fn readable_ratio(text: &str) -> f32 {
    let mut total = 0usize;
    let mut readable = 0usize;
    for c in text.chars() {
        total += 1;
        if c == '\r' || c == '\n' || c == '\t' || (!c.is_control() && c != '\u{fffd}') {
            readable += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        readable as f32 / total as f32
    }
}

fn hex_preview(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn ascii_preview(bytes: &[u8], limit: usize) -> String {
    bytes
        .iter()
        .take(limit)
        .map(|b| {
            if b.is_ascii_graphic() || matches!(*b, b' ' | b'\r' | b'\n' | b'\t') {
                char::from(*b)
            } else {
                '.'
            }
        })
        .collect()
}

fn decode_tls_payload(payload: &[u8]) -> Option<serde_json::Value> {
    let summary = tls_summary(payload)?;
    let sni = tls_client_hello_sni(payload).unwrap_or_default();
    Some(serde_json::json!({
        "kind": "tls",
        "method": "TLS",
        "path": "",
        "host": sni,
        "headers": serde_json::json!({
            "tls": "encrypted",
            "sni": sni,
        }),
        "body_preview": summary,
        "encrypted": true,
    }))
}

fn tls_summary(payload: &[u8]) -> Option<String> {
    if !is_tls_record(payload) {
        return None;
    }
    let sni = tls_client_hello_sni(payload);
    let head = match sni {
        Some(host) if !host.is_empty() => format!("HTTPS/TLS encrypted payload. SNI: {}.", host),
        _ => "HTTPS/TLS encrypted payload.".to_string(),
    };
    Some(format!(
        "{}\n当前包是 TLS 密文，网络原包内没有 HTTP 明文请求方法/正文。\n要还原 HTTP 原始明文，必须提供 TLS session keys，或让流量经过受信任解密代理后再抓取。\nTLS 原始字节 HEX {}\nTLS 原始字节 ASCII {}",
        head,
        hex_preview(payload, 128),
        ascii_preview(payload, 128)
    ))
}

fn is_tls_record(payload: &[u8]) -> bool {
    payload.len() >= 5
        && matches!(payload[0], 0x14 | 0x15 | 0x16 | 0x17)
        && payload[1] == 0x03
        && payload[2] <= 0x04
}

fn tls_client_hello_sni(payload: &[u8]) -> Option<String> {
    if payload.len() < 5 || payload[0] != 0x16 {
        return None;
    }
    let record_len = usize::from(u16::from_be_bytes([payload[3], payload[4]]));
    if payload.len() < 5 + record_len || payload.get(5).copied()? != 0x01 {
        return None;
    }
    let mut offset = 9;
    if payload.len() < offset + 2 + 32 {
        return None;
    }
    offset += 2 + 32;
    let session_len = usize::from(*payload.get(offset)?);
    offset += 1 + session_len;
    if payload.len() < offset + 2 {
        return None;
    }
    let cipher_len = usize::from(u16::from_be_bytes([payload[offset], payload[offset + 1]]));
    offset += 2 + cipher_len;
    if payload.len() < offset + 1 {
        return None;
    }
    let compression_len = usize::from(payload[offset]);
    offset += 1 + compression_len;
    if payload.len() < offset + 2 {
        return None;
    }
    let extensions_len = usize::from(u16::from_be_bytes([payload[offset], payload[offset + 1]]));
    offset += 2;
    let extensions_end = (offset + extensions_len).min(payload.len());
    while offset + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        let ext_len = usize::from(u16::from_be_bytes([
            payload[offset + 2],
            payload[offset + 3],
        ]));
        offset += 4;
        if offset + ext_len > extensions_end {
            return None;
        }
        if ext_type == 0 {
            return parse_sni_extension(&payload[offset..offset + ext_len]);
        }
        offset += ext_len;
    }
    None
}

fn parse_sni_extension(ext: &[u8]) -> Option<String> {
    if ext.len() < 5 {
        return None;
    }
    let list_len = usize::from(u16::from_be_bytes([ext[0], ext[1]]));
    let mut offset = 2;
    let end = (offset + list_len).min(ext.len());
    while offset + 3 <= end {
        let name_type = ext[offset];
        let name_len = usize::from(u16::from_be_bytes([ext[offset + 1], ext[offset + 2]]));
        offset += 3;
        if offset + name_len > end {
            return None;
        }
        if name_type == 0 {
            let host = std::str::from_utf8(&ext[offset..offset + name_len]).ok()?;
            return Some(host.to_string());
        }
        offset += name_len;
    }
    None
}

fn parse_http_headers<'a>(
    lines: impl Iterator<Item = &'a str>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut headers = serde_json::Map::new();
    for line in lines {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        headers.insert(
            key.trim().to_ascii_lowercase(),
            serde_json::json!(value.trim()),
        );
    }
    headers
}

fn decoded_summary(protocol: &str, http: &serde_json::Value, _summary: &str) -> String {
    let method = http["method"].as_str().unwrap_or_default();
    let path = http["path"].as_str().unwrap_or_default();
    let host = http["host"].as_str().unwrap_or_default();
    if http["kind"].as_str() == Some("tls") {
        return if host.is_empty() {
            "HTTPS/TLS 加密流量".to_string()
        } else {
            format!("HTTPS/TLS {}", host)
        };
    }
    if http["kind"].as_str() == Some("response") {
        return format!(
            "HTTP response {} {}",
            http["status"].as_i64().unwrap_or(0),
            http["status_text"].as_str().unwrap_or_default()
        );
    }
    if !method.is_empty() {
        return format!("HTTP {} {}{}", method, host, path);
    }
    if protocol == "TCP" {
        return "TCP 流量".to_string();
    }
    "UDP 数据报".to_string()
}

#[cfg(test)]
fn traffic_packet_matches(packet: &serde_json::Value, domain: &String, path: &String) -> bool {
    let haystack = format!(
        "{} {} {}",
        packet["http"]["host"].as_str().unwrap_or_default(),
        packet["http"]["path"].as_str().unwrap_or_default(),
        packet["raw"].as_str().unwrap_or_default()
    )
    .to_ascii_lowercase();
    let domain = domain.trim().to_ascii_lowercase();
    let path = path.trim().to_ascii_lowercase();
    (domain.is_empty() || haystack.contains(&domain))
        && (path.is_empty() || haystack.contains(&path))
}

fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            while let Some(&next) = chars.peek() {
                chars.next();
                if next == 'm' {
                    break;
                }
            }
        } else if c == '\x08' {
            continue;
        } else {
            result.push(c);
        }
    }
    result
}

fn clean_terminal_output(input: &str) -> String {
    let stripped = strip_ansi_codes(input);
    let mut result = String::with_capacity(stripped.len());
    for line in stripped.lines() {
        let cleaned = line
            .replace("╔", "+")
            .replace("╗", "+")
            .replace("╚", "+")
            .replace("╝", "+")
            .replace("═", "-")
            .replace("║", "|")
            .replace("┌", "+")
            .replace("┐", "+")
            .replace("└", "+")
            .replace("┘", "+")
            .replace("─", "-")
            .replace("│", "|")
            .replace("├", "+")
            .replace("┤", "+")
            .replace("┬", "+")
            .replace("┴", "+")
            .replace("┼", "+");
        result.push_str(&cleaned);
        result.push('\n');
    }
    result.trim().to_string()
}

fn strip_markdown_fence(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if !trimmed.starts_with("```") {
        return None;
    }

    let mut lines = trimmed.lines();
    lines.next()?;
    let mut body: Vec<&str> = lines.collect();
    if body.last().is_some_and(|line| line.trim() == "```") {
        body.pop();
    }
    Some(body.join("\n").trim().to_string())
}

fn fenced_blocks(input: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    let mut in_fence = false;

    for line in input.lines() {
        if line.trim_start().starts_with("```") {
            if in_fence {
                blocks.push(current.join("\n").trim().to_string());
                current.clear();
                in_fence = false;
            } else {
                in_fence = true;
            }
            continue;
        }

        if in_fence {
            current.push(line);
        }
    }

    blocks
}

fn extract_balanced_json(input: &str) -> Option<String> {
    let chars: Vec<char> = input.chars().collect();
    for start in 0..chars.len() {
        let open = chars[start];
        let close = match open {
            '{' => '}',
            '[' => ']',
            _ => continue,
        };

        let mut stack = vec![close];
        let mut in_string = false;
        let mut escape = false;

        for end in start + 1..chars.len() {
            let c = chars[end];
            if in_string {
                if escape {
                    escape = false;
                } else if c == '\\' {
                    escape = true;
                } else if c == '"' {
                    in_string = false;
                }
                continue;
            }

            match c {
                '"' => in_string = true,
                '{' => stack.push('}'),
                '[' => stack.push(']'),
                '}' | ']' => {
                    if stack.pop() != Some(c) {
                        break;
                    }
                    if stack.is_empty() {
                        return Some(chars[start..=end].iter().collect::<String>());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn parse_json_candidate(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    if let Some(unfenced) = strip_markdown_fence(trimmed) {
        if serde_json::from_str::<serde_json::Value>(&unfenced).is_ok() {
            return Some(unfenced);
        }
    }

    if let Some(extracted) = extract_balanced_json(trimmed) {
        if serde_json::from_str::<serde_json::Value>(&extracted).is_ok() {
            return Some(extracted);
        }
    }

    None
}

async fn run_structured_script(script_id: &str) -> Option<serde_json::Value> {
    if !matches!(
        script_id,
        "system"
            | "security"
            | "network"
            | "resource"
            | "service"
            | "environment"
            | "container"
            | "middleware"
            | "schedule"
            | "smart-check"
            | "service-manage"
    ) {
        return None;
    }

    let id = script_id.to_string();
    tokio::task::spawn_blocking(move || crate::checks::run_check(&id))
        .await
        .ok()
        .flatten()
        .and_then(|result| serde_json::to_value(result).ok())
}

fn json_value_to_result(
    value: serde_json::Value,
    exit_code: i32,
    elapsed_ms: u64,
) -> serde_json::Value {
    if value.get("sections").is_some() {
        return value;
    }

    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .filter(|s| matches!(*s, "ok" | "warn" | "error" | "info"))
        .unwrap_or(if exit_code == 0 { "ok" } else { "error" });
    let mut summary_items = vec![
        serde_json::json!({"type": "label", "key": "状态", "value": if exit_code == 0 { "成功" } else { "失败" }, "status": status}),
        serde_json::json!({"type": "label", "key": "退出码", "value": exit_code.to_string()}),
        serde_json::json!({"type": "label", "key": "耗时", "value": format!("{}ms", elapsed_ms)}),
    ];

    let mut data_items = Vec::new();
    match &value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                if key == "stdout" || key == "stderr" {
                    continue;
                }
                if val.is_string() || val.is_number() || val.is_boolean() || val.is_null() {
                    data_items.push(serde_json::json!({
                        "type": "label",
                        "key": key,
                        "value": scalar_json_display(val),
                    }));
                }
            }

            if let Some(stdout) = map
                .get("stdout")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                data_items.push(serde_json::json!({"type": "info", "text": stdout.trim()}));
            }
            if let Some(stderr) = map
                .get("stderr")
                .and_then(|v| v.as_str())
                .filter(|v| !v.trim().is_empty())
            {
                data_items.push(serde_json::json!({"type": "error", "text": stderr.trim()}));
            }
            for (key, val) in map {
                if val.is_array() || val.is_object() {
                    data_items.push(json_complex_item(key, val));
                }
            }
        }
        serde_json::Value::Array(rows) => {
            summary_items.push(serde_json::json!({"type": "label", "key": "数据条数", "value": rows.len().to_string()}));
            data_items.push(json_array_item("返回列表", rows));
        }
        _ => {
            data_items.push(serde_json::json!({"type": "label", "key": "返回值", "value": scalar_json_display(&value)}));
        }
    }

    if data_items.is_empty() {
        data_items.push(serde_json::json!({"type": "info", "text": serde_json::to_string_pretty(&value).unwrap_or_default()}));
    }

    serde_json::json!({
        "name": "脚本执行结果",
        "status": status,
        "exit_code": exit_code,
        "elapsed_ms": elapsed_ms,
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "sections": [
            {
                "title": "执行摘要",
                "icon": "SUMMARY",
                "items": summary_items
            },
            {
                "title": "返回数据",
                "icon": "DATA",
                "items": data_items
            }
        ]
    })
}

fn scalar_json_display(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "-".to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn json_complex_item(key: &str, value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(rows) => json_array_item(key, rows),
        _ => serde_json::json!({
            "type": "info",
            "text": serde_json::to_string_pretty(value).unwrap_or_default()
        }),
    }
}

fn json_array_item(title: &str, rows: &[serde_json::Value]) -> serde_json::Value {
    if rows.iter().all(|row| row.is_object()) {
        let mut headers = Vec::<String>::new();
        for row in rows {
            if let Some(map) = row.as_object() {
                for key in map.keys() {
                    if !headers.contains(key) {
                        headers.push(key.clone());
                    }
                }
            }
        }
        let table_rows: Vec<Vec<String>> = rows
            .iter()
            .filter_map(|row| row.as_object())
            .map(|map| {
                headers
                    .iter()
                    .map(|key| {
                        map.get(key)
                            .map(scalar_json_display)
                            .unwrap_or_else(|| "-".to_string())
                    })
                    .collect()
            })
            .collect();
        return serde_json::json!({
            "type": "table",
            "headers": headers,
            "rows": table_rows
        });
    }

    serde_json::json!({
        "type": "info",
        "text": format!("{}:\n{}", title, serde_json::to_string_pretty(rows).unwrap_or_default())
    })
}

fn try_parse_json_output(output: &str, exit_code: i32, elapsed_ms: u64) -> String {
    let cleaned = clean_terminal_output(output);
    let trimmed = cleaned.trim();

    if let Some(json) = parse_json_candidate(trimmed) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
            return json_value_to_result(value, exit_code, elapsed_ms).to_string();
        }
        return json;
    }

    for block in fenced_blocks(trimmed) {
        if let Some(json) = parse_json_candidate(&block) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                return json_value_to_result(value, exit_code, elapsed_ms).to_string();
            }
            return json;
        }
    }

    for line in trimmed.lines() {
        if let Some(json) = parse_json_candidate(line) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) {
                return json_value_to_result(value, exit_code, elapsed_ms).to_string();
            }
            return json;
        }
    }

    let status = if exit_code == 0 { "ok" } else { "error" };
    let lines: Vec<&str> = trimmed.lines().collect();
    let line_count = lines.len();

    serde_json::json!({
        "name": "脚本执行结果",
        "status": status,
        "exit_code": exit_code,
        "elapsed_ms": elapsed_ms,
        "line_count": line_count,
        "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        "sections": [
            {
                "title": "执行摘要",
                "icon": "SUMMARY",
                "items": [
                    {"type": "label", "key": "状态", "value": if exit_code == 0 { "成功" } else { "失败" }, "status": status},
                    {"type": "label", "key": "退出码", "value": exit_code.to_string()},
                    {"type": "label", "key": "耗时", "value": format!("{}ms", elapsed_ms)},
                    {"type": "label", "key": "输出行数", "value": line_count.to_string()}
                ]
            },
            {
                "title": "执行输出",
                "icon": "OUTPUT",
                "items": [
                    {"type": "info", "text": trimmed}
                ]
            }
        ]
    }).to_string()
}

fn effective_exit_code_from_result_json(result_json: &str, process_exit_code: i32) -> i32 {
    if process_exit_code != 0 {
        return process_exit_code;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(result_json) else {
        return process_exit_code;
    };
    match value.get("status").and_then(|v| v.as_str()) {
        Some("error") => 1,
        _ => process_exit_code,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_traffic_filter, effective_exit_code_from_result_json, parse_linux_packet,
        traffic_packet_matches, try_parse_json_output, TrafficCaptureRequest,
        TrafficExchangeTracker,
    };

    #[test]
    fn parses_markdown_fenced_json_output() {
        let out = "说明\n```json\n{\"status\":\"ok\",\"sections\":[]}\n```";
        let parsed = try_parse_json_output(out, 0, 12);
        let value: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert_eq!(value["status"], "ok");
        assert!(value["sections"].is_array());
    }

    #[test]
    fn extracts_json_from_noisy_output() {
        let out = "prefix {\"status\":\"warn\",\"items\":[1,2]} suffix";
        let parsed = try_parse_json_output(out, 0, 12);
        let value: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert!(value.get("sections").is_some());
        assert_eq!(value["status"], "warn");
    }

    #[test]
    fn wraps_json_arrays_as_renderable_result() {
        let out = r#"[{"name":"nginx","status":"running"},{"name":"redis","status":"running"}]"#;
        let parsed = try_parse_json_output(out, 0, 12);
        let value: serde_json::Value = serde_json::from_str(&parsed).unwrap();
        assert!(value.get("sections").is_some());
        assert_eq!(value["sections"][1]["items"][0]["type"], "table");
    }

    #[test]
    fn structured_error_result_overrides_successful_process_exit_for_history() {
        let out = r#"{"name":"维护结果","status":"error","sections":[]}"#;
        let parsed = try_parse_json_output(out, 0, 12);
        assert_eq!(effective_exit_code_from_result_json(&parsed, 0), 1);
        assert_eq!(effective_exit_code_from_result_json(&parsed, 7), 7);
    }

    #[test]
    fn traffic_filter_combines_protocol_and_safe_fields() {
        let req = TrafficCaptureRequest {
            action: "start".to_string(),
            interface: "ens18".to_string(),
            protocol: "tcp".to_string(),
            ip: "10.0.0.8".to_string(),
            port: "443".to_string(),
            domain: "example.com".to_string(),
            path: "/api/v1".to_string(),
            limit: Some(200),
        };

        let filter = build_traffic_filter(&req);

        assert!(filter.contains("tcp"));
        assert!(filter.contains("host 10.0.0.8"));
        assert!(filter.contains("port 443"));
        assert!(!filter.contains(";"));
    }

    #[test]
    fn raw_packet_parser_decodes_tcp_udp_and_http_fields() {
        let tcp = sample_ipv4_tcp_frame(
            [10, 0, 0, 8],
            [93, 184, 216, 34],
            55312,
            80,
            b"GET /api/orders?id=7 HTTP/1.1\r\nHost: example.com\r\n\r\n",
        );
        let udp = sample_ipv4_udp_frame([10, 0, 0, 8], [224, 0, 0, 251], 5353, 5353, b"dns");

        let parsed_tcp = parse_linux_packet(&tcp, 7).unwrap();
        let parsed_udp = parse_linux_packet(&udp, 8).unwrap();

        assert_eq!(parsed_tcp["protocol"], "TCP");
        assert_eq!(parsed_tcp["src_ip"], "10.0.0.8");
        assert_eq!(parsed_tcp["src_port"], "55312");
        assert_eq!(parsed_tcp["dst_port"], "80");
        assert_eq!(parsed_tcp["http"]["method"], "GET");
        assert_eq!(parsed_tcp["http"]["path"], "/api/orders?id=7");
        assert_eq!(parsed_tcp["http"]["host"], "example.com");
        assert!(traffic_packet_matches(
            &parsed_tcp,
            &"example.com".to_string(),
            &"/api".to_string()
        ));
        assert_eq!(parsed_udp["protocol"], "UDP");
        assert_eq!(parsed_udp["dst_port"], "5353");
    }

    #[test]
    fn traffic_exchange_tracker_merges_http_request_and_response() {
        let request = parse_linux_packet(
            &sample_ipv4_tcp_frame(
                [10, 0, 0, 8],
                [93, 184, 216, 34],
                55312,
                80,
                b"GET /api/orders?id=7 HTTP/1.1\r\nHost: example.com\r\nUser-Agent: dm-test\r\n\r\n",
            ),
            1,
        )
        .unwrap();
        let response = parse_linux_packet(
            &sample_ipv4_tcp_frame(
                [93, 184, 216, 34],
                [10, 0, 0, 8],
                80,
                55312,
                b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nServer: unit-test\r\n\r\n{\"ok\":true}",
            ),
            2,
        )
        .unwrap();

        let mut tracker = TrafficExchangeTracker::new();
        assert!(tracker.ingest_packet(request).is_empty());
        let records = tracker.ingest_packet(response);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["protocol"], "HTTP");
        assert_eq!(records[0]["transport_protocol"], "TCP");
        assert_eq!(records[0]["request"]["method"], "GET");
        assert_eq!(records[0]["request"]["path"], "/api/orders?id=7");
        assert_eq!(records[0]["request"]["headers"]["host"], "example.com");
        assert_eq!(records[0]["response"]["status"], 200);
        assert_eq!(
            records[0]["response"]["headers"]["content-type"],
            "application/json"
        );
        assert_eq!(records[0]["response"]["body_preview"], "{\"ok\":true}");
    }

    #[test]
    fn raw_payload_preview_preserves_utf8_text() {
        let frame = sample_ipv4_tcp_frame(
            [10, 0, 0, 8],
            [10, 0, 0, 9],
            55312,
            9000,
            "明文 TCP payload: 订单状态=成功".as_bytes(),
        );

        let parsed = parse_linux_packet(&frame, 9).unwrap();

        assert_eq!(parsed["protocol"], "TCP");
        assert!(parsed["raw"].as_str().unwrap().contains("明文 TCP payload"));
        assert!(parsed["raw"].as_str().unwrap().contains("订单状态=成功"));
    }

    #[test]
    fn tls_client_hello_is_marked_as_https_encrypted() {
        let payload = sample_tls_client_hello("example.com");
        let frame = sample_ipv4_tcp_frame([10, 0, 0, 8], [93, 184, 216, 34], 55312, 443, &payload);
        let packet = parse_linux_packet(&frame, 10).unwrap();
        let mut tracker = TrafficExchangeTracker::new();
        let records = tracker.ingest_packet(packet);

        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["protocol"], "HTTPS");
        assert_eq!(records[0]["transport_protocol"], "TCP");
        assert_eq!(records[0]["host"], "example.com");
        assert!(records[0]["decoded"]
            .as_str()
            .unwrap()
            .contains("HTTPS/TLS"));
    }

    fn sample_ipv4_tcp_frame(
        src: [u8; 4],
        dst: [u8; 4],
        src_port: u16,
        dst_port: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut frame = ethernet_ipv4_prefix(6, src, dst, 20 + payload.len());
        frame.extend_from_slice(&src_port.to_be_bytes());
        frame.extend_from_slice(&dst_port.to_be_bytes());
        frame.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1, 0x50, 0x18, 0x20, 0, 0, 0, 0, 0]);
        frame.extend_from_slice(payload);
        frame
    }

    fn sample_tls_client_hello(host: &str) -> Vec<u8> {
        let host_bytes = host.as_bytes();
        let mut sni = Vec::new();
        sni.extend_from_slice(&(host_bytes.len() as u16).to_be_bytes());
        sni.insert(0, 0);
        sni.extend_from_slice(host_bytes);
        let list_len = sni.len() as u16;
        let mut sni_ext = Vec::new();
        sni_ext.extend_from_slice(&list_len.to_be_bytes());
        sni_ext.extend_from_slice(&sni);

        let mut extensions = Vec::new();
        extensions.extend_from_slice(&0u16.to_be_bytes());
        extensions.extend_from_slice(&(sni_ext.len() as u16).to_be_bytes());
        extensions.extend_from_slice(&sni_ext);

        let mut hello = Vec::new();
        hello.extend_from_slice(&[0x03, 0x03]);
        hello.extend_from_slice(&[0u8; 32]);
        hello.push(0);
        hello.extend_from_slice(&2u16.to_be_bytes());
        hello.extend_from_slice(&0x1301u16.to_be_bytes());
        hello.push(1);
        hello.push(0);
        hello.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        hello.extend_from_slice(&extensions);

        let mut handshake = Vec::new();
        handshake.push(0x01);
        let hello_len = hello.len();
        handshake.extend_from_slice(&[
            ((hello_len >> 16) & 0xff) as u8,
            ((hello_len >> 8) & 0xff) as u8,
            (hello_len & 0xff) as u8,
        ]);
        handshake.extend_from_slice(&hello);

        let mut record = Vec::new();
        record.extend_from_slice(&[0x16, 0x03, 0x01]);
        record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
        record.extend_from_slice(&handshake);
        record
    }

    fn sample_ipv4_udp_frame(
        src: [u8; 4],
        dst: [u8; 4],
        src_port: u16,
        dst_port: u16,
        payload: &[u8],
    ) -> Vec<u8> {
        let udp_len = 8 + payload.len();
        let mut frame = ethernet_ipv4_prefix(17, src, dst, udp_len);
        frame.extend_from_slice(&src_port.to_be_bytes());
        frame.extend_from_slice(&dst_port.to_be_bytes());
        frame.extend_from_slice(&(udp_len as u16).to_be_bytes());
        frame.extend_from_slice(&[0, 0]);
        frame.extend_from_slice(payload);
        frame
    }

    fn ethernet_ipv4_prefix(
        proto: u8,
        src: [u8; 4],
        dst: [u8; 4],
        transport_len: usize,
    ) -> Vec<u8> {
        let total_len = 20 + transport_len;
        let mut frame = vec![
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0x08, 0x00, 0x45, 0, 0, 0, 0, 1, 0, 0, 64, proto,
            0, 0,
        ];
        frame[16..18].copy_from_slice(&(total_len as u16).to_be_bytes());
        frame.extend_from_slice(&src);
        frame.extend_from_slice(&dst);
        frame
    }
}

async fn handle_dashboard_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut update_interval = interval(Duration::from_secs(10));

    let msg = serde_json::json!({
        "type": "connected",
        "message": "仪表盘实时数据已连接"
    });
    if let Ok(json) = serde_json::to_string(&msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    loop {
        tokio::select! {
            _ = update_interval.tick() => {
                let config = state.config.clone();
                let (sys, script_count) = tokio::task::spawn_blocking(move || {
                    let sys = crate::dashboard::get_system_info();
                    let dirs = crate::config::all_script_dirs(&config);
                    let script_count = script::discover_scripts(&dirs).map(|s| s.len()).unwrap_or(0);
                    (sys, script_count)
                })
                .await
                .unwrap_or_else(|_| (crate::dashboard::get_system_info(), 0));
                crate::web::api::record_metric_sample(&state.db, &sys);
                let (total_exec, success, failure) = state.db.get_stats();

                let mut processes: Vec<serde_json::Value> = sys.top_processes.iter().map(|p| {
                    serde_json::json!({
                        "pid": p.pid,
                        "name": p.name,
                        "cpu_usage": p.cpu_usage,
                        "memory_bytes": p.memory_bytes,
                    })
                }).collect();

                processes.sort_by(|a, b| {
                    let a_cpu = a["cpu_usage"].as_f64().unwrap_or(0.0);
                    let b_cpu = b["cpu_usage"].as_f64().unwrap_or(0.0);
                    let a_mem = a["memory_bytes"].as_u64().unwrap_or(0);
                    let b_mem = b["memory_bytes"].as_u64().unwrap_or(0);
                    let score_a = a_cpu * 0.6 + (a_mem as f64 / 1024.0 / 1024.0) * 0.4;
                    let score_b = b_cpu * 0.6 + (b_mem as f64 / 1024.0 / 1024.0) * 0.4;
                    score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
                });

                let data = serde_json::json!({
                    "type": "update",
                    "timestamp": chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                    "system": {
                        "cpu_usage": sys.cpu_usage,
                        "memory_total": sys.memory_total,
                        "memory_used": sys.memory_used,
                        "memory_usage": sys.memory_usage,
                        "disk_total": sys.disk_total,
                        "disk_used": sys.disk_used,
                        "disk_usage": sys.disk_usage,
                        "load_avg": {
                            "one": sys.load_avg.one,
                            "five": sys.load_avg.five,
                            "fifteen": sys.load_avg.fifteen,
                        },
                        "cpu_count": sys.cpu_count,
                        "cpu_brand": sys.cpu_brand,
                        "hostname": sys.hostname,
                        "os": sys.os,
                        "kernel": sys.kernel,
                        "arch": sys.arch,
                        "uptime": sys.uptime,
                        "process_count": sys.process_count,
                        "networks": sys.networks.iter().map(|n| {
                            serde_json::json!({
                                "name": n.name,
                                "ip": n.ip,
                                "mac": n.mac,
                                "received_bytes": n.received_bytes,
                                "transmitted_bytes": n.transmitted_bytes,
                            })
                        }).collect::<Vec<_>>(),
                        "disks": sys.disks.iter().map(|d| {
                            serde_json::json!({
                                "name": d.name,
                                "mount_point": d.mount_point,
                                "total": d.total,
                                "used": d.used,
                                "usage": d.usage,
                                "fs_type": d.fs_type,
                            })
                        }).collect::<Vec<_>>(),
                        "top_processes": processes,
                    },
                    "stats": {
                        "total_scripts": script_count,
                        "total_executions": total_exec,
                        "success_count": success,
                        "failure_count": failure,
                    }
                });

                if let Ok(json) = serde_json::to_string(&data) {
                    if sender.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    _ => continue,
                }
            }
        }
    }
}
