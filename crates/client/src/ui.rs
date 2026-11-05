use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc::UnboundedSender;
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub enum UiCommand {
    ShowLock,
    HideLock,
    SetStatus(String),
    SetAuthMode(shared::ClientAuthConfig),
}

pub enum UiEvent {
    LoginSubmit {
        username: String,
        password: String,
    },
    ReportIssue {
        issue: String,
    },
}

// Window custom messages
const WM_APP_HIDE: u32 = WM_USER + 1;
const WM_APP_SHOW: u32 = WM_USER + 2;
const WM_APP_STATUS: u32 = WM_USER + 3;
const WM_APP_AUTH_MODE: u32 = WM_USER + 4;

// Control IDs
const ID_BTN_LOGIN: usize = 101;
const ID_EDIT_USER: usize = 102;
const ID_EDIT_PASS: usize = 103;
const ID_LBL_STATUS: usize = 104;
const ID_BTN_TOGGLE_PASS: usize = 105;
const ID_BTN_REPORT_ISSUE: usize = 106;

// Hotkey ID
const HOTKEY_REPORT_ISSUE: i32 = 1001;
const HOTKEY_SETTINGS_MODE: i32 = 1002;

// Win32 Static Control Style & Edit Control Message
const SS_CENTER: u32 = 0x00000001;
const EM_SETPASSWORDCHAR: u32 = 0x00CC;

// Mode Autentikasi: 0 = Identity, 1 = NoAuth, 2 = GeneralPassword
static CURRENT_AUTH_MODE_TYPE: AtomicU8 = AtomicU8::new(0);
static IS_DEBUG_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

static mut EVENT_TX: Option<UnboundedSender<UiEvent>> = None;
static mut HWND_USER: HWND = 0;
static mut HWND_PASS: HWND = 0;
static mut HWND_STATUS: HWND = 0;
static mut HWND_SUBTITLE: HWND = 0;
static mut HWND_LBL_USER: HWND = 0;
static mut HWND_LBL_PASS: HWND = 0;
static mut HWND_BTN_LOGIN: HWND = 0;
static mut HWND_BTN_TOGGLE_PASS: HWND = 0;
static mut BG_BRUSH: HBRUSH = 0;
static mut CARD_BRUSH: HBRUSH = 0;
static mut PASS_VISIBLE: bool = false;

