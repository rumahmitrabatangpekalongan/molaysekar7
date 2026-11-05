use crate::AppState;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
};
use chrono::Utc;
use serde::Deserialize;
use shared::GlobalErrorLog;
use std::net::SocketAddr;

pub const ERROR_LOGS_FILE: &str = "global_error_logs.jsonl";
pub const GLOBAL_ERROR_ENDPOINT: &str = "https://molay.sekolahkartini7.web.id/logerorglobal";
pub const SERVER_VERSION: &str = "1.0.2";

#[derive(Debug, Deserialize)]
pub struct ErrorLogsQuery {
    pub limit: Option<usize>,
    pub source: Option<String>,
    pub format: Option<String>,
}

/// Menyimpan entri error ke file log lokal global_error_logs.jsonl
pub fn append_error_log(entry: &GlobalErrorLog) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(ERROR_LOGS_FILE)
    {
        if let Ok(json) = serde_json::to_string(entry) {
            let _ = writeln!(file, "{}", json);
        }
    }
}

/// Membaca entri error terbaru dari file
pub fn load_recent_error_logs(limit: usize) -> Vec<GlobalErrorLog> {
    let mut list = Vec::new();
    if let Ok(content) = std::fs::read_to_string(ERROR_LOGS_FILE) {
        for line in content.lines().rev() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Ok(entry) = serde_json::from_str::<GlobalErrorLog>(trimmed) {
                    list.push(entry);
                    if list.len() >= limit {
                        break;
                    }
                }
            }
        }
    }
    list
}

/// Handler POST /logerorglobal untuk menerima laporan error dari Klien atau Server
pub async fn receive_error_log_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(mut payload): Json<GlobalErrorLog>,
) -> impl IntoResponse {
    // Deteksi IP klien dari Header Cloudflare / Proxy jika ada
    let real_ip = headers
        .get("cf-connecting-ip")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get("x-forwarded-for")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split(',').next())
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| addr.ip().to_string());

    if payload.ip.is_none() {
        payload.ip = Some(real_ip);
    }

    if payload.timestamp.timestamp() == 0 {
        payload.timestamp = Utc::now();
    }

    println!(
        "[GLOBAL-LOG] [{}] [{}] Dari: {:?} ({:?}) - {}",
        payload.source, payload.error_type, payload.client_id, payload.ip, payload.message
    );

    append_error_log(&payload);

    // Broadcast ke dashboard admin jika ada browser guru yang sedang membuka
    let broadcast_msg = serde_json::json!({
        "type": "GlobalErrorLog",
        "log": payload
    });
    let _ = state.dashboard_tx.send(broadcast_msg.to_string());

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "message": "Error log berhasil dicatat",
            "id": payload.id
        })),
    )
}

/// Handler POST /api/logerorglobal/clear untuk mengosongkan riwayat error log
pub async fn clear_error_logs_handler() -> impl IntoResponse {
    let _ = std::fs::write(ERROR_LOGS_FILE, "");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "ok",
            "message": "Seluruh riwayat error log berhasil dibersihkan"
        })),
    )
}

