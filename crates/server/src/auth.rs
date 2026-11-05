use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

use crate::AppState;

/// File tempat session persisten disimpan agar token tidak hilang saat server restart
const SESSIONS_FILE: &str = "sessions.json";
/// Masa berlaku session: 7 hari (604800 detik)
const SESSION_TTL_HOURS: i64 = 168;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub username: String,
    pub role: String,
    pub full_name: String,
    pub created_at: DateTime<Utc>,
}

impl SessionInfo {
    /// Cek apakah session masih dalam masa berlaku (TTL 7 hari)
    pub fn is_still_valid(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.created_at);
        age < Duration::hours(SESSION_TTL_HOURS)
    }
}

/// Muat session yang tersimpan di file (tahan restart server)
fn load_persisted_sessions() -> HashMap<String, SessionInfo> {
    if let Ok(data) = std::fs::read_to_string(SESSIONS_FILE) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, SessionInfo>>(&data) {
            // Buang session yang sudah expired sebelum dimuat
            let now = Utc::now();
            let valid: HashMap<String, SessionInfo> = map
                .into_iter()
                .filter(|(_, info)| {
                    let age = now.signed_duration_since(info.created_at);
                    age < Duration::hours(SESSION_TTL_HOURS)
                })
                .collect();
            println!("[AUTH] Memuat {} session persisten dari {}", valid.len(), SESSIONS_FILE);
            return valid;
        }
    }
    HashMap::new()
}

/// Simpan semua session aktif ke file
fn save_sessions_to_file(sessions: &HashMap<String, SessionInfo>) {
    if let Ok(json) = serde_json::to_string(sessions) {
        let _ = std::fs::write(SESSIONS_FILE, json);
    }
}

#[derive(Clone)]
pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    service_keys: Arc<RwLock<crate::api_service::ServiceKeyConfig>>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(load_persisted_sessions())),
            service_keys: Arc::new(RwLock::new(crate::api_service::load_or_init_service_keys())),
        }
    }

    pub fn new_with_keys(service_keys: Arc<RwLock<crate::api_service::ServiceKeyConfig>>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(load_persisted_sessions())),
            service_keys,
        }
    }

    /// Buat SessionStore kosong (tanpa membaca file) — khusus untuk unit test
    #[cfg(test)]
    fn new_empty_for_test() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            service_keys: Arc::new(RwLock::new(crate::api_service::load_or_init_service_keys())),
        }
    }

    pub async fn create_session(&self, username: String, role: String, full_name: String) -> String {
        let token = generate_token(&username);
        let info = SessionInfo {
            username,
            role,
            full_name,
            created_at: Utc::now(),
        };
        let mut map = self.sessions.write().await;
        map.insert(token.clone(), info);
        // Simpan ke file agar session tahan restart server
        save_sessions_to_file(&map);
        token
    }

    pub async fn get_session(&self, token: &str) -> Option<SessionInfo> {
        let map = self.sessions.read().await;
        if let Some(info) = map.get(token) {
            // Tolak session yang sudah expired (> 7 hari)
            if info.is_still_valid() {
                return Some(info.clone());
            }
            // Session expired — akan dibersihkan saat prune berikutnya
            return None;
        }
        // Fallback: periksa persistent service API keys (misal: Workbuddy AI)
        let keys = self.service_keys.read().await;
        if let Some(k) = keys.find_valid_key(token) {
            return Some(SessionInfo {
                username: k.name.clone(),
                role: k.role.clone(),
                full_name: format!("API Service ({})", k.name),
                created_at: Utc::now(),
            });
        }
        None
    }

    pub async fn remove_session(&self, token: &str) {
        let mut map = self.sessions.write().await;
        map.remove(token);
        save_sessions_to_file(&map);
    }

    pub async fn is_valid(&self, token: &str) -> bool {
        {
            let map = self.sessions.read().await;
            if let Some(info) = map.get(token) {
                if info.is_still_valid() {
                    return true;
                }
            }
        }
        self.service_keys.read().await.find_valid_key(token).is_some()
    }

    /// Bersihkan session yang sudah expired dari memori dan file
    pub async fn prune_expired_sessions(&self) {
        let mut map = self.sessions.write().await;
        let before = map.len();
        map.retain(|_, info| info.is_still_valid());
        let after = map.len();
        if before != after {
            println!("[AUTH] Membersihkan {} session expired", before - after);
            save_sessions_to_file(&map);
        }
    }
}

pub fn generate_token(seed: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let nanos = now.as_nanos();
    let secs = now.as_secs();
    let pid = std::process::id();
    
    // Hash-like hex token tanpa dependency eksternal berat
    let mut h1: u64 = 0xcbf29ce484222325;
    for b in format!("{}:{}:{}:{}", seed, secs, nanos, pid).bytes() {
        h1 ^= b as u64;
        h1 = h1.wrapping_mul(0x100000001b3);
    }
    let mut h2: u64 = 0x84222325cbf29ce4;
    for b in format!("{}:{}:{}:{}", nanos, pid, seed, secs).bytes() {
        h2 ^= b as u64;
        h2 = h2.wrapping_mul(0x100000001b3);
    }
    format!("molay_tok_{:016x}{:016x}", h1, h2)
}

pub fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_val) = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()) {
        let trimmed = auth_val.trim();
        if let Some(token) = trimmed.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        } else if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(custom) = headers.get("x-molay-token").and_then(|v| v.to_str().ok()) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    if let Some(api_key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let trimmed = api_key.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    None
}

