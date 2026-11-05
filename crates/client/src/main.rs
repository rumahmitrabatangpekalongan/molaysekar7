mod connection;
mod guard;
mod remote_input;
mod screen;
mod tracker;
mod ui;
mod updater;
mod error_logger;
mod input_tracker;
mod auth_cache;
mod local_db;
mod redirector;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use screen::ScreenCapturer;
use shared::{ClientMessage, ServerMessage};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::{sync::mpsc, sync::Arc, time::Duration};
use tokio::sync::{Notify, RwLock};
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use ui::{UiCommand, UiEvent};

/// Menyimpan entri aktivitas ke database lokal klien dan mengirimkannya secara live jika koneksi sedang aktif
async fn record_and_dispatch_activity(
    entry: shared::ActivityEntry,
    active_ws_tx: &Arc<RwLock<Option<tokio::sync::mpsc::UnboundedSender<ClientMessage>>>>,
) {
    let rec_id = local_db::save_activity(entry.clone());
    let guard = active_ws_tx.read().await;
    if let Some(ref tx) = *guard {
        let msg = ClientMessage::Activity {
            client_id: entry.client_id,
            app_name: entry.app_name,
            window_title: entry.window_title,
            category: entry.category,
            severity: entry.severity,
            details: entry.details,
            keystrokes: entry.keystrokes,
            mouse_clicks: entry.mouse_clicks,
            mouse_distance_px: entry.mouse_distance_px,
            idle_seconds: entry.idle_seconds,
            is_afk: entry.is_afk,
            timestamp: Some(entry.timestamp),
            username: entry.username,
        };
        if tx.send(msg).is_ok() {
            local_db::mark_as_synced(&[rec_id]);
        }
    } else {
        println!(
            "[LOCAL-DB] Offline: Log aktivitas disimpan ke database lokal ({} - {})",
            entry.app_name, entry.window_title
        );
    }
}

