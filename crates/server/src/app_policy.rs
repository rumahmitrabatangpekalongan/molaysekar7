use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use shared::{AppPolicyConfig, ServerMessage};
use crate::{auth, AppState};

pub const POLICY_FILE: &str = "app_policy.json";

pub fn load_or_init_app_policy() -> AppPolicyConfig {
    if let Ok(data) = std::fs::read_to_string(POLICY_FILE) {
        if let Ok(policy) = serde_json::from_str::<AppPolicyConfig>(&data) {
            println!("[POLICY] Memuat konfigurasi kebijakan aplikasi dari {} ({} app dilarang)", POLICY_FILE, policy.blocked_apps.len());
            return policy;
        }
    }

    let policy = AppPolicyConfig::default();
    let _ = save_app_policy(&policy);
    println!("[POLICY] Inisialisasi konfigurasi default kebijakan aplikasi di {}", POLICY_FILE);
    policy
}

pub fn save_app_policy(policy: &AppPolicyConfig) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(policy)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    std::fs::write(POLICY_FILE, json)
}

pub async fn get_policy_handler(
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

    let policy = state.app_policy.read().await.clone();
    Json(policy).into_response()
}

pub async fn save_policy_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(new_policy): Json<AppPolicyConfig>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    if let Err(e) = save_app_policy(&new_policy) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Gagal menyimpan file kebijakan: {}", e) })),
        )
            .into_response();
    }

    // Update in-memory state
    {
        let mut policy_guard = state.app_policy.write().await;
        *policy_guard = new_policy.clone();
    }

    // Broadcast ke seluruh klien yang terhubung secara realtime
    let senders = state.client_senders.read().await;
    let mut sent_count = 0;
    for (cid, tx) in senders.iter() {
        let msg = ServerMessage::AppPolicyUpdate {
            policy: new_policy.clone(),
        };
        if tx.send(msg).is_ok() {
            sent_count += 1;
        } else {
            println!("[POLICY] Gagal mengirim update kebijakan ke PC {}", cid);
        }
    }

    println!("[POLICY] Kebijakan aplikasi berhasil diperbarui & dikirim ke {} PC klien", sent_count);

    // Kirim notifikasi event ke dashboard browser
    let _ = state.dashboard_tx.send(serde_json::json!({
        "type": "PolicyUpdated",
        "policy": new_policy
    }).to_string());

    Json(serde_json::json!({
        "status": "ok",
        "message": format!("Kebijakan berhasil disimpan dan diterapkan ke {} PC klien terhubung", sent_count),
        "policy": new_policy
    }))
    .into_response()
}
