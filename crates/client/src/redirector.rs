//! Modul Pengalihan Browser Otomatis (Browser & Web Game Redirector)
//!
//! Mendeteksi jika siswa membuka web game (Poki, CrazyGames, Roblox, link tertentu)
//! di browser manapun (Chrome, Edge, Firefox, dll.), menutup tab/jendela tersebut,
//! dan mengalihkannya ke browser khusus yang dipasang oleh Guru/Admin.

use shared::{AppPolicyConfig, BrowserRedirectRule};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

pub const BROWSER_REDIRECT_FILE: &str = "browser_redirect.json";

static LAST_REDIRECT_TIME: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_TRIGGERED_KEYWORD: Mutex<Option<String>> = Mutex::new(None);

fn get_local_config_path() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join(BROWSER_REDIRECT_FILE);
        }
    }
    PathBuf::from(BROWSER_REDIRECT_FILE)
}

/// Memuat konfigurasi pengalihan browser lokal jika ada
pub fn load_local_redirect_config() -> Option<BrowserRedirectRule> {
    let path = get_local_config_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(rule) = serde_json::from_str::<BrowserRedirectRule>(&content) {
            return Some(rule);
        }
    }
    None
}

/// Menyimpan konfigurasi pengalihan browser lokal
#[allow(dead_code)]
pub fn save_local_redirect_config(rule: &BrowserRedirectRule) -> Result<(), String> {
    let path = get_local_config_path();
    let json = serde_json::to_string_pretty(rule).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Memeriksa apakah teks judul atau nama aplikasi mengandung salah satu kata kunci trigger
pub fn matches_trigger_keyword(
    app_name: &str,
    window_title: &str,
    keywords: &[String],
) -> Option<String> {
    let lower_title = window_title.to_lowercase();
    let lower_app = app_name.to_lowercase();

    for kw in keywords {
        let clean_kw = kw.trim().to_lowercase();
        if clean_kw.is_empty() {
            continue;
        }

        if lower_title.contains(&clean_kw) || lower_app.contains(&clean_kw) {
            return Some(kw.clone());
        }
    }

    None
}

/// Memeriksa apakah jendela aktif saat ini perlu dialihkan ke browser khusus.
/// Mengembalikan `Some(keterangan)` jika pengalihan dieksekusi.
pub fn check_and_execute_redirect(
    app_name: &str,
    window_title: &str,
    hwnd: isize,
    policy: &AppPolicyConfig,
) -> Option<String> {
    // 1. Tentukan aturan aktif: dari Server Policy atau fallback file lokal
    let active_rule = if let Some(ref r) = policy.browser_redirect {
        r.clone()
    } else if let Some(local_r) = load_local_redirect_config() {
        local_r
    } else {
        return None;
    };

    if !active_rule.enabled {
        return None;
    }

    // Abaikan jika jendela aktif saat ini adalah website tujuan pengalihan itu sendiri
    if let Some(ref target_url) = active_rule.redirect_url {
        let clean_target = target_url
            .trim_start_matches("https://")
            .trim_start_matches("http://");
        let host = clean_target.split('/').next().unwrap_or("").trim().to_lowercase();
        if !host.is_empty() && window_title.to_lowercase().contains(&host) {
            return None;
        }
    }

    // 2. Cek apakah judul jendela atau nama aplikasi cocok dengan kata kunci game/link
    let matched_kw = match matches_trigger_keyword(app_name, window_title, &active_rule.trigger_keywords) {
        Some(kw) => kw,
        None => return None,
    };

    // 3. Cek Cooldown agar tidak meluncurkan browser berkali-kali dalam hitungan detik
    let cooldown = Duration::from_secs(active_rule.cooldown_seconds.max(1));
    {
        let mut last_time = LAST_REDIRECT_TIME.lock().unwrap();
        if let Some(prev) = *last_time {
            if prev.elapsed() < cooldown {
                return None;
            }
        }
        *last_time = Some(Instant::now());
    }

    {
        let mut last_kw = LAST_TRIGGERED_KEYWORD.lock().unwrap();
        *last_kw = Some(matched_kw.clone());
    }

    println!(
        "[REDIRECTOR] 🎯 Terdeteksi link/web game '{}' di {} [{}]. Memulai pengalihan...",
        matched_kw, app_name, window_title
    );

    // 4. Tutup tab/jendela browser lama jika diaktifkan
    if active_rule.close_original_window && hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0);
        }
        println!("[REDIRECTOR] ⏹ Mengirim sinyal WM_CLOSE ke jendela asal (HWND: {})", hwnd);
    }

    // 5. Jalankan Browser Khusus
    launch_target_browser(&active_rule);

    Some(format!(
        "Terdeteksi akses link game web '{}' di {} - Otomatis dialihkan ke browser khusus",
        matched_kw, app_name
    ))
}

