use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    response::{Html, IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use shared::{ActivityEntry, ClientMessage, ClientSession, ClientStatus, ServerMessage, User};
use tokio::sync::{broadcast, mpsc, RwLock};
use tower_http::cors::CorsLayer;

mod auth;
mod discovery;
mod updater;
mod app_policy;
mod client_auth_mode;
mod analisa;
pub mod error_logger;
pub mod api_service;
pub mod ai_detector;
pub mod portal;

#[derive(Clone)]
pub struct AppState {
    pub users: Arc<RwLock<HashMap<String, User>>>,
    pub clients: Arc<RwLock<HashMap<String, ClientSession>>>,
    pub activities: Arc<RwLock<Vec<ActivityEntry>>>,
    pub client_senders: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<ServerMessage>>>>,
    pub dashboard_tx: broadcast::Sender<String>,
    pub update_config: Arc<RwLock<updater::UpdateConfig>>,
    pub session_store: auth::SessionStore,
    pub app_policy: Arc<RwLock<shared::AppPolicyConfig>>,
    pub client_auth_mode: Arc<RwLock<shared::ClientAuthConfig>>,
    pub service_keys: Arc<RwLock<api_service::ServiceKeyConfig>>,
    pub ai_detector: Arc<RwLock<ai_detector::AiDetectorConfig>>,
}

const USERS_FILE: &str = "users.json";

fn load_or_init_users() -> HashMap<String, User> {
    if let Ok(data) = std::fs::read_to_string(USERS_FILE) {
        if let Ok(mut users) = serde_json::from_str::<HashMap<String, User>>(&data) {
            let mut modified = false;
            if !users.contains_key("superadmin") {
                users.insert("superadmin".to_string(), User {
                    username: "superadmin".to_string(),
                    password: "admin123".to_string(),
                    full_name: "Super Administrator Lab".to_string(),
                    role: "superadmin".to_string(),
                });
                modified = true;
            }
            if modified {
                save_users_to_file(&users);
            }
            println!("[SERVER] Berhasil memuat {} user dari {}", users.len(), USERS_FILE);
            return users;
        }
    }

    let mut users = HashMap::new();
    let default_users = vec![
        User {
            username: "superadmin".to_string(),
            password: "admin123".to_string(),
            full_name: "Super Administrator Lab".to_string(),
            role: "superadmin".to_string(),
        },
        User {
            username: "admin".to_string(),
            password: "admin123".to_string(),
            full_name: "Administrator Lab".to_string(),
            role: "admin".to_string(),
        },
        User {
            username: "guru".to_string(),
            password: "guru123".to_string(),
            full_name: "Instruktur / Guru Lab".to_string(),
            role: "admin".to_string(),
        },
        User {
            username: "siswa01".to_string(),
            password: "lab123".to_string(),
            full_name: "Ahmad Siswa 01".to_string(),
            role: "student".to_string(),
        },
        User {
            username: "siswa02".to_string(),
            password: "lab123".to_string(),
            full_name: "Budi Siswa 02".to_string(),
            role: "student".to_string(),
        },
        User {
            username: "siswa03".to_string(),
            password: "lab123".to_string(),
            full_name: "Citra Siswa 03".to_string(),
            role: "student".to_string(),
        },
    ];

    for u in default_users {
        users.insert(u.username.clone(), u);
    }

    if let Ok(json) = serde_json::to_string_pretty(&users) {
        let _ = std::fs::write(USERS_FILE, json);
    }
    println!("[SERVER] Inisialisasi default user lab di {}", USERS_FILE);
    users
}

fn save_users_to_file(users: &HashMap<String, User>) {
    if let Ok(json) = serde_json::to_string_pretty(users) {
        let _ = std::fs::write(USERS_FILE, json);
    }
}

pub const CLIENTS_FILE: &str = "clients.json";

pub fn resolve_clients_path() -> std::path::PathBuf {
    let candidates = [
        std::path::PathBuf::from(CLIENTS_FILE),
        std::path::PathBuf::from("dist/Molay-Server/clients.json"),
        std::path::PathBuf::from("../dist/Molay-Server/clients.json"),
        std::path::PathBuf::from("target/release/clients.json"),
    ];
    for c in &candidates {
        if c.exists() && c.is_file() {
            return c.clone();
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let exe_candidate = dir.join(CLIENTS_FILE);
            if exe_candidate.exists() && exe_candidate.is_file() {
                return exe_candidate;
            }
        }
    }
    std::path::PathBuf::from(CLIENTS_FILE)
}

pub fn load_persisted_clients() -> HashMap<String, ClientSession> {
    let path = resolve_clients_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(mut map) = serde_json::from_str::<HashMap<String, ClientSession>>(&data) {
            println!("[SERVER] 📌 Berhasil memuat {} PC klien ter-pin dari {:?}", map.len(), path);
            for client in map.values_mut() {
                client.status = ClientStatus::Disconnected;
                client.latest_screen = None;
            }
            return map;
        }
    }
    HashMap::new()
}

pub fn save_clients_to_file(clients: &HashMap<String, ClientSession>) {
    let path = resolve_clients_path();
    if let Ok(json) = serde_json::to_string_pretty(clients) {
        let _ = std::fs::write(&path, json);
    }
}

pub const ACTIVITIES_FILE: &str = "activities.jsonl";

pub fn resolve_activities_path() -> std::path::PathBuf {
    let candidates = [
        std::path::PathBuf::from(ACTIVITIES_FILE),
        std::path::PathBuf::from("dist/Molay-Server/activities.jsonl"),
        std::path::PathBuf::from("../dist/Molay-Server/activities.jsonl"),
        std::path::PathBuf::from("target/release/activities.jsonl"),
    ];
    for c in &candidates {
        if c.exists() && c.is_file() {
            return c.clone();
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let exe_candidate = dir.join(ACTIVITIES_FILE);
            if exe_candidate.exists() && exe_candidate.is_file() {
                return exe_candidate;
            }
        }
    }
    std::path::PathBuf::from(ACTIVITIES_FILE)
}

pub fn load_persisted_activities() -> Vec<ActivityEntry> {
    let mut list = Vec::new();
    let path = resolve_activities_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Ok(entry) = serde_json::from_str::<ActivityEntry>(trimmed) {
                    list.push(entry.enriched());
                }
            }
        }
        println!("[SERVER] Berhasil memuat {} riwayat log aktivitas dari {:?}", list.len(), path);
    } else {
        println!("[SERVER] Belum ada file log aktivitas di {:?}", path);
    }
    list
}

pub fn append_activity_to_file(entry: &ActivityEntry) {
    use std::io::Write;
    let path = resolve_activities_path();
    let enriched = entry.clone().enriched();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        if let Ok(json) = serde_json::to_string(&enriched) {
            let _ = writeln!(file, "{}", json);
        }
    }
    // Jika path berbeda dari "./activities.jsonl" dan file "./activities.jsonl" ada, jaga agar tetap tersinkron
    if path != std::path::Path::new(ACTIVITIES_FILE) && std::path::Path::new(ACTIVITIES_FILE).exists() {
        if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(ACTIVITIES_FILE) {
            if let Ok(json) = serde_json::to_string(&enriched) {
                let _ = writeln!(file, "{}", json);
            }
        }
    }
}

