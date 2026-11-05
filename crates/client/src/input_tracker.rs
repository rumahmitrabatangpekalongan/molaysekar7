//! input_tracker.rs — Modul pelacak metrik input pengguna (ala WhatPulse)
//!
//! Melacak:
//! - Total ketikan keyboard (keystrokes) secara non-intrusif (hanya counter atomik, BUKAN keylogger)
//! - Total klik mouse (kiri, kanan, tengah)
//! - Estimasi jarak tempuh kursor mouse (piksel)
//! - Deteksi durasi bengong / idle (AFK) via GetLastInputInfo
//! - Penghitungan kecepatan ketikan (keystrokes per menit)
//!
//! Modul ini terisolasi dan mandiri, zero regression terhadap fitur yang sudah ada.

use shared::InputStatsSnapshot;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};
use windows_sys::Win32::System::SystemInformation::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

static TRACKER_STARTED: AtomicBool = AtomicBool::new(false);

static TOTAL_KEYSTROKES: AtomicU64 = AtomicU64::new(0);
static TOTAL_CLICKS: AtomicU64 = AtomicU64::new(0);
static TOTAL_MOUSE_DISTANCE: AtomicU64 = AtomicU64::new(0);

// Delta untuk jendela/aplikasi aktif saat ini
static WINDOW_KEYSTROKES: AtomicU64 = AtomicU64::new(0);
static WINDOW_CLICKS: AtomicU64 = AtomicU64::new(0);
static WINDOW_MOUSE_DISTANCE: AtomicU64 = AtomicU64::new(0);

// Mouse tracking previous position
static LAST_MOUSE_X: AtomicI32 = AtomicI32::new(i32::MIN);
static LAST_MOUSE_Y: AtomicI32 = AtomicI32::new(i32::MIN);

// Cadangan internal last input tick
static LAST_INPUT_TICK: AtomicU64 = AtomicU64::new(0);

// Rolling window untuk menghitung keystrokes per minute (KPM)
static RECENT_KEYS: AtomicU32 = AtomicU32::new(0);
static LAST_KPM_CALC: AtomicU64 = AtomicU64::new(0);
static CURRENT_KPM: AtomicU32 = AtomicU32::new(0);

/// Batas detik tanpa interaksi untuk dianggap AFK (3 menit)
pub const AFK_THRESHOLD_SECONDS: u32 = 180;

/// Callback low-level keyboard hook
unsafe extern "system" fn low_level_keyboard_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        let wp = wparam as u32;
        if wp == WM_KEYDOWN || wp == WM_SYSKEYDOWN {
            TOTAL_KEYSTROKES.fetch_add(1, Ordering::Relaxed);
            WINDOW_KEYSTROKES.fetch_add(1, Ordering::Relaxed);
            RECENT_KEYS.fetch_add(1, Ordering::Relaxed);
            LAST_INPUT_TICK.store(GetTickCount64(), Ordering::Relaxed);
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

/// Callback low-level mouse hook
unsafe extern "system" fn low_level_mouse_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0 {
        let wp = wparam as u32;
        if wp == WM_LBUTTONDOWN || wp == WM_RBUTTONDOWN || wp == WM_MBUTTONDOWN {
            TOTAL_CLICKS.fetch_add(1, Ordering::Relaxed);
            WINDOW_CLICKS.fetch_add(1, Ordering::Relaxed);
            LAST_INPUT_TICK.store(GetTickCount64(), Ordering::Relaxed);
        } else if wp == WM_MOUSEMOVE {
            let ms = &*(lparam as *const MSLLHOOKSTRUCT);
            let prev_x = LAST_MOUSE_X.swap(ms.pt.x, Ordering::Relaxed);
            let prev_y = LAST_MOUSE_Y.swap(ms.pt.y, Ordering::Relaxed);
            if prev_x != i32::MIN {
                let dx = (ms.pt.x - prev_x).abs() as u64;
                let dy = (ms.pt.y - prev_y).abs() as u64;
                let dist = (((dx * dx + dy * dy) as f64).sqrt()) as u64;
                // Batasi outlier / jump lintas monitor
                if dist > 0 && dist < 4000 {
                    TOTAL_MOUSE_DISTANCE.fetch_add(dist, Ordering::Relaxed);
                    WINDOW_MOUSE_DISTANCE.fetch_add(dist, Ordering::Relaxed);
                }
            }
            LAST_INPUT_TICK.store(GetTickCount64(), Ordering::Relaxed);
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

/// Mulai background input tracker pada dedicated thread dengan Windows Message Loop.
pub fn start_input_tracker() {
    if TRACKER_STARTED.swap(true, Ordering::SeqCst) {
        return; // Sudah berjalan
    }

    unsafe {
        LAST_INPUT_TICK.store(GetTickCount64(), Ordering::Relaxed);
    }

    std::thread::Builder::new()
        .name("input-tracker-hook".to_string())
        .spawn(|| {
            unsafe {
                let h_k_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(low_level_keyboard_proc), 0, 0);
                let h_m_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), 0, 0);

                if h_k_hook == 0 && h_m_hook == 0 {
                    eprintln!("[INPUT_TRACKER] Gagal memasang Windows Low-Level Hooks");
                    return;
                }

                // Standard message pump untuk low-level hooks
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                if h_k_hook != 0 {
                    UnhookWindowsHookEx(h_k_hook);
                }
                if h_m_hook != 0 {
                    UnhookWindowsHookEx(h_m_hook);
                }
            }
        })
        .expect("Gagal membuat thread input tracker");
}

