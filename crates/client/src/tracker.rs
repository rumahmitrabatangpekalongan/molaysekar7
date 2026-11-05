use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::ProcessStatus::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

#[derive(Debug, Clone)]
pub struct ActiveWindowInfo {
    pub app_name: String,
    pub window_title: String,
    pub pid: u32,
    pub hwnd: isize,
}

pub fn get_active_window() -> Option<ActiveWindowInfo> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }

        // Get window title
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), 512);
        let window_title = if len > 0 {
            String::from_utf16_lossy(&title_buf[..len as usize])
        } else {
            String::new()
        };

        // Get application executable name
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut app_name = String::from("unknown");

        if pid > 0 {
            let h_proc = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if h_proc != 0 {
                let mut name_buf = [0u16; 260];
                let name_len = GetModuleBaseNameW(h_proc, 0, name_buf.as_mut_ptr(), 260);
                if name_len > 0 {
                    app_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
                }
                CloseHandle(h_proc);
            }
        }

        if window_title.is_empty() && app_name == "unknown" {
            None
        } else {
            Some(ActiveWindowInfo {
                app_name,
                window_title,
                pid,
                hwnd: hwnd as isize,
            })
        }
    }
}
