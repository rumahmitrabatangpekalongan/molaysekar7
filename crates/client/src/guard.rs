use shared::AppPolicyConfig;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Diagnostics::ToolHelp::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

static RECENT_KILLED_PIDS: Mutex<Option<HashSet<u32>>> = Mutex::new(None);
static LAST_CLEANUP: Mutex<Option<Instant>> = Mutex::new(None);

pub fn is_installer(app_name: &str, window_title: &str) -> bool {
    let lower_app = app_name.to_lowercase();
    let lower_title = window_title.to_lowercase();

    // Abaikan aplikasi Molay sendiri dan tool development umum jika ada
    if lower_app.contains("molay") || lower_app.contains("cargo") || lower_app.contains("rustc") {
        return false;
    }

    // Deteksi nama binary installer
    let is_installer_exe = lower_app == "setup.exe"
        || lower_app == "installer.exe"
        || lower_app == "install.exe"
        || lower_app == "msiexec.exe"
        || lower_app.contains("setup_")
        || lower_app.contains("installer_")
        || lower_app.ends_with("-setup.exe")
        || lower_app.ends_with("_setup.exe")
        || lower_app.ends_with("-installer.exe")
        || lower_app.ends_with("_installer.exe")
        || lower_app.contains("inno")
        || lower_app.contains("nsis");

    // Deteksi judul jendela instalasi
    let is_installer_title = lower_title.contains("setup wizard")
        || lower_title.contains("installation wizard")
        || lower_title.contains("install wizard")
        || lower_title.contains("setup - ")
        || lower_title.contains("wisaya penyiapan")
        || lower_title.contains("wisaya instalasi")
        || lower_title.contains("pemasangan aplikasi");

    is_installer_exe || is_installer_title
}

pub fn check_blocked_app(
    app_name: &str,
    window_title: &str,
    policy: &AppPolicyConfig,
) -> Option<String> {
    let lower_app = app_name.to_lowercase();
    let lower_title = window_title.to_lowercase();

    // Jangan pernah blokir aplikasi sistem inti atau Molay
    if lower_app.contains("molay")
        || lower_app == "explorer.exe"
        || lower_app == "csrss.exe"
        || lower_app == "dwm.exe"
        || lower_app == "system"
        || lower_app == "svchost.exe"
    {
        return None;
    }

    // Cek daftar executable yang dilarang
    for blocked in &policy.blocked_apps {
        let b = blocked.trim().to_lowercase();
        if !b.is_empty() && (lower_app == b || lower_app.starts_with(&b)) {
            return Some(format!("Aplikasi '{}' masuk dalam daftar terlarang Lab", blocked));
        }
    }

    // Cek kata kunci di judul jendela yang dilarang
    for keyword in &policy.blocked_keywords {
        let kw = keyword.trim().to_lowercase();
        if !kw.is_empty() && lower_title.contains(&kw) {
            return Some(format!("Judul jendela terdeteksi mengandung kata kunci terlarang: '{}'", keyword));
        }
    }

    // Cek apakah installer dilarang
    if policy.block_installers && is_installer(app_name, window_title) {
        return Some(format!("Eksekusi file instalasi software ('{}') dilarang oleh Admin Lab", app_name));
    }

    None
}

pub fn kill_process_by_pid(pid: u32) -> bool {
    // Jangan pernah kill PID 0 atau 4 (System)
    if pid <= 4 {
        return false;
    }

    // Hindari membunuh proses diri sendiri
    unsafe {
        if pid == GetCurrentProcessId() {
            return false;
        }

        let h_proc = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h_proc != 0 {
            let res = TerminateProcess(h_proc, 1);
            CloseHandle(h_proc);
            res != 0
        } else {
            false
        }
    }
}

pub fn show_blocked_alert_async(app_name: String, reason: String) {
    std::thread::spawn(move || {
        let title_wide: Vec<u16> = "MOLAY SEKAR7 - KEBIJAKAN LAB\0".encode_utf16().collect();
        let message = format!(
            "AKSES DITOLAK OLEH SISTEM!\n\n\
            Aplikasi : {}\n\
            Alasan   : {}\n\n\
            Aplikasi telah dihentikan secara otomatis sesuai kebijakan Guru / Administrator Lab Komputer.",
            app_name, reason
        );
        let msg_wide: Vec<u16> = format!("{}\0", message).encode_utf16().collect();

        unsafe {
            MessageBoxW(
                0,
                msg_wide.as_ptr(),
                title_wide.as_ptr(),
                MB_OK | MB_ICONSTOP | MB_TOPMOST | MB_SETFOREGROUND,
            );
        }
    });
}

