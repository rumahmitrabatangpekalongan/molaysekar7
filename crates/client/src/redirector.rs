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
use windows_sys::Win32::UI::WindowsAndMessaging::{BringWindowToTop, PostMessageW, SetForegroundWindow, WM_CLOSE};

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

    // 2b. Periksa apakah jendela atau kata kunci masuk dalam pengecualian aktif (Exemption / Whitelist)
    if policy.is_exempt(app_name, window_title) || policy.is_keyword_exempt(&matched_kw) {
        return None;
    }

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

    let raw_url = active_rule.redirect_url.as_deref().unwrap_or("about:blank");
    let target_url = normalize_redirect_url(raw_url);
    let custom_exe = active_rule.target_browser_exe.trim();
    let is_browser = is_supported_browser(app_name);

    // 4. In-Place Tab Redirect:
    // Jika tidak disetel browser khusus dan aplikasi adalah browser web (Chrome, Edge, Firefox, dll),
    // langsung ganti alamat di tab aktif tanpa menutup jendela dan tanpa membuat tab baru!
    if custom_exe.is_empty() && is_browser && hwnd != 0 {
        if navigate_active_tab(hwnd, &target_url) {
            return Some(format!(
                "Terdeteksi akses link game web '{}' di {} - Alamat tab langsung dialihkan ke website tujuan",
                matched_kw, app_name
            ));
        }
    }

    // 5. Fallback: Untuk aplikasi non-browser (misal game desktop Roblox) atau browser khusus yang disetel manual
    if active_rule.close_original_window && hwnd != 0 {
        unsafe {
            PostMessageW(hwnd as HWND, WM_CLOSE, 0, 0);
        }
        println!("[REDIRECTOR] ⏹ Mengirim sinyal WM_CLOSE ke jendela asal (HWND: {})", hwnd);
    }

    // Jalankan Browser Khusus / Browser Sistem
    launch_target_browser(&active_rule);

    let dest_info = if !custom_exe.is_empty() {
        "ke browser khusus"
    } else {
        "ke browser sistem"
    };

    Some(format!(
        "Terdeteksi akses link game web '{}' di {} - Otomatis dialihkan {}",
        matched_kw, app_name, dest_info
    ))
}

/// Memeriksa apakah nama aplikasi merupakan salah satu browser web populer
pub fn is_supported_browser(app_name: &str) -> bool {
    let lower = app_name.to_lowercase();
    lower.contains("chrome")
        || lower.contains("msedge")
        || lower.contains("firefox")
        || lower.contains("brave")
        || lower.contains("opera")
        || lower.contains("vivaldi")
}

/// Melakukan navigasi in-place pada tab browser yang sedang aktif
/// dengan memfokuskan jendela, menekan Ctrl+L untuk memilih address bar,
/// memasukkan URL tujuan, lalu menekan Enter.
pub fn navigate_active_tab(hwnd: isize, url: &str) -> bool {
    if hwnd == 0 {
        return false;
    }

    unsafe {
        SetForegroundWindow(hwnd as HWND);
        BringWindowToTop(hwnd as HWND);
    }
    std::thread::sleep(std::time::Duration::from_millis(35));

    // 1. Kirim Ctrl + L untuk fokus dan menyorot seluruh address bar browser
    const VK_CONTROL: u16 = 0x11;
    const VK_L: u16 = 0x4C;
    const VK_RETURN: u16 = 0x0D;

    crate::remote_input::send_key_event(VK_CONTROL, false);
    std::thread::sleep(std::time::Duration::from_millis(15));
    crate::remote_input::send_key_event(VK_L, false);
    std::thread::sleep(std::time::Duration::from_millis(15));
    crate::remote_input::send_key_event(VK_L, true);
    std::thread::sleep(std::time::Duration::from_millis(15));
    crate::remote_input::send_key_event(VK_CONTROL, true);
    std::thread::sleep(std::time::Duration::from_millis(35));

    // 2. Ketik URL tujuan langsung ke address bar
    crate::remote_input::send_text(url);
    std::thread::sleep(std::time::Duration::from_millis(35));

    // 3. Tekan Enter untuk navigasi di tab yang sama
    crate::remote_input::send_key_press(VK_RETURN);

    println!("[REDIRECTOR] 🔄 Alamat tab aktif berhasil diarahkan ke: {}", url);
    true
}