#[cfg(windows)]
fn try_register_firewall_rule() {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let _ = std::process::Command::new("netsh")
        .args(["advfirewall", "firewall", "add", "rule", "name=Molay Sekar7 Server (TCP 1916)", "dir=in", "action=allow", "protocol=TCP", "localport=1916", "profile=any"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let _ = std::process::Command::new("netsh")
        .args(["advfirewall", "firewall", "add", "rule", "name=Molay Sekar7 Server (UDP 1916)", "dir=in", "action=allow", "protocol=UDP", "localport=1916", "profile=any"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = exe_path.to_string_lossy();
        let _ = std::process::Command::new("netsh")
            .args(["advfirewall", "firewall", "add", "rule", "name=Molay Server Program", "dir=in", "action=allow", &format!("program={}", exe_str), "profile=any"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }
}

#[cfg(not(windows))]
fn try_register_firewall_rule() {}

#[cfg(windows)]
fn disable_quick_edit_mode() {
    type HANDLE = *mut std::ffi::c_void;
    type BOOL = i32;
    type DWORD = u32;
    const STD_INPUT_HANDLE: DWORD = 0xFFFFFFF6; // ((DWORD)-10)
    const ENABLE_QUICK_EDIT_MODE: DWORD = 0x0040;
    const ENABLE_EXTENDED_FLAGS: DWORD = 0x0080;

    extern "system" {
        fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
        fn GetConsoleMode(hConsoleHandle: HANDLE, lpMode: *mut DWORD) -> BOOL;
        fn SetConsoleMode(hConsoleHandle: HANDLE, dwMode: DWORD) -> BOOL;
    }

    unsafe {
        let handle = GetStdHandle(STD_INPUT_HANDLE);
        if !handle.is_null() && handle as isize != -1 {
            let mut mode: DWORD = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                let new_mode = (mode & !ENABLE_QUICK_EDIT_MODE) | ENABLE_EXTENDED_FLAGS;
                let _ = SetConsoleMode(handle, new_mode);
            }
        }
    }
}

#[cfg(not(windows))]
fn disable_quick_edit_mode() {}

#[tokio::main]
async fn main() {
    // Matikan QuickEdit Mode agar klik mouse di jendela konsol tidak membekukan (pause/Select) server
    disable_quick_edit_mode();

    // Pastikan working directory selalu di folder file .exe berada (agar file json terbaca sempurna)
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let _ = std::env::set_current_dir(exe_dir);
        }
    }

    println!("========================================================");
    println!("   MOLAY SEKAR7 - MONITORING LAYAR SERVER v{}", crate::error_logger::SERVER_VERSION);
    println!("   Memulai server dan layanan jaringan lab...");
    println!("========================================================");

    tracing_subscriber::fmt::init();
    tokio::spawn(async move {
        try_register_firewall_rule();
    });

    let (dashboard_tx, _) = broadcast::channel(512);
    let service_keys = Arc::new(RwLock::new(api_service::load_or_init_service_keys()));
    let session_store = auth::SessionStore::new_with_keys(service_keys.clone());
    let ai_detector = Arc::new(RwLock::new(ai_detector::load_or_init_ai_detector_config()));
    let state = AppState {
        users: Arc::new(RwLock::new(load_or_init_users())),
        clients: Arc::new(RwLock::new(load_persisted_clients())),
        activities: Arc::new(RwLock::new(load_persisted_activities())),
        client_senders: Arc::new(RwLock::new(HashMap::new())),
        dashboard_tx,
        update_config: Arc::new(RwLock::new(updater::load_or_init_update_config())),
        session_store,
        app_policy: Arc::new(RwLock::new(app_policy::load_or_init_app_policy())),
        client_auth_mode: Arc::new(RwLock::new(client_auth_mode::load_or_init_client_auth_config())),
        service_keys,
        ai_detector,
    };

    updater::cleanup_old_server_binary();
    updater::start_background_cloud_checker(state.clone());
    ai_detector::spawn_periodic_scanner(state.clone());

    // Background task: bersihkan session expired setiap jam
    {
        let store_clone = state.session_store.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                store_clone.prune_expired_sessions().await;
            }
        });
    }

    // Background task: deteksi PC klien yang mati / putus koneksi secara otomatis (Zero Panic)
    {
        let state_sweep = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
            loop {
                interval.tick().await;
                let now = Utc::now();
                let mut updates = Vec::new();
                {
                    let mut clients = state_sweep.clients.write().await;
                    for (id, client) in clients.iter_mut() {
                        if client.status != ClientStatus::Disconnected {
                            let idle_secs = (now - client.last_seen).num_seconds();
                            // Jika PC tidak mengirim frame/heartbeat > 15 detik, otomatis tandai Terputus
                            if idle_secs >= 15 {
                                println!("[SERVER-SWEEPER] PC Klien '{}' ({}) tidak aktif selama {}s -> otomatis ditandai Terputus.", client.hostname, id, idle_secs);
                                client.status = ClientStatus::Disconnected;
                                updates.push((id.clone(), client.clone()));
                            }
                        }
                    }
                }
                if !updates.is_empty() {
                    let mut senders = state_sweep.client_senders.write().await;
                    for (id, client) in updates {
                        senders.remove(&id);
                        let update_json = serde_json::json!({
                            "type": "ClientUpdate",
                            "client": client
                        });
                        let _ = state_sweep.dashboard_tx.send(update_json.to_string());
                    }
                    let clients = state_sweep.clients.read().await;
                    save_clients_to_file(&clients);
                }
            }
        });
    }

    let app = Router::new()
        .route("/", get(portal::index_dispatch_handler))
        .route("/portal", get(portal::portal_page_handler))
        .route("/admin", get(portal::admin_page_handler))
        .route("/dashboard", get(portal::admin_page_handler))
        .route("/dist/:filename", get(portal::download_dist_file_handler))
        .route("/download/:filename", get(portal::download_dist_file_handler))
        .route("/analisaaktivitas", get(analisa_page_handler))
        .route("/analisa", get(analisa_page_handler))
        .route("/api/analisa", get(analisa::get_analisa))
        .route("/analisa/data", get(analisa::get_analisa))
        .route("/api/auth/login", post(auth::login_handler))
        .route("/auth/login", post(auth::login_handler))
        .route("/api/auth/me", get(auth::me_handler))
        .route("/auth/me", get(auth::me_handler))
        .route("/api/auth/logout", post(auth::logout_handler))
        .route("/auth/logout", post(auth::logout_handler))
        .route("/api/clients", get(get_clients))
        .route("/api/clients/cleanup-offline", post(cleanup_offline_clients_handler))
        .route("/api/clients/:id", delete(delete_client_handler))
        .route("/api/activities", get(get_activities))
        .route("/api/policy", get(app_policy::get_policy_handler).post(app_policy::save_policy_handler))
        .route("/api/policy/exemptions", post(app_policy::add_exemption_handler))
        .route("/api/policy/exemptions/remove", post(app_policy::remove_exemption_handler))
        .route("/api/policy/ai-detector/config", get(ai_detector::get_config_handler).post(ai_detector::save_config_handler))
        .route("/api/policy/ai-detector/scan", post(ai_detector::scan_activities_handler))
        .route("/api/policy/ai-detector/test", post(ai_detector::test_detect_handler))
        .route("/api/policy/ai-detector/remove-keyword", post(ai_detector::remove_detected_keyword_handler))
        .route("/api/client-auth-mode", get(client_auth_mode::get_auth_mode_handler).post(client_auth_mode::save_auth_mode_handler))
        .route("/api/users", get(get_users).post(create_user))
        .route("/api/users/sync-cloud", post(sync_users_from_cloud_handler))
        .route("/api/clients/:id/command", post(post_client_command))
        .route("/api/service-keys", get(api_service::list_service_keys_handler))
        .route("/api/service-keys/regenerate", post(api_service::regenerate_service_key_handler))
        .route("/api/sync/status", get(api_service::sync_status_handler))
        .route("/api/update/check", get(updater::check_update))
        .route("/api/update/client/download", get(updater::download_client))
        .route("/api/update/config", get(updater::get_update_config).post(updater::save_update_config_handler))
        .route("/api/update/broadcast", post(updater::broadcast_update))
        .route("/api/cloud-manifest.json", get(updater::get_cloud_manifest))
        .route("/api/update/server/check", get(updater::check_server_cloud_update))
        .route("/api/update/server/apply", post(updater::apply_server_cloud_update))
        .route("/api/update/client/sync-cloud", post(updater::sync_client_from_cloud))
        .route("/api/server-info", get(server_info_handler))
        .route("/logerorglobal", get(error_logger::get_error_logs_handler).post(error_logger::receive_error_log_handler))
        .route("/api/logerorglobal", get(error_logger::get_error_logs_handler).post(error_logger::receive_error_log_handler))
        .route("/api/logerorglobal/clear", post(error_logger::clear_error_logs_handler))
        .route("/ws/client", get(ws_client_handler))
        .route("/ws/dashboard", get(ws_dashboard_handler))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let port = 1916;
    discovery::start_udp_discovery_responder(port).await;
    let local_ip = discovery::get_local_lan_ip();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    println!("========================================================");
    println!("   MOLAY SEKAR7 - MONITORING LAYAR SERVER (LAN)");
    println!("   Dashboard Web Guru: http://localhost:{}", port);
    if local_ip != "127.0.0.1" {
        println!("   Dashboard via LAN:  http://{}:{}", local_ip, port);
        println!("   Alamat Server LAN:  {}:{}", local_ip, port);
        println!("   (Gunakan IP di atas untuk SEMUA PC Siswa & Guru)");
    } else {
        println!("   Dashboard via LAN:  http://192.168.20.100:{}", port);
        println!("   Alamat Server LAN:  192.168.20.100:{}", port);
        println!("   (Gunakan IP di atas untuk SEMUA PC Siswa & Guru)");
    }
    println!("   Status Keamanan:    Auth Terpasang (Akun Superadmin / Guru / Admin)");
    println!("========================================================");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error_logger::log_server_error("PortBindError", &format!("Gagal membuka port {}: {}", port, e), None);
            eprintln!("========================================================");
            eprintln!("   [FATAL ERROR] GAGAL MEMBUKA PORT {}: {}", port, e);
            eprintln!("   Port 1916 sedang dipakai oleh proses lain atau instance lama!");
            eprintln!("   Solusi: Buka Task Manager -> End Task 'MolayServer-Sekar7.exe'");
            eprintln!("   lalu jalankan ulang Jalankan-Molay-Server.bat.");
            eprintln!("========================================================");
            println!("Tekan Enter untuk keluar...");
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
            return;
        }
    };

    // Bridge kompatibilitas Cloudflare Tunnel (Port 3000 & IPv6)
    // Cloudflare Tunnel sering kali dikonfigurasi ke http://localhost:3000 atau [::1]:3000 secara default
    let app_bridge_3000_v6 = app.clone();
    tokio::spawn(async move {
        if let Ok(l) = tokio::net::TcpListener::bind("[::1]:3000").await {
            println!("[TUNNEL BRIDGE] Mendengarkan di [::1]:3000 (IPv6 Cloudflare Tunnel Ingress)");
            let _ = axum::serve(l, app_bridge_3000_v6.into_make_service_with_connect_info::<SocketAddr>()).await;
        }
    });

    let app_bridge_3000_v4 = app.clone();
    tokio::spawn(async move {
        let listener_res = match tokio::net::TcpListener::bind("0.0.0.0:3000").await {
            Ok(l) => {
                println!("[TUNNEL BRIDGE] Mendengarkan di 0.0.0.0:3000 (IPv4 Cloudflare Tunnel Ingress)");
                Some(l)
            }
            Err(_) => tokio::net::TcpListener::bind("127.0.0.1:3000").await.ok(),
        };
        if let Some(l) = listener_res {
            let _ = axum::serve(l, app_bridge_3000_v4.into_make_service_with_connect_info::<SocketAddr>()).await;
        }
    });

    let app_ipv6_1916 = app.clone();
    tokio::spawn(async move {
        if let Ok(l) = tokio::net::TcpListener::bind("[::1]:1916").await {
            println!("[SERVER] Mendengarkan di [::1]:1916 (IPv6 Localhost)");
            let _ = axum::serve(l, app_ipv6_1916.into_make_service_with_connect_info::<SocketAddr>()).await;
        }
    });

    // Buka browser dashboard secara otomatis begitu server berhasil mendengarkan port
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
        #[cfg(windows)]
        {
            let target_url = format!("http://localhost:{}", port);
            // 1. Coba explorer.exe lebih dahulu (andal di Windows untuk menembus isolasi UAC/Admin ke desktop user interaktif)
            let spawned = std::process::Command::new("explorer")
                .arg(&target_url)
                .spawn();
            if spawned.is_err() {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", &target_url])
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn();
            }
        }
    });

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

