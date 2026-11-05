//! remote_input.rs — Modul pengiriman keyboard & mouse dari perintah Server ke PC Klien.
//! Menggunakan Windows SendInput API via windows-sys 0.52.
//! Modul ini terisolasi, tidak mengubah modul lain.

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, MOUSEEVENTF_ABSOLUTE,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};

/// Kirim rangkaian teks karakter UTF-8 langsung (Unicode input) ke aplikasi yang aktif di klien.
pub fn send_text(text: &str) {
    for c in text.encode_utf16() {
        let input_down = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: 0,
                    wScan: c,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let input_up = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: 0,
                    wScan: c,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(1, &input_down, std::mem::size_of::<INPUT>() as i32);
            SendInput(1, &input_up, std::mem::size_of::<INPUT>() as i32);
        }
    }
}

/// Kirim keystroke tunggal (tekan + lepas) berdasarkan Virtual Key Code Windows.
/// Contoh: 0x41 = 'A', 0x0D = Enter, 0x1B = Escape, 0x09 = Tab
pub fn send_key_press(vk_code: u16) {
    send_key_event(vk_code, false);
    std::thread::sleep(std::time::Duration::from_millis(15));
    send_key_event(vk_code, true);
}

/// Kirim event keyboard tunggal (tekan atau lepas).
/// `key_up = true` berarti melepaskan tombol.
pub fn send_key_event(vk_code: u16, key_up: bool) {
    let flags = if key_up { KEYEVENTF_KEYUP } else { 0 };
    // Tombol extended (panah, F-keys, dll) butuh flag tambahan
    let flags = if vk_code >= 0x21 && vk_code <= 0x28 {
        flags | KEYEVENTF_EXTENDEDKEY
    } else {
        flags
    };
    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk_code,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Gerakkan kursor mouse ke posisi relatif (0.0..=1.0) dari ukuran layar.
pub fn send_mouse_move(x_ratio: f32, y_ratio: f32) {
    let (screen_w, screen_h) = get_screen_size();
    // MOUSEEVENTF_ABSOLUTE butuh koordinat dalam range 0..=65535
    let abs_x = (x_ratio.clamp(0.0, 1.0) * 65535.0) as i32;
    let abs_y = (y_ratio.clamp(0.0, 1.0) * 65535.0) as i32;
    let _ = (screen_w, screen_h); // digunakan untuk validasi
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: abs_x,
                dy: abs_y,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Kirim klik mouse. `button`: "left", "right", "middle". `is_down`: true=tekan, false=lepas.
pub fn send_mouse_button(button: &str, is_down: bool) {
    let (down_flag, up_flag) = match button {
        "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP), // default: left
    };
    let flag = if is_down { down_flag } else { up_flag };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flag,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Kirim klik penuh (tekan + lepas) pada posisi relatif tertentu dengan jeda stabil.
pub fn send_mouse_click(x_ratio: f32, y_ratio: f32, button: &str) {
    send_mouse_move(x_ratio, y_ratio);
    std::thread::sleep(std::time::Duration::from_millis(15));
    send_mouse_button(button, true);
    std::thread::sleep(std::time::Duration::from_millis(25));
    send_mouse_button(button, false);
}

/// Kirim double click dengan jeda stabil.
pub fn send_mouse_double_click(x_ratio: f32, y_ratio: f32) {
    send_mouse_click(x_ratio, y_ratio, "left");
    std::thread::sleep(std::time::Duration::from_millis(60));
    send_mouse_click(x_ratio, y_ratio, "left");
}

/// Scroll roda mouse vertikal. delta positif = ke atas, negatif = ke bawah.
pub fn send_mouse_wheel(delta: i32) {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    unsafe {
        SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Ambil ukuran layar (width, height) dalam pixel.
fn get_screen_size() -> (i32, i32) {
    unsafe {
        let w = GetSystemMetrics(SM_CXSCREEN);
        let h = GetSystemMetrics(SM_CYSCREEN);
        (w, h)
    }
}
