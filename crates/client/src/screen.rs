use std::mem::size_of;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub struct ScreenCapturer {
    pub width: i32,
    pub height: i32,
}

impl ScreenCapturer {
    pub fn new() -> Self {
        unsafe {
            let width = GetSystemMetrics(SM_CXSCREEN);
            let height = GetSystemMetrics(SM_CYSCREEN);
            Self { width, height }
        }
    }

    pub fn capture_jpeg(&self, quality: u8) -> Option<Vec<u8>> {
        unsafe {
            let hdc_screen = GetDC(0);
            if hdc_screen == 0 {
                return None;
            }
            let hdc_mem = CreateCompatibleDC(hdc_screen);
            let hbm = CreateCompatibleBitmap(hdc_screen, self.width, self.height);
            let old_bm = SelectObject(hdc_mem, hbm);

            // Copy screen contents to memory DC
            BitBlt(
                hdc_mem,
                0,
                0,
                self.width,
                self.height,
                hdc_screen,
                0,
                0,
                SRCCOPY,
            );

            // Prepare DIB structure to extract pixels
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width;
            bmi.bmiHeader.biHeight = -self.height; // Negative for top-down bitmap
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB as u32;

            let mut raw_pixels = vec![0u8; (self.width * self.height * 4) as usize];
            GetDIBits(
                hdc_mem,
                hbm,
                0,
                self.height as u32,
                raw_pixels.as_mut_ptr() as *mut _,
                &mut bmi,
                DIB_RGB_COLORS,
            );

            // Clean up GDI objects
            SelectObject(hdc_mem, old_bm);
            DeleteObject(hbm);
            DeleteDC(hdc_mem);
            ReleaseDC(0, hdc_screen);

            // Convert BGRA (Windows default) to RGB for JPEG
            let pixel_count = (self.width * self.height) as usize;
            let mut rgb_pixels = Vec::with_capacity(pixel_count * 3);
            for chunk in raw_pixels.chunks_exact(4) {
                let b = chunk[0];
                let g = chunk[1];
                let r = chunk[2];
                rgb_pixels.push(r);
                rgb_pixels.push(g);
                rgb_pixels.push(b);
            }

            // Encode to JPEG using pure Rust jpeg-encoder
            let mut jpeg_bytes = Vec::new();
            let encoder = jpeg_encoder::Encoder::new(&mut jpeg_bytes, quality);
            if encoder
                .encode(
                    &rgb_pixels,
                    self.width as u16,
                    self.height as u16,
                    jpeg_encoder::ColorType::Rgb,
                )
                .is_ok()
            {
                Some(jpeg_bytes)
            } else {
                None
            }
        }
    }
}
