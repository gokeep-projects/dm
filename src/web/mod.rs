pub mod api;
pub mod ws;

use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{
    routing::{get, post, put},
    Router,
};
use rust_embed::Embed;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use self::api::{AlertCache, AppState};
use crate::config::Config;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

#[derive(Embed)]
#[folder = "web/dist/"]
struct FrontendAssets;

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    if let Some(content) = FrontendAssets::get(path) {
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, static_cache_control(path))
            .body(axum::body::Body::from(content.data.to_vec()))
            .unwrap();
    }
    if let Some(content) = FrontendAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-store, no-cache, must-revalidate")
            .body(axum::body::Body::from(content.data.to_vec()))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(axum::body::Body::from("Not Found"))
        .unwrap()
}

fn static_cache_control(path: &str) -> &'static str {
    if path == "index.html" {
        "no-store, no-cache, must-revalidate"
    } else {
        "no-cache, must-revalidate"
    }
}

pub fn build_router(config: Config) -> Router {
    let db_path = crate::config::db_path(&config);
    let db = crate::db::Database::open(&db_path);

    let state = AppState {
        config,
        db,
        alert_cache: Arc::new(RwLock::new(AlertCache::default())),
        health_tasks: Arc::new(RwLock::new(std::collections::HashMap::new())),
        java_cancel_tokens: Arc::new(RwLock::new(std::collections::HashMap::new())),
        java_cancelled_keys: Arc::new(RwLock::new(std::collections::HashSet::new())),
        alert_refreshing: Arc::new(AtomicBool::new(false)),
    };

    let analyzer_state = state.clone();
    tokio::spawn(async move {
        let scan_interval = std::time::Duration::from_secs(60);
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        loop {
            if analyzer_state
                .alert_refreshing
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                let state = analyzer_state.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    crate::web::api::refresh_alert_cache(&state);
                    state.alert_refreshing.store(false, Ordering::SeqCst);
                })
                .await;
            }
            tokio::time::sleep(scan_interval).await;
        }
    });

    let cleanup_db = state.db.clone();
    tokio::spawn(async move {
        loop {
            cleanup_db.cleanup_metric_history();
            tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        }
    });

    Router::new()
        .route("/api/scripts", get(api::list_scripts))
        .route(
            "/api/scripts/:id",
            get(api::get_script)
                .put(api::update_script)
                .delete(api::delete_script),
        )
        .route("/api/scripts/:id/source", get(api::get_script_source))
        .route("/api/scripts/stats/all", get(api::all_scripts_stats))
        .route("/api/scripts/upload", post(api::upload_script))
        .route("/api/scripts/:id/file", put(api::replace_script_file))
        .route("/api/scripts/:id/stats", get(api::get_script_stats))
        .route("/api/scripts/:id/run", post(api::run_script))
        .route("/api/scripts/:id/duplicate", post(api::duplicate_script))
        .route("/api/checks", get(api::list_checks))
        .route("/api/checks/export", get(api::export_checks))
        .route("/api/checks/:id", get(api::run_check))
        .route("/api/check-configs/import", post(api::import_check_configs))
        .route("/api/check-configs/export", get(api::export_check_configs))
        .route(
            "/api/check-configs/template",
            get(api::check_config_template),
        )
        .route(
            "/api/check-configs/:id",
            get(api::get_check_config).put(api::update_check_config),
        )
        .route("/api/dashboard/stats", get(api::dashboard_stats))
        .route("/api/dashboard/metrics", get(api::dashboard_metrics))
        .route(
            "/api/dashboard/history",
            get(api::get_history).delete(api::clear_history),
        )
        .route("/api/system/info", get(api::system_info))
        .route("/api/system/processes", get(api::get_all_processes))
        .route("/api/java/processes", get(api::java_processes))
        .route("/api/java/analyze", post(api::java_analyze))
        .route("/api/java/scan", post(api::java_scan))
        .route("/api/java/hprof", post(api::java_hprof))
        .route("/api/java/cancel/:key", post(api::java_cancel))
        .route("/api/traffic/interfaces", get(api::traffic_interfaces))
        .route("/api/docs", get(api::list_docs).post(api::create_doc_api))
        .route("/api/docs/dirs", post(api::create_doc_dir_api))
        .route("/api/docs/import", post(api::import_doc_api))
        .route(
            "/api/docs/:id",
            get(api::get_doc_api)
                .put(api::update_doc_api)
                .delete(api::delete_doc_api),
        )
        .route("/api/docs/:id/attachments", get(api::list_doc_attachments))
        .route(
            "/api/docs/:id/attachments/:filename",
            get(api::download_doc_file).put(api::upload_doc_file),
        )
        .route("/api/files", get(api::list_directory))
        .route("/api/health", get(api::health_check))
        .route(
            "/api/maintenance",
            get(api::list_maintenance).post(api::create_maintenance),
        )
        .route("/api/maintenance/import", post(api::import_maintenance_api))
        .route(
            "/api/maintenance/:id",
            get(api::get_maintenance)
                .put(api::update_maintenance)
                .delete(api::delete_maintenance),
        )
        .route(
            "/api/maintenance/:id/complete",
            post(api::complete_maintenance),
        )
        .route("/api/services/:name/action", post(api::service_action))
        .route("/api/services/:name/logs", get(api::get_service_logs))
        .route("/api/services/:name/health", get(api::check_service_health))
        .route("/api/health/full", get(api::full_health_check))
        .route("/api/health/full/start", post(api::start_full_health_check))
        .route("/api/health/full/:id", get(api::get_full_health_progress))
        .route(
            "/api/alerts",
            get(api::get_alerts).delete(api::clear_alerts),
        )
        .route("/api/rules", get(api::list_rules).post(api::create_rule))
        .route("/api/rules/import", post(api::import_rules))
        .route("/api/rules/:id", put(api::update_rule))
        .route("/api/config", get(api::get_config).put(api::update_config))
        .route("/ws/dashboard", get(ws::ws_dashboard_handler))
        .route("/ws/exec/:id", get(ws::ws_handler))
        .route("/ws/traffic", get(ws::ws_traffic_handler))
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