pub struct Violation {
    pub pid: u32,
    pub app_name: String,
    pub reason: String,
    pub is_installer: bool,
}

pub fn scan_and_enforce_policy(policy: &AppPolicyConfig) -> Vec<Violation> {
    let mut violations = Vec::new();

    // Bersihkan cache PID lama setiap 30 detik
    {
        let mut cleanup_guard = LAST_CLEANUP.lock().unwrap();
        let should_clean = match *cleanup_guard {
            None => true,
            Some(t) => t.elapsed() > Duration::from_secs(30),
        };
        if should_clean {
            *cleanup_guard = Some(Instant::now());
            let mut set_guard = RECENT_KILLED_PIDS.lock().unwrap();
            *set_guard = Some(HashSet::new());
        }
    }

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return violations;
        }

        let mut pe: PROCESSENTRY32W = std::mem::zeroed();
        pe.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut pe) != 0 {
            loop {
                let end = pe
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(pe.szExeFile.len());
                let exe_name = String::from_utf16_lossy(&pe.szExeFile[..end]);
                let pid = pe.th32ProcessID;

                let is_inst = is_installer(&exe_name, "");
                let check_result = check_blocked_app(&exe_name, "", policy);

                if let Some(reason) = check_result {
                    // Cek apakah baru saja kita kill agar tidak spam log per detik
                    let already_handled = {
                        let mut set_guard = RECENT_KILLED_PIDS.lock().unwrap();
                        let set = set_guard.get_or_insert_with(HashSet::new);
                        if set.contains(&pid) {
                            true
                        } else {
                            set.insert(pid);
                            false
                        }
                    };

                    if !already_handled {
                        let killed = kill_process_by_pid(pid);
                        if killed {
                            println!("[GUARD] Berhasil memblokir & menutup aplikasi: {} (PID: {}) - {}", exe_name, pid, reason);
                            show_blocked_alert_async(exe_name.clone(), reason.clone());
                        }

                        violations.push(Violation {
                            pid,
                            app_name: exe_name,
                            reason,
                            is_installer: is_inst,
                        });
                    }
                }

                if Process32NextW(snapshot, &mut pe) == 0 {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_installer_detection() {
        assert!(is_installer("setup.exe", ""));
        assert!(is_installer("installer.exe", ""));
        assert!(is_installer("msiexec.exe", ""));
        assert!(is_installer("game_setup.exe", ""));
        assert!(is_installer("software-installer.exe", ""));
        assert!(is_installer("chrome.exe", "Pemasangan Aplikasi Google Chrome"));
        assert!(is_installer("app.exe", "Setup Wizard - MyGame"));

        // Aplikasi normal tidak boleh dianggap installer
        assert!(!is_installer("notepad.exe", "Catatan.txt - Notepad"));
        assert!(!is_installer("word.exe", "Document1 - Word"));
        assert!(!is_installer("MolayClient-Sekar7.exe", "Molay Client"));
        assert!(!is_installer("explorer.exe", "File Explorer"));
    }

    #[test]
    fn test_blocked_app_detection() {
        let policy = AppPolicyConfig {
            block_installers: true,
            alert_on_installer: true,
            blocked_apps: vec!["roblox.exe".into(), "steam.exe".into()],
            blocked_keywords: vec!["Cheat Engine".into(), "Roblox".into()],
            browser_redirect: None,
        };

        // Harus terblokir jika nama proses sama
        assert!(check_blocked_app("roblox.exe", "Game", &policy).is_some());
        assert!(check_blocked_app("steam.exe", "Steam Store", &policy).is_some());

        // Harus terblokir jika judul mengandung kata kunci
        assert!(check_blocked_app("cheat_tool.exe", "Cheat Engine v7.4", &policy).is_some());

        // Harus terblokir jika installer
        assert!(check_blocked_app("setup.exe", "Setup Wizard", &policy).is_some());

        // Aplikasi yang diizinkan tidak boleh terblokir
        assert!(check_blocked_app("notepad.exe", "Tugas Sekolah.txt", &policy).is_none());
        assert!(check_blocked_app("code.exe", "Visual Studio Code", &policy).is_none());
        assert!(check_blocked_app("explorer.exe", "Desktop", &policy).is_none());
    }
}