#[allow(dead_code)]
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn analisa_page_handler() -> Html<&'static str> {
    Html(include_str!("analisa.html"))
}

async fn server_info_handler() -> Json<serde_json::Value> {
    let local_ip = discovery::get_local_lan_ip();
    Json(serde_json::json!({
        "local_ip": local_ip,
        "port": 1916,
        "tunnel_domain": "molay.sekolahkartini7.web.id"
    }))
}

async fn get_clients(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
) -> axum::response::Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let clients = state.clients.read().await;
    let list: Vec<ClientSession> = clients.values().cloned().collect();
    Json(list).into_response()
}

async fn cleanup_offline_clients_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
) -> axum::response::Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let removed_ids: Vec<String> = {
        let mut clients = state.clients.write().await;
        let offline_keys: Vec<String> = clients
            .iter()
            .filter(|(_, c)| c.status == ClientStatus::Disconnected)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &offline_keys {
            clients.remove(k);
        }
        save_clients_to_file(&clients);
        offline_keys
    };
    if !removed_ids.is_empty() {
        let mut senders = state.client_senders.write().await;
        for id in &removed_ids {
            senders.remove(id);
            let msg = serde_json::json!({
                "type": "ClientRemoved",
                "client_id": id
            });
            let _ = state.dashboard_tx.send(msg.to_string());
        }
    }
    Json(serde_json::json!({
        "status": "ok",
        "message": format!("Berhasil membersihkan {} PC offline", removed_ids.len()),
        "removed_count": removed_ids.len()
    }))
    .into_response()
}