/// Dapatkan durasi diam / idle komputer dalam detik menggunakan API Windows GetLastInputInfo.
pub fn get_idle_seconds() -> u32 {
    unsafe {
        let mut lii = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut lii) != 0 {
            let current_tick = GetTickCount() as u32;
            let diff_ms = current_tick.wrapping_sub(lii.dwTime);
            diff_ms / 1000
        } else {
            let now = GetTickCount64();
            let last = LAST_INPUT_TICK.load(Ordering::Relaxed);
            if now >= last {
                ((now - last) / 1000) as u32
            } else {
                0
            }
        }
    }
}

/// Ambil snapshot metrik input saat ini untuk dikirim ke server.
pub fn get_current_stats() -> InputStatsSnapshot {
    let now = unsafe { GetTickCount64() };
    let last_calc = LAST_KPM_CALC.load(Ordering::Relaxed);
    if now.saturating_sub(last_calc) >= 5000 {
        // Update perkiraan KPM setiap minimal 5 detik
        let elapsed_sec = (now.saturating_sub(last_calc) / 1000).max(1);
        let recent = RECENT_KEYS.swap(0, Ordering::Relaxed) as u64;
        let kpm = (recent * 60 / elapsed_sec) as u32;
        CURRENT_KPM.store(kpm, Ordering::Relaxed);
        LAST_KPM_CALC.store(now, Ordering::Relaxed);
    }

    let idle_sec = get_idle_seconds();
    let is_afk = idle_sec >= AFK_THRESHOLD_SECONDS;

    InputStatsSnapshot {
        total_keystrokes: TOTAL_KEYSTROKES.load(Ordering::Relaxed),
        total_clicks: TOTAL_CLICKS.load(Ordering::Relaxed),
        mouse_distance_px: TOTAL_MOUSE_DISTANCE.load(Ordering::Relaxed),
        idle_seconds: idle_sec,
        is_afk,
        keystrokes_per_min: CURRENT_KPM.load(Ordering::Relaxed),
    }
}

/// Ambil dan reset akumulasi metrik input pada jendela aplikasi yang baru saja aktif (delta).
pub fn drain_window_deltas() -> (u64, u64, u64) {
    let keys = WINDOW_KEYSTROKES.swap(0, Ordering::Relaxed);
    let clicks = WINDOW_CLICKS.swap(0, Ordering::Relaxed);
    let dist = WINDOW_MOUSE_DISTANCE.swap(0, Ordering::Relaxed);
    (keys, clicks, dist)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_snapshot_initial() {
        let stats = get_current_stats();
        // Cek bahwa stats terisi angka valid tanpa panic
        assert_eq!(stats.is_afk, stats.idle_seconds >= AFK_THRESHOLD_SECONDS);
    }

    #[test]
    fn test_window_deltas_drain() {
        WINDOW_KEYSTROKES.store(50, Ordering::Relaxed);
        WINDOW_CLICKS.store(20, Ordering::Relaxed);
        WINDOW_MOUSE_DISTANCE.store(1500, Ordering::Relaxed);

        let (k, c, d) = drain_window_deltas();
        assert_eq!(k, 50);
        assert_eq!(c, 20);
        assert_eq!(d, 1500);

        // Setelah di-drain harus kembali 0
        let (k2, c2, d2) = drain_window_deltas();
        assert_eq!(k2, 0);
        assert_eq!(c2, 0);
        assert_eq!(d2, 0);
    }
}