pub fn start_ui_thread(event_tx: UnboundedSender<UiEvent>, cmd_rx: Receiver<UiCommand>, is_debug: bool) {
    IS_DEBUG_MODE.store(is_debug, Ordering::SeqCst);
    std::thread::spawn(move || unsafe {
        EVENT_TX = Some(event_tx);

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);

        // Brush background warna dark blue / slate
        BG_BRUSH = CreateSolidBrush(0x00170F); // RGB(15, 23, 15) in BGR = 0x00170F (hex 0x00BBGGRR)
        CARD_BRUSH = CreateSolidBrush(0x003B291E); // Slate dark card

        let class_name = wide_str("SekarLabLockerClass");

        let mut wc: WNDCLASSEXW = std::mem::zeroed();
        wc.cbSize = std::mem::size_of::<WNDCLASSEXW>() as u32;
        wc.lpfnWndProc = Some(wnd_proc);
        wc.lpszClassName = class_name.as_ptr();
        wc.hbrBackground = BG_BRUSH;
        wc.hCursor = LoadCursorW(0, IDC_ARROW);

        RegisterClassExW(&wc);

        let initial_cfg = crate::auth_cache::load_local_auth_config();
        let initial_visibility = if initial_cfg.mode == shared::AuthModeType::NoAuth && initial_cfg.auto_unlock_on_no_auth {
            0
        } else {
            WS_VISIBLE
        };

        // Jika bukan mode debug, gunakan WS_EX_TOPMOST agar siswa tidak bisa bypass layar kunci.
        // Jika mode debug aktif, jangan gunakan WS_EX_TOPMOST agar Command Prompt bisa tampil di depan.
        let ex_style = if is_debug {
            WS_EX_TOOLWINDOW
        } else {
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW
        };

        let hwnd_main = CreateWindowExW(
            ex_style,
            class_name.as_ptr(),
            wide_str("Molay Sekar7 - Client Locker").as_ptr(),
            WS_POPUP | initial_visibility,
            0,
            0,
            screen_w,
            screen_h,
            0,
            0,
            0,
            std::ptr::null(),
        );

        // Konfigurasi dan lindungi jendela konsol (anti-close & hide pada mode normal)
        configure_console_window(is_debug);

        // Daftarkan global hotkey Ctrl+Alt+K untuk Lapor Kendala dan Ctrl+Alt+S untuk Pengaturan Mode
        const MOD_ALT: u32 = 0x0001;
        const MOD_CONTROL: u32 = 0x0002;
        RegisterHotKey(hwnd_main, HOTKEY_REPORT_ISSUE, MOD_CONTROL | MOD_ALT, 0x4B); // 'K'
        RegisterHotKey(hwnd_main, HOTKEY_SETTINGS_MODE, MOD_CONTROL | MOD_ALT, 0x53); // 'S'

        // Create UI elements centered on screen
        let card_w = 440;
        let card_h = 430;
        let start_x = (screen_w - card_w) / 2;
        let start_y = (screen_h - card_h) / 2;

        let edit_cls = wide_str("EDIT");
        let btn_cls = wide_str("BUTTON");
        let static_cls = wide_str("STATIC");

        // Title
        CreateWindowExW(
            0,
            static_cls.as_ptr(),
            wide_str("MOLAY SEKAR7").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER as u32,
            start_x,
            start_y + 16,
            card_w,
            28,
            hwnd_main,
            0,
            0,
            std::ptr::null(),
        );

        // Subtitle
        HWND_SUBTITLE = CreateWindowExW(
            0,
            static_cls.as_ptr(),
            wide_str("Monitoring Layar & Akses Komputer Lab").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER as u32,
            start_x,
            start_y + 44,
            card_w,
            20,
            hwnd_main,
            0,
            0,
            std::ptr::null(),
        );

        // Label Username
        HWND_LBL_USER = CreateWindowExW(
            0,
            static_cls.as_ptr(),
            wide_str("Username Siswa:").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            start_x + 30,
            start_y + 76,
            card_w - 60,
            18,
            hwnd_main,
            0,
            0,
            std::ptr::null(),
        );

        // Edit Username
        HWND_USER = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_cls.as_ptr(),
            wide_str("").as_ptr(),
            WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL as u32,
            start_x + 30,
            start_y + 96,
            card_w - 60,
            30,
            hwnd_main,
            ID_EDIT_USER as HMENU,
            0,
            std::ptr::null(),
        );

        // Label Password
        HWND_LBL_PASS = CreateWindowExW(
            0,
            static_cls.as_ptr(),
            wide_str("Password:").as_ptr(),
            WS_CHILD | WS_VISIBLE,
            start_x + 30,
            start_y + 134,
            card_w - 60,
            18,
            hwnd_main,
            0,
            0,
            std::ptr::null(),
        );

        // Edit Password
        HWND_PASS = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_cls.as_ptr(),
            wide_str("").as_ptr(),
            WS_CHILD | WS_VISIBLE | ES_PASSWORD as u32 | ES_AUTOHSCROLL as u32,
            start_x + 30,
            start_y + 154,
            card_w - 60 - 64,
            30,
            hwnd_main,
            ID_EDIT_PASS as HMENU,
            0,
            std::ptr::null(),
        );

        // Tombol Lihat / Tutup Mata Password
        HWND_BTN_TOGGLE_PASS = CreateWindowExW(
            0,
            btn_cls.as_ptr(),
            wide_str("Lihat").as_ptr(),
            WS_CHILD | WS_VISIBLE | 0x00000000, // BS_PUSHBUTTON
            start_x + 30 + card_w - 60 - 58,
            start_y + 154,
            58,
            30,
            hwnd_main,
            ID_BTN_TOGGLE_PASS as HMENU,
            0,
            std::ptr::null(),
        );

        // Tombol Login
        HWND_BTN_LOGIN = CreateWindowExW(
            0,
            btn_cls.as_ptr(),
            wide_str("MASUK / LOGIN").as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_DEFPUSHBUTTON as u32,
            start_x + 30,
            start_y + 196,
            card_w - 60,
            38,
            hwnd_main,
            ID_BTN_LOGIN as HMENU,
            0,
            std::ptr::null(),
        );

        // Tombol Lapor Kendala ke Guru
        CreateWindowExW(
            0,
            btn_cls.as_ptr(),
            wide_str("🚨 LAPOR KENDALA KE GURU").as_ptr(),
            WS_CHILD | WS_VISIBLE | 0x00000000, // BS_PUSHBUTTON
            start_x + 30,
            start_y + 242,
            card_w - 60,
            36,
            hwnd_main,
            ID_BTN_REPORT_ISSUE as HMENU,
            0,
            std::ptr::null(),
        );

        // Status Label
        HWND_STATUS = CreateWindowExW(
            0,
            static_cls.as_ptr(),
            wide_str("Status: Menghubungkan ke Jaringan Server (Localhost & LAN)...").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER as u32,
            start_x + 30,
            start_y + 288,
            card_w - 60,
            42,
            hwnd_main,
            ID_LBL_STATUS as HMENU,
            0,
            std::ptr::null(),
        );

        // Sub-note Hotkey
        CreateWindowExW(
            0,
            static_cls.as_ptr(),
            wide_str("💡 Jaringan: Multi-LAN & Cloud Auto-Failover | Ctrl+Alt+S: Pengaturan").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_CENTER as u32,
            start_x + 20,
            start_y + 395,
            card_w - 40,
            20,
            hwnd_main,
            0,
            0,
            std::ptr::null(),
        );

        // Thread to listen to incoming commands from async runtime
        let hwnd_for_cmds = hwnd_main;
        std::thread::spawn(move || {
            while let Ok(cmd) = cmd_rx.recv() {
                match cmd {
                    UiCommand::ShowLock => {
                        PostMessageW(hwnd_for_cmds, WM_APP_SHOW, 0, 0);
                    }
                    UiCommand::HideLock => {
                        PostMessageW(hwnd_for_cmds, WM_APP_HIDE, 0, 0);
                    }
                    UiCommand::SetStatus(text) => {
                        let text_ptr = Box::into_raw(Box::new(text));
                        PostMessageW(hwnd_for_cmds, WM_APP_STATUS, text_ptr as usize, 0);
                    }
                    UiCommand::SetAuthMode(cfg) => {
                        let cfg_ptr = Box::into_raw(Box::new(cfg));
                        PostMessageW(hwnd_for_cmds, WM_APP_AUTH_MODE, cfg_ptr as usize, 0);
                    }
                }
            }
        });

        // Windows Message Pump
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    const WM_HOTKEY: u32 = 0x0312;

    match msg {
        WM_COMMAND => {
            let ctrl_id = (wparam & 0xFFFF) as usize;
            if ctrl_id == ID_BTN_LOGIN {
                handle_login_click();
            } else if ctrl_id == ID_BTN_TOGGLE_PASS {
                handle_toggle_password(lparam as HWND);
            } else if ctrl_id == ID_BTN_REPORT_ISSUE {
                handle_report_issue_click(hwnd);
            }
            0
        }
        WM_HOTKEY => {
            if wparam == HOTKEY_REPORT_ISSUE as usize {
                handle_report_issue_click(hwnd);
            } else if wparam == HOTKEY_SETTINGS_MODE as usize {
                handle_mode_settings_click(hwnd);
            }
            0
        }
        WM_APP_HIDE => {
            ShowWindow(hwnd, SW_HIDE);
            if IS_DEBUG_MODE.load(Ordering::Relaxed) {
                println!("[LOCKER] Layar kunci disembunyikan (Desktop terbuka)");
            }
            0
        }
        WM_APP_SHOW => {
            ShowWindow(hwnd, SW_SHOW);
            if !IS_DEBUG_MODE.load(Ordering::Relaxed) {
                SetForegroundWindow(hwnd);
            }
            if IS_DEBUG_MODE.load(Ordering::Relaxed) {
                println!("[LOCKER] Layar kunci ditampilkan (Desktop terkunci)");
            }
            0
        }
        WM_APP_STATUS => {
            let text_ptr = wparam as *mut String;
            if !text_ptr.is_null() {
                let text = Box::from_raw(text_ptr);
                let w_text = wide_str(&text);
                SetWindowTextW(HWND_STATUS, w_text.as_ptr());
                if IS_DEBUG_MODE.load(Ordering::Relaxed) {
                    println!("[STATUS KLIEN] {}", text);
                }
            }
            0
        }
        WM_APP_AUTH_MODE => {
            let cfg_ptr = wparam as *mut shared::ClientAuthConfig;
            if !cfg_ptr.is_null() {
                let cfg = Box::from_raw(cfg_ptr);
                let mode_code = match cfg.mode {
                    shared::AuthModeType::Identity => 0,
                    shared::AuthModeType::NoAuth => 1,
                    shared::AuthModeType::GeneralPassword => 2,
                };
                CURRENT_AUTH_MODE_TYPE.store(mode_code, Ordering::SeqCst);
                apply_auth_mode_to_ui(&cfg);
                if IS_DEBUG_MODE.load(Ordering::Relaxed) {
                    println!("[MODE AUTH] Mode autentikasi diperbarui: {:?}", cfg.mode);
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn apply_auth_mode_to_ui(cfg: &shared::ClientAuthConfig) {
    if HWND_SUBTITLE == 0 || HWND_LBL_USER == 0 || HWND_BTN_LOGIN == 0 {
        return;
    }

    match cfg.mode {
        shared::AuthModeType::Identity => {
            SetWindowTextW(HWND_SUBTITLE, wide_str("Monitoring Layar & Akses Komputer Lab").as_ptr());
            SetWindowTextW(HWND_LBL_USER, wide_str("Username Siswa:").as_ptr());
            SetWindowTextW(HWND_LBL_PASS, wide_str("Password:").as_ptr());
            SetWindowTextW(HWND_BTN_LOGIN, wide_str("MASUK / LOGIN").as_ptr());

            ShowWindow(HWND_LBL_PASS, SW_SHOW);
            ShowWindow(HWND_PASS, SW_SHOW);
            ShowWindow(HWND_BTN_TOGGLE_PASS, SW_SHOW);

            SetWindowTextW(HWND_STATUS, wide_str("Status: Siap. Silakan Login.").as_ptr());
        }
        shared::AuthModeType::NoAuth => {
            SetWindowTextW(HWND_SUBTITLE, wide_str("Akses Bebas: Komputer dapat digunakan tanpa login").as_ptr());
            SetWindowTextW(HWND_LBL_USER, wide_str("Nama Siswa / No. Meja (Opsional):").as_ptr());
            SetWindowTextW(HWND_BTN_LOGIN, wide_str("🔓 MASUK LANGSUNG (TANPA LOGIN)").as_ptr());

            ShowWindow(HWND_LBL_PASS, SW_HIDE);
            ShowWindow(HWND_PASS, SW_HIDE);
            ShowWindow(HWND_BTN_TOGGLE_PASS, SW_HIDE);

            SetWindowTextW(HWND_STATUS, wide_str("Akses terbuka. Klik tombol di atas untuk mulai.").as_ptr());
        }
        shared::AuthModeType::GeneralPassword => {
            let welcome = cfg.welcome_message.as_deref().unwrap_or("Gunakan Password Kelas untuk Membuka Komputer");
            SetWindowTextW(HWND_SUBTITLE, wide_str(welcome).as_ptr());
            SetWindowTextW(HWND_LBL_USER, wide_str("Nama Siswa / No. Meja (Opsional):").as_ptr());
            SetWindowTextW(HWND_LBL_PASS, wide_str("Password Kelas / Umum:").as_ptr());
            SetWindowTextW(HWND_BTN_LOGIN, wide_str("🔑 MASUK DENGAN PASSWORD KELAS").as_ptr());

            ShowWindow(HWND_LBL_PASS, SW_SHOW);
            ShowWindow(HWND_PASS, SW_SHOW);
            ShowWindow(HWND_BTN_TOGGLE_PASS, SW_SHOW);

            SetWindowTextW(HWND_STATUS, wide_str("Silakan masukkan Password Kelas yang diberikan Guru.").as_ptr());
        }
    }

    if cfg.lan_only {
        SetWindowTextW(HWND_STATUS, wide_str("Mode Lokal Saja (Ketat LAN): Menunggu Server Guru...").as_ptr());
    }
}

unsafe fn handle_mode_settings_click(hwnd: HWND) {
    let mut cfg = crate::auth_cache::load_local_auth_config();
    let current_mode_str = if cfg.lan_only {
        "Mode KETAT LOKAL SAJA (Hanya LAN, offline login dinonaktifkan)"
    } else {
        "Mode OTOMATIS / SMART FAILOVER (Bisa login mandiri tanpa server guru)"
    };

    const MB_YESNOCANCEL: u32 = 0x00000003;
    const MB_ICONQUESTION: u32 = 0x00000020;
    const MB_TOPMOST: u32 = 0x00040000;
    const IDYES: i32 = 6;
    const IDNO: i32 = 7;

    let prompt_msg = format!(
        "⚙️ PENGATURAN MODE AKSES LAB SEKAR7\n\n\
        Status Saat Ini:\n{}\n\n\
        Pilih mode akses untuk komputer ini:\n\n\
        [YES] = Aktifkan Mode Otomatis / Smart Failover (Default)\n\
                (Siswa bisa langsung login mandiri tanpa menunggu server guru).\n\n\
        [NO]  = Aktifkan Mode KETAT LOKAL SAJA\n\
                (Hanya konek ke LAN server guru, matikan cloud & login mandiri).\n\n\
        [Batal] = Keluar tanpa mengubah pengaturan.",
        current_mode_str
    );

    let res = MessageBoxW(
        hwnd,
        wide_str(&prompt_msg).as_ptr(),
        wide_str("Molay Sekar7 - Pengaturan Mode Akses").as_ptr(),
        MB_YESNOCANCEL | MB_ICONQUESTION | MB_TOPMOST,
    );

    if res == IDYES {
        cfg.lan_only = false;
        cfg.allow_offline_cache = true;
        crate::auth_cache::save_local_auth_config(&cfg);
        apply_auth_mode_to_ui(&cfg);
        const MB_OK: u32 = 0x00000000;
        const MB_ICONINFORMATION: u32 = 0x00000040;
        MessageBoxW(
            hwnd,
            wide_str("✅ Mode Otomatis / Smart Failover BERHASIL DIAKTIFKAN!\nSiswa sekarang dapat login mandiri.").as_ptr(),
            wide_str("Molay Sekar7").as_ptr(),
            MB_OK | MB_ICONINFORMATION | MB_TOPMOST,
        );
    } else if res == IDNO {
        cfg.lan_only = true;
        cfg.allow_offline_cache = false;
        crate::auth_cache::save_local_auth_config(&cfg);
        apply_auth_mode_to_ui(&cfg);
        const MB_OK: u32 = 0x00000000;
        const MB_ICONINFORMATION: u32 = 0x00000040;
        MessageBoxW(
            hwnd,
            wide_str("🔒 Mode KETAT LOKAL SAJA BERHASIL DIAKTIFKAN!\nKlien hanya akan terhubung via LAN dan wajib menunggu Server Guru.").as_ptr(),
            wide_str("Molay Sekar7").as_ptr(),
            MB_OK | MB_ICONINFORMATION | MB_TOPMOST,
        );
    }
}

unsafe fn handle_report_issue_click(hwnd: HWND) {
    const MF_STRING: u32 = 0x00000000;
    const MF_SEPARATOR: u32 = 0x00000800;
    const TPM_RETURNCMD: u32 = 0x0100;
    const TPM_NONOTIFY: u32 = 0x0080;
    const TPM_LEFTALIGN: u32 = 0x0000;
    const TPM_TOPALIGN: u32 = 0x0000;

    let hmenu = CreatePopupMenu();
    AppendMenuW(hmenu, MF_STRING, 1, wide_str("1. 🖥️ Layar Monitor Macet / Blank").as_ptr());
    AppendMenuW(hmenu, MF_STRING, 2, wide_str("2. ⌨️ Keyboard / Mouse Rusak / Tidak Merespon").as_ptr());
    AppendMenuW(hmenu, MF_STRING, 3, wide_str("3. 🔑 Tidak Bisa Login / Masalah Akun Siswa").as_ptr());
    AppendMenuW(hmenu, MF_STRING, 4, wide_str("4. ⚠️ Aplikasi Error / Komputer Macet").as_ptr());
    AppendMenuW(hmenu, MF_STRING, 5, wide_str("5. 🌐 Jaringan Internet Putus / Lemot").as_ptr());
    AppendMenuW(hmenu, MF_STRING, 6, wide_str("6. 🙋 Butuh Bantuan Guru di Meja Ini").as_ptr());
    AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(hmenu, MF_STRING, 7, wide_str("7. ✏️ Ambil Pesan dari Kotak Username").as_ptr());

    let mut pt: POINT = std::mem::zeroed();
    GetCursorPos(&mut pt);

    SetForegroundWindow(hwnd);
    let selected = TrackPopupMenu(
        hmenu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_LEFTALIGN | TPM_TOPALIGN,
        pt.x,
        pt.y,
        0,
        hwnd,
        std::ptr::null(),
    );
    DestroyMenu(hmenu);

    let issue_opt = match selected {
        1 => Some("Layar Monitor Macet / Blank".to_string()),
        2 => Some("Keyboard atau Mouse Rusak / Tidak Merespon".to_string()),
        3 => Some("Tidak Bisa Login / Masalah Akun Siswa".to_string()),
        4 => Some("Aplikasi Error / Komputer Macet".to_string()),
        5 => Some("Jaringan Internet Putus / Lemot".to_string()),
        6 => Some("Butuh Bantuan Guru di Meja Ini".to_string()),
        7 => {
            let mut buf = [0u16; 128];
            let len = GetWindowTextW(HWND_USER, buf.as_mut_ptr(), 128);
            let text = String::from_utf16_lossy(&buf[..len as usize]).trim().to_string();
            if text.is_empty() {
                let msg = wide_str("Tuliskan kendala di kotak Username lalu klik lagi menu nomor 7.");
                SetWindowTextW(HWND_STATUS, msg.as_ptr());
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    };

    if let Some(issue) = issue_opt {
        if !issue.is_empty() {
            let status_msg = format!("Laporan terkirim ke Guru: {}", issue);
            SetWindowTextW(HWND_STATUS, wide_str(&status_msg).as_ptr());
            if let Some(ref tx) = EVENT_TX {
                let _ = tx.send(UiEvent::ReportIssue { issue: issue.clone() });
            }

            if IsWindowVisible(hwnd) == 0 {
                const MB_OK: u32 = 0x00000000;
                const MB_ICONINFORMATION: u32 = 0x00000040;
                const MB_TOPMOST: u32 = 0x00040000;
                MessageBoxW(
                    0,
                    wide_str(&format!("Laporan kendala berhasil dikirim ke Guru:\n\"{}\"", issue)).as_ptr(),
                    wide_str("Molay Sekar7 - Lapor Kendala").as_ptr(),
                    MB_OK | MB_ICONINFORMATION | MB_TOPMOST,
                );
            }
        }
    }
}

unsafe fn handle_login_click() {
    let mut user_buf = [0u16; 128];
    let user_len = GetWindowTextW(HWND_USER, user_buf.as_mut_ptr(), 128);
    let username = String::from_utf16_lossy(&user_buf[..user_len as usize]).trim().to_string();

    let mut pass_buf = [0u16; 128];
    let pass_len = GetWindowTextW(HWND_PASS, pass_buf.as_mut_ptr(), 128);
    let password = String::from_utf16_lossy(&pass_buf[..pass_len as usize]).trim().to_string();

    let active_mode_code = CURRENT_AUTH_MODE_TYPE.load(Ordering::SeqCst);

    match active_mode_code {
        1 => {
            // Mode 1: NoAuth (Tanpa Autentikasi)
            let msg = wide_str("Membuka akses komputer...");
            SetWindowTextW(HWND_STATUS, msg.as_ptr());
            if let Some(ref tx) = EVENT_TX {
                let _ = tx.send(UiEvent::LoginSubmit {
                    username,
                    password: String::new(),
                });
            }
        }
        2 => {
            // Mode 2: GeneralPassword (Password Bersama)
            if password.is_empty() {
                let msg = wide_str("Harap masukkan Password Kelas yang diberikan Guru!");
                SetWindowTextW(HWND_STATUS, msg.as_ptr());
                return;
            }
            let msg = wide_str("Memverifikasi Password Kelas...");
            SetWindowTextW(HWND_STATUS, msg.as_ptr());
            if let Some(ref tx) = EVENT_TX {
                let _ = tx.send(UiEvent::LoginSubmit { username, password });
            }
        }
        _ => {
            // Mode 0: Identity (Autentikasi Identitas Siswa)
            if username.is_empty() || password.is_empty() {
                let msg = wide_str("Harap masukkan username dan password!");
                SetWindowTextW(HWND_STATUS, msg.as_ptr());
                return;
            }

            let msg = wide_str("Memverifikasi ke server...");
            SetWindowTextW(HWND_STATUS, msg.as_ptr());

            if let Some(ref tx) = EVENT_TX {
                let _ = tx.send(UiEvent::LoginSubmit { username, password });
            }
        }
    }
}

unsafe fn handle_toggle_password(btn_hwnd: HWND) {
    PASS_VISIBLE = !PASS_VISIBLE;
    if PASS_VISIBLE {
        SendMessageW(HWND_PASS, EM_SETPASSWORDCHAR, 0, 0);
        SetWindowTextW(btn_hwnd, wide_str("Tutup").as_ptr());
    } else {
        SendMessageW(HWND_PASS, EM_SETPASSWORDCHAR, '*' as usize, 0);
        SetWindowTextW(btn_hwnd, wide_str("Lihat").as_ptr());
    }
    InvalidateRect(HWND_PASS, std::ptr::null(), 1);
}

fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Mengatur tampilan dan proteksi jendela konsol Windows:
/// - Pada Mode Normal (Siswa): Jendela CMD disembunyikan sepenuhnya (SW_HIDE) agar tidak ada jendela hitam bertanda [X].
/// - Pada Mode Teknisi (--debug / --teknisi): Jendela CMD tetap tampil, namun tombol close [X] dinonaktifkan (gray-out)
///   dan event CTRL_CLOSE dicegah agar proses agen tidak mati mendadak saat tombol [X] diklik.
pub fn configure_console_window(is_debug: bool) {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleWindow() -> HWND;
        fn SetConsoleTitleW(lpConsoleTitle: *const u16) -> BOOL;
        fn SetConsoleCtrlHandler(
            HandlerRoutine: Option<unsafe extern "system" fn(u32) -> BOOL>,
            Add: BOOL,
        ) -> BOOL;
    }

    unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
        const CTRL_CLOSE_EVENT: u32 = 2;
        if ctrl_type == CTRL_CLOSE_EVENT {
            // Mencegah Windows mematikan proses saat event close konsol diterima
            return 1;
        }
        0
    }

    unsafe {
        // Pasang control handler untuk memblokir event close paksa pada konsol
        let _ = SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);

        let h_console = GetConsoleWindow();
        if h_console != 0 {
            // Nonaktifkan dan hilangkan opsi Close ([X]) dari System Menu jendela konsol
            let h_menu = GetSystemMenu(h_console, 0);
            if h_menu != 0 {
                EnableMenuItem(h_menu, SC_CLOSE, MF_BYCOMMAND | MF_DISABLED | MF_GRAYED);
                DeleteMenu(h_menu, SC_CLOSE, MF_BYCOMMAND);
            }

            if is_debug {
                let c_title = wide_str("Molay Sekar7 - [MODE TEKNISI / LIVE MONITORING KONSOL]");
                SetConsoleTitleW(c_title.as_ptr());
                SetWindowPos(
                    h_console,
                    -1_isize as HWND,
                    30,
                    30,
                    880,
                    540,
                    SWP_SHOWWINDOW,
                );
                ShowWindow(h_console, SW_SHOW);
                SetForegroundWindow(h_console);
            } else {
                // Sembunyikan total jendela konsol CMD agar siswa tidak melihat dan tidak bisa mengklik [X]
                ShowWindow(h_console, SW_HIDE);
            }
        }
    }
}