async fn delete_client_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
    Path(client_id): Path<String>,
) -> axum::response::Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let removed = {
        let mut clients = state.clients.write().await;
        let r = clients.remove(&client_id).is_some();
        if r {
            save_clients_to_file(&clients);
        }
        r
    };
    if removed {
        state.client_senders.write().await.remove(&client_id);
        let msg = serde_json::json!({
            "type": "ClientRemoved",
            "client_id": client_id
        });
        let _ = state.dashboard_tx.send(msg.to_string());
        Json(serde_json::json!({ "status": "ok", "message": "PC klien berhasil dihapus dari daftar" }))
            .into_response()
    } else {
        (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Klien tidak ditemukan" })),
        )
            .into_response()
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct ActivitiesQuery {
    pub token: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub tanggal: Option<String>,
    #[serde(default)]
    pub dari_tanggal: Option<String>,
    #[serde(default)]
    pub sampai_tanggal: Option<String>,
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub sort_by: Option<String>,
    #[serde(default)]
    pub order: Option<String>,
}

pub fn filter_and_sort_activities(
    activities: &[ActivityEntry],
    query: &ActivitiesQuery,
) -> Vec<ActivityEntry> {
    let wib = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
    let mut list: Vec<ActivityEntry> = if let Some(ref tgl) = query.tanggal {
        let tgl_str = tgl.trim().to_lowercase();
        if tgl_str == "all" || tgl_str == "semua" || tgl_str == "*" {
            let limit = query.limit.unwrap_or(5000).min(10000);
            activities.iter().rev().take(limit).map(|e| e.clone().enriched()).collect()
        } else {
            // Filter pakai WIB agar cocok dengan tanggal yang dikirim frontend (WIB)
            activities
                .iter()
                .filter(|e| e.timestamp.with_timezone(&wib).format("%Y-%m-%d").to_string() == tgl_str)
                .map(|e| e.clone().enriched())
                .collect()
        }
    } else if query.dari_tanggal.is_some() || query.sampai_tanggal.is_some() {
        let dari = query.dari_tanggal.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
        let sampai = query.sampai_tanggal.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty());
        let limit = query.limit.unwrap_or(5000).min(10000);

        activities
            .iter()
            .filter(|e| {
                let entry_date = e.timestamp.with_timezone(&wib).format("%Y-%m-%d").to_string();
                if let Some(d) = dari {
                    if entry_date.as_str() < d {
                        return false;
                    }
                }
                if let Some(s) = sampai {
                    if entry_date.as_str() > s {
                        return false;
                    }
                }
                true
            })
            .rev()
            .take(limit)
            .map(|e| e.clone().enriched())
            .collect()
    } else {
        // Default: ambil semua aktivitas hari ini dalam WIB, fallback ke 1000 terbaru
        let today_wib = chrono::Utc::now().with_timezone(&wib).format("%Y-%m-%d").to_string();
        let today_acts: Vec<ActivityEntry> = activities
            .iter()
            .filter(|e| e.timestamp.with_timezone(&wib).format("%Y-%m-%d").to_string() == today_wib)
            .map(|e| e.clone().enriched())
            .collect();
        if !today_acts.is_empty() {
            today_acts
        } else {
            // Fallback: 1000 terbaru jika hari ini belum ada aktivitas (misal server baru nyala)
            let limit = query.limit.unwrap_or(1000).min(5000);
            activities.iter().rev().take(limit).map(|e| e.clone().enriched()).collect()
        }
    };

    if let Some(ref cid) = query.client_id {
        let cid_trimmed = cid.trim();
        if !cid_trimmed.is_empty() && cid_trimmed != "ALL" {
            list.retain(|e| e.client_id.eq_ignore_ascii_case(cid_trimmed));
        }
    }

    if let Some(ref sort_col) = query.sort_by {
        let is_asc = query.order.as_deref().map(|o| o.eq_ignore_ascii_case("asc")).unwrap_or(false);
        match sort_col.to_lowercase().as_str() {
            "waktu" | "timestamp" | "time" => {
                list.sort_by(|a, b| if is_asc { a.timestamp.cmp(&b.timestamp) } else { b.timestamp.cmp(&a.timestamp) });
            }
            "client_id" | "pc" => {
                list.sort_by(|a, b| {
                    let cmp = a.client_id.to_lowercase().cmp(&b.client_id.to_lowercase());
                    if is_asc { cmp } else { cmp.reverse() }
                });
            }
            "username" | "siswa" | "user" => {
                list.sort_by(|a, b| {
                    let u_a = a.username.as_deref().unwrap_or("").to_lowercase();
                    let u_b = b.username.as_deref().unwrap_or("").to_lowercase();
                    let cmp = u_a.cmp(&u_b);
                    if is_asc { cmp } else { cmp.reverse() }
                });
            }
            "app_name" | "aplikasi" | "app" => {
                list.sort_by(|a, b| {
                    let cmp = a.app_name.to_lowercase().cmp(&b.app_name.to_lowercase());
                    if is_asc { cmp } else { cmp.reverse() }
                });
            }
            "category" | "kategori" => {
                list.sort_by(|a, b| {
                    let c_a = a.category.as_deref().unwrap_or("").to_lowercase();
                    let c_b = b.category.as_deref().unwrap_or("").to_lowercase();
                    let cmp = c_a.cmp(&c_b);
                    if is_asc { cmp } else { cmp.reverse() }
                });
            }
            _ => {}
        }
    } else if let Some(ref ord) = query.order {
        let is_asc = ord.eq_ignore_ascii_case("asc");
        list.sort_by(|a, b| if is_asc { a.timestamp.cmp(&b.timestamp) } else { b.timestamp.cmp(&a.timestamp) });
    }

    list
}

async fn get_activities(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<ActivitiesQuery>,
) -> axum::response::Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    // Jika memori masih kosong, coba muat ulang dari file agar tahan cold-start
    {
        let acts = state.activities.read().await;
        if acts.is_empty() {
            drop(acts);
            let fresh = load_persisted_activities();
            if !fresh.is_empty() {
                let mut acts_w = state.activities.write().await;
                *acts_w = fresh;
            }
        }
    }

    let activities = state.activities.read().await;
    let list = filter_and_sort_activities(&activities, &query);
    Json(list).into_response()
}

async fn get_users(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
) -> axum::response::Response {
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    if auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let users = state.users.read().await;
    let list: Vec<User> = users.values().cloned().collect();
    Json(list).into_response()
}

#[derive(Deserialize)]
struct CreateUserReq {
    username: String,
    password: String,
    full_name: String,
    role: String,
}

async fn create_user(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
    Json(payload): Json<CreateUserReq>,
) -> axum::response::Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let mut users = state.users.write().await;
    let new_user = User {
        username: payload.username.clone(),
        password: payload.password,
        full_name: payload.full_name,
        role: payload.role,
    };
    users.insert(payload.username, new_user.clone());
    save_users_to_file(&users);
    Json(new_user).into_response()
}

async fn sync_users_from_cloud_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
) -> axum::response::Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let cloud_url = {
        let auth_cfg = state.client_auth_mode.read().await;
        auth_cfg.cloud_users_url.clone().unwrap_or_else(shared::default_cloud_users_url)
    };

    println!("[SERVER-USER-SYNC] Menghubungi Cloud VPS: {}", cloud_url);
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(6)).build() {
        Ok(c) => c,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    };

    let res = match client.get(&cloud_url).header("User-Agent", "Molay-Server-UserSync/1.0").send().await {
        Ok(r) => r,
        Err(e) => return (axum::http::StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "success": false, "error": format!("Gagal menghubungi Cloud VPS: {}", e) }))).into_response(),
    };

    if !res.status().is_success() {
        return (axum::http::StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "success": false, "error": format!("Cloud VPS merespons dengan status: {}", res.status()) }))).into_response();
    }

    let text = match res.text().await {
        Ok(t) => t,
        Err(e) => return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "success": false, "error": e.to_string() }))).into_response(),
    };

    let parsed_users = match shared::parse_users_json(&text) {
        Ok(u) => u,
        Err(e) => return (axum::http::StatusCode::BAD_REQUEST, Json(serde_json::json!({ "success": false, "error": e }))).into_response(),
    };

    let count = parsed_users.len();
    {
        let mut users = state.users.write().await;
        for (k, v) in parsed_users {
            users.insert(k, v);
        }
        save_users_to_file(&users);
    }

    println!("[SERVER-USER-SYNC] ✅ Berhasil menyinkronkan {} akun siswa dari Cloud VPS!", count);
    Json(serde_json::json!({ "success": true, "message": format!("Berhasil menyinkronkan {} akun dari Cloud VPS!", count), "count": count })).into_response()
}

#[derive(Deserialize)]
struct CommandReq {
    action: String,
    payload: Option<String>,
}

