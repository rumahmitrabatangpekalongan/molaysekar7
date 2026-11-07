//! Modul Deteksi Otomatis Link & Game via AI (SumoPod GLM Engine)
//!
//! Modul ini berjalan di Molay Server untuk mendeteksi secara otomatis
//! website / game online / portal hiburan yang tidak bermanfaat bagi siswa selama jam lab.
//! Kata kunci hasil deteksi langsung dimasukkan ke `browser_redirect.trigger_keywords`
//! pada `app_policy.json` dan disebarkan seketika (realtime) ke seluruh PC klien siswa.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::{app_policy, auth, AppState};
use shared::{AppPolicyConfig, BrowserRedirectRule, ServerMessage};

pub const AI_CONFIG_FILE: &str = "ai_detector_config.json";
pub const DEFAULT_AI_KEY: &str = "";
pub const DEFAULT_AI_BASE: &str = "https://ai.sumopod.com/v1";
pub const DEFAULT_AI_MODEL: &str = "glm-5.3-flash";

fn read_api_key_from_env_file() -> Option<String> {
    let candidates = [".env", "dist/Molay-Server/.env", "../.env"];
    for p in candidates {
        if let Ok(content) = std::fs::read_to_string(p) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("OPENAI_KEY=") || trimmed.starts_with("OPENAI_API_KEY=") {
                    let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                    if parts.len() == 2 {
                        let val = parts[1].trim().trim_matches('"').trim_matches('\'');
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Daftar kata kunci edukasi & sistem yang HARUS dilindungi (Anti-False-Positive Whitelist)
pub const EDU_WHITELIST: &[&str] = &[
    "google", "classroom", "drive", "docs", "sheets", "slides", "gmail",
    "wikipedia", "sekolahkartini7", "sch.id", "kemdikbud", "belajar",
    "word", "excel", "powerpnt", "powerpoint", "notepad", "calculator",
    "github", "stackoverflow", "w3schools", "canva", "figma", "geogebra",
    "quizizz", "kahoot", "edmodo", "duolingo", "scratch", "code.org",
    "zoom", "meet", "microsoft", "edge", "chrome", "firefox", "brave",
    "opera", "windows", "explorer", "cmd", "powershell", "molay",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedItem {
    pub keyword: String,
    pub target: String,
    pub category: String,
    pub reason: String,
    pub confidence: f64,
    pub detected_at: String,
    pub auto_blocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDetectorConfig {
    pub api_key: String,
    pub api_base: String,
    pub model: String,
    pub auto_block_enabled: bool,
    pub scan_interval_seconds: u64,
    #[serde(default)]
    pub total_detected_count: usize,
    #[serde(default)]
    pub last_scan_time: Option<String>,
    #[serde(default)]
    pub recent_detections: Vec<DetectedItem>,
}

impl Default for AiDetectorConfig {
    fn default() -> Self {
        Self {
            api_key: DEFAULT_AI_KEY.to_string(),
            api_base: DEFAULT_AI_BASE.to_string(),
            model: DEFAULT_AI_MODEL.to_string(),
            auto_block_enabled: true,
            scan_interval_seconds: 60,
            total_detected_count: 0,
            last_scan_time: None,
            recent_detections: Vec::new(),
        }
    }
}

pub fn resolve_ai_config_path() -> PathBuf {
    let candidates = [
        PathBuf::from(AI_CONFIG_FILE),
        PathBuf::from("dist/Molay-Server/ai_detector_config.json"),
        PathBuf::from("../dist/Molay-Server/ai_detector_config.json"),
    ];
    for c in &candidates {
        if c.exists() && c.is_file() {
            return c.clone();
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(AI_CONFIG_FILE);
            if candidate.exists() && candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from(AI_CONFIG_FILE)
}

pub fn load_or_init_ai_detector_config() -> AiDetectorConfig {
    let path = resolve_ai_config_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<AiDetectorConfig>(&data) {
            println!(
                "[AI-DETECTOR] Memuat konfigurasi AI Detector dari {} (Model: {}, Auto-Block: {})",
                path.display(),
                cfg.model,
                cfg.auto_block_enabled
            );
            return cfg;
        }
    }

    // Periksa apakah ada API key di environment variabel atau file .env
    let env_key = std::env::var("OPENAI_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok()
        .or_else(read_api_key_from_env_file)
        .unwrap_or_default();

    let mut default_cfg = AiDetectorConfig::default();
    default_cfg.api_key = env_key;

    let _ = save_ai_detector_config(&default_cfg);
    println!(
        "[AI-DETECTOR] Inisialisasi konfigurasi default AI Detector di {}",
        path.display()
    );
    default_cfg
}

pub fn save_ai_detector_config(config: &AiDetectorConfig) -> Result<(), String> {
    let path = resolve_ai_config_path();
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, &json).map_err(|e| e.to_string())?;

    // Sinkronkan ke dist/Molay-Server jika folder tersebut ada
    let dist_target = Path::new("dist/Molay-Server/ai_detector_config.json");
    if dist_target.parent().map(|p| p.exists()).unwrap_or(false) && path != dist_target {
        let _ = std::fs::write(dist_target, &json);
    }

    Ok(())
}

/// Memeriksa apakah kata kunci aman (bukan bagian dari whitelist edukasi)
pub fn is_whitelisted_educational(keyword: &str) -> bool {
    let lower = keyword.trim().to_lowercase();
    if lower.len() < 3 {
        return true;
    }
    for edu in EDU_WHITELIST {
        if lower.contains(edu) {
            return true;
        }
    }
    false
}

#[derive(Deserialize)]
struct RawDetectedItem {
    keyword: String,
    target: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    confidence: Option<f64>,
}

/// Menghubungi SumoPod GLM AI untuk mendeteksi link/game yang tidak bermanfaat
pub async fn call_ai_classifier(
    api_key: &str,
    api_base: &str,
    model: &str,
    candidate_titles: &[String],
) -> Result<Vec<DetectedItem>, String> {
    if candidate_titles.is_empty() {
        return Ok(Vec::new());
    }

    // Filter awal: abaikan judul yang jelas-jelas edukasi / sistem kosong
    let filtered_candidates: Vec<&String> = candidate_titles
        .iter()
        .filter(|t| {
            let s = t.trim();
            !s.is_empty()
                && s.len() > 3
                && !s.eq_ignore_ascii_case("new tab")
                && !s.eq_ignore_ascii_case("about:blank")
        })
        .collect();

    if filtered_candidates.is_empty() {
        return Ok(Vec::new());
    }

    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));
    let client = Client::builder()
        .timeout(Duration::from_secs(25))
        .build()
        .map_err(|e| format!("Gagal inisialisasi HTTP client: {}", e))?;

    let system_prompt = r#"Kamu adalah AI Lab Guard untuk lab komputer sekolah.
Tugasmu menganalisis daftar judul jendela atau website yang dibuka siswa untuk mendeteksi website/link yang TIDAK BERMANFAAT / DISTRAKSI / GAME selama jam pelajaran lab.
Target deteksi:
1. Web game online (misalnya: Poki, CrazyGames, Slope, Roblox, Krunker, 1v1.lol, Friv, Y8, Unblocked Games, Paper.io, Agar.io, Chess.com game santai, Subway Surfers, dll.)
2. Hiburan / Distraksi berat non-edukasi (streaming anime ilegal, streaming film bajakan, judi online, sosmed non-pembelajaran).

PENTING:
- Abaikan website pembelajaran/produktivitas: Google Classroom, Google Docs/Drive/Sheets, Wikipedia, Canva, Figma, Github, Stack Overflow, Portal Sekolah, Microsoft Word/Excel/PowerPoint.
- Buat 'keyword' sesingkat dan seefektif mungkin berupa domain atau nama game/situs unik (contoh: "slope", "krunker", "1v1.lol", "otakudesu", "poki", "roblox", "unblocked games").
- DILARANG menggunakan kata umum seperti "google", "chrome", "windows", "game", "free" sebagai keyword mandiri.

Format output WAJIB HANYA JSON array valid tanpa markdown formatting:
[
  {
    "keyword": "nama_game_atau_domain_lowercase",
    "target": "nama judul/situs",
    "category": "game" atau "distraksi",
    "reason": "alasan singkat dalam bahasa Indonesia",
    "confidence": 0.95
  }
]
Jika semua judul bersih/edukasi, kembalikan array kosong: []"#;

    let payload = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            {
                "role": "user",
                "content": format!("Analisis daftar judul jendela siswa berikut:\n{}", serde_json::to_string_pretty(&filtered_candidates).unwrap_or_default())
            }
        ],
        "temperature": 0.1
    });

    let res = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Gagal menghubungi SumoPod AI: {}", e))?;

    if !res.status().is_success() {
        let err_text = res.text().await.unwrap_or_default();
        return Err(format!("SumoPod AI mengembalikan error: {}", err_text));
    }

    let body: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("Gagal membaca respons JSON AI: {}", e))?;

    let raw_content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("[]");

    let clean_json = extract_clean_json_array(raw_content);
    let raw_items: Vec<RawDetectedItem> = serde_json::from_str(&clean_json)
        .map_err(|e| format!("Gagal parsing output AI ({}): {}", e, raw_content))?;

    let now_str = Utc::now().to_rfc3339();
    let mut results = Vec::new();

    for item in raw_items {
        let kw = item.keyword.trim().to_lowercase();
        // Validasi: tidak boleh kosong, tidak boleh whitelisted
        if kw.len() >= 3 && !is_whitelisted_educational(&kw) {
            results.push(DetectedItem {
                keyword: kw,
                target: item.target.trim().to_string(),
                category: if item.category.is_empty() {
                    "game".to_string()
                } else {
                    item.category
                },
                reason: if item.reason.is_empty() {
                    "Terdeteksi sebagai game/distraksi oleh AI".to_string()
                } else {
                    item.reason
                },
                confidence: item.confidence.unwrap_or(0.95),
                detected_at: now_str.clone(),
                auto_blocked: false,
            });
        }
    }

    Ok(results)
}

/// Menghapus markdown code blocks (```json ... ```) dari teks respons AI
pub fn extract_clean_json_array(text: &str) -> String {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find('[') {
        if let Some(end) = trimmed.rfind(']') {
            if end >= start {
                return trimmed[start..=end].to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Menggabungkan kata kunci yang terdeteksi AI ke dalam `AppPolicyConfig`
/// Mengembalikan daftar kata kunci yang BENAR-BENAR BARU ditambahkan.
pub fn apply_detected_to_policy(
    policy: &mut AppPolicyConfig,
    detected: &[DetectedItem],
) -> Vec<String> {
    if policy.browser_redirect.is_none() {
        policy.browser_redirect = Some(BrowserRedirectRule::default());
    }

    let br = policy.browser_redirect.as_mut().unwrap();
    let mut existing_set: HashSet<String> = br
        .trigger_keywords
        .iter()
        .map(|k| k.trim().to_lowercase())
        .collect();

    let mut newly_added = Vec::new();

    for item in detected {
        let kw = item.keyword.trim().to_lowercase();
        if !kw.is_empty() && !is_whitelisted_educational(&kw) && !existing_set.contains(&kw) {
            existing_set.insert(kw.clone());
            br.trigger_keywords.push(kw.clone());
            newly_added.push(kw);
        }
    }

    newly_added
}

/// Menerapkan perubahan kebijakan aplikasi dan menyebarkannya ke seluruh PC Klien & Dashboard
pub async fn broadcast_policy_update(state: &AppState, new_policy: &AppPolicyConfig) -> usize {
    // 1. Simpan ke file app_policy.json
    let _ = app_policy::save_app_policy(new_policy);

    // 2. Broadcast ke seluruh klien terhubung via WebSocket
    let senders = state.client_senders.read().await;
    let mut sent_count = 0;
    for (_cid, tx) in senders.iter() {
        let msg = ServerMessage::AppPolicyUpdate {
            policy: new_policy.clone(),
        };
        if tx.send(msg).is_ok() {
            sent_count += 1;
        }
    }

    // 3. Notifikasi dashboard browser
    let _ = state.dashboard_tx.send(
        serde_json::json!({
            "type": "PolicyUpdated",
            "policy": new_policy
        })
        .to_string(),
    );

    sent_count
}

// =========================================================================
// HANDLER REST API UNTUK DASHBOARD WEB
// =========================================================================

/// Mengambil konfigurasi & status AI Detector
pub async fn get_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store)
        .await
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let cfg = state.ai_detector.read().await.clone();
    Json(cfg).into_response()
}

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub model: Option<String>,
    pub auto_block_enabled: Option<bool>,
    pub scan_interval_seconds: Option<u64>,
}

/// Memperbarui konfigurasi AI Detector
pub async fn save_config_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(payload): Json<UpdateConfigPayload>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store)
        .await
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let mut cfg = state.ai_detector.write().await;
    if let Some(k) = payload.api_key {
        let trimmed = k.trim().to_string();
        if !trimmed.is_empty() {
            cfg.api_key = trimmed;
        }
    }
    if let Some(b) = payload.api_base {
        let trimmed = b.trim().to_string();
        if !trimmed.is_empty() {
            cfg.api_base = trimmed;
        }
    }
    if let Some(m) = payload.model {
        let trimmed = m.trim().to_string();
        if !trimmed.is_empty() {
            cfg.model = trimmed;
        }
    }
    if let Some(ab) = payload.auto_block_enabled {
        cfg.auto_block_enabled = ab;
    }
    if let Some(sec) = payload.scan_interval_seconds {
        cfg.scan_interval_seconds = sec.max(10);
    }

    if let Err(e) = save_ai_detector_config(&cfg) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Gagal menyimpan konfigurasi: {}", e) })),
        )
            .into_response();
    }

    Json(serde_json::json!({
        "status": "ok",
        "message": "Konfigurasi AI Detector berhasil diperbarui",
        "config": cfg.clone()
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct ScanPayload {
    pub custom_titles: Option<Vec<String>>,
    pub auto_block: Option<bool>,
}

/// Memindai aktivitas lab (atau daftar judul kustom) menggunakan AI
pub async fn scan_activities_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(payload): Json<ScanPayload>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store)
        .await
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let (api_key, api_base, model, auto_block_cfg) = {
        let cfg = state.ai_detector.read().await;
        (
            cfg.api_key.clone(),
            cfg.api_base.clone(),
            cfg.model.clone(),
            cfg.auto_block_enabled,
        )
    };

    let should_auto_block = payload.auto_block.unwrap_or(auto_block_cfg);

    // Kumpulkan calon judul jendela
    let mut candidates: Vec<String> = if let Some(titles) = payload.custom_titles {
        titles
    } else {
        // Ambil judul jendela terakhir dari PC klien yang terhubung
        let clients = state.clients.read().await;
        let mut list = Vec::new();
        for session in clients.values() {
            if let Some(ref win) = session.latest_window {
                let trimmed = win.trim();
                if !trimmed.is_empty() && !list.contains(&trimmed.to_string()) {
                    list.push(trimmed.to_string());
                }
            }
        }
        // Tambahkan juga riwayat aktivitas browser terbaru dari memori
        let activities = state.activities.read().await;
        for act in activities.iter().rev().take(50) {
            let app_lower = act.app_name.to_lowercase();
            if app_lower.contains("chrome")
                || app_lower.contains("edge")
                || app_lower.contains("firefox")
                || app_lower.contains("brave")
                || app_lower.contains("opera")
            {
                let win = act.window_title.trim();
                if !win.is_empty() && !list.contains(&win.to_string()) {
                    list.push(win.to_string());
                }
            }
        }
        list
    };

    // Batasi maksimum 30 judul per scan agar cepat & hemat token
    if candidates.len() > 30 {
        candidates.truncate(30);
    }

    if candidates.is_empty() {
        return Json(serde_json::json!({
            "status": "ok",
            "message": "Tidak ada judul jendela aktif untuk dianalisis.",
            "detected_count": 0,
            "detected": [],
            "newly_blocked": []
        }))
        .into_response();
    }

    // Panggil SumoPod AI
    let detected_items = match call_ai_classifier(&api_key, &api_base, &model, &candidates).await {
        Ok(items) => items,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("AI Scanner Error: {}", e) })),
            )
                .into_response();
        }
    };

    let mut newly_blocked = Vec::new();

    if should_auto_block && !detected_items.is_empty() {
        let mut policy_guard = state.app_policy.write().await;
        newly_blocked = apply_detected_to_policy(&mut policy_guard, &detected_items);
        if !newly_blocked.is_empty() {
            let updated_policy = policy_guard.clone();
            drop(policy_guard);
            let sent = broadcast_policy_update(&state, &updated_policy).await;
            println!(
                "[AI-DETECTOR] 🛡️ Auto-Block berhasil! {} kata kunci baru disiarkan ke {} PC klien: {:?}",
                newly_blocked.len(),
                sent,
                newly_blocked
            );
        }
    }

    // Perbarui riwayat deteksi di state
    {
        let mut cfg = state.ai_detector.write().await;
        cfg.last_scan_time = Some(Utc::now().to_rfc3339());
        cfg.total_detected_count += detected_items.len();

        for mut item in detected_items.clone() {
            item.auto_blocked = newly_blocked.contains(&item.keyword);
            cfg.recent_detections.insert(0, item);
        }
        if cfg.recent_detections.len() > 50 {
            cfg.recent_detections.truncate(50);
        }
        let _ = save_ai_detector_config(&cfg);
    }

    // Kirim event pemberitahuan ke Dashboard Web
    let _ = state.dashboard_tx.send(
        serde_json::json!({
            "type": "AiAutoBlockDetected",
            "detected_count": detected_items.len(),
            "newly_blocked": newly_blocked,
            "items": detected_items
        })
        .to_string(),
    );

    Json(serde_json::json!({
        "status": "ok",
        "scanned_candidates_count": candidates.len(),
        "detected_count": detected_items.len(),
        "detected": detected_items,
        "newly_blocked": newly_blocked
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct TestDetectPayload {
    pub title_or_url: String,
}

/// Menguji satu judul website atau URL dengan AI secara instan
pub async fn test_detect_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(payload): Json<TestDetectPayload>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store)
        .await
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let input = payload.title_or_url.trim().to_string();
    if input.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Judul atau URL tidak boleh kosong" })),
        )
            .into_response();
    }

    let (api_key, api_base, model) = {
        let cfg = state.ai_detector.read().await;
        (cfg.api_key.clone(), cfg.api_base.clone(), cfg.model.clone())
    };

    match call_ai_classifier(&api_key, &api_base, &model, &[input.clone()]).await {
        Ok(items) => {
            let is_detected = !items.is_empty();
            Json(serde_json::json!({
                "status": "ok",
                "input": input,
                "is_unproductive": is_detected,
                "details": items
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub struct RemoveKeywordPayload {
    pub keyword: String,
}

/// Menghapus kata kunci hasil deteksi dari daftar blokir/pengalihan
pub async fn remove_detected_keyword_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<auth::AuthQuery>,
    Json(payload): Json<RemoveKeywordPayload>,
) -> Response {
    if auth::check_dashboard_auth(&headers, query.token.as_deref(), &state.session_store)
        .await
        .is_none()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let target = payload.keyword.trim().to_lowercase();
    let mut updated_policy = None;

    {
        let mut policy_guard = state.app_policy.write().await;
        if let Some(ref mut br) = policy_guard.browser_redirect {
            let initial_len = br.trigger_keywords.len();
            br.trigger_keywords.retain(|k| k.trim().to_lowercase() != target);
            if br.trigger_keywords.len() != initial_len {
                updated_policy = Some(policy_guard.clone());
            }
        }
    }

    if let Some(policy) = updated_policy {
        let sent = broadcast_policy_update(&state, &policy).await;
        Json(serde_json::json!({
            "status": "ok",
            "message": format!("Kata kunci '{}' berhasil dihapus dan diperbarui ke {} PC klien", target, sent)
        }))
        .into_response()
    } else {
        Json(serde_json::json!({
            "status": "not_found",
            "message": format!("Kata kunci '{}' tidak ditemukan dalam daftar blokir", target)
        }))
        .into_response()
    }
}

/// Background Worker: Memindai jendela siswa yang aktif secara periodik (setiap 60s)
pub fn spawn_periodic_scanner(state: AppState) {
    tokio::spawn(async move {
        // Beri jeda 10 detik saat server pertama kali menyala
        tokio::time::sleep(Duration::from_secs(10)).await;

        loop {
            let (interval_secs, auto_block, api_key, api_base, model) = {
                let cfg = state.ai_detector.read().await;
                (
                    cfg.scan_interval_seconds.max(15),
                    cfg.auto_block_enabled,
                    cfg.api_key.clone(),
                    cfg.api_base.clone(),
                    cfg.model.clone(),
                )
            };

            if auto_block {
                // Kumpulkan jendela aktif dari klien terhubung
                let mut window_candidates = Vec::new();
                {
                    let clients = state.clients.read().await;
                    for session in clients.values() {
                        if let Some(ref win) = session.latest_window {
                            let trimmed = win.trim();
                            if !trimmed.is_empty()
                                && trimmed.len() > 3
                                && !window_candidates.contains(&trimmed.to_string())
                            {
                                window_candidates.push(trimmed.to_string());
                            }
                        }
                    }
                }

                if !window_candidates.is_empty() {
                    match call_ai_classifier(&api_key, &api_base, &model, &window_candidates).await
                    {
                        Ok(detected) => {
                            if !detected.is_empty() {
                                let newly_blocked = {
                                    let mut policy_guard = state.app_policy.write().await;
                                    let added = apply_detected_to_policy(&mut policy_guard, &detected);
                                    if !added.is_empty() {
                                        let updated = policy_guard.clone();
                                        drop(policy_guard);
                                        let sent = broadcast_policy_update(&state, &updated).await;
                                        println!(
                                            "[AI-BACKGROUND] 🛡️ Terdeteksi {} situs game baru! Otomatis diblokir ke {} PC klien: {:?}",
                                            added.len(),
                                            sent,
                                            added
                                        );
                                    }
                                    added
                                };

                                if !newly_blocked.is_empty() {
                                    let mut cfg = state.ai_detector.write().await;
                                    cfg.last_scan_time = Some(Utc::now().to_rfc3339());
                                    cfg.total_detected_count += newly_blocked.len();
                                    for mut item in detected.clone() {
                                        item.auto_blocked = newly_blocked.contains(&item.keyword);
                                        cfg.recent_detections.insert(0, item);
                                    }
                                    if cfg.recent_detections.len() > 50 {
                                        cfg.recent_detections.truncate(50);
                                    }
                                    let _ = save_ai_detector_config(&cfg);

                                    let _ = state.dashboard_tx.send(
                                        serde_json::json!({
                                            "type": "AiAutoBlockDetected",
                                            "detected_count": detected.len(),
                                            "newly_blocked": newly_blocked,
                                            "items": detected
                                        })
                                        .to_string(),
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("[AI-BACKGROUND] Peringatan: Gagal memindai jendela: {}", e);
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    });
}

// =========================================================================
// UNIT TESTS (Standar Zero Regression)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_clean_json_array() {
        let markdown_json = "Berikut adalah hasilnya:\n```json\n[\n  {\"keyword\": \"slope\", \"target\": \"Slope Game\"}\n]\n```\nSemoga membantu!";
        let clean = extract_clean_json_array(markdown_json);
        assert!(clean.starts_with('['));
        assert!(clean.ends_with(']'));
        assert!(clean.contains("\"slope\""));
    }

    #[test]
    fn test_educational_whitelist_protection() {
        // Whitelist tidak boleh lolos sebagai kata kunci blokir
        assert!(is_whitelisted_educational("Google Classroom - Kelas X"));
        assert!(is_whitelisted_educational("classroom"));
        assert!(is_whitelisted_educational("google docs"));
        assert!(is_whitelisted_educational("wikipedia"));
        assert!(is_whitelisted_educational("sekolahkartini7.sch.id"));
        assert!(is_whitelisted_educational("canva"));
        assert!(is_whitelisted_educational("Microsoft Word"));

        // Game dan distraksi TIDAK boleh di-whitelist
        assert!(!is_whitelisted_educational("krunker"));
        assert!(!is_whitelisted_educational("slope game"));
        assert!(!is_whitelisted_educational("1v1.lol"));
        assert!(!is_whitelisted_educational("otakudesu"));
        assert!(!is_whitelisted_educational("poki"));
    }

    #[test]
    fn test_apply_detected_to_policy_deduplication() {
        let mut policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![],
            blocked_keywords: vec![],
            browser_redirect: Some(BrowserRedirectRule {
                enabled: true,
                trigger_keywords: vec!["poki".to_string(), "roblox".to_string()],
                target_browser_exe: String::new(),
                target_browser_args: None,
                redirect_url: Some("https://sekolahkartini7.sch.id".to_string()),
                close_original_window: true,
                cooldown_seconds: 3,
            }),
            exemptions: Vec::new(),
        };

        let detected = vec![
            DetectedItem {
                keyword: "poki".to_string(), // Sudah ada, harus di-skip
                target: "Poki Games".to_string(),
                category: "game".to_string(),
                reason: "Game web".to_string(),
                confidence: 0.99,
                detected_at: "2026-11-05T00:00:00Z".to_string(),
                auto_blocked: false,
            },
            DetectedItem {
                keyword: "slope".to_string(), // Baru
                target: "Slope 3D".to_string(),
                category: "game".to_string(),
                reason: "Game rintangan".to_string(),
                confidence: 0.98,
                detected_at: "2026-11-05T00:00:00Z".to_string(),
                auto_blocked: false,
            },
            DetectedItem {
                keyword: "google docs".to_string(), // Edukasi, harus di-skip
                target: "Google Docs".to_string(),
                category: "edukasi".to_string(),
                reason: "Pelajaran".to_string(),
                confidence: 0.1,
                detected_at: "2026-11-05T00:00:00Z".to_string(),
                auto_blocked: false,
            },
        ];

        let newly_added = apply_detected_to_policy(&mut policy, &detected);

        // Hanya "slope" yang boleh ditambahkan
        assert_eq!(newly_added, vec!["slope"]);
        let current_keywords = &policy.browser_redirect.unwrap().trigger_keywords;
        assert_eq!(current_keywords.len(), 3);
        assert!(current_keywords.contains(&"poki".to_string()));
        assert!(current_keywords.contains(&"roblox".to_string()));
        assert!(current_keywords.contains(&"slope".to_string()));
        assert!(!current_keywords.contains(&"google docs".to_string()));
    }

    #[test]
    fn test_ai_detector_config_serialization_roundtrip() {
        let config = AiDetectorConfig {
            api_key: "sk-test-key".to_string(),
            api_base: "https://ai.sumopod.com/v1".to_string(),
            model: "glm-5.3-flash".to_string(),
            auto_block_enabled: true,
            scan_interval_seconds: 60,
            total_detected_count: 5,
            last_scan_time: Some("2026-11-05T12:00:00Z".to_string()),
            recent_detections: vec![DetectedItem {
                keyword: "1v1.lol".to_string(),
                target: "1v1.LOL".to_string(),
                category: "game".to_string(),
                reason: "Game tembak-menembak".to_string(),
                confidence: 0.99,
                detected_at: "2026-11-05T12:00:00Z".to_string(),
                auto_blocked: true,
            }],
        };

        let json = serde_json::to_string_pretty(&config).expect("Serialize AiDetectorConfig gagal");
        assert!(json.contains("\"sk-test-key\""));
        assert!(json.contains("\"1v1.lol\""));

        let deserialized: AiDetectorConfig = serde_json::from_str(&json).expect("Deserialize AiDetectorConfig gagal");
        assert_eq!(deserialized.api_key, "sk-test-key");
        assert_eq!(deserialized.total_detected_count, 5);
        assert_eq!(deserialized.recent_detections.len(), 1);
        assert_eq!(deserialized.recent_detections[0].keyword, "1v1.lol");
        assert!(deserialized.recent_detections[0].auto_blocked);
    }
}
