use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use shared::{AppPolicyConfig, ServerMessage};
use crate::{auth, AppState};

pub const POLICY_FILE: &str = "app_policy.json";

/// Memastikan URL pengalihan pada konfigurasi kebijakan selalu memiliki skema http:// atau https://
pub fn sanitize_policy_urls(policy: &mut AppPolicyConfig) {
    if let Some(ref mut br) = policy.browser_redirect {
        if let Some(ref mut url) = br.redirect_url {
            let trimmed = url.trim();
            if !trimmed.is_empty()
                && !trimmed.starts_with("http://")
                && !trimmed.starts_with("https://")
                && !trimmed.eq_ignore_ascii_case("about:blank")
            {
                *url = format!("https://{}", trimmed);
            }
        }
    }
}

pub fn load_or_init_app_policy() -> AppPolicyConfig {
    if let Ok(data) = std::fs::read_to_string(POLICY_FILE) {
        if let Ok(mut policy) = serde_json::from_str::<AppPolicyConfig>(&data) {
            sanitize_policy_urls(&mut policy);
            println!("[POLICY] Memuat konfigurasi kebijakan aplikasi dari {} ({} app dilarang)", POLICY_FILE, policy.blocked_apps.len());
            return policy;
        }
    }

    let mut policy = AppPolicyConfig::default();
    sanitize_policy_urls(&mut policy);
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
    Json(mut new_policy): Json<AppPolicyConfig>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    sanitize_policy_urls(&mut new_policy);

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

#[derive(Debug, serde::Deserialize)]
pub struct AddExemptionRequest {
    pub target: String,
    #[serde(default)]
    pub exemption_type: Option<String>, // "permanent" or "timed"
    #[serde(default)]
    pub duration_minutes: Option<i64>, // e.g. 15, 45, 90
    #[serde(default)]
    pub expires_at: Option<String>,    // ISO string e.g. 2026-11-05T15:30:00
    #[serde(default)]
    pub reason: Option<String>,
}

pub async fn add_exemption_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(req): Json<AddExemptionRequest>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let target = req.target.trim().to_string();
    if target.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Target website / aplikasi pengecualian tidak boleh kosong" })),
        )
            .into_response();
    }

    let is_timed = req.exemption_type.as_deref() == Some("timed")
        || req.duration_minutes.is_some()
        || req.expires_at.is_some();

    let (ex_type, expires_at) = if is_timed {
        let exp_iso = if let Some(mins) = req.duration_minutes {
            (chrono::Utc::now() + chrono::Duration::minutes(mins)).to_rfc3339()
        } else if let Some(ref exp_str) = req.expires_at {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(exp_str) {
                dt.to_rfc3339()
            } else if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(exp_str, "%Y-%m-%dT%H:%M:%S") {
                let dt = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
                dt.to_rfc3339()
            } else if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(exp_str, "%Y-%m-%dT%H:%M") {
                let dt = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
                dt.to_rfc3339()
            } else {
                exp_str.clone()
            }
        } else {
            // Default durasi 45 menit (1 jam pelajaran)
            (chrono::Utc::now() + chrono::Duration::minutes(45)).to_rfc3339()
        };
        ("timed".to_string(), Some(exp_iso))
    } else {
        ("permanent".to_string(), None)
    };

    let new_ex = shared::PolicyExemption {
        id: format!("ex-{}", chrono::Utc::now().timestamp_millis()),
        target: target.clone(),
        exemption_type: ex_type,
        expires_at,
        created_at: chrono::Utc::now().to_rfc3339(),
        reason: req.reason.unwrap_or_else(|| "Pengecualian akses oleh Guru Lab".to_string()),
    };

    let updated_policy = {
        let mut policy_guard = state.app_policy.write().await;
        // Hapus pengecualian sebelumnya untuk target yang sama jika ada (update)
        let lower = target.to_lowercase();
        policy_guard.exemptions.retain(|e| e.target.to_lowercase() != lower);
        policy_guard.exemptions.push(new_ex.clone());
        let _ = save_app_policy(&policy_guard);
        policy_guard.clone()
    };

    // Broadcast ke seluruh klien siswa
    let senders = state.client_senders.read().await;
    let mut sent_count = 0;
    for (cid, tx) in senders.iter() {
        let msg = ServerMessage::AppPolicyUpdate {
            policy: updated_policy.clone(),
        };
        if tx.send(msg).is_ok() {
            sent_count += 1;
        } else {
            println!("[POLICY] Gagal mengirim update pengecualian ke PC {}", cid);
        }
    }

    println!(
        "[POLICY] Pengecualian '{}' ({}) berhasil ditambahkan dan disiarkan ke {} PC klien",
        new_ex.target, new_ex.exemption_type, sent_count
    );

    // Kirim notifikasi event ke dashboard browser
    let _ = state.dashboard_tx.send(serde_json::json!({
        "type": "PolicyUpdated",
        "policy": updated_policy
    }).to_string());

    Json(serde_json::json!({
        "status": "ok",
        "message": format!("Pengecualian untuk '{}' berhasil diaktifkan", new_ex.target),
        "exemption": new_ex,
        "sent_count": sent_count
    }))
    .into_response()
}