async fn post_client_command(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
    Path(client_id): Path<String>,
    Json(body): Json<CommandReq>,
) -> axum::response::Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }
    let senders = state.client_senders.read().await;
    if let Some(tx) = senders.get(&client_id) {
        if body.action == "stream_config" {
            #[derive(serde::Deserialize)]
            struct StreamCfgPayload {
                #[serde(default)]
                enabled: Option<bool>,
                #[serde(default)]
                fps: Option<u32>,
                #[serde(default)]
                quality: Option<u8>,
            }
            if let Some(ref p) = body.payload {
                if let Ok(cfg) = serde_json::from_str::<StreamCfgPayload>(p) {
                    let _ = tx.send(ServerMessage::StreamConfig {
                        enabled: cfg.enabled.unwrap_or(true),
                        fps: cfg.fps.unwrap_or(5),
                        quality: cfg.quality.unwrap_or(65),
                    });
                }
            }
        } else {
            let msg = ServerMessage::Command {
                action: body.action.clone(),
                payload: body.payload.clone(),
            };
            let _ = tx.send(msg);
        }

        // Update local status & activity log hanya untuk perintah status/kebijakan admin
        // Abaikan input real-time streaming (mouse_event, key_event, key_press, text, stream_config) agar tidak membanjiri log
        let is_stream_input = matches!(
            body.action.as_str(),
            "mouse_event" | "key_press" | "key_event" | "text" | "stream_config"
        );

        if !is_stream_input {
            let mut clients = state.clients.write().await;
            if let Some(client) = clients.get_mut(&client_id) {
                if body.action == "lock" {
                    client.status = ClientStatus::Locked;
                    client.is_admin_locked = true;
                } else if body.action == "unlock" {
                    client.is_admin_locked = false;
                    if client.current_user.is_some() {
                        client.status = ClientStatus::LoggedIn;
                    } else {
                        client.status = ClientStatus::Locked;
                    }
                } else if body.action == "resolve_issue" || body.action == "clear_issue" {
                    client.current_issue = None;
                    client.issue_time = None;
                }
                let update_json = serde_json::json!({
                    "type": "ClientUpdate",
                    "client": client.clone()
                });
                let _ = state.dashboard_tx.send(update_json.to_string());

                let log_title = match body.action.as_str() {
                    "lock" => "Layar Komputer Dikunci oleh Guru / Admin".to_string(),
                    "unlock" => "Layar Komputer Dibuka oleh Guru / Admin".to_string(),
                    "resolve_issue" | "clear_issue" => "Laporan Kendala Siswa Diselesaikan oleh Guru".to_string(),
                    "alert" => format!("Pesan Peringatan Dikirim: {}", body.payload.as_deref().unwrap_or("Perhatian")),
                    _ => format!("Perintah: {}", body.action),
                };
                let entry = ActivityEntry {
                    client_id: client_id.clone(),
                    username: client.current_user.as_ref().map(|u| u.full_name.clone()),
                    app_name: "KONTROL GURU".to_string(),
                    window_title: log_title.clone(),
                    timestamp: Utc::now(),
                    category: Some("SYSTEM".to_string()),
                    severity: Some("info".to_string()),
                    details: Some(log_title),
                    keystrokes: None,
                    mouse_clicks: None,
                    mouse_distance_px: None,
                    idle_seconds: None,
                    is_afk: None,
                };
                state.activities.write().await.push(entry.clone());
                append_activity_to_file(&entry);
                let act_json = serde_json::json!({
                    "type": "NewActivity",
                    "activity": entry
                });
                let _ = state.dashboard_tx.send(act_json.to_string());
            }
        }

        Json(serde_json::json!({ "status": "ok", "message": "Command sent" })).into_response()
    } else {
        Json(serde_json::json!({ "status": "error", "message": "Client not connected" })).into_response()
    }
}

// WebSocket Klien (Agent di PC Lab Siswa)
async fn ws_client_handler(
    ws: WebSocketUpgrade,
    axum::extract::ConnectInfo(peer_addr): axum::extract::ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_client_socket(socket, state, peer_addr))
}

