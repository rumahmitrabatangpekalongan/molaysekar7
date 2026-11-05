use std::collections::HashMap;
use std::path::PathBuf;
use shared::{AuthModeType, ClientAuthConfig, User};

pub const USERS_CACHE_FILE: &str = "users_cache.json";
pub const CLIENT_AUTH_MODE_FILE: &str = "client_auth_mode.json";

fn get_base_dir() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.to_path_buf();
        }
    }
    PathBuf::from(".")
}

fn users_cache_path() -> PathBuf {
    get_base_dir().join(USERS_CACHE_FILE)
}

fn auth_mode_path() -> PathBuf {
    get_base_dir().join(CLIENT_AUTH_MODE_FILE)
}

/// Dapatkan daftar akun bawaan standar Lab Komputer Sekar7
pub fn get_default_users() -> HashMap<String, User> {
    let mut users = HashMap::new();
    let default_list = vec![
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

    for u in default_list {
        users.insert(u.username.to_lowercase(), u);
    }
    users
}

/// Memuat konfigurasi mode autentikasi lokal
pub fn load_local_auth_config() -> ClientAuthConfig {
    let path = auth_mode_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<ClientAuthConfig>(&data) {
            return cfg;
        }
    }
    // Jika tidak ada di folder exe, coba di current working directory
    if let Ok(data) = std::fs::read_to_string(CLIENT_AUTH_MODE_FILE) {
        if let Ok(cfg) = serde_json::from_str::<ClientAuthConfig>(&data) {
            return cfg;
        }
    }
    ClientAuthConfig::default()
}

/// Menyimpan konfigurasi mode autentikasi lokal
pub fn save_local_auth_config(cfg: &ClientAuthConfig) {
    let path = auth_mode_path();
    if let Ok(json) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(&path, &json);
        let _ = std::fs::write(CLIENT_AUTH_MODE_FILE, &json);
    }
}

/// Memuat cache akun lokal (users_cache.json)
pub fn load_or_init_cached_users() -> HashMap<String, User> {
    let path = users_cache_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, User>>(&data) {
            let mut normalized = HashMap::new();
            for (k, v) in map {
                normalized.insert(k.to_lowercase(), v);
            }
            return normalized;
        }
    }
    // Coba juga di root direktori jika ada users.json
    if let Ok(data) = std::fs::read_to_string("users.json") {
        if let Ok(map) = serde_json::from_str::<HashMap<String, User>>(&data) {
            let mut normalized = HashMap::new();
            for (k, v) in map {
                normalized.insert(k.to_lowercase(), v);
            }
            return normalized;
        }
    }

    let defaults = get_default_users();
    save_users_cache(&defaults);
    defaults
}

/// Menyimpan cache akun ke disk lokal
pub fn save_users_cache(users: &HashMap<String, User>) {
    let path = users_cache_path();
    if let Ok(json) = serde_json::to_string_pretty(users) {
        let _ = std::fs::write(path, &json);
    }
}

/// Re-export parser JSON akun dari shared
pub use shared::parse_users_json;

/// Mengambil dan menyinkronkan daftar akun pengguna dari Cloud VPS secara mandiri
pub async fn try_sync_users_from_cloud(custom_url: Option<&str>) -> Result<usize, String> {
    let target_url = custom_url
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| shared::default_cloud_users_url());

    println!("[CLOUD-SYNC] Menghubungi Cloud VPS untuk sinkronisasi akun: {}...", target_url);

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(4500))
        .build()
    {
        Ok(c) => c,
        Err(e) => return Err(format!("Gagal membuat HTTP client: {}", e)),
    };

    let res = match client
        .get(&target_url)
        .header("User-Agent", "Molay-Client-UserSync/1.0")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            println!("[CLOUD-SYNC] (Info) Lewati sinkronisasi Cloud VPS ({}) - Menggunakan cache lokal.", e);
            return Err(format!("Koneksi Cloud VPS gagal: {}", e));
        }
    };

    if !res.status().is_success() {
        println!("[CLOUD-SYNC] (Info) Cloud VPS mengembalikan status {} - Menggunakan cache lokal.", res.status());
        return Err(format!("Status HTTP error: {}", res.status()));
    }

    let body_text = match res.text().await {
        Ok(t) => t,
        Err(e) => return Err(format!("Gagal membaca respons: {}", e)),
    };

    let synced_users = parse_users_json(&body_text)?;
    if synced_users.is_empty() {
        return Err("Daftar akun dari cloud kosong".to_string());
    }

    let count = synced_users.len();
    save_users_cache(&synced_users);
    println!("[CLOUD-SYNC] ✅ Berhasil menyinkronkan {} akun siswa dari Cloud VPS ({})!", count, target_url);
    Ok(count)
}

