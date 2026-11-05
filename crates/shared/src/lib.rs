use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password: String, // Plain or hashed
    pub full_name: String,
    pub role: String, // "superadmin", "admin" or "student"
}

impl User {
    pub fn is_admin_or_superadmin(&self) -> bool {
        self.role == "admin" || self.role == "superadmin"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientStatus {
    Locked,
    LoggedIn,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthModeType {
    #[serde(rename = "identity")]
    Identity, // 1. Autentikasi identitas client (username + password masing-masing)
    #[serde(rename = "no_auth")]
    NoAuth, // 2. Tanpa autentikasi (bebas masuk tanpa login)
    #[serde(rename = "general_password")]
    GeneralPassword, // 3. Autentikasi general password yang diatur oleh admin
}

impl Default for AuthModeType {
    fn default() -> Self {
        AuthModeType::Identity
    }
}

fn default_true() -> bool {
    true
}

use std::collections::HashMap;

pub fn default_cloud_users_url() -> String {
    "https://molay.sekolahkartini7.web.id/dist/users.json".to_string()
}

/// Helper untuk mem-parse JSON akun pengguna baik dalam format Map maupun Array
pub fn parse_users_json(json_str: &str) -> Result<HashMap<String, User>, String> {
    // Coba format Map: { "siswa01": { "username": "siswa01", ... } }
    if let Ok(map) = serde_json::from_str::<HashMap<String, User>>(json_str) {
        let mut normalized = HashMap::new();
        for (k, v) in map {
            normalized.insert(k.to_lowercase(), v);
        }
        return Ok(normalized);
    }

    // Coba format Array: [ { "username": "siswa01", ... }, ... ]
    if let Ok(list) = serde_json::from_str::<Vec<User>>(json_str) {
        let mut normalized = HashMap::new();
        for u in list {
            normalized.insert(u.username.to_lowercase(), u);
        }
        return Ok(normalized);
    }

    Err("Format JSON akun tidak valid (harus objek map atau array user)".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClientAuthConfig {
    pub mode: AuthModeType,
    #[serde(default)]
    pub general_password: Option<String>,
    #[serde(default)]
    pub welcome_message: Option<String>,
    #[serde(default)]
    pub auto_unlock_on_no_auth: bool,
    #[serde(default = "default_true")]
    pub allow_offline_cache: bool,
    #[serde(default)]
    pub lan_only: bool,
    #[serde(default)]
    pub cloud_users_url: Option<String>,
}

impl Default for ClientAuthConfig {
    fn default() -> Self {
        Self {
            mode: AuthModeType::Identity,
            general_password: Some("lab123".to_string()),
            welcome_message: Some("Selamat Datang di Lab Komputer".to_string()),
            auto_unlock_on_no_auth: true,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: Some(default_cloud_users_url()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct InputStatsSnapshot {
    pub total_keystrokes: u64,
    pub total_clicks: u64,
    pub mouse_distance_px: u64,
    pub idle_seconds: u32,
    pub is_afk: bool,
    #[serde(default)]
    pub keystrokes_per_min: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivityEntry {
    pub client_id: String,
    pub username: Option<String>,
    pub app_name: String,
    pub window_title: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub category: Option<String>, // "APP", "INSTALLER", "BLOCKED", "SYSTEM", "KENDALA"
    #[serde(default)]
    pub severity: Option<String>, // "info", "warning", "danger"
    #[serde(default)]
    pub details: Option<String>,  // Detail keterangan tindakan / kejadian
    #[serde(default)]
    pub keystrokes: Option<u64>,  // Jumlah ketikan saat event/jendela ini
    #[serde(default)]
    pub mouse_clicks: Option<u64>, // Jumlah klik saat event/jendela ini
    #[serde(default)]
    pub mouse_distance_px: Option<u64>, // Jarak pergerakan mouse pada event ini
    #[serde(default)]
    pub idle_seconds: Option<u32>, // Detik idle/diam
    #[serde(default)]
    pub is_afk: Option<bool>,      // Apakah terindikasi AFK
}

impl ActivityEntry {
    /// Mengisi field metadata yang kosong (backward compatibility untuk log lama)
    /// agar integrasi eksternal (Workbuddy AI) selalu menerima data konsisten
    pub fn enriched(mut self) -> Self {
        let cat = match self.category {
            Some(ref c) if !c.trim().is_empty() => c.clone(),
            _ => {
                let app = self.app_name.to_lowercase();
                let title = self.window_title.to_lowercase();
                if app.contains("molayclient") || title.contains("client locker") {
                    "SYSTEM".to_string()
                } else if app.contains("sentinel") || title.contains("hard disk") || title.contains("st1000") {
                    "SYSTEM".to_string()
                } else if app.contains("lively") {
                    "SYSTEM".to_string()
                } else if title.contains("google drive") || title.contains("classroom") || title.contains("docs") || title.contains("monitoring layar lab") {
                    "APP".to_string()
                } else {
                    "APP".to_string()
                }
            }
        };

        let sev = match self.severity {
            Some(ref s) if !s.trim().is_empty() => s.clone(),
            _ => {
                if cat == "BLOCKED" || cat == "INSTALLER" {
                    "warning".to_string()
                } else if cat == "KENDALA" {
                    "danger".to_string()
                } else {
                    "info".to_string()
                }
            }
        };

        let det = match self.details {
            Some(ref d) if !d.trim().is_empty() => Some(d.clone()),
            _ => Some(format!("Aktivitas: {} [{}]", self.app_name, self.window_title)),
        };

        self.category = Some(cat);
        self.severity = Some(sev);
        self.details = det;
        if self.keystrokes.is_none() {
            self.keystrokes = Some(0);
        }
        if self.mouse_clicks.is_none() {
            self.mouse_clicks = Some(0);
        }
        if self.mouse_distance_px.is_none() {
            self.mouse_distance_px = Some(0);
        }
        if self.idle_seconds.is_none() {
            self.idle_seconds = Some(0);
        }
        if self.is_afk.is_none() {
            self.is_afk = Some(false);
        }
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BrowserRedirectRule {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub trigger_keywords: Vec<String>,
    #[serde(default)]
    pub target_browser_exe: String,
    #[serde(default)]
    pub target_browser_args: Option<String>,
    #[serde(default)]
    pub redirect_url: Option<String>,
    #[serde(default = "default_true")]
    pub close_original_window: bool,
    #[serde(default = "default_cooldown_seconds")]
    pub cooldown_seconds: u64,
}

pub fn default_cooldown_seconds() -> u64 {
    3
}

impl Default for BrowserRedirectRule {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_keywords: vec![
                "poki.com".to_string(),
                "poki".to_string(),
                "crazygames".to_string(),
                "kbhgames".to_string(),
                "roblox.com".to_string(),
                "roblox".to_string(),
                "unblocked games".to_string(),
                "game online".to_string(),
                "y8.com".to_string(),
                "game gratis".to_string(),
            ],
            target_browser_exe: String::new(),
            target_browser_args: None,
            redirect_url: Some("https://sekolahkartini7.sch.id".to_string()),
            close_original_window: true,
            cooldown_seconds: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppPolicyConfig {
    pub block_installers: bool,
    pub alert_on_installer: bool,
    pub blocked_apps: Vec<String>,
    pub blocked_keywords: Vec<String>,
    #[serde(default)]
    pub browser_redirect: Option<BrowserRedirectRule>,
}

impl Default for AppPolicyConfig {
    fn default() -> Self {
        Self {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![
                "steam.exe".to_string(),
                "steamwebhelper.exe".to_string(),
                "epicgameslauncher.exe".to_string(),
                "robloxplayerbeta.exe".to_string(),
                "discord.exe".to_string(),
                "telegram.exe".to_string(),
                "utorrent.exe".to_string(),
                "bittorrent.exe".to_string(),
                "cheatengine.exe".to_string(),
                "genshinimpact.exe".to_string(),
                "valorant.exe".to_string(),
                "riotclientservices.exe".to_string(),
            ],
            blocked_keywords: vec![
                "Cheat Engine".to_string(),
                "Roblox".to_string(),
                "Torrent".to_string(),
                "Steam".to_string(),
            ],
            browser_redirect: Some(BrowserRedirectRule::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMessage {
    Register {
        client_id: String,
        hostname: String,
        ip: String,
        is_locked: bool,
    },
    Login {
        client_id: String,
        username: String,
        password: String,
    },
    ScreenFrame {
        client_id: String,
        width: u32,
        height: u32,
        image_base64: String, // JPEG data base64
    },
    Activity {
        client_id: String,
        app_name: String,
        window_title: String,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        severity: Option<String>,
        #[serde(default)]
        details: Option<String>,
        #[serde(default)]
        keystrokes: Option<u64>,
        #[serde(default)]
        mouse_clicks: Option<u64>,
        #[serde(default)]
        mouse_distance_px: Option<u64>,
        #[serde(default)]
        idle_seconds: Option<u32>,
        #[serde(default)]
        is_afk: Option<bool>,
        #[serde(default)]
        timestamp: Option<DateTime<Utc>>,
        #[serde(default)]
        username: Option<String>,
    },
    ActivityBatch {
        client_id: String,
        entries: Vec<ActivityEntry>,
    },
    Heartbeat {
        client_id: String,
        #[serde(default)]
        input_stats: Option<InputStatsSnapshot>,
    },
    Logout {
        client_id: String,
    },
    CheckUpdate {
        client_id: String,
        current_version: String,
    },
    ReportIssue {
        client_id: String,
        issue: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: String,
    pub download_url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub mandatory: bool,
}

/// Payload event keyboard untuk perintah remote dari Server.
/// `vk_code`: Windows Virtual Key Code (misal 0x41 = 'A', 0x0D = Enter).
/// `action`: "press" (tekan+lepas), "down" (tekan saja), "up" (lepas saja).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteKeyEvent {
    pub vk_code: u16,
    pub action: String,
}

/// Payload event mouse untuk perintah remote dari Server.
/// `x`, `y`: posisi relatif 0.0–1.0 terhadap ukuran layar klien.
/// `button`: "left", "right", "middle".
/// `action`: "move", "click", "down", "up", "double_click".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteMouseEvent {
    pub x: f32,
    pub y: f32,
    pub button: Option<String>,
    pub action: String,
    #[serde(default)]
    pub delta: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMessage {
    LoginResponse {
        success: bool,
        message: String,
        username: Option<String>,
        full_name: Option<String>,
    },
    Command {
        action: String, // "lock", "unlock", "alert", "key_press", "key_event", "mouse_event"
        payload: Option<String>,
    },
    StreamConfig {
        enabled: bool,
        fps: u32,
        quality: u8,
    },
    UpdateNotification {
        info: UpdateInfo,
    },
    AppPolicyUpdate {
        policy: AppPolicyConfig,
    },
    AuthModeUpdate {
        config: ClientAuthConfig,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientSession {
    pub client_id: String,
    pub hostname: String,
    pub ip: String,
    pub status: ClientStatus,
    pub current_user: Option<User>,
    pub last_seen: DateTime<Utc>,
    pub latest_window: Option<String>,
    pub latest_app: Option<String>,
    pub latest_screen: Option<String>, // Base64 JPEG thumbnail
    #[serde(default)]
    pub is_admin_locked: bool,
    #[serde(default)]
    pub current_issue: Option<String>,
    #[serde(default)]
    pub issue_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub input_stats: Option<InputStatsSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalErrorLog {
    #[serde(default = "generate_error_id")]
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub source: String, // "client" atau "server"
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub ip: Option<String>,
    pub error_type: String, // "ConnectionError", "AuthError", "CaptureError", "UpdateError", "SystemError", "Panic"
    pub message: String,
    #[serde(default)]
    pub details: Option<String>,
    #[serde(default)]
    pub app_version: Option<String>,
}

fn generate_error_id() -> String {
    format!("err_{}", chrono::Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_activity_entry_backward_compatibility() {
        // Data JSON lama tanpa field category, severity, details
        let legacy_json = r#"{
            "client_id": "PC-LAB-01",
            "username": "siswa01",
            "app_name": "notepad.exe",
            "window_title": "Catatan.txt - Notepad",
            "timestamp": "2026-10-27T12:00:00Z"
        }"#;

        let entry: ActivityEntry = serde_json::from_str(legacy_json).expect("Harus bisa deserialize JSON lama");
        assert_eq!(entry.client_id, "PC-LAB-01");
        assert_eq!(entry.app_name, "notepad.exe");
        assert_eq!(entry.category, None);
        assert_eq!(entry.severity, None);
        assert_eq!(entry.details, None);
    }

    #[test]
    fn test_new_activity_entry_with_metadata() {
        let entry = ActivityEntry {
            client_id: "PC-LAB-02".to_string(),
            username: Some("siswa02".to_string()),
            app_name: "setup_game.exe".to_string(),
            window_title: "Setup Wizard".to_string(),
            timestamp: Utc::now(),
            category: Some("INSTALLER".to_string()),
            severity: Some("danger".to_string()),
            details: Some("Proses instalasi diblokir".to_string()),
            keystrokes: None,
            mouse_clicks: None,
            mouse_distance_px: None,
            idle_seconds: None,
            is_afk: None,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: ActivityEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.category.as_deref(), Some("INSTALLER"));
        assert_eq!(deserialized.severity.as_deref(), Some("danger"));
        assert_eq!(deserialized.details.as_deref(), Some("Proses instalasi diblokir"));
    }

    #[test]
    fn test_legacy_client_message_activity_compatibility() {
        let legacy_msg = r#"{"type":"Activity","payload":{"client_id":"PC-01","app_name":"chrome.exe","window_title":"Google"}}"#;
        let parsed: ClientMessage = serde_json::from_str(legacy_msg).expect("Pesan client lama harus kompatibel");
        if let ClientMessage::Activity { client_id, app_name, category, timestamp, username, .. } = parsed {
            assert_eq!(client_id, "PC-01");
            assert_eq!(app_name, "chrome.exe");
            assert_eq!(category, None);
            assert_eq!(timestamp, None);
            assert_eq!(username, None);
        } else {
            panic!("Tipe pesan tidak cocok");
        }
    }

    #[test]
    fn test_client_message_activity_batch_serialization() {
        let entry = ActivityEntry {
            client_id: "PC-01".to_string(),
            username: Some("siswa01".to_string()),
            app_name: "code.exe".to_string(),
            window_title: "main.rs - VS Code".to_string(),
            timestamp: Utc::now(),
            category: Some("APP".to_string()),
            severity: Some("info".to_string()),
            details: None,
            keystrokes: Some(42),
            mouse_clicks: Some(10),
            mouse_distance_px: Some(500),
            idle_seconds: Some(0),
            is_afk: Some(false),
        };
        let msg = ClientMessage::ActivityBatch {
            client_id: "PC-01".to_string(),
            entries: vec![entry],
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"ActivityBatch\""));
        let parsed: ClientMessage = serde_json::from_str(&json).unwrap();
        if let ClientMessage::ActivityBatch { client_id, entries } = parsed {
            assert_eq!(client_id, "PC-01");
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].app_name, "code.exe");
            assert_eq!(entries[0].username.as_deref(), Some("siswa01"));
        } else {
            panic!("Expected ActivityBatch");
        }
    }

    #[test]
    fn test_client_auth_config_modes_serialization() {
        // Test 1: Identity mode
        let config_identity = ClientAuthConfig::default();
        assert_eq!(config_identity.mode, AuthModeType::Identity);
        let json_id = serde_json::to_string(&config_identity).unwrap();
        assert!(json_id.contains("\"mode\":\"identity\""));
        let de_id: ClientAuthConfig = serde_json::from_str(&json_id).unwrap();
        assert_eq!(de_id.mode, AuthModeType::Identity);

        // Test 2: NoAuth mode
        let config_no_auth = ClientAuthConfig {
            mode: AuthModeType::NoAuth,
            general_password: None,
            welcome_message: Some("Bebas Masuk".to_string()),
            auto_unlock_on_no_auth: true,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };
        let json_no_auth = serde_json::to_string(&config_no_auth).unwrap();
        assert!(json_no_auth.contains("\"mode\":\"no_auth\""));
        let de_no_auth: ClientAuthConfig = serde_json::from_str(&json_no_auth).unwrap();
        assert_eq!(de_no_auth.mode, AuthModeType::NoAuth);

        // Test 3: GeneralPassword mode
        let config_gen = ClientAuthConfig {
            mode: AuthModeType::GeneralPassword,
            general_password: Some("kelas7a_2026".to_string()),
            welcome_message: Some("Selamat Datang".to_string()),
            auto_unlock_on_no_auth: false,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };
        let json_gen = serde_json::to_string(&config_gen).unwrap();
        assert!(json_gen.contains("\"mode\":\"general_password\""));
        assert!(json_gen.contains("kelas7a_2026"));
        let de_gen: ClientAuthConfig = serde_json::from_str(&json_gen).unwrap();
        assert_eq!(de_gen.mode, AuthModeType::GeneralPassword);
        assert_eq!(de_gen.general_password.as_deref(), Some("kelas7a_2026"));
    }

    #[test]
    fn test_server_message_auth_mode_update() {
        let msg = ServerMessage::AuthModeUpdate {
            config: ClientAuthConfig {
                mode: AuthModeType::GeneralPassword,
                general_password: Some("sekar123".to_string()),
                welcome_message: None,
                auto_unlock_on_no_auth: true,
                allow_offline_cache: true,
                lan_only: false,
                cloud_users_url: None,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let de: ServerMessage = serde_json::from_str(&json).unwrap();
        if let ServerMessage::AuthModeUpdate { config } = de {
            assert_eq!(config.mode, AuthModeType::GeneralPassword);
            assert_eq!(config.general_password.as_deref(), Some("sekar123"));
            assert_eq!(config.allow_offline_cache, true);
            assert_eq!(config.lan_only, false);
        } else {
            panic!("Expected AuthModeUpdate variant");
        }
    }

    #[test]
    fn test_client_auth_config_backward_compatibility() {
        // JSON format lama tanpa field allow_offline_cache, lan_only, dan cloud_users_url
        let legacy_json = r#"{"mode":"identity","general_password":"lab123","welcome_message":"Halo Lab","auto_unlock_on_no_auth":true}"#;
        let cfg: ClientAuthConfig = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(cfg.mode, AuthModeType::Identity);
        assert_eq!(cfg.allow_offline_cache, true); // Harus default true!
        assert_eq!(cfg.lan_only, false);          // Harus default false!
        assert_eq!(cfg.cloud_users_url, None);    // Harus None pada JSON legacy!

        // Default struct harus menyediakan URL default resmi
        let def_cfg = ClientAuthConfig::default();
        assert_eq!(def_cfg.cloud_users_url.as_deref(), Some("https://molay.sekolahkartini7.web.id/dist/users.json"));
    }

    #[test]
    fn test_heartbeat_backward_compatibility() {
        // Heartbeat lama tanpa input_stats
        let legacy_hb = r#"{"type":"Heartbeat","payload":{"client_id":"PC-LAB-01"}}"#;
        let de: ClientMessage = serde_json::from_str(legacy_hb).unwrap();
        if let ClientMessage::Heartbeat { client_id, input_stats } = de {
            assert_eq!(client_id, "PC-LAB-01");
            assert_eq!(input_stats, None);
        } else {
            panic!("Expected Heartbeat variant");
        }

        // Heartbeat baru dengan input_stats
        let new_hb = ClientMessage::Heartbeat {
            client_id: "PC-LAB-02".to_string(),
            input_stats: Some(InputStatsSnapshot {
                total_keystrokes: 1250,
                total_clicks: 450,
                mouse_distance_px: 89000,
                idle_seconds: 12,
                is_afk: false,
                keystrokes_per_min: 85,
            }),
        };
        let json = serde_json::to_string(&new_hb).unwrap();
        let de_new: ClientMessage = serde_json::from_str(&json).unwrap();
        if let ClientMessage::Heartbeat { client_id, input_stats } = de_new {
            assert_eq!(client_id, "PC-LAB-02");
            let s = input_stats.expect("input_stats should exist");
            assert_eq!(s.total_keystrokes, 1250);
            assert_eq!(s.total_clicks, 450);
            assert_eq!(s.keystrokes_per_min, 85);
            assert_eq!(s.is_afk, false);
        } else {
            panic!("Expected Heartbeat variant");
        }
    }

    #[test]
    fn test_activity_entry_enriched_fills_defaults() {
        let legacy = ActivityEntry {
            client_id: "PC5LAB2".to_string(),
            username: Some("Ahmad Siswa 01".to_string()),
            app_name: "MolayClient-Sekar7.exe".to_string(),
            window_title: "Molay Sekar7 - Client Locker".to_string(),
            timestamp: Utc::now(),
            category: None,
            severity: None,
            details: None,
            keystrokes: None,
            mouse_clicks: None,
            mouse_distance_px: None,
            idle_seconds: None,
            is_afk: None,
        };

        let enriched = legacy.enriched();
        assert_eq!(enriched.category.as_deref(), Some("SYSTEM"));
        assert_eq!(enriched.severity.as_deref(), Some("info"));
        assert!(enriched.details.is_some());
        assert_eq!(enriched.keystrokes, Some(0));
        assert_eq!(enriched.mouse_clicks, Some(0));
        assert_eq!(enriched.mouse_distance_px, Some(0));
        assert_eq!(enriched.idle_seconds, Some(0));
        assert_eq!(enriched.is_afk, Some(false));
    }

    #[test]
    fn test_app_policy_config_backward_compatibility() {
        // Legacy JSON format without browser_redirect field
        let legacy_json = r#"{
            "block_installers": true,
            "alert_on_installer": true,
            "blocked_apps": ["steam.exe"],
            "blocked_keywords": ["Roblox"]
        }"#;

        let policy: AppPolicyConfig = serde_json::from_str(legacy_json).expect("Harus kompatibel dengan format lama");
        assert!(policy.block_installers);
        assert_eq!(policy.blocked_apps.len(), 1);
        assert_eq!(policy.browser_redirect, None);

        // New JSON format with browser_redirect
        let rule = BrowserRedirectRule {
            enabled: true,
            trigger_keywords: vec!["poki".to_string()],
            target_browser_exe: "C:\\Browser\\browser.exe".to_string(),
            target_browser_args: None,
            redirect_url: Some("https://sekolahkartini7.sch.id".to_string()),
            close_original_window: true,
            cooldown_seconds: 5,
        };
        let new_policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec!["steam.exe".to_string()],
            blocked_keywords: vec!["Roblox".to_string()],
            browser_redirect: Some(rule),
        };
        let json = serde_json::to_string(&new_policy).unwrap();
        assert!(json.contains("\"browser_redirect\""));
        assert!(json.contains("\"poki\""));
        let parsed: AppPolicyConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.browser_redirect.is_some());
        assert!(parsed.browser_redirect.unwrap().enabled);
    }
}