async fn handle_client_socket(socket: WebSocket, state: AppState, peer_addr: SocketAddr) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (client_tx, mut client_rx) = mpsc::unbounded_channel::<ServerMessage>();

    // Forward server messages to client
    tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sink.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut current_client_id: Option<String> = None;
    let mut disconnect_detail = None;

    let socket_timeout = std::time::Duration::from_secs(25);
    while let Ok(Some(msg_res)) = tokio::time::timeout(socket_timeout, ws_stream.next()).await {
        match msg_res {
            Ok(msg) => {
                match msg {
                    Message::Text(text) => {
                        if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                match client_msg {
                    ClientMessage::Register {
                        client_id,
                        hostname,
                        ip,
                        is_locked,
                    } => {
                        current_client_id = Some(client_id.clone());
                        state
                            .client_senders
                            .write()
                            .await
                            .insert(client_id.clone(), client_tx.clone());

                        let was_admin_locked = {
                            let clients = state.clients.read().await;
                            clients.get(&client_id).map(|c| c.is_admin_locked).unwrap_or(false)
                        };

                        let current_auth_cfg = state.client_auth_mode.read().await.clone();
                        let should_auto_unlock = current_auth_cfg.mode == shared::AuthModeType::NoAuth && current_auth_cfg.auto_unlock_on_no_auth && !was_admin_locked;

                        // Pastikan IP lokal yang sebenarnya tercatat (dari klien atau fallback ke peer_addr socket)
                        let resolved_ip = if ip.trim().is_empty() || ip == "Client-LAN" || ip.starts_with("Client-") {
                            peer_addr.ip().to_string()
                        } else {
                            ip
                        };

                        let session = ClientSession {
                            client_id: client_id.clone(),
                            hostname: hostname.clone(),
                            ip: resolved_ip,
                            status: if should_auto_unlock {
                                ClientStatus::LoggedIn
                            } else if is_locked || was_admin_locked {
                                ClientStatus::Locked
                            } else {
                                ClientStatus::LoggedIn
                            },
                            current_user: if should_auto_unlock {
                                Some(User {
                                    username: client_id.clone(),
                                    password: String::new(),
                                    full_name: format!("Siswa ({})", hostname),
                                    role: "student".to_string(),
                                })
                            } else {
                                None
                            },
                            last_seen: Utc::now(),
                            latest_window: None,
                            latest_app: None,
                            latest_screen: None,
                            is_admin_locked: was_admin_locked,
                            current_issue: None,
                            issue_time: None,
                            input_stats: None,
                        };

                        state
                            .clients
                            .write()
                            .await
                            .insert(client_id.clone(), session.clone());

                        {
                            let clients_snapshot = state.clients.read().await;
                            save_clients_to_file(&clients_snapshot);
                        }

                        println!("[SERVER] PC Klien Terhubung: {} ({})", session.hostname, session.ip);

                        if was_admin_locked {
                            let _ = client_tx.send(ServerMessage::Command {
                                action: "lock".to_string(),
                                payload: None,
                            });
                        } else if should_auto_unlock {
                            let _ = client_tx.send(ServerMessage::Command {
                                action: "unlock".to_string(),
                                payload: None,
                            });
                        }

                        // Kirim konfigurasi mode autentikasi klien ke klien
                        let _ = client_tx.send(ServerMessage::AuthModeUpdate { config: current_auth_cfg });

                        // Kirim kebijakan aplikasi terkini ke klien
                        let current_policy = state.app_policy.read().await.clone();
                        let _ = client_tx.send(ServerMessage::AppPolicyUpdate { policy: current_policy });

                        let update_json = serde_json::json!({
                            "type": "ClientUpdate",
                            "client": session
                        });
                        let _ = state.dashboard_tx.send(update_json.to_string());
                    }

                    ClientMessage::Login {
                        client_id,
                        username,
                        password,
                    } => {
                        let is_admin_locked = {
                            let clients = state.clients.read().await;
                            clients.get(&client_id).map(|c| c.is_admin_locked).unwrap_or(false)
                        };

                        let active_auth_cfg = state.client_auth_mode.read().await.clone();
                        let users = state.users.read().await;

                        // 1. Cek apakah cocok dengan akun terdaftar di users.json (Akun Siswa atau Admin/Guru)
                        if let Some(user) = users.get(&username) {
                            if user.password == password {
                                // Jika komputer sedang dikunci oleh Guru/Admin dan yang login adalah siswa biasa:
                                if is_admin_locked && !user.is_admin_or_superadmin() {
                                    println!("[AUTH] Login DITOLAK (Admin Locked): user '{}' di PC '{}'", username, client_id);
                                    let _ = client_tx.send(ServerMessage::LoginResponse {
                                        success: false,
                                        message: "Komputer sedang dikunci oleh Guru/Admin! Tidak bisa login.".to_string(),
                                        username: None,
                                        full_name: None,
                                    });
                                    continue;
                                }

                                // Login Berhasil dengan akun terdaftar
                                println!("[AUTH] Login SUKSES (Identitas): user '{}' di PC '{}'", username, client_id);
                                let mut clients = state.clients.write().await;
                                if let Some(client) = clients.get_mut(&client_id) {
                                    client.status = ClientStatus::LoggedIn;
                                    client.is_admin_locked = false; // Buka kunci jika akun admin/guru login
                                    client.current_user = Some(user.clone());

                                    let update_json = serde_json::json!({
                                        "type": "ClientUpdate",
                                        "client": client.clone()
                                    });
                                    let _ = state.dashboard_tx.send(update_json.to_string());
                                }

                                let _ = client_tx.send(ServerMessage::LoginResponse {
                                    success: true,
                                    message: format!("Selamat datang, {}!", user.full_name),
                                    username: Some(user.username.clone()),
                                    full_name: Some(user.full_name.clone()),
                                });
                                continue;
                            }
                        }

                        // 2. Jika bukan akun user terdaftar, sesuaikan dengan Mode Akses Lab yang aktif:
                        match active_auth_cfg.mode {
                            shared::AuthModeType::NoAuth => {
                                if is_admin_locked {
                                    println!("[AUTH] Login DITOLAK (Admin Locked): PC '{}' coba masuk saat terkunci admin", client_id);
                                    let _ = client_tx.send(ServerMessage::LoginResponse {
                                        success: false,
                                        message: "Komputer sedang dikunci oleh Guru/Admin! Tidak bisa masuk.".to_string(),
                                        username: None,
                                        full_name: None,
                                    });
                                    continue;
                                }

                                let display_name = if !username.trim().is_empty() {
                                    username.trim().to_string()
                                } else {
                                    format!("Siswa ({})", client_id)
                                };

                                let student_user = User {
                                    username: client_id.clone(),
                                    password: String::new(),
                                    full_name: display_name.clone(),
                                    role: "student".to_string(),
                                };

                                println!("[AUTH] Login SUKSES (Tanpa Autentikasi): '{}' di PC '{}'", display_name, client_id);
                                let mut clients = state.clients.write().await;
                                if let Some(client) = clients.get_mut(&client_id) {
                                    client.status = ClientStatus::LoggedIn;
                                    client.current_user = Some(student_user);

                                    let update_json = serde_json::json!({
                                        "type": "ClientUpdate",
                                        "client": client.clone()
                                    });
                                    let _ = state.dashboard_tx.send(update_json.to_string());
                                }

                                let _ = client_tx.send(ServerMessage::LoginResponse {
                                    success: true,
                                    message: format!("Selamat datang, {}!", display_name),
                                    username: Some(client_id.clone()),
                                    full_name: Some(display_name),
                                });
                                continue;
                            }
                            shared::AuthModeType::GeneralPassword => {
                                if is_admin_locked {
                                    println!("[AUTH] Login DITOLAK (Admin Locked): PC '{}' coba masuk saat terkunci admin", client_id);
                                    let _ = client_tx.send(ServerMessage::LoginResponse {
                                        success: false,
                                        message: "Komputer sedang dikunci oleh Guru/Admin! Tidak bisa login.".to_string(),
                                        username: None,
                                        full_name: None,
                                    });
                                    continue;
                                }

                                let expected_gen_pass = active_auth_cfg.general_password.as_deref().unwrap_or("").trim();
                                if !expected_gen_pass.is_empty() && password.trim() == expected_gen_pass {
                                    let display_name = if !username.trim().is_empty() {
                                        username.trim().to_string()
                                    } else {
                                        format!("Siswa ({})", client_id)
                                    };

                                    let student_user = User {
                                        username: client_id.clone(),
                                        password: String::new(),
                                        full_name: display_name.clone(),
                                        role: "student".to_string(),
                                    };

                                    println!("[AUTH] Login SUKSES (General Password): '{}' di PC '{}'", display_name, client_id);
                                    let mut clients = state.clients.write().await;
                                    if let Some(client) = clients.get_mut(&client_id) {
                                        client.status = ClientStatus::LoggedIn;
                                        client.current_user = Some(student_user);

                                        let update_json = serde_json::json!({
                                            "type": "ClientUpdate",
                                            "client": client.clone()
                                        });
                                        let _ = state.dashboard_tx.send(update_json.to_string());
                                    }

                                    let _ = client_tx.send(ServerMessage::LoginResponse {
                                        success: true,
                                        message: format!("Password benar! Selamat datang, {}!", display_name),
                                        username: Some(client_id.clone()),
                                        full_name: Some(display_name),
                                    });
                                    continue;
                                } else {
                                    println!("[AUTH] Login GAGAL (General Password salah) di PC '{}'", client_id);
                                    let _ = client_tx.send(ServerMessage::LoginResponse {
                                        success: false,
                                        message: "Password Kelas salah! Silakan tanyakan ke Guru.".to_string(),
                                        username: None,
                                        full_name: None,
                                    });
                                    continue;
                                }
                            }
                            shared::AuthModeType::Identity => {
                                // Login Gagal untuk mode identitas siswa
                                println!("[AUTH] Login GAGAL: user '{}' di PC '{}'", username, client_id);
                                let _ = client_tx.send(ServerMessage::LoginResponse {
                                    success: false,
                                    message: "Username atau Password salah!".to_string(),
                                    username: None,
                                    full_name: None,
                                });
                            }
                        }
                    }

                    ClientMessage::ScreenFrame {
                        client_id,
                        image_base64,
                        ..
                    } => {
                        let mut clients = state.clients.write().await;
                        if let Some(client) = clients.get_mut(&client_id) {
                            client.latest_screen = Some(image_base64.clone());
                            client.last_seen = Utc::now();

                            // Kirim event ScreenUpdate yang ringan (hanya frame, tanpa reflow metadata client)
                            let update_json = serde_json::json!({
                                "type": "ScreenUpdate",
                                "client_id": client_id,
                                "latest_screen": image_base64
                            });
                            let _ = state.dashboard_tx.send(update_json.to_string());
                        }
                    }

                    ClientMessage::Activity {
                        client_id,
                        app_name,
                        window_title,
                        category,
                        severity,
                        details,
                        keystrokes,
                        mouse_clicks,
                        mouse_distance_px,
                        idle_seconds,
                        is_afk,
                        timestamp,
                        username,
                    } => {
                        let (user_name, client_clone) = {
                            let mut clients = state.clients.write().await;
                            if let Some(client) = clients.get_mut(&client_id) {
                                client.latest_app = Some(app_name.clone());
                                client.latest_window = Some(window_title.clone());
                                client.last_seen = Utc::now();
                                if let Some(stats) = client.input_stats.as_mut() {
                                    if let Some(k) = keystrokes { stats.total_keystrokes = stats.total_keystrokes.saturating_add(k); }
                                    if let Some(c) = mouse_clicks { stats.total_clicks = stats.total_clicks.saturating_add(c); }
                                    if let Some(d) = mouse_distance_px { stats.mouse_distance_px = stats.mouse_distance_px.saturating_add(d); }
                                    if let Some(sec) = idle_seconds { stats.idle_seconds = sec; }
                                    if let Some(afk) = is_afk { stats.is_afk = afk; }
                                } else if keystrokes.is_some() || mouse_clicks.is_some() || idle_seconds.is_some() {
                                    client.input_stats = Some(shared::InputStatsSnapshot {
                                        total_keystrokes: keystrokes.unwrap_or(0),
                                        total_clicks: mouse_clicks.unwrap_or(0),
                                        mouse_distance_px: mouse_distance_px.unwrap_or(0),
                                        idle_seconds: idle_seconds.unwrap_or(0),
                                        is_afk: is_afk.unwrap_or(false),
                                        keystrokes_per_min: 0,
                                    });
                                }
                                (
                                    client.current_user.as_ref().map(|u| u.full_name.clone()),
                                    Some(client.clone()),
                                )
                            } else {
                                (None, None)
                            }
                        };

                        let effective_category = category.unwrap_or_else(|| {
                            let lower_app = app_name.to_lowercase();
                            let lower_title = window_title.to_lowercase();
                            if lower_app.contains("setup")
                                || lower_app.contains("install")
                                || lower_app.contains("msiexec")
                                || lower_title.contains("setup")
                                || lower_title.contains("wizard")
                                || lower_title.contains("instalasi")
                            {
                                "INSTALLER".to_string()
                            } else {
                                "APP".to_string()
                            }
                        });

                        let effective_severity = severity.unwrap_or_else(|| {
                            if effective_category == "BLOCKED" {
                                "danger".to_string()
                            } else if effective_category == "INSTALLER" {
                                "warning".to_string()
                            } else {
                                "info".to_string()
                            }
                        });

                        let effective_details = details.or_else(|| {
                            if effective_category == "INSTALLER" {
                                Some(format!("Terdeteksi aktivitas proses instalasi: {}", app_name))
                            } else if effective_category == "BLOCKED" {
                                Some(format!("Aplikasi terlarang '{}' diblokir oleh kebijakan Lab", app_name))
                            } else {
                                None
                            }
                        });

                        let final_timestamp = timestamp.unwrap_or_else(Utc::now);
                        let final_username = username.or(user_name);

                        let entry = ActivityEntry {
                            client_id: client_id.clone(),
                            username: final_username,
                            app_name,
                            window_title,
                            timestamp: final_timestamp,
                            category: Some(effective_category),
                            severity: Some(effective_severity),
                            details: effective_details,
                            keystrokes,
                            mouse_clicks,
                            mouse_distance_px,
                            idle_seconds,
                            is_afk,
                        };

                        state.activities.write().await.push(entry.clone());
                        append_activity_to_file(&entry);

                        // Broadcast activity to dashboard
                        let act_json = serde_json::json!({
                            "type": "NewActivity",
                            "activity": entry
                        });
                        let _ = state.dashboard_tx.send(act_json.to_string());

                        if let Some(c) = client_clone {
                            let update_json = serde_json::json!({
                                "type": "ClientUpdate",
                                "client": c
                            });
                            let _ = state.dashboard_tx.send(update_json.to_string());
                        }
                    }

                    ClientMessage::ActivityBatch { client_id, entries } => {
                        let (user_name, client_clone) = {
                            let mut clients = state.clients.write().await;
                            if let Some(client) = clients.get_mut(&client_id) {
                                client.last_seen = Utc::now();
                                if let Some(last) = entries.last() {
                                    client.latest_app = Some(last.app_name.clone());
                                    client.latest_window = Some(last.window_title.clone());
                                }
                                (
                                    client.current_user.as_ref().map(|u| u.full_name.clone()),
                                    Some(client.clone()),
                                )
                            } else {
                                (None, None)
                            }
                        };

                        let mut acts = state.activities.write().await;
                        for mut entry in entries {
                            if entry.username.is_none() {
                                entry.username = user_name.clone();
                            }
                            let enriched = entry.enriched();
                            acts.push(enriched.clone());
                            append_activity_to_file(&enriched);

                            let act_json = serde_json::json!({
                                "type": "NewActivity",
                                "activity": enriched
                            });
                            let _ = state.dashboard_tx.send(act_json.to_string());
                        }

                        if let Some(c) = client_clone {
                            let update_json = serde_json::json!({
                                "type": "ClientUpdate",
                                "client": c
                            });
                            let _ = state.dashboard_tx.send(update_json.to_string());
                        }
                    }

                    ClientMessage::Heartbeat { client_id, input_stats } => {
                        let mut clients = state.clients.write().await;
                        if let Some(client) = clients.get_mut(&client_id) {
                            client.last_seen = Utc::now();
                            if input_stats.is_some() {
                                client.input_stats = input_stats;
                                let update_json = serde_json::json!({
                                    "type": "ClientUpdate",
                                    "client": client.clone()
                                });
                                let _ = state.dashboard_tx.send(update_json.to_string());
                            }
                        }
                    }

                    ClientMessage::Logout { client_id } => {
                        let mut clients = state.clients.write().await;
                        if let Some(client) = clients.get_mut(&client_id) {
                            client.status = ClientStatus::Locked;
                            client.current_user = None;
                            let update_json = serde_json::json!({
                                "type": "ClientUpdate",
                                "client": client.clone()
                            });
                            let _ = state.dashboard_tx.send(update_json.to_string());
                        }
                    }

                    ClientMessage::CheckUpdate { client_id: _, current_version } => {
                        let cfg = state.update_config.read().await;
                        let update_available = current_version.trim() != cfg.latest_version.trim();
                        let info = shared::UpdateInfo {
                            update_available,
                            current_version,
                            latest_version: cfg.latest_version.clone(),
                            release_notes: cfg.release_notes.clone(),
                            download_url: "/api/update/client/download".to_string(),
                            sha256: None,
                            mandatory: cfg.mandatory,
                        };
                        let _ = client_tx.send(ServerMessage::UpdateNotification { info });
                    }

                    ClientMessage::ReportIssue { client_id, issue } => {
                        println!("[KENDALA] Laporan kendala dari PC '{}': {}", client_id, issue);
                        let (user_name, client_clone) = {
                            let mut clients = state.clients.write().await;
                            if let Some(client) = clients.get_mut(&client_id) {
                                client.current_issue = Some(issue.clone());
                                client.issue_time = Some(Utc::now());
                                client.last_seen = Utc::now();
                                (
                                    client.current_user.as_ref().map(|u| u.full_name.clone()),
                                    Some(client.clone()),
                                )
                            } else {
                                (None, None)
                            }
                        };

                        let entry = ActivityEntry {
                            client_id: client_id.clone(),
                            username: user_name,
                            app_name: "🚨 LAPOR KENDALA".to_string(),
                            window_title: format!("KENDALA: {}", issue),
                            timestamp: Utc::now(),
                            category: Some("KENDALA".to_string()),
                            severity: Some("danger".to_string()),
                            details: Some(format!("Siswa melaporkan kendala: {}", issue)),
                            keystrokes: None,
                            mouse_clicks: None,
                            mouse_distance_px: None,
                            idle_seconds: None,
                            is_afk: None,
                        };

                        state.activities.write().await.push(entry.clone());
                        append_activity_to_file(&entry);

                        // Broadcast activity to dashboard
                        let act_json = serde_json::json!({
                            "type": "NewActivity",
                            "activity": entry
                        });
                        let _ = state.dashboard_tx.send(act_json.to_string());

                        if let Some(c) = client_clone {
                            let update_json = serde_json::json!({
                                "type": "ClientUpdate",
                                "client": c
                            });
                            let _ = state.dashboard_tx.send(update_json.to_string());
                        }
                    }
                }
            }
        }
        Message::Close(close_frame) => {
                        let reason = close_frame
                            .map(|f| format!("Kode: {}, Alasan: {}", f.code, f.reason))
                            .unwrap_or_else(|| "Tanpa detail frame close".to_string());
                        println!("[SERVER] PC Klien {:?} menutup koneksi secara normal: {}", current_client_id, reason);
                        disconnect_detail = Some(format!("Client Normal Close: {}", reason));
                        break;
                    }
                    Message::Ping(_) | Message::Pong(_) => {}
                    _ => {}
                }
            }
            Err(e) => {
                println!("[SERVER] Error stream WebSocket dari client {:?}: {:?}", current_client_id, e);
                disconnect_detail = Some(format!("WebSocket Stream Error: {:?}", e));
                break;
            }
        }
    }

    if let Some(id) = current_client_id {
        let reason_text = disconnect_detail.unwrap_or_else(|| "Klien menutup stream atau timeout socket".to_string());
        println!("[SERVER] PC Klien Terputus: {} ({})", id, reason_text);
        error_logger::log_server_error(
            "ClientDisconnected",
            &format!("PC Klien Terputus: {}", id),
            Some(&reason_text),
        );
        state.client_senders.write().await.remove(&id);
        let mut clients = state.clients.write().await;
        if let Some(client) = clients.get_mut(&id) {
            let idle_secs = (Utc::now() - client.last_seen).num_seconds();
            if idle_secs >= 10 || client.status != ClientStatus::LoggedIn {
                client.status = ClientStatus::Disconnected;
                let update_json = serde_json::json!({
                    "type": "ClientUpdate",
                    "client": client.clone()
                });
                let _ = state.dashboard_tx.send(update_json.to_string());
                save_clients_to_file(&clients);
            }
        }
    }
}