/// Memastikan URL memiliki skema http:// atau https:// agar dikenali oleh Windows Shell
pub fn normalize_redirect_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("about:blank") {
        return "about:blank".to_string();
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("https://{}", trimmed)
    }
}

/// Meluncurkan browser khusus yang ditentukan guru
fn launch_target_browser(rule: &BrowserRedirectRule) {
    let custom_exe = rule.target_browser_exe.trim();
    let raw_url = rule.redirect_url.as_deref().unwrap_or("about:blank");
    let redirect_url = normalize_redirect_url(raw_url);

    if !custom_exe.is_empty() && Path::new(custom_exe).exists() {
        let mut cmd = std::process::Command::new(custom_exe);
        if let Some(ref args_str) = rule.target_browser_args {
            for arg in args_str.split_whitespace() {
                cmd.arg(arg);
            }
        }
        if !redirect_url.is_empty() && redirect_url != "about:blank" {
            cmd.arg(&redirect_url);
        }

        match cmd.spawn() {
            Ok(_) => println!("[REDIRECTOR] ✅ Berhasil membuka browser khusus: {}", custom_exe),
            Err(e) => eprintln!("[REDIRECTOR] ❌ Gagal membuka browser khusus ({}): {}", custom_exe, e),
        }
    } else {
        // Fallback: Jika path browser khusus kosong/belum dipasang, buka URL menggunakan default browser / Edge
        println!(
            "[REDIRECTOR] Browser khusus belum disetel/tidak ditemukan. Membuka URL pengalihan di browser sistem: {}",
            redirect_url
        );
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &redirect_url])
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
            exemptions: Vec::new(),
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
            exemptions: Vec::new(),
        };

        let res = check_and_execute_redirect("chrome.exe", "Portal Resmi - sekolahkartini7.sch.id - Google Chrome", 0, &policy);
        assert_eq!(res, None);
    }

    #[test]
    fn test_exemption_bypasses_redirect() {
        let mut policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec![],
            blocked_keywords: vec![],
            browser_redirect: Some(BrowserRedirectRule {
                enabled: true,
                trigger_keywords: vec!["crazygames".to_string()],
                target_browser_exe: String::new(),
                target_browser_args: None,
                redirect_url: Some("https://sekolahkartini7.sch.id".to_string()),
                close_original_window: false,
                cooldown_seconds: 0,
            }),
            exemptions: Vec::new(),
        };

        // Tanpa pengecualian -> dialihkan
        let res1 = check_and_execute_redirect("chrome.exe", "Play on Crazygames now", 0, &policy);
        assert!(res1.is_some());

        // Tambah pengecualian aktif untuk crazygames
        policy.exemptions.push(shared::PolicyExemption {
            id: "ex-cg".to_string(),
            target: "crazygames".to_string(),
            exemption_type: "permanent".to_string(),
            expires_at: None,
            created_at: "2026-11-05T00:00:00Z".to_string(),
            reason: "Uji Coba Lab".to_string(),
        });

        // Dengan pengecualian aktif -> TIDAK dialihkan (None)
        let res2 = check_and_execute_redirect("chrome.exe", "Play on Crazygames now", 0, &policy);
        assert_eq!(res2, None);
    }

    #[test]
    fn test_normalize_redirect_url() {
        assert_eq!(normalize_redirect_url("smpn6pekalongan.sch.id"), "https://smpn6pekalongan.sch.id");
        assert_eq!(normalize_redirect_url("https://smpn6pekalongan.sch.id"), "https://smpn6pekalongan.sch.id");
        assert_eq!(normalize_redirect_url("http://smpn6pekalongan.sch.id"), "http://smpn6pekalongan.sch.id");
        assert_eq!(normalize_redirect_url(""), "about:blank");
        assert_eq!(normalize_redirect_url("about:blank"), "about:blank");
    }

    #[test]
    fn test_is_supported_browser() {
        assert!(is_supported_browser("chrome.exe"));
        assert!(is_supported_browser("msedge.exe"));
        assert!(is_supported_browser("firefox.exe"));
        assert!(is_supported_browser("brave.exe"));
        assert!(is_supported_browser("opera.exe"));
        assert!(is_supported_browser("vivaldi.exe"));
        assert!(!is_supported_browser("RobloxPlayerBeta.exe"));
        assert!(!is_supported_browser("notepad.exe"));
    }
}