/// Menyinkronkan seluruh log aktivitas yang tertampung di database lokal saat offline ke server
async fn sync_offline_activities(
    ws_tx: &tokio::sync::mpsc::UnboundedSender<ClientMessage>,
    client_id: &str,
) {
    loop {
        let unsynced = local_db::get_unsynced_activities(local_db::MAX_SYNC_BATCH_SIZE);
        if unsynced.is_empty() {
            break;
        }
        let count = unsynced.len();
        println!(
            "[SYNC] Ditemukan {} log aktivitas di database lokal klien. Mengirim ke Server...",
            count
        );
        let mut ids = Vec::with_capacity(count);
        let mut batch = Vec::with_capacity(count);
        for rec in unsynced {
            ids.push(rec.id);
            batch.push(rec.activity);
        }
        let msg = ClientMessage::ActivityBatch {
            client_id: client_id.to_string(),
            entries: batch,
        };
        if ws_tx.send(msg).is_err() {
            println!("[SYNC] Pengiriman log offline gagal (koneksi terputus)");
            break;
        }
        let marked = local_db::mark_as_synced(&ids);
        println!(
            "[SYNC] ✅ Berhasil menyinkronkan {} log aktivitas dari database lokal!",
            marked
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn read_server_ip_config(custom_arg: Option<&str>) -> String {
    if let Some(target) = custom_arg {
        target.trim().to_string()
    } else if let Some(content) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("server_ip.txt"))).and_then(|p| std::fs::read_to_string(p).ok()) {
        content
    } else if let Ok(content) = std::fs::read_to_string("server_ip.txt") {
        content
    } else {
        "127.0.0.1:1916\n192.168.20.70:1916\n192.168.20.100:1916\nmonitoringmolay.sekolahkartini7.web.id\nmolay.sekolahkartini7.web.id".to_string()
    }
}

#[tokio::main]
async fn main() {
    // Bersihkan file sisa pembaruan lama (.old atau .new)
    updater::cleanup_old_binary();

    // Mulai background input tracker (WhatPulse metrics)
    input_tracker::start_input_tracker();

    let args: Vec<String> = std::env::args().collect();
    let is_debug = args.iter().any(|a| a == "--debug" || a == "-d" || a == "--teknisi" || a == "--console")
        || std::env::var("MOLAY_DEBUG").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);

    // Konfigurasi dan proteksi jendela konsol CMD (sembunyikan di mode normal, nonaktifkan tombol [X] di mode teknisi)
    ui::configure_console_window(is_debug);

    let custom_target_arg = args.iter().skip(1).find(|a| {
        let t = a.trim();
        !t.is_empty() && !t.starts_with('-') && !t.starts_with('#') && !t.starts_with("//")
    });

    let raw_config = read_server_ip_config(custom_target_arg.map(|s| s.as_str()));
    let mut last_loaded_config = raw_config.clone();

    let mut endpoints = connection::parse_endpoints(&raw_config);
    for ep in &endpoints {
        error_logger::register_fallback_endpoint(ep.http_base_url());
    }

    // Hostname
    let hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "PC-LAB-KLIEN".to_string());
    let client_id = hostname.clone();
    error_logger::init_panic_hook(client_id.clone());
    let api_key = updater::get_api_key();
    let http_client = updater::create_http_client(&api_key);

    println!("========================================================");
    println!("   MOLAY SEKAR7 - CLIENT AGENT (WINDOWS 10)");
    println!("   Versi:      v{}", updater::CURRENT_VERSION);
    if is_debug {
        println!("   Mode:       [MODE TEKNISI / LIVE DEBUG LOG]");
        println!("   Status CMD: Command Prompt tetap aktif di depan layar");
    }
    println!("   Client ID:  {}", client_id);
    println!("   Target Server ({} endpoint terdeteksi):", endpoints.len());
    for (_i, ep) in endpoints.iter().enumerate() {
        println!("     [{}] {} -> {}", ep.mode.label(), ep.raw, ep.ws_url);
    }
    println!("   API Key:    {}... (Terpasang)", &api_key[..12.min(api_key.len())]);
    println!("========================================================");

    // UI Communication Channels
    let (ui_event_tx, mut ui_event_rx) = tokio::sync::mpsc::unbounded_channel::<UiEvent>();
    let (ui_cmd_tx, ui_cmd_rx) = mpsc::channel::<UiCommand>();

    // Start Lock Screen GUI in dedicated thread
    ui::start_ui_thread(ui_event_tx, ui_cmd_rx, is_debug);

    // Active WebSocket sender channel shared with UI handler
    let active_ws_tx = Arc::new(RwLock::new(None::<tokio::sync::mpsc::UnboundedSender<ClientMessage>>));

    // Active Application Policy (disinkronkan dari server)
    let active_policy = Arc::new(RwLock::new(shared::AppPolicyConfig::default()));

    // Catatan user yang sedang login aktif (baik online maupun offline mandiri): (username, password)
    let current_logged_in_user = Arc::new(RwLock::new(None::<(String, String)>));

    // Terapkan mode awal dari file konfigurasi lokal
    let initial_auth_cfg = auth_cache::load_local_auth_config();
    let _ = ui_cmd_tx.send(UiCommand::SetAuthMode(initial_auth_cfg.clone()));
    if initial_auth_cfg.mode == shared::AuthModeType::NoAuth && initial_auth_cfg.auto_unlock_on_no_auth {
        println!("[STARTUP] Mode Tanpa Autentikasi aktif pada startup - otomatis membuka desktop langsung tanpa form login");
        let _ = ui_cmd_tx.send(UiCommand::HideLock);
        let mut logged = current_logged_in_user.write().await;
        *logged = Some(("NoAuth".to_string(), String::new()));
    }

    // Background Task: Sinkronisasi otomatis daftar akun siswa dari Cloud VPS (molay.sekolahkartini7.web.id)
    // Berjalan non-blocking di latar belakang sehingga login siswa tetap instan tanpa delay
    let auth_cfg_for_cloud = initial_auth_cfg.clone();
    tokio::spawn(async move {
        if !auth_cfg_for_cloud.lan_only {
            let _ = auth_cache::try_sync_users_from_cloud(auth_cfg_for_cloud.cloud_users_url.as_deref()).await;
        }
    });

    // Background Task: Listen to UI events (e.g. Login button clicked)
    let ws_tx_for_ui = active_ws_tx.clone();
    let cid_for_ui = client_id.clone();
    let cmd_tx_for_ui = ui_cmd_tx.clone();
    let logged_in_for_ui = current_logged_in_user.clone();
    tokio::spawn(async move {
        while let Some(event) = ui_event_rx.recv().await {
            match event {
                UiEvent::LoginSubmit { username, password } => {
                    let guard = ws_tx_for_ui.read().await;
                    if let Some(ref tx) = *guard {
                        let msg = ClientMessage::Login {
                            client_id: cid_for_ui.clone(),
                            username: username.clone(),
                            password: password.clone(),
                        };
                        let _ = tx.send(msg);
                    } else {
                        // Jalur Server belum tersambung: Gunakan Smart Offline Cache & Auto-Unlock
                        let cfg = auth_cache::load_local_auth_config();
                        match auth_cache::verify_offline_credentials(&username, &password, &cfg) {
                            Ok(display_name) => {
                                println!("[OFFLINE-AUTH] ✅ Login mandiri berhasil untuk '{}' ({})", username, display_name);
                                let _ = cmd_tx_for_ui.send(UiCommand::HideLock);
                                let _ = cmd_tx_for_ui.send(UiCommand::SetStatus(format!(
                                    "Mode Mandiri Aktif ({}). Selamat Belajar!",
                                    display_name
                                )));
                                let mut logged = logged_in_for_ui.write().await;
                                *logged = Some((username.clone(), password.clone()));
                            }
                            Err(err_msg) => {
                                println!("[OFFLINE-AUTH] ❌ Login mandiri ditolak: {}", err_msg);
                                let _ = cmd_tx_for_ui.send(UiCommand::SetStatus(err_msg));
                            }
                        }
                    }
                }
                UiEvent::ReportIssue { issue } => {
                    let guard = ws_tx_for_ui.read().await;
                    if let Some(ref tx) = *guard {
                        let msg = ClientMessage::ReportIssue {
                            client_id: cid_for_ui.clone(),
                            issue: issue.clone(),
                        };
                        let _ = tx.send(msg);
                        let _ = cmd_tx_for_ui.send(UiCommand::SetStatus(format!("Status: Kendala terkirim ('{}')", issue)));
                    } else {
                        // Catat laporan kendala ke database lokal klien saat offline
                        let current_user = logged_in_for_ui.read().await.as_ref().map(|(u, _)| u.clone());
                        let act_issue = shared::ActivityEntry {
                            client_id: cid_for_ui.clone(),
                            username: current_user,
                            app_name: "MolayClient-Sekar7.exe".to_string(),
                            window_title: format!("Laporan Kendala: {}", issue),
                            timestamp: chrono::Utc::now(),
                            category: Some("KENDALA".to_string()),
                            severity: Some("danger".to_string()),
                            details: Some(format!("Kendala dilaporkan siswa saat offline: {}", issue)),
                            keystrokes: None,
                            mouse_clicks: None,
                            mouse_distance_px: None,
                            idle_seconds: None,
                            is_afk: None,
                        };
                        local_db::save_activity(act_issue);
                        let _ = cmd_tx_for_ui.send(UiCommand::SetStatus("Kendala dicatat di database lokal (Disinkronkan saat server online)".to_string()));
                    }
                }
            }
        }
    });

    // Background Task Permanen: Activity Tracker & Foreground Logger (Berjalan terus baik online maupun offline)
    let ws_tx_for_act = active_ws_tx.clone();
    let cid_act = client_id.clone();
    let policy_act = active_policy.clone();
    let logged_in_for_act = current_logged_in_user.clone();
    tokio::spawn(async move {
        let mut last_title = String::new();
        let mut last_sample_time = std::time::Instant::now();
        loop {
            sleep(Duration::from_millis(1500)).await;
            if let Some(info) = tracker::get_active_window() {
                let pol = policy_act.read().await.clone();
                let current_user = logged_in_for_act.read().await.as_ref().map(|(u, _)| u.clone());

                // 0. Cek apakah ada pengalihan browser / link web game (Browser Redirection)
                if let Some(redirect_reason) = redirector::check_and_execute_redirect(
                    &info.app_name,
                    &info.window_title,
                    info.hwnd,
                    &pol,
                ) {
                    let entry = shared::ActivityEntry {
                        client_id: cid_act.clone(),
                        username: current_user.clone(),
                        app_name: info.app_name,
                        window_title: info.window_title,
                        timestamp: chrono::Utc::now(),
                        category: Some("BLOCKED".to_string()),
                        severity: Some("warning".to_string()),
                        details: Some(redirect_reason),
                        keystrokes: None,
                        mouse_clicks: None,
                        mouse_distance_px: None,
                        idle_seconds: None,
                        is_afk: None,
                    };
                    record_and_dispatch_activity(entry, &ws_tx_for_act).await;
                    last_title.clear();
                    last_sample_time = std::time::Instant::now();
                    continue;
                }

                // 1. Cek apakah foreground window dilarang oleh kebijakan
                if let Some(reason) = guard::check_blocked_app(&info.app_name, &info.window_title, &pol) {
                    let killed = guard::kill_process_by_pid(info.pid);
                    if killed {
                        println!("[GUARD] Jendela aktif diblokir: {} (PID: {}) - {}", info.app_name, info.pid, reason);
                        guard::show_blocked_alert_async(info.app_name.clone(), reason.clone());
                    }
                    let is_inst = guard::is_installer(&info.app_name, &info.window_title);
                    let (k, c, d) = input_tracker::drain_window_deltas();
                    let stats = input_tracker::get_current_stats();
                    let entry = shared::ActivityEntry {
                        client_id: cid_act.clone(),
                        username: current_user.clone(),
                        app_name: info.app_name,
                        window_title: info.window_title,
                        timestamp: chrono::Utc::now(),
                        category: Some(if is_inst { "INSTALLER".to_string() } else { "BLOCKED".to_string() }),
                        severity: Some("danger".to_string()),
                        details: Some(format!("Aplikasi ditutup paksa: {}", reason)),
                        keystrokes: Some(k),
                        mouse_clicks: Some(c),
                        mouse_distance_px: Some(d),
                        idle_seconds: Some(stats.idle_seconds),
                        is_afk: Some(stats.is_afk),
                    };
                    record_and_dispatch_activity(entry, &ws_tx_for_act).await;
                    last_title.clear();
                    last_sample_time = std::time::Instant::now();
                    continue;
                }

                // 2. Cek apakah ini installer (namun kebijakan tidak memblokir/hanya memantau)
                if guard::is_installer(&info.app_name, &info.window_title) {
                    let is_new = info.window_title != last_title && !info.window_title.is_empty();
                    let is_tick = last_sample_time.elapsed() >= Duration::from_secs(30);
                    if is_new || is_tick {
                        last_title = info.window_title.clone();
                        last_sample_time = std::time::Instant::now();
                        let (k, c, d) = input_tracker::drain_window_deltas();
                        let stats = input_tracker::get_current_stats();
                        let entry = shared::ActivityEntry {
                            client_id: cid_act.clone(),
                            username: current_user.clone(),
                            app_name: info.app_name.clone(),
                            window_title: info.window_title,
                            timestamp: chrono::Utc::now(),
                            category: Some("INSTALLER".to_string()),
                            severity: Some("warning".to_string()),
                            details: Some(format!("Terdeteksi siswa menjalankan installer: {}", info.app_name)),
                            keystrokes: Some(k),
                            mouse_clicks: Some(c),
                            mouse_distance_px: Some(d),
                            idle_seconds: Some(stats.idle_seconds),
                            is_afk: Some(stats.is_afk),
                        };
                        record_and_dispatch_activity(entry, &ws_tx_for_act).await;
                    }
                    continue;
                }

                // 3. Jendela aplikasi normal (laporkan saat ganti jendela ATAU sampling periodik 30 detik)
                let is_new = info.window_title != last_title && !info.window_title.is_empty();
                let is_tick = last_sample_time.elapsed() >= Duration::from_secs(30);
                if (is_new || is_tick) && !info.window_title.is_empty() {
                    last_title = info.window_title.clone();
                    last_sample_time = std::time::Instant::now();
                    let (k, c, d) = input_tracker::drain_window_deltas();
                    let stats = input_tracker::get_current_stats();
                    let entry = shared::ActivityEntry {
                        client_id: cid_act.clone(),
                        username: current_user.clone(),
                        app_name: info.app_name,
                        window_title: info.window_title,
                        timestamp: chrono::Utc::now(),
                        category: Some("APP".to_string()),
                        severity: Some("info".to_string()),
                        details: None,
                        keystrokes: Some(k),
                        mouse_clicks: Some(c),
                        mouse_distance_px: Some(d),
                        idle_seconds: Some(stats.idle_seconds),
                        is_afk: Some(stats.is_afk),
                    };
                    record_and_dispatch_activity(entry, &ws_tx_for_act).await;
                }
            }
        }
    });

    // Background Task Permanen: Process Watchdog Guard (Memindai seluruh proses aktif, online maupun offline)
    let ws_tx_for_guard = active_ws_tx.clone();
    let cid_guard = client_id.clone();
    let policy_guard = active_policy.clone();
    let logged_in_for_guard = current_logged_in_user.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(3)).await;
            let pol = policy_guard.read().await.clone();
            let current_user = logged_in_for_guard.read().await.as_ref().map(|(u, _)| u.clone());
            let violations = guard::scan_and_enforce_policy(&pol);
            for v in violations {
                let entry = shared::ActivityEntry {
                    client_id: cid_guard.clone(),
                    username: current_user.clone(),
                    app_name: v.app_name.clone(),
                    window_title: format!("Proses Latar Belakang (PID {})", v.pid),
                    timestamp: chrono::Utc::now(),
                    category: Some(if v.is_installer { "INSTALLER".to_string() } else { "BLOCKED".to_string() }),
                    severity: Some("danger".to_string()),
                    details: Some(format!("Proses '{}' dilarang & otomatis ditutup: {}", v.app_name, v.reason)),
                    keystrokes: None,
                    mouse_clicks: None,
                    mouse_distance_px: None,
                    idle_seconds: None,
                    is_afk: None,
                };
                record_and_dispatch_activity(entry, &ws_tx_for_guard).await;
            }
        }
    });

    // Loop koneksi ke Server (Auto-Reconnect & Smart Failover Dual Mode)
    let mut current_idx = 0;
    let mut last_scan_attempt: Option<std::time::Instant> = None;
    loop {
        // Hot-Reload: Cek secara otomatis jika file server_ip.txt baru saja diedit teknisi/guru
        if custom_target_arg.is_none() {
            let current_cfg_str = read_server_ip_config(None);
            if current_cfg_str != last_loaded_config {
                let reloaded_endpoints = connection::parse_endpoints(&current_cfg_str);
                if !reloaded_endpoints.is_empty() {
                    println!("[CONFIG] 🔄 Terdeteksi pembaruan pada server_ip.txt! Memuat {} target server baru...", reloaded_endpoints.len());
                    for ep in &reloaded_endpoints {
                        error_logger::register_fallback_endpoint(ep.http_base_url());
                    }
                    endpoints = reloaded_endpoints;
                    current_idx = 0;
                    last_loaded_config = current_cfg_str;
                }
            }
        }

        // Coba auto-discovery LAN jika:
        // 1. Belum pernah dicoba (saat startup)
        // 2. Berada di indeks awal (current_idx == 0) dan sudah lewat minimal 30 detik
        // Ini mencegah scan memotong rotasi endpoint cadangan (Cloudflare Tunnel) di tengah jalan
        let should_scan = match last_scan_attempt {
            None => true,
            Some(t) => current_idx == 0 && t.elapsed() > std::time::Duration::from_secs(30),
        };

        if should_scan {
            last_scan_attempt = Some(std::time::Instant::now());
            let _ = ui_cmd_tx.send(UiCommand::SetStatus(
                "🔍 Memindai Server Guru di jaringan LAN...".to_string(),
            ));
            if let Some(discovered_lan) = connection::discover_lan_server_ip(4000).await {
                if let Some(ep_discovered) = connection::ServerEndpoint::from_raw(&discovered_lan) {
                    error_logger::register_fallback_endpoint(ep_discovered.http_base_url());
                    // Posisikan server yang aktif di urutan pertama tanpa menghapus endpoint cadangan lainnya
                    endpoints.retain(|e| e.ws_url != ep_discovered.ws_url);
                    println!("[DISCOVERY] ✅ Server Guru ditemukan: {}! Konek sekarang.", discovered_lan);
                    endpoints.insert(0, ep_discovered);
                    current_idx = 0;
                }
            }
        }

        // Periksa apakah mode lokal saja aktif di konfigurasi lokal
        let local_cfg = auth_cache::load_local_auth_config();
        if local_cfg.lan_only {
            endpoints.retain(|e| e.mode == connection::ConnectionMode::Lan);
        }

        let ep = &endpoints[current_idx % endpoints.len()];
        let ep_label = ep.mode.label();

        println!("[CLIENT] Mencoba terhubung ke [{}] {}...", ep_label, ep.ws_url);
        let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Menghubungkan ke {} ({})...", ep_label, ep.raw)));

        // Batas waktu koneksi agar tidak hang 21 detik jika firewall server memblokir
        let connect_timeout = match ep.mode {
            connection::ConnectionMode::Lan => Duration::from_millis(3500), // 3.5s untuk LAN
            connection::ConnectionMode::Tunnel => Duration::from_millis(8000), // 8s untuk Cloudflare Tunnel
        };

        match tokio::time::timeout(connect_timeout, connect_async(&ep.ws_url)).await {
            Ok(Ok((ws_stream, _))) => {
                let net_desc = if ep.raw.starts_with("127.") || ep.raw.contains("localhost") {
                    "Localhost (127.0.0.1:1916)".to_string()
                } else {
                    format!("{} ({})", ep_label, ep.raw)
                };
                println!("[CLIENT] Berhasil terhubung ke Server via {}!", net_desc);
                let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("✅ Terhubung ke Server Jaringan {}. Silakan Login.", net_desc)));

                let (mut ws_sink, mut ws_source) = ws_stream.split();

                // 1. Registrasi Klien ke Server (perhatikan jika sudah login offline sebelumnya)
                let is_already_logged_in = current_logged_in_user.read().await.is_some();
                let actual_ip = match connection::get_local_lan_ip() {
                    Some(ip) => {
                        if ep.mode == connection::ConnectionMode::Tunnel {
                            format!("{} (Tunnel)", ip)
                        } else {
                            ip.to_string()
                        }
                    }
                    None => format!("Client-{}", ep_label),
                };
                let reg_msg = ClientMessage::Register {
                    client_id: client_id.clone(),
                    hostname: hostname.clone(),
                    ip: actual_ip,
                    is_locked: !is_already_logged_in,
                };
                let reg_json = serde_json::to_string(&reg_msg).unwrap();
                let _ = ws_sink.send(Message::Text(reg_json)).await;

                // Jika siswa sudah login offline sebelumnya, sinkronkan ke server sekarang!
                if let Some((ref u, ref p)) = *current_logged_in_user.read().await {
                    println!("[AUTH-SYNC] Menyinkronkan login mandiri siswa '{}' ke Server...", u);
                    let sync_login = ClientMessage::Login {
                        client_id: client_id.clone(),
                        username: u.clone(),
                        password: p.clone(),
                    };
                    let _ = ws_sink.send(Message::Text(serde_json::to_string(&sync_login).unwrap())).await;
                    let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Terhubung ke {} (Akun: {}).", ep_label, u)));
                }

                // Channel untuk mengirim pesan async ke WebSocket
                let (send_tx, mut send_rx) = tokio::sync::mpsc::unbounded_channel::<ClientMessage>();
                {
                    let mut guard = active_ws_tx.write().await;
                    *guard = Some(send_tx.clone());
                }

                // Sinkronkan seluruh antrean log aktivitas yang tertampung di database lokal saat offline
                let send_tx_for_sync = send_tx.clone();
                let cid_for_sync = client_id.clone();
                tokio::spawn(async move {
                    sync_offline_activities(&send_tx_for_sync, &cid_for_sync).await;
                });

                // Background Task: Sender ke WebSocket (termasuk Ping Heartbeat)
                let send_task = tokio::spawn(async move {
                    let mut ping_interval = tokio::time::interval(Duration::from_secs(15));
                    loop {
                        tokio::select! {
                            Some(msg) = send_rx.recv() => {
                                if let Ok(json) = serde_json::to_string(&msg) {
                                    if ws_sink.send(Message::Text(json)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                            _ = ping_interval.tick() => {
                                // Kirim WebSocket Ping frame untuk keep-alive (mencegah timeout Cloudflare Tunnel & router LAN)
                                if ws_sink.send(Message::Ping(vec![])).await.is_err() {
                                    break;
                                }
                            }
                            else => break,
                        }
                    }
                });

                // Background Task: Screen Capturer & Streamer (Adaptive Refresh & Quality + Turbo Remote)
                let screen_tx = send_tx.clone();
                let cid_screen = client_id.clone();
                let refresh_interval_ms = ep.mode.screen_refresh_interval_ms();
                let jpeg_quality = ep.mode.screen_jpeg_quality();

                let screen_trigger = Arc::new(Notify::new());
                let current_interval_ms = Arc::new(AtomicU64::new(refresh_interval_ms));
                let current_jpeg_quality = Arc::new(AtomicU8::new(jpeg_quality));

                let trigger_clone = screen_trigger.clone();
                let interval_clone = current_interval_ms.clone();
                let quality_clone = current_jpeg_quality.clone();

                let screen_task = tokio::spawn(async move {
                    loop {
                        let interval = interval_clone.load(Ordering::Relaxed);
                        tokio::select! {
                            _ = sleep(Duration::from_millis(interval)) => {},
                            _ = trigger_clone.notified() => {
                                // Jeda 25ms agar sistem operasi Windows & aplikasi aktif selesai merender efek klik/tombol
                                sleep(Duration::from_millis(25)).await;
                            }
                        }
                        let q = quality_clone.load(Ordering::Relaxed);
                        let cid = cid_screen.clone();
                        let frame_res = tokio::task::spawn_blocking(move || {
                            let capturer = ScreenCapturer::new();
                            capturer.capture_jpeg(q).map(|bytes| (bytes, capturer.width, capturer.height))
                        }).await.unwrap_or(None);

                        if let Some((jpeg_bytes, w, h)) = frame_res {
                            let b64 = BASE64.encode(&jpeg_bytes);
                            let msg = ClientMessage::ScreenFrame {
                                client_id: cid,
                                width: w as u32,
                                height: h as u32,
                                image_base64: b64,
                            };
                            if screen_tx.send(msg).is_err() {
                                break;
                            }
                        }
                    }
                });

                // Background Task: Heartbeat & Input Stats Live Broadcaster (WhatPulse metrics live stream)
                let hb_tx = send_tx.clone();
                let cid_hb = client_id.clone();
                let heartbeat_task = tokio::spawn(async move {
                    let mut hb_interval = tokio::time::interval(Duration::from_secs(3));
                    loop {
                        hb_interval.tick().await;
                        let stats = input_tracker::get_current_stats();
                        let msg = ClientMessage::Heartbeat {
                            client_id: cid_hb.clone(),
                            input_stats: Some(stats),
                        };
                        if hb_tx.send(msg).is_err() {
                            break;
                        }
                    }
                });

                // Background Task: Cek Pembaruan Sistem (Auto-Update Check)
                let base_url = ep.http_base_url();
                let http_cli = http_client.clone();
                let cmd_tx_update = ui_cmd_tx.clone();
                let ep_label_update = ep_label.to_string();
                let cid_update = client_id.clone();
                let update_task = tokio::spawn(async move {
                    if let Ok(cli) = http_cli {
                        println!("[CLIENT] Memeriksa pembaruan sistem ke {}...", base_url);
                        match updater::check_for_update(&cli, &base_url).await {
                            Ok(Some(info)) => {
                                println!("========================================================");
                                println!("📢 PEMBARUAN SISTEM TERSEDIA!");
                                println!("   Versi Saat Ini: v{}", updater::CURRENT_VERSION);
                                println!("   Versi Terbaru:  v{}", info.latest_version);
                                println!("   Catatan Rilis:  {}", info.release_notes);
                                println!("========================================================");

                                let _ = cmd_tx_update.send(UiCommand::SetStatus(format!(
                                    "Memperbarui ke v{}... Mohon tunggu",
                                    info.latest_version
                                )));

                                if let Err(e) = updater::download_and_self_update(&cli, &base_url, &info).await {
                                    eprintln!("[UPDATER ERROR] Gagal self-update: {}", e);
                                    error_logger::send_global_error("SelfUpdateError", format!("Gagal self-update: {}", e), None, Some(cid_update.clone()));
                                    let _ = cmd_tx_update.send(UiCommand::SetStatus(format!("Update gagal: {}", e)));
                                    sleep(Duration::from_secs(3)).await;
                                    // Pulihkan status UI agar siswa tetap bisa login dengan nyaman
                                    let _ = cmd_tx_update.send(UiCommand::SetStatus(format!("Siap ({}). Silakan Login.", ep_label_update)));
                                }
                            }
                            Ok(None) => {
                                println!("[UPDATER] Klien sudah menggunakan versi terbaru (v{}).", updater::CURRENT_VERSION);
                            }
                            Err(e) => {
                                println!("[UPDATER] Cek update dilewati: {}", e);
                            }
                        }
                    }
                });

                // Main loop: Receive messages from Server
                let mut disconnect_detail = None;
                while let Some(msg_res) = ws_source.next().await {
                    match msg_res {
                        Ok(msg) => {
                            match msg {
                                Message::Text(text) => {
                                    if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                                        match server_msg {
                                            ServerMessage::LoginResponse {
                                                success,
                                                message,
                                                ..
                                            } => {
                                                if success {
                                                    println!("[AUTH] Login berhasil diterima dari server!");
                                                    let _ = ui_cmd_tx.send(UiCommand::HideLock);
                                                    let mut logged = current_logged_in_user.write().await;
                                                    if logged.is_none() {
                                                        *logged = Some((message.clone(), String::new()));
                                                    }
                                                } else {
                                                    println!("[AUTH] Login ditolak: {}", message);
                                                    let _ = ui_cmd_tx.send(UiCommand::SetStatus(message));
                                                }
                                            }
                                            ServerMessage::Command { action, payload } => {
                                                match action.as_str() {
                                                    "lock" => {
                                                        println!("[CMD] Server meminta KUNCI LAYAR");
                                                        let _ = ui_cmd_tx.send(UiCommand::ShowLock);
                                                        let _ = ui_cmd_tx.send(UiCommand::SetStatus("KOMPUTER DIKUNCI OLEH GURU / ADMIN".to_string()));
                                                        let mut logged = current_logged_in_user.write().await;
                                                        *logged = None;
                                                    }
                                                    "unlock" => {
                                                        println!("[CMD] Server meminta BUKA KUNCI");
                                                        let _ = ui_cmd_tx.send(UiCommand::HideLock);
                                                        let mut logged = current_logged_in_user.write().await;
                                                        *logged = Some(("AdminUnlock".to_string(), String::new()));
                                                    }
                                                    // --- REMOTE INPUT: Text (Unicode) ---
                                                    "text" => {
                                                        if let Some(ref text) = payload {
                                                            println!("[REMOTE] Kirim input teks ({} karakter)", text.len());
                                                            remote_input::send_text(text);
                                                            screen_trigger.notify_one();
                                                        }
                                                    }
                                                    // --- REMOTE INPUT: Keyboard ---
                                                    "key_press" => {
                                                        if let Some(ref data) = payload {
                                                            if let Ok(ev) = serde_json::from_str::<shared::RemoteKeyEvent>(data) {
                                                                println!("[REMOTE] Kirim keypress VK=0x{:02X}", ev.vk_code);
                                                                remote_input::send_key_press(ev.vk_code);
                                                                screen_trigger.notify_one();
                                                            }
                                                        }
                                                    }
                                                    "key_event" => {
                                                        if let Some(ref data) = payload {
                                                            if let Ok(ev) = serde_json::from_str::<shared::RemoteKeyEvent>(data) {
                                                                let key_up = ev.action == "up";
                                                                println!("[REMOTE] Key event VK=0x{:02X} action={}", ev.vk_code, ev.action);
                                                                remote_input::send_key_event(ev.vk_code, key_up);
                                                                if !key_up {
                                                                    screen_trigger.notify_one();
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // --- REMOTE INPUT: Mouse ---
                                                    "mouse_event" => {
                                                        if let Some(ref data) = payload {
                                                            if let Ok(ev) = serde_json::from_str::<shared::RemoteMouseEvent>(data) {
                                                                match ev.action.as_str() {
                                                                    "move" => {
                                                                        remote_input::send_mouse_move(ev.x, ev.y);
                                                                    }
                                                                    "click" => {
                                                                        let btn = ev.button.as_deref().unwrap_or("left");
                                                                        remote_input::send_mouse_click(ev.x, ev.y, btn);
                                                                        screen_trigger.notify_one();
                                                                    }
                                                                    "down" => {
                                                                        remote_input::send_mouse_move(ev.x, ev.y);
                                                                        let btn = ev.button.as_deref().unwrap_or("left");
                                                                        remote_input::send_mouse_button(btn, true);
                                                                        screen_trigger.notify_one();
                                                                    }
                                                                    "up" => {
                                                                        let btn = ev.button.as_deref().unwrap_or("left");
                                                                        remote_input::send_mouse_button(btn, false);
                                                                        screen_trigger.notify_one();
                                                                    }
                                                                    "double_click" => {
                                                                        remote_input::send_mouse_double_click(ev.x, ev.y);
                                                                        screen_trigger.notify_one();
                                                                    }
                                                                    "wheel" => {
                                                                        let delta = ev.delta.unwrap_or(120);
                                                                        remote_input::send_mouse_wheel(delta);
                                                                        screen_trigger.notify_one();
                                                                    }
                                                                    _ => {}
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // --- STREAM CONFIG (Turbo Remote Mode) via Command ---
                                                    "stream_config" => {
                                                        if let Some(ref data) = payload {
                                                            #[derive(serde::Deserialize)]
                                                            struct StreamCfgHelper {
                                                                #[serde(default)]
                                                                enabled: Option<bool>,
                                                                #[serde(default)]
                                                                fps: Option<u32>,
                                                                #[serde(default)]
                                                                quality: Option<u8>,
                                                            }
                                                            if let Ok(cfg) = serde_json::from_str::<StreamCfgHelper>(data) {
                                                                let enabled = cfg.enabled.unwrap_or(true);
                                                                let fps = cfg.fps.unwrap_or(5);
                                                                let q = cfg.quality.unwrap_or(65);
                                                                if enabled && fps > 0 {
                                                                    let interval = (1000 / fps).clamp(80, 2000) as u64;
                                                                    current_interval_ms.store(interval, Ordering::Relaxed);
                                                                    current_jpeg_quality.store(q.clamp(20, 95), Ordering::Relaxed);
                                                                    println!("[STREAM] Mode Turbo Remote aktif: {} FPS ({}ms), kualitas {}", fps, interval, q);
                                                                } else {
                                                                    let def_interval = ep.mode.screen_refresh_interval_ms();
                                                                    let def_q = ep.mode.screen_jpeg_quality();
                                                                    current_interval_ms.store(def_interval, Ordering::Relaxed);
                                                                    current_jpeg_quality.store(def_q, Ordering::Relaxed);
                                                                    println!("[STREAM] Mode stream kembali ke normal ({}ms)", def_interval);
                                                                }
                                                                screen_trigger.notify_one();
                                                            }
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            ServerMessage::StreamConfig { enabled, fps, quality } => {
                                                if enabled && fps > 0 {
                                                    let interval = (1000 / fps).clamp(80, 2000) as u64;
                                                    current_interval_ms.store(interval, Ordering::Relaxed);
                                                    current_jpeg_quality.store(quality.clamp(20, 95), Ordering::Relaxed);
                                                    println!("[STREAM] Mode Turbo Remote aktif dari Server: {} FPS ({}ms), kualitas {}", fps, interval, quality);
                                                } else {
                                                    let def_interval = ep.mode.screen_refresh_interval_ms();
                                                    let def_q = ep.mode.screen_jpeg_quality();
                                                    current_interval_ms.store(def_interval, Ordering::Relaxed);
                                                    current_jpeg_quality.store(def_q, Ordering::Relaxed);
                                                    println!("[STREAM] Mode stream kembali ke normal ({}ms)", def_interval);
                                                }
                                                screen_trigger.notify_one();
                                            }
                                            ServerMessage::UpdateNotification { info } => {
                                                if info.update_available && info.latest_version.trim() != updater::CURRENT_VERSION {
                                                    println!("========================================================");
                                                    println!("📢 PERINTAH UPDATE DITERIMA DARI SERVER!");
                                                    println!("   Target Versi: v{}", info.latest_version);
                                                    println!("   Catatan:      {}", info.release_notes);
                                                    println!("========================================================");

                                                    let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!(
                                                        "Update v{} diterima! Memperbarui...",
                                                        info.latest_version
                                                    )));

                                                    let base_url = ep.http_base_url();
                                                    if let Ok(ref cli) = http_client {
                                                        if let Err(e) = updater::download_and_self_update(cli, &base_url, &info).await {
                                                            eprintln!("[UPDATER ERROR] Gagal update otomatis: {}", e);
                                                            error_logger::send_global_error("AutoUpdateError", format!("Gagal update otomatis: {}", e), None, Some(client_id.clone()));
                                                            let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Gagal update: {}", e)));
                                                        }
                                                    }
                                                }
                                            }
                                            ServerMessage::AppPolicyUpdate { policy } => {
                                                println!(
                                                    "[GUARD] Kebijakan aplikasi diterima dari Server ({} aplikasi dilarang, blokir installer: {})",
                                                    policy.blocked_apps.len(),
                                                    policy.block_installers
                                                );
                                                let mut p = active_policy.write().await;
                                                *p = policy;
                                            }
                                            ServerMessage::AuthModeUpdate { config } => {
                                                println!(
                                                    "[AUTH] Mode autentikasi diterima dari Server: {:?} (Auto Unlock: {}, Offline Cache: {}, LAN Only: {})",
                                                    config.mode,
                                                    config.auto_unlock_on_no_auth,
                                                    config.allow_offline_cache,
                                                    config.lan_only
                                                );
                                                auth_cache::save_local_auth_config(&config);
                                                let _ = ui_cmd_tx.send(UiCommand::SetAuthMode(config.clone()));

                                                if config.mode == shared::AuthModeType::NoAuth && config.auto_unlock_on_no_auth {
                                                    println!("[AUTH] Mode Tanpa Autentikasi aktif - otomatis membuka kunci layar");
                                                    let _ = ui_cmd_tx.send(UiCommand::HideLock);
                                                    let mut logged = current_logged_in_user.write().await;
                                                    *logged = Some(("NoAuth".to_string(), String::new()));
                                                }
                                            }
                                        }
                                    }
                                }
                                Message::Close(close_frame) => {
                                    let reason = close_frame
                                        .map(|f| format!("Kode: {}, Alasan: {}", f.code, f.reason))
                                        .unwrap_or_else(|| "Tanpa detail frame close".to_string());
                                    println!("[CLIENT] Server menutup koneksi secara normal: {}", reason);
                                    disconnect_detail = Some(format!("Server Normal Close: {}", reason));
                                    break;
                                }
                                Message::Ping(_) | Message::Pong(_) => {}
                                _ => {}
                            }
                        }
                        Err(e) => {
                            println!("[CLIENT] Error membaca stream WebSocket: {:?}", e);
                            disconnect_detail = Some(format!("WebSocket Stream Error: {:?}", e));
                            break;
                        }
                    }
                }

                let details_str = disconnect_detail.unwrap_or_else(|| "Koneksi terputus (Stream EOF tanpa error eksplisit)".to_string());
                println!("[CLIENT] Koneksi ke [{}] terputus! Detail: {}", ep_label, details_str);
                error_logger::send_global_error("ConnectionLost", format!("Koneksi ke [{}] terputus: {}", ep_label, details_str), Some(details_str), Some(client_id.clone()));
                {
                    let mut guard = active_ws_tx.write().await;
                    *guard = None;
                }

                let is_logged_in = current_logged_in_user.read().await.is_some();
                let local_cfg = auth_cache::load_local_auth_config();
                if is_logged_in && !local_cfg.lan_only && local_cfg.allow_offline_cache {
                    println!("[CLIENT] Siswa sedang belajar. Tetap membuka layar dalam Mode Mandiri...");
                    let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Koneksi ke [{}] terputus (Mode Mandiri Aktif)", ep_label)));
                } else {
                    println!("[CLIENT] Mengunci layar demi keamanan...");
                    let _ = ui_cmd_tx.send(UiCommand::ShowLock);
                    let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Koneksi ke [{}] Terputus!", ep_label)));
                }

                send_task.abort();
                screen_task.abort();
                heartbeat_task.abort();
                update_task.abort();

                // Pindah ke endpoint berikutnya jika ada lebih dari 1 (Smart Failover)
                current_idx = (current_idx + 1) % endpoints.len();
            }
            Ok(Err(e)) => {
                println!("[CLIENT] Gagal menghubungi [{}] {}: {}. Beralih target...", ep_label, ep.raw, e);
                error_logger::send_global_error("ConnectionFailed", format!("Gagal menghubungi [{}] {}: {}", ep_label, ep.raw, e), Some(format!("Target: {}", ep.ws_url)), Some(client_id.clone()));
                let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Gagal ke {} ({}). Beralih target...", ep_label, ep.raw)));
                current_idx = (current_idx + 1) % endpoints.len();
            }
            Err(_elapsed) => {
                println!("[CLIENT] Waktu koneksi habis (timeout {:?}) ke [{}] {}. Beralih target...", connect_timeout, ep_label, ep.raw);
                error_logger::send_global_error("ConnectionTimeout", format!("Waktu koneksi habis (timeout {:?}) ke [{}] {}", connect_timeout, ep_label, ep.raw), Some(format!("Target: {}", ep.ws_url)), Some(client_id.clone()));
                let _ = ui_cmd_tx.send(UiCommand::SetStatus(format!("Timeout ke {} ({}). Beralih target...", ep_label, ep.raw)));
                current_idx = (current_idx + 1) % endpoints.len();
            }
        }

        sleep(Duration::from_millis(1500)).await;
    }
}