// WebSocket Dashboard Browser (Guru)
async fn ws_dashboard_handler(
    ws: WebSocketUpgrade,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<auth::AuthQuery>,
    State(state): State<AppState>,
) -> axum::response::Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            "Akses ditolak: Silakan login sebagai Guru/Admin terlebih dahulu.",
        )
            .into_response();
    }
    ws.on_upgrade(|socket| handle_dashboard_socket(socket, state)).into_response()
}

async fn handle_dashboard_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.dashboard_tx.subscribe();

    // Kirim initial state saat dashboard pertama kali dibuka
    let initial_clients = {
        let clients = state.clients.read().await;
        clients.clone()
    };
    let initial_msg = serde_json::json!({
        "type": "FullState",
        "clients": initial_clients
    });
    if socket
        .send(Message::Text(initial_msg.to_string()))
        .await
        .is_err()
    {
        return;
    }

    // Loop stream update ke dashboard browser
    // Tangani RecvError::Lagged dengan 'continue' agar jika ada frame yang terlewat,
    // koneksi WebSocket TIDAK PUTUS dan stream layar tetap berjalan lancar tanpa macet (zero freeze).
    loop {
        match rx.recv().await {
            Ok(msg_text) => {
                if socket.send(Message::Text(msg_text)).await.is_err() {
                    break;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::debug!("Dashboard WebSocket lagged by {} frames, continuing...", n);
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_timeout_sweeper_logic() {
        let now = Utc::now();
        let dead_time = now - chrono::Duration::seconds(20);
        let active_time = now - chrono::Duration::seconds(2);

        let client_dead = ClientSession {
            client_id: "PC-DEAD".to_string(),
            hostname: "PC-DEAD".to_string(),
            ip: "192.168.20.101".to_string(),
            status: ClientStatus::LoggedIn,
            current_user: None,
            last_seen: dead_time,
            latest_window: None,
            latest_app: None,
            latest_screen: None,
            is_admin_locked: false,
            current_issue: None,
            issue_time: None,
            input_stats: None,
        };

        let mut client_active = client_dead.clone();
        client_active.client_id = "PC-ACTIVE".to_string();
        client_active.hostname = "PC-ACTIVE".to_string();
        client_active.last_seen = active_time;

        let mut clients = HashMap::new();
        clients.insert(client_dead.client_id.clone(), client_dead);
        clients.insert(client_active.client_id.clone(), client_active);

        // Sweeper evaluation logic test
        for (_, client) in clients.iter_mut() {
            if client.status != ClientStatus::Disconnected {
                let idle_secs = (now - client.last_seen).num_seconds();
                if idle_secs >= 15 {
                    client.status = ClientStatus::Disconnected;
                }
            }
        }

        assert_eq!(clients["PC-DEAD"].status, ClientStatus::Disconnected);
        assert_eq!(clients["PC-ACTIVE"].status, ClientStatus::LoggedIn);
    }

    #[test]
    fn test_client_persistence_roundtrip() {
        let mut clients = HashMap::new();
        let session = ClientSession {
            client_id: "PC-TEST-01".to_string(),
            hostname: "PC-TEST-01".to_string(),
            ip: "192.168.20.101".to_string(),
            status: ClientStatus::LoggedIn,
            current_user: None,
            last_seen: Utc::now(),
            latest_window: None,
            latest_app: None,
            latest_screen: None,
            is_admin_locked: false,
            current_issue: None,
            issue_time: None,
            input_stats: None,
        };
        clients.insert(session.client_id.clone(), session);

        let json = serde_json::to_string_pretty(&clients).unwrap();
        let mut loaded: HashMap<String, ClientSession> = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.len(), 1);
        let loaded_client = loaded.get_mut("PC-TEST-01").unwrap();
        // Saat server baru menyala, status klien ter-pin ditandai Disconnected
        loaded_client.status = ClientStatus::Disconnected;
        assert_eq!(loaded_client.status, ClientStatus::Disconnected);
        assert_eq!(loaded_client.hostname, "PC-TEST-01");
    }

    #[test]
    fn test_filter_activities_date_range() {
        let make_act = |date_str: &str, app: &str, user: &str| -> ActivityEntry {
            let dt = chrono::DateTime::parse_from_rfc3339(&format!("{}T08:00:00+07:00", date_str))
                .unwrap()
                .with_timezone(&Utc);
            ActivityEntry {
                client_id: "PC-01".to_string(),
                username: Some(user.to_string()),
                app_name: app.to_string(),
                window_title: format!("Window {}", app),
                timestamp: dt,
                category: Some("APP".to_string()),
                severity: Some("info".to_string()),
                details: None,
                keystrokes: None,
                mouse_clicks: None,
                mouse_distance_px: None,
                idle_seconds: None,
                is_afk: None,
            }
        };

        let acts = vec![
            make_act("2026-10-25", "word.exe", "Siswa A"),
            make_act("2026-10-27", "excel.exe", "Siswa B"),
            make_act("2026-10-28", "chrome.exe", "Siswa C"),
            make_act("2026-11-01", "code.exe", "Siswa D"),
        ];

        // Rentang 2026-10-27 s.d. 2026-10-28
        let q_range = ActivitiesQuery {
            dari_tanggal: Some("2026-10-27".to_string()),
            sampai_tanggal: Some("2026-10-28".to_string()),
            ..Default::default()
        };
        let res_range = filter_and_sort_activities(&acts, &q_range);
        assert_eq!(res_range.len(), 2);
        // Default range adalah terbaru dulu (rev)
        assert_eq!(res_range[0].app_name, "chrome.exe");
        assert_eq!(res_range[1].app_name, "excel.exe");

        // Filter single date via query.tanggal
        let q_single = ActivitiesQuery {
            tanggal: Some("2026-10-27".to_string()),
            ..Default::default()
        };
        let res_single = filter_and_sort_activities(&acts, &q_single);
        assert_eq!(res_single.len(), 1);
        assert_eq!(res_single[0].app_name, "excel.exe");

        // Filter semua via tanggal="all"
        let q_all = ActivitiesQuery {
            tanggal: Some("all".to_string()),
            ..Default::default()
        };
        let res_all = filter_and_sort_activities(&acts, &q_all);
        assert_eq!(res_all.len(), 4);
    }

    #[test]
    fn test_filter_activities_sort_and_client_filter() {
        let make_act = |client: &str, user: &str, app: &str, hour: u32| -> ActivityEntry {
            let dt = chrono::DateTime::parse_from_rfc3339(&format!("2026-10-28T{:02}:00:00+07:00", hour))
                .unwrap()
                .with_timezone(&Utc);
            ActivityEntry {
                client_id: client.to_string(),
                username: Some(user.to_string()),
                app_name: app.to_string(),
                window_title: format!("Window {}", app),
                timestamp: dt,
                category: Some("APP".to_string()),
                severity: Some("info".to_string()),
                details: None,
                keystrokes: None,
                mouse_clicks: None,
                mouse_distance_px: None,
                idle_seconds: None,
                is_afk: None,
            }
        };

        let acts = vec![
            make_act("PC-03", "Budi", "notepad.exe", 9),
            make_act("PC-01", "Citra", "calc.exe", 10),
            make_act("PC-02", "Andi", "word.exe", 8),
        ];

        // Sort by username ASC
        let q_user_asc = ActivitiesQuery {
            tanggal: Some("2026-10-28".to_string()),
            sort_by: Some("username".to_string()),
            order: Some("asc".to_string()),
            ..Default::default()
        };
        let res_user = filter_and_sort_activities(&acts, &q_user_asc);
        assert_eq!(res_user.len(), 3);
        assert_eq!(res_user[0].username.as_deref(), Some("Andi"));
        assert_eq!(res_user[1].username.as_deref(), Some("Budi"));
        assert_eq!(res_user[2].username.as_deref(), Some("Citra"));

        // Sort by timestamp ASC
        let q_time_asc = ActivitiesQuery {
            tanggal: Some("2026-10-28".to_string()),
            sort_by: Some("timestamp".to_string()),
            order: Some("asc".to_string()),
            ..Default::default()
        };
        let res_time = filter_and_sort_activities(&acts, &q_time_asc);
        assert_eq!(res_time[0].app_name, "word.exe"); // jam 8
        assert_eq!(res_time[1].app_name, "notepad.exe"); // jam 9
        assert_eq!(res_time[2].app_name, "calc.exe"); // jam 10

        // Client filter PC-01
        let q_client = ActivitiesQuery {
            tanggal: Some("2026-10-28".to_string()),
            client_id: Some("PC-01".to_string()),
            ..Default::default()
        };
        let res_client = filter_and_sort_activities(&acts, &q_client);
        assert_eq!(res_client.len(), 1);
        assert_eq!(res_client[0].client_id, "PC-01");
    }
}