/// Handler GET /logerorglobal untuk melihat daftar log error
pub async fn get_error_logs_handler(
    headers: HeaderMap,
    Query(query): Query<ErrorLogsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(500);
    let mut logs = load_recent_error_logs(limit);

    if let Some(ref src) = query.source {
        logs.retain(|l| l.source.eq_ignore_ascii_case(src));
    }

    let is_html_requested = query.format.as_deref() == Some("html")
        || headers
            .get("accept")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.contains("text/html"))
            .unwrap_or(false);

    if is_html_requested {
        let client_count = logs.iter().filter(|l| l.source == "client").count();
        let server_count = logs.iter().filter(|l| l.source == "server").count();

        let mut rows = String::new();
        for log in &logs {
            let client = log.client_id.as_deref().unwrap_or("-");
            let ip = log.ip.as_deref().unwrap_or("-");
            let details = log.details.as_deref().unwrap_or("-");
            let badge_color = if log.source == "client" { "#0284c7" } else { "#e11d48" };

            rows.push_str(&format!(
                r#"<tr>
                    <td style="white-space:nowrap;">{}</td>
                    <td><span style="background:{}; color:white; padding:3px 8px; border-radius:4px; font-weight:600; font-size:0.75rem;">{}</span></td>
                    <td style="font-weight:600; color:#fca5a5;">{}</td>
                    <td><code style="color:#38bdf8;">{}</code></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td style="color:#94a3b8; font-size:0.8rem; word-break:break-all;">{}</td>
                </tr>"#,
                log.timestamp.format("%Y-%m-%d %H:%M:%S"),
                badge_color,
                log.source.to_uppercase(),
                log.error_type,
                client,
                ip,
                log.message,
                details
            ));
        }

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="id">
<head>
    <meta charset="UTF-8">
    <title>Molay Sekar7 - Global Error Logger</title>
    <style>
        body {{ font-family: system-ui, -apple-system, sans-serif; background: #0f172a; color: #f8fafc; padding: 24px; }}
        h1 {{ font-size: 1.5rem; margin-bottom: 4px; color: #38bdf8; display: flex; align-items: center; gap: 10px; }}
        p {{ color: #94a3b8; font-size: 0.9rem; margin-bottom: 16px; }}
        .stats-grid {{ display: flex; gap: 12px; margin-bottom: 20px; flex-wrap: wrap; }}
        .stat-card {{ background: #1e293b; border: 1px solid #334155; padding: 12px 18px; border-radius: 8px; flex: 1; min-width: 140px; }}
        .stat-val {{ font-size: 1.6rem; font-weight: 700; color: #f8fafc; }}
        .stat-lbl {{ font-size: 0.75rem; text-transform: uppercase; color: #94a3b8; margin-top: 4px; }}
        .live-badge {{ background: #065f46; color: #34d399; font-size: 0.75rem; padding: 4px 10px; border-radius: 12px; font-weight: 600; display: inline-flex; align-items: center; gap: 6px; }}
        .live-dot {{ width: 8px; height: 8px; background: #34d399; border-radius: 50%; animation: pulse 1.5s infinite; }}
        @keyframes pulse {{ 0% {{ opacity: 1; }} 50% {{ opacity: 0.3; }} 100% {{ opacity: 1; }} }}
        table {{ width: 100%; border-collapse: collapse; background: #1e293b; border-radius: 8px; overflow: hidden; font-size: 0.85rem; margin-top: 10px; }}
        th, td {{ padding: 12px 14px; border-bottom: 1px solid #334155; text-align: left; }}
        th {{ background: #0b1120; color: #94a3b8; text-transform: uppercase; font-size: 0.75rem; letter-spacing: 0.05em; }}
        tr:hover {{ background: #243247; }}
        .header-box {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px; flex-wrap: wrap; gap: 10px; }}
        .btn {{ background: #0284c7; color: white; padding: 8px 16px; border-radius: 6px; text-decoration: none; font-size: 0.85rem; font-weight: 600; cursor: pointer; border: none; }}
        .btn:hover {{ background: #0369a1; }}
        .btn-danger {{ background: #dc2626; }}
        .btn-danger:hover {{ background: #b91c1c; }}
        .hint-box {{ background: #1e3a5f; border-left: 4px solid #38bdf8; padding: 12px 16px; border-radius: 6px; margin-bottom: 16px; font-size: 0.85rem; line-height: 1.4; color: #e2e8f0; }}
    </style>
</head>
<body>
    <div class="header-box">
        <div>
            <h1>🚨 Molay Sekar7 - Global Error Logger <span class="live-badge"><span class="live-dot"></span> LIVE MONITORING</span></h1>
            <p>Memantau riwayat & penyebab kendala teknis dari seluruh Klien Siswa dan Server Lab secara real-time.</p>
        </div>
        <div style="display:flex; gap:8px;">
            <button onclick="clearLogs()" class="btn btn-danger">🗑️ Bersihkan Log</button>
            <a href="/logerorglobal?format=json" class="btn" style="background:#334155;">JSON</a>
            <a href="/" class="btn">Kembali ke Dashboard</a>
        </div>
    </div>

    <div class="stats-grid">
        <div class="stat-card">
            <div class="stat-val">{}</div>
            <div class="stat-lbl">Total Error Tercatat</div>
        </div>
        <div class="stat-card">
            <div class="stat-val" style="color: #38bdf8;">{}</div>
            <div class="stat-lbl">Kendala Sisi Klien</div>
        </div>
        <div class="stat-card">
            <div class="stat-val" style="color: #f87171;">{}</div>
            <div class="stat-lbl">Kendala Sisi Server</div>
        </div>
    </div>

    <div class="hint-box">
        <strong>💡 Panduan Penanganan Cepat Koneksi Client-Server:</strong><br/>
        1. <strong>ConnectionRefused / ConnectionTimeout</strong>: Server belum jalan di IP tersebut, atau port 1916 belum diizinkan di Windows Firewall Server (jalankan <code>Izinkan-Firewall-Server.bat</code> di komputer guru).<br/>
        2. <strong>ConnectionLost</strong>: Periksa kabel LAN, WiFi putus-nyambung, atau IP Server berganti (jalankan scan auto-discovery atau sesuaikan file <code>server_ip.txt</code> di klien).
    </div>

    <table>
        <thead>
            <tr>
                <th>Waktu</th>
                <th>Sumber</th>
                <th>Jenis Error</th>
                <th>PC / Client</th>
                <th>Alamat IP</th>
                <th>Pesan Kendala</th>
                <th>Keterangan / Root Cause</th>
            </tr>
        </thead>
        <tbody id="logs-tbody">
            {}
        </tbody>
    </table>

    <script>
        // Auto-refresh setiap 4 detik tanpa me-reload seluruh halaman
        let isRefreshing = false;
        async function fetchLogs() {{
            if (isRefreshing) return;
            isRefreshing = true;
            try {{
                const res = await fetch('/logerorglobal?limit=100');
                if (res.ok) {{
                    const logs = await res.json();
                    let html = '';
                    for (const log of logs) {{
                        const client = log.client_id || '-';
                        const ip = log.ip || '-';
                        const details = log.details || '-';
                        const badgeColor = log.source === 'client' ? '#0284c7' : '#e11d48';
                        const timeStr = new Date(log.timestamp).toLocaleString('id-ID');
                        html += `<tr>
                            <td style="white-space:nowrap;">${{timeStr}}</td>
                            <td><span style="background:${{badgeColor}}; color:white; padding:3px 8px; border-radius:4px; font-weight:600; font-size:0.75rem;">${{log.source.toUpperCase()}}</span></td>
                            <td style="font-weight:600; color:#fca5a5;">${{log.error_type}}</td>
                            <td><code style="color:#38bdf8;">${{client}}</code></td>
                            <td>${{ip}}</td>
                            <td>${{log.message}}</td>
                            <td style="color:#94a3b8; font-size:0.8rem; word-break:break-all;">${{details}}</td>
                        </tr>`;
                    }}
                    const tbody = document.getElementById('logs-tbody');
                    if (tbody && html) tbody.innerHTML = html;
                }}
            }} catch (e) {{
                console.error("Auto refresh error", e);
            }} finally {{
                isRefreshing = false;
            }}
        }}

        setInterval(fetchLogs, 4000);

        async function clearLogs() {{
            if (!confirm('Apakah Anda yakin ingin mengosongkan seluruh riwayat log error?')) return;
            try {{
                const res = await fetch('/api/logerorglobal/clear', {{ method: 'POST' }});
                if (res.ok) {{
                    location.reload();
                }}
            }} catch (e) {{
                alert('Gagal membersihkan log: ' + e);
            }}
        }}
    </script>
</body>
</html>"#,
            logs.len(),
            client_count,
            server_count,
            rows
        );
        Html(html).into_response()
    } else {
        Json(logs).into_response()
    }
}

/// Fungsi untuk server mencatat dan melaporkan error internalnya sendiri
pub fn log_server_error(error_type: &str, message: &str, details: Option<&str>) {
    eprintln!("[SERVER-ERROR] [{}]: {} ({:?})", error_type, message, details);

    let entry = GlobalErrorLog {
        id: format!("srv_err_{}", Utc::now().timestamp_millis()),
        timestamp: Utc::now(),
        source: "server".to_string(),
        client_id: Some("MOLAY-SERVER".to_string()),
        hostname: std::env::var("COMPUTERNAME").ok(),
        ip: Some("127.0.0.1".to_string()),
        error_type: error_type.to_string(),
        message: message.to_string(),
        details: details.map(|s| s.to_string()),
        app_version: Some(SERVER_VERSION.to_string()),
    };

    // Catat ke file lokal
    append_error_log(&entry);

    // Kirim juga ke endpoint cloud jika server ini terhubung ke cloud
    tokio::spawn(async move {
        if let Ok(c) = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
        {
            let _ = c.post(GLOBAL_ERROR_ENDPOINT).json(&entry).send().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_log_serialization_and_append() {
        let entry = GlobalErrorLog {
            id: "test_srv_err".to_string(),
            timestamp: Utc::now(),
            source: "server".to_string(),
            client_id: Some("MOLAY-SERVER".to_string()),
            hostname: Some("TEST-SRV".to_string()),
            ip: Some("127.0.0.1".to_string()),
            error_type: "TestServerError".to_string(),
            message: "Testing server error logging".to_string(),
            details: Some("Extra error context".to_string()),
            app_version: Some("1.0.2".to_string()),
        };
        append_error_log(&entry);
        let logs = load_recent_error_logs(5);
        assert!(logs.iter().any(|l| l.id == "test_srv_err"));

        // Test clear
        let _ = std::fs::write(ERROR_LOGS_FILE, "");
        let empty_logs = load_recent_error_logs(5);
        assert!(empty_logs.is_empty());
    }
}