#[derive(Debug, serde::Deserialize)]
pub struct RemoveExemptionRequest {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub target: Option<String>,
}

pub async fn remove_exemption_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(req): Json<RemoveExemptionRequest>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store).await.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let updated_policy = {
        let mut policy_guard = state.app_policy.write().await;
        if let Some(ref ex_id) = req.id {
            policy_guard.exemptions.retain(|e| &e.id != ex_id);
        } else if let Some(ref target) = req.target {
            let lower = target.trim().to_lowercase();
            policy_guard.exemptions.retain(|e| e.target.to_lowercase() != lower);
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Parameter id atau target harus diisi" })),
            )
                .into_response();
        }
        let _ = save_app_policy(&policy_guard);
        policy_guard.clone()
    };

    // Broadcast ke seluruh klien siswa
    let senders = state.client_senders.read().await;
    let mut sent_count = 0;
    for (cid, tx) in senders.iter() {
        let msg = ServerMessage::AppPolicyUpdate {
            policy: updated_policy.clone(),
        };
        if tx.send(msg).is_ok() {
            sent_count += 1;
        } else {
            println!("[POLICY] Gagal mengirim pencabutan pengecualian ke PC {}", cid);
        }
    }

    println!("[POLICY] Pengecualian berhasil dicabut dan disiarkan ke {} PC klien", sent_count);

    let _ = state.dashboard_tx.send(serde_json::json!({
        "type": "PolicyUpdated",
        "policy": updated_policy
    }).to_string());

    Json(serde_json::json!({
        "status": "ok",
        "message": "Pengecualian berhasil dicabut",
        "sent_count": sent_count
    }))
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::BrowserRedirectRule;

    #[test]
    fn test_sanitize_policy_urls_adds_https() {
        let mut policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![],
            blocked_keywords: vec![],
            browser_redirect: Some(BrowserRedirectRule {
                enabled: true,
                trigger_keywords: vec!["game".to_string()],
                target_browser_exe: String::new(),
                target_browser_args: None,
                redirect_url: Some("smpn6pekalongan.sch.id".to_string()),
                close_original_window: true,
                cooldown_seconds: 3,
            }),
            exemptions: vec![],
        };

        sanitize_policy_urls(&mut policy);
        assert_eq!(
            policy.browser_redirect.unwrap().redirect_url.as_deref(),
            Some("https://smpn6pekalongan.sch.id")
        );
    }

    #[test]
    fn test_sanitize_policy_urls_preserves_existing_scheme() {
        let mut policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![],
            blocked_keywords: vec![],
            browser_redirect: Some(BrowserRedirectRule {
                enabled: true,
                trigger_keywords: vec!["game".to_string()],
                target_browser_exe: String::new(),
                target_browser_args: None,
                redirect_url: Some("http://internal-exam.local".to_string()),
                close_original_window: true,
                cooldown_seconds: 3,
            }),
            exemptions: vec![],
        };

        sanitize_policy_urls(&mut policy);
        assert_eq!(
            policy.browser_redirect.unwrap().redirect_url.as_deref(),
            Some("http://internal-exam.local")
        );
    }
}