/// Meluncurkan browser khusus yang ditentukan guru
fn launch_target_browser(rule: &BrowserRedirectRule) {
    let custom_exe = rule.target_browser_exe.trim();
    let redirect_url = rule.redirect_url.as_deref().unwrap_or("about:blank");

    if !custom_exe.is_empty() && Path::new(custom_exe).exists() {
        let mut cmd = std::process::Command::new(custom_exe);
        if let Some(ref args_str) = rule.target_browser_args {
            for arg in args_str.split_whitespace() {
                cmd.arg(arg);
            }
        }
        if !redirect_url.is_empty() && redirect_url != "about:blank" {
            cmd.arg(redirect_url);
        }

        match cmd.spawn() {
            Ok(_) => println!("[REDIRECTOR] ✅ Berhasil membuka browser khusus: {}", custom_exe),
            Err(e) => eprintln!("[REDIRECTOR] ❌ Gagal membuka browser khusus ({}): {}", custom_exe, e),
        }
    } else {
        // Fallback: Jika path browser khusus kosong/belum dipasang, buka URL menggunakan default browser / Edge
        println!(
            "[REDIRECTOR] Browser khusus belum disetel/tidak ditemukan. Membuka URL pengalihan di browser sistem..."
        );
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", redirect_url])
            .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_trigger_keyword_found() {
        let keywords = vec![
            "poki".to_string(),
            "crazygames".to_string(),
            "roblox".to_string(),
        ];

        let res = matches_trigger_keyword("chrome.exe", "Poki - Main Game Gratis Online - Google Chrome", &keywords);
        assert_eq!(res.as_deref(), Some("poki"));

        let res2 = matches_trigger_keyword("msedge.exe", "CrazyGames: Free Online Games - Microsoft Edge", &keywords);
        assert_eq!(res2.as_deref(), Some("crazygames"));
    }

    #[test]
    fn test_matches_trigger_keyword_not_found() {
        let keywords = vec!["poki.com".to_string(), "roblox".to_string()];

        let res = matches_trigger_keyword("winword.exe", "Laporan Praktikum.docx - Word", &keywords);
        assert_eq!(res, None);

        let res2 = matches_trigger_keyword("chrome.exe", "Google Docs - Catatan Pelajaran", &keywords);
        assert_eq!(res2, None);
    }

    #[test]
    fn test_disabled_policy_does_not_redirect() {
        let policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![],
            blocked_keywords: vec![],
            browser_redirect: Some(BrowserRedirectRule {
                enabled: false,
                trigger_keywords: vec!["poki".to_string()],
                target_browser_exe: String::new(),
                target_browser_args: None,
                redirect_url: None,
                close_original_window: true,
                cooldown_seconds: 3,
            }),
        };

        let res = check_and_execute_redirect("chrome.exe", "Poki Games", 0, &policy);
        assert_eq!(res, None);
    }

    #[test]
    fn test_target_url_window_does_not_loop_redirect() {
        let policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![],
            blocked_keywords: vec![],
            browser_redirect: Some(BrowserRedirectRule {
                enabled: true,
                trigger_keywords: vec!["sekolah".to_string()],
                target_browser_exe: String::new(),
                target_browser_args: None,
                redirect_url: Some("https://sekolahkartini7.sch.id".to_string()),
                close_original_window: true,
                cooldown_seconds: 3,
            }),
        };

        let res = check_and_execute_redirect("chrome.exe", "Portal Resmi - sekolahkartini7.sch.id - Google Chrome", 0, &policy);
        assert_eq!(res, None);
    }
}