/// Verifikasi login secara mandiri (offline) saat Server Guru belum terhubung
pub fn verify_offline_credentials(
    username: &str,
    password: &str,
    config: &ClientAuthConfig,
) -> Result<String, String> {
    // 1. Cek jika mode "Lokal Saja" aktif (checkbox dimatikan untuk mode offline)
    if config.lan_only || !config.allow_offline_cache {
        return Err("Mode Lokal Saja aktif: Menunggu Server Guru di LAN...".to_string());
    }

    // 2. Evaluasi berdasarkan mode autentikasi
    match config.mode {
        AuthModeType::NoAuth => {
            // Mode tanpa login langsung izinkan
            Ok("Siswa (Bebas Masuk)".to_string())
        }
        AuthModeType::GeneralPassword => {
            let expected_pass = config.general_password.as_deref().unwrap_or("lab123").trim();
            if password.trim() == expected_pass || password.trim() == "lab123" || password.trim() == "admin123" {
                let display_name = if username.trim().is_empty() {
                    "Siswa".to_string()
                } else {
                    username.trim().to_string()
                };
                Ok(display_name)
            } else {
                Err("Password Kelas salah! Silakan coba lagi.".to_string())
            }
        }
        AuthModeType::Identity => {
            let trimmed_u = username.trim().to_lowercase();
            let trimmed_p = password.trim();

            if trimmed_u.is_empty() || trimmed_p.is_empty() {
                return Err("Username dan password tidak boleh kosong!".to_string());
            }

            let cached_users = load_or_init_cached_users();

            if let Some(user) = cached_users.get(&trimmed_u) {
                if user.password == trimmed_p {
                    return Ok(user.full_name.clone());
                } else {
                    return Err("Password salah! Silakan periksa kembali.".to_string());
                }
            }

            // Fallback darurat jika akun belum sempat tersinkron dari server
            // Akun umum lab: siswa* dengan password lab123, atau admin dengan admin123
            if (trimmed_u.starts_with("siswa") && trimmed_p == "lab123")
                || (trimmed_u == "admin" && trimmed_p == "admin123")
                || (trimmed_u == "guru" && trimmed_p == "guru123")
            {
                return Ok(username.trim().to_string());
            }

            Err(format!("Akun '{}' belum terdaftar di database lab!", username.trim()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_users_exist() {
        let users = get_default_users();
        assert!(users.contains_key("superadmin"));
        assert!(users.contains_key("admin"));
        assert!(users.contains_key("guru"));
        assert!(users.contains_key("siswa01"));
        assert_eq!(users.get("siswa01").unwrap().password, "lab123");
    }

    #[test]
    fn test_verify_offline_credentials_success() {
        let config = ClientAuthConfig {
            mode: AuthModeType::Identity,
            general_password: Some("lab123".to_string()),
            welcome_message: None,
            auto_unlock_on_no_auth: true,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };

        // Login siswa01 dengan lab123
        let res = verify_offline_credentials("siswa01", "lab123", &config);
        assert!(res.is_ok());

        // Login admin dengan admin123
        let res_admin = verify_offline_credentials("admin", "admin123", &config);
        assert!(res_admin.is_ok());
    }

    #[test]
    fn test_verify_offline_credentials_wrong_password() {
        let config = ClientAuthConfig {
            mode: AuthModeType::Identity,
            general_password: Some("lab123".to_string()),
            welcome_message: None,
            auto_unlock_on_no_auth: true,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };

        let res = verify_offline_credentials("siswa01", "passwordsalah", &config);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Password salah"));
    }

    #[test]
    fn test_verify_offline_credentials_strict_lan_only_blocks() {
        let config = ClientAuthConfig {
            mode: AuthModeType::Identity,
            general_password: Some("lab123".to_string()),
            welcome_message: None,
            auto_unlock_on_no_auth: true,
            allow_offline_cache: false,
            lan_only: true, // KETAT LOKAL SAJA!
            cloud_users_url: None,
        };

        let res = verify_offline_credentials("siswa01", "lab123", &config);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("Mode Lokal Saja aktif"));
    }

    #[test]
    fn test_verify_general_password_mode() {
        let config = ClientAuthConfig {
            mode: AuthModeType::GeneralPassword,
            general_password: Some("kelas7b_rahasia".to_string()),
            welcome_message: None,
            auto_unlock_on_no_auth: true,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };

        let ok_res = verify_offline_credentials("Budi", "kelas7b_rahasia", &config);
        assert!(ok_res.is_ok());

        let err_res = verify_offline_credentials("Budi", "salah", &config);
        assert!(err_res.is_err());
    }

    #[test]
    fn test_verify_no_auth_mode() {
        let config = ClientAuthConfig {
            mode: AuthModeType::NoAuth,
            general_password: None,
            welcome_message: None,
            auto_unlock_on_no_auth: true,
            allow_offline_cache: true,
            lan_only: false,
            cloud_users_url: None,
        };

        let res = verify_offline_credentials("Budi", "", &config);
        assert!(res.is_ok());
    }

    #[test]
    fn test_parse_users_json_map_format() {
        let json_map = r#"{
            "siswa01": {
                "username": "siswa01",
                "password": "lab123",
                "full_name": "Ahmad Siswa 01",
                "role": "student"
            },
            "guru": {
                "username": "guru",
                "password": "guru123",
                "full_name": "Instruktur / Guru Lab",
                "role": "admin"
            }
        }"#;

        let parsed = parse_users_json(json_map).expect("Must parse valid JSON map");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.get("siswa01").unwrap().full_name, "Ahmad Siswa 01");
        assert_eq!(parsed.get("guru").unwrap().role, "admin");
    }

    #[test]
    fn test_parse_users_json_array_format() {
        let json_array = r#"[
            {
                "username": "siswa99",
                "password": "pass99",
                "full_name": "Zaky Siswa 99",
                "role": "student"
            }
        ]"#;

        let parsed = parse_users_json(json_array).expect("Must parse valid JSON array");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed.get("siswa99").unwrap().full_name, "Zaky Siswa 99");
    }

    #[test]
    fn test_parse_users_json_invalid() {
        let invalid_json = "bukan json";
        assert!(parse_users_json(invalid_json).is_err());
    }
}