#[derive(Deserialize)]
pub struct AuthQuery {
    pub token: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

pub async fn check_dashboard_auth(
    headers: &HeaderMap,
    query_token: Option<&str>,
    sessions: &SessionStore,
) -> Option<SessionInfo> {
    if let Some(q_tok) = query_token {
        if !q_tok.trim().is_empty() {
            if let Some(info) = sessions.get_session(q_tok.trim()).await {
                return Some(info);
            }
        }
    }
    if let Some(h_tok) = extract_token_from_headers(headers) {
        if let Some(info) = sessions.get_session(&h_tok).await {
            return Some(info);
        }
    }
    None
}

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginSuccessResponse {
    pub status: String,
    pub token: String,
    pub user: SessionInfo,
}

#[derive(Serialize)]
pub struct AuthErrorResponse {
    pub status: String,
    pub message: String,
}

pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Response {
    let username = payload.username.trim();
    let password = payload.password.trim();

    let users = state.users.read().await;
    if let Some(user) = users.get(username) {
        if user.password == password {
            // Hanya role "admin", "superadmin", atau instruktur/guru yang diizinkan mengakses dashboard
            if !user.is_admin_or_superadmin() {
                return (
                    StatusCode::FORBIDDEN,
                    Json(AuthErrorResponse {
                        status: "error".to_string(),
                        message: "Akun ini adalah akun Siswa. Hanya Guru/Admin/Superadmin yang dapat mengakses Dashboard!".to_string(),
                    }),
                )
                    .into_response();
            }

            let token = state
                .session_store
                .create_session(
                    user.username.clone(),
                    user.role.clone(),
                    user.full_name.clone(),
                )
                .await;

            let session_info = SessionInfo {
                username: user.username.clone(),
                role: user.role.clone(),
                full_name: user.full_name.clone(),
                created_at: Utc::now(),
            };

            println!("[AUTH-WEB] Guru/Admin '{}' berhasil login ke Dashboard Web.", username);

            return (
                StatusCode::OK,
                Json(LoginSuccessResponse {
                    status: "ok".to_string(),
                    token,
                    user: session_info,
                }),
            )
                .into_response();
        }
    }

    println!("[AUTH-WEB] Percobaan login gagal untuk user '{}'", username);
    (
        StatusCode::UNAUTHORIZED,
        Json(AuthErrorResponse {
            status: "error".to_string(),
            message: "Username atau Password salah!".to_string(),
        }),
    )
        .into_response()
}

pub async fn me_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Response {
    if let Some(info) = check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await {
        (StatusCode::OK, Json(serde_json::json!({
            "status": "ok",
            "user": info
        }))).into_response()
    } else {
        (StatusCode::UNAUTHORIZED, Json(AuthErrorResponse {
            status: "error".to_string(),
            message: "Sesi telah berakhir atau belum login.".to_string(),
        })).into_response()
    }
}

pub async fn logout_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Response {
    let token = query.token.or_else(|| extract_token_from_headers(&headers));
    if let Some(tok) = token {
        state.session_store.remove_session(&tok).await;
    }
    (StatusCode::OK, Json(serde_json::json!({ "status": "ok", "message": "Logout berhasil" }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token() {
        let tok1 = generate_token("admin");
        let tok2 = generate_token("admin");
        assert!(tok1.starts_with("molay_tok_"));
        assert_eq!(tok1.len(), 42); // "molay_tok_" (10) + 32 hex
        assert!(!tok1.is_empty());
        assert!(!tok2.is_empty());
    }

    #[tokio::test]
    async fn test_session_store() {
        let store = SessionStore::new_empty_for_test();
        let token = store.create_session("admin".to_string(), "admin".to_string(), "Administrator".to_string()).await;
        assert!(store.is_valid(&token).await);

        let info = store.get_session(&token).await.unwrap();
        assert_eq!(info.username, "admin");
        assert_eq!(info.role, "admin");

        store.remove_session(&token).await;
        assert!(!store.is_valid(&token).await);
    }

    #[tokio::test]
    async fn test_extract_token_from_headers() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer test_secret_token_123".parse().unwrap());
        assert_eq!(extract_token_from_headers(&headers), Some("test_secret_token_123".to_string()));

        let mut headers_custom = HeaderMap::new();
        headers_custom.insert("x-molay-token", "custom_token_abc".parse().unwrap());
        assert_eq!(extract_token_from_headers(&headers_custom), Some("custom_token_abc".to_string()));

        let empty_headers = HeaderMap::new();
        assert_eq!(extract_token_from_headers(&empty_headers), None);

        let mut headers_api_key = HeaderMap::new();
        headers_api_key.insert("x-api-key", "molay_sec_workbuddy_123".parse().unwrap());
        assert_eq!(extract_token_from_headers(&headers_api_key), Some("molay_sec_workbuddy_123".to_string()));
    }

    #[tokio::test]
    async fn test_service_key_fallback_in_session_store() {
        let store = SessionStore::new_empty_for_test();
        // Cek bahwa default service key (workbuddy) dikenali sebagai sesi admin
        let info = store.get_session(crate::api_service::DEFAULT_WORKBUDDY_KEY).await;
        assert!(info.is_some());
        let session = info.unwrap();
        assert_eq!(session.role, "admin");
        assert_eq!(session.username, "Workbuddy AI Integration");

        // Key palsu tidak boleh dikenali
        assert!(store.get_session("key_palsu_random_xyz").await.is_none());
    }

    #[tokio::test]
    async fn test_superadmin_role_session() {
        let store = SessionStore::new_empty_for_test();
        let token = store.create_session("superadmin".to_string(), "superadmin".to_string(), "Super Administrator".to_string()).await;
        assert!(store.is_valid(&token).await);

        let info = store.get_session(&token).await.unwrap();
        assert_eq!(info.username, "superadmin");
        assert_eq!(info.role, "superadmin");
        assert_eq!(info.full_name, "Super Administrator");
    }
}
