use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::Utc;
use shared::{ActivityEntry, AuthModeType, ClientAuthConfig, ClientStatus, ServerMessage, User};
use crate::{auth, AppState};

pub const AUTH_MODE_FILE: &str = "client_auth_mode.json";

pub fn load_or_init_client_auth_config() -> ClientAuthConfig {
    if let Ok(data) = std::fs::read_to_string(AUTH_MODE_FILE) {
        if let Ok(cfg) = serde_json::from_str::<ClientAuthConfig>(&data) {
            println!("[AUTH-MODE] Memuat konfigurasi mode akses siswa dari {} (Mode: {:?})", AUTH_MODE_FILE, cfg.mode);
            return cfg;
        }
    }

    let default_cfg = ClientAuthConfig::default();
    let _ = save_client_auth_config(&default_cfg);
    println!("[AUTH-MODE] Inisialisasi konfigurasi default mode akses siswa di {}", AUTH_MODE_FILE);
    default_cfg
}

pub fn save_client_auth_config(config: &ClientAuthConfig) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(AUTH_MODE_FILE, json)
}

pub async fn get_auth_mode_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let cfg = state.client_auth_mode.read().await.clone();
    Json(cfg).into_response()
}

pub async fn save_auth_mode_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(mut new_config): Json<ClientAuthConfig>,
) -> Response {
    let admin_session = match auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await {
        Some(info) => info,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Unauthorized" })),
            )
                .into_response();
        }
    };

    // Validasi untuk mode GeneralPassword
    if new_config.mode == AuthModeType::GeneralPassword {
        let pass = new_config.general_password.as_deref().unwrap_or("").trim();
        if pass.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "General Password tidak boleh kosong jika mode General Password dipilih!" })),
            )
                .into_response();
        }
        new_config.general_password = Some(pass.to_string());
    }

    if let Err(e) = save_client_auth_config(&new_config) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Gagal menyimpan file mode akses: {}", e) })),
        )
            .into_response();
    }

    // Update in-memory state
    {
        let mut guard = state.client_auth_mode.write().await;
        *guard = new_config.clone();
    }

    let mode_desc = match new_config.mode {
        AuthModeType::Identity => "Autentikasi Identitas Siswa (Akun Masing-masing)",
        AuthModeType::NoAuth => "Tanpa Autentikasi (Bebas Masuk Langsung)",
        AuthModeType::GeneralPassword => "General Password Bersama yang Diatur Guru",
    };

    println!("[AUTH-MODE] Mode akses siswa diubah oleh '{}' menjadi: {}", admin_session.username, mode_desc);

    // Broadcast konfigurasi baru ke seluruh PC klien yang sedang terhubung
    let senders = state.client_senders.read().await;
    let mut sent_count = 0;
    for (cid, tx) in senders.iter() {
        let msg = ServerMessage::AuthModeUpdate {
            config: new_config.clone(),
        };
        if tx.send(msg).is_ok() {
            sent_count += 1;
        } else {
            println!("[AUTH-MODE] Gagal mengirim update mode akses ke PC {}", cid);
        }
    }

    // Jika mode diubah ke Tanpa Autentikasi dan opsi auto unlock aktif:
    // Buka kunci semua PC yang tidak sedang dikunci manual oleh admin
    if new_config.mode == AuthModeType::NoAuth && new_config.auto_unlock_on_no_auth {
        let mut clients = state.clients.write().await;
        for (cid, client) in clients.iter_mut() {
            if !client.is_admin_locked {
                client.status = ClientStatus::LoggedIn;
                if client.current_user.is_none() {
                    client.current_user = Some(User {
                        username: client.client_id.clone(),
                        password: String::new(),
                        full_name: format!("Siswa ({})", client.client_id),
                        role: "student".to_string(),
                    });
                }
                if let Some(tx) = senders.get(cid) {
                    let _ = tx.send(ServerMessage::Command {
                        action: "unlock".to_string(),
                        payload: None,
                    });
                }
                let update_json = serde_json::json!({
                    "type": "ClientUpdate",
                    "client": client.clone()
                });
                let _ = state.dashboard_tx.send(update_json.to_string());
            }
        }
    }

    // Catat log aktivitas sistem
    let log_title = format!("Mode Akses Klien Diubah ke: {}", mode_desc);
    let entry = ActivityEntry {
        client_id: "SERVER".to_string(),
        username: Some(admin_session.full_name),
        app_name: "PENGATURAN AKSES".to_string(),
        window_title: log_title.clone(),
        timestamp: Utc::now(),
        category: Some("SYSTEM".to_string()),
        severity: Some("info".to_string()),
        details: Some(format!("Guru mengubah mode login siswa menjadi '{}'. Diterapkan ke {} PC.", mode_desc, sent_count)),
        keystrokes: None,
        mouse_clicks: None,
        mouse_distance_px: None,
        idle_seconds: None,
        is_afk: None,
    };
    state.activities.write().await.push(entry.clone());
    let _ = state.dashboard_tx.send(serde_json::json!({
        "type": "NewActivity",
        "activity": entry
    }).to_string());

    // Broadcast notifikasi event ke dashboard browser
    let _ = state.dashboard_tx.send(serde_json::json!({
        "type": "AuthModeUpdated",
        "config": new_config
    }).to_string());

    Json(serde_json::json!({
        "status": "ok",
        "message": format!("Mode akses berhasil disimpan ({}) dan disinkronkan ke {} PC klien", mode_desc, sent_count),
        "config": new_config
    })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_default_initialization() {
        let cfg = ClientAuthConfig::default();
        assert_eq!(cfg.mode, AuthModeType::Identity);
        assert_eq!(cfg.general_password.as_deref(), Some("lab123"));
        assert!(cfg.auto_unlock_on_no_auth);
    }

    #[test]
    fn test_save_and_load_config() {
        let test_file = "test_client_auth_mode.json";
        let cfg = ClientAuthConfig {
            mode: AuthModeType::GeneralPassword,
            general_password: Some("kelaskomputer9".to_string()),
            welcome_message: Some("Selamat Belajar!".to_string()),
            auto_unlock_on_no_auth: false,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };

        let json = serde_json::to_string_pretty(&cfg).unwrap();
        std::fs::write(test_file, json).unwrap();

        let loaded_data = std::fs::read_to_string(test_file).unwrap();
        let loaded: ClientAuthConfig = serde_json::from_str(&loaded_data).unwrap();
        assert_eq!(loaded.mode, AuthModeType::GeneralPassword);
        assert_eq!(loaded.general_password.as_deref(), Some("kelaskomputer9"));
        assert_eq!(loaded.welcome_message.as_deref(), Some("Selamat Belajar!"));
        assert!(!loaded.auto_unlock_on_no_auth);

        let _ = std::fs::remove_file(test_file);
    }
}
