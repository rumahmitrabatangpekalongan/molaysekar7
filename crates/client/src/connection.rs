use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Lan,
    Tunnel,
}

impl ConnectionMode {
    pub fn label(&self) -> &'static str {
        match self {
            ConnectionMode::Lan => "LAN",
            ConnectionMode::Tunnel => "Cloudflare Tunnel",
        }
    }

    pub fn screen_refresh_interval_ms(&self) -> u64 {
        match self {
            ConnectionMode::Lan => 500,       // ~2 FPS, responsif, mulus, dan stabil untuk jaringan lokal (LAN)
            ConnectionMode::Tunnel => 1500,   // ~0.67 FPS, hemat kuota dan stabil lewat internet
        }
    }

    pub fn screen_jpeg_quality(&self) -> u8 {
        match self {
            ConnectionMode::Lan => 75,        // Kualitas tinggi (75) agar teks/kode tajam, jernih, dan tidak blur
            ConnectionMode::Tunnel => 35,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEndpoint {
    pub raw: String,
    pub ws_url: String,
    pub mode: ConnectionMode,
}

impl ServerEndpoint {
    pub fn from_raw(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            return None;
        }

        let mut addr = trimmed.to_string();

        // Tentukan skema awal jika sudah ada
        let (has_https, has_http, has_wss, has_ws) = (
            addr.starts_with("https://"),
            addr.starts_with("http://"),
            addr.starts_with("wss://"),
            addr.starts_with("ws://"),
        );

        let (mode, ws_prefix) = if has_https || has_wss {
            // Sudah jelas aman / domain
            if has_https {
                addr = addr.trim_start_matches("https://").to_string();
            } else {
                addr = addr.trim_start_matches("wss://").to_string();
            }
            (ConnectionMode::Tunnel, "wss://")
        } else if has_http || has_ws {
            if has_http {
                addr = addr.trim_start_matches("http://").to_string();
            } else {
                addr = addr.trim_start_matches("ws://").to_string();
            }
            (ConnectionMode::Lan, "ws://")
        } else {
            // Tidak ada skema: deteksi otomatis apakah IP LAN atau Domain Luar
            let host_part = addr.split(':').next().unwrap_or("").trim().to_lowercase();
            let is_ip_or_localhost = host_part == "localhost"
                || host_part.starts_with("127.")
                || host_part.starts_with("192.168.")
                || host_part.starts_with("10.")
                || (host_part.starts_with("172.")
                    && host_part.split('.').nth(1).and_then(|o| o.parse::<u8>().ok()).map_or(false, |b| (16..=31).contains(&b)))
                || host_part.parse::<std::net::IpAddr>().is_ok();

            if is_ip_or_localhost {
                (ConnectionMode::Lan, "ws://")
            } else {
                // Domain (Cloudflare Tunnel atau FQDN)
                (ConnectionMode::Tunnel, "wss://")
            }
        };

        // Otomatis sematkan port 1916 jika IP lokal/LAN belum menentukan port
        // Mencegah kegagalan koneksi jika user hanya mengetik IP (misal 192.168.20.100) tanpa :1916
        if mode == ConnectionMode::Lan {
            let host_and_port = addr.split('/').next().unwrap_or(&addr);
            if !host_and_port.contains(':') {
                if let Some(pos) = addr.find('/') {
                    let host = &addr[..pos];
                    let rest = &addr[pos..];
                    addr = format!("{}:1916{}", host, rest);
                } else {
                    addr = format!("{}:1916", addr);
                }
            }
        }

        // Pastikan path /ws/client ada di akhir
        let ws_url = if addr.contains("/ws/") {
            format!("{}{}", ws_prefix, addr)
        } else {
            let clean_addr = addr.trim_end_matches('/');
            format!("{}{}/ws/client", ws_prefix, clean_addr)
        };

        Some(Self {
            raw: trimmed.to_string(),
            ws_url,
            mode,
        })
    }

    pub fn http_base_url(&self) -> String {
        if self.ws_url.starts_with("wss://") {
            let host = self.ws_url.trim_start_matches("wss://").trim_end_matches("/ws/client").trim_end_matches('/');
            format!("https://{}", host)
        } else {
            let host = self.ws_url.trim_start_matches("ws://").trim_end_matches("/ws/client").trim_end_matches('/');
            format!("http://{}", host)
        }
    }
}

pub fn parse_endpoints(content: &str) -> Vec<ServerEndpoint> {
    let mut endpoints = Vec::new();

    // Bisa dipisah baris baru (\n) atau koma (,)
    for line in content.split(|c| c == '\n' || c == ',' || c == ';') {
        if let Some(ep) = ServerEndpoint::from_raw(line) {
            endpoints.push(ep);
        }
    }

    if endpoints.is_empty() {
        if let Some(local) = ServerEndpoint::from_raw("127.0.0.1:1916") {
            endpoints.push(local);
        }
        if let Some(superadmin) = ServerEndpoint::from_raw("192.168.20.70:1916") {
            endpoints.push(superadmin);
        }
        if let Some(tunnel) = ServerEndpoint::from_raw("monitoringmolay.sekolahkartini7.web.id") {
            endpoints.push(tunnel);
        }
        if let Some(tunnel) = ServerEndpoint::from_raw("molay.sekolahkartini7.web.id") {
            endpoints.push(tunnel);
        }
    } else {
        // Pastikan selalu ada endpoint fallback Cloudflare Tunnel jika belum ada
        let has_tunnel = endpoints.iter().any(|e| e.mode == ConnectionMode::Tunnel);
        if !has_tunnel {
            if let Some(tunnel) = ServerEndpoint::from_raw("monitoringmolay.sekolahkartini7.web.id") {
                endpoints.push(tunnel);
            }
            if let Some(tunnel) = ServerEndpoint::from_raw("molay.sekolahkartini7.web.id") {
                endpoints.push(tunnel);
            }
        }
    }

    endpoints
}

pub async fn discover_lan_server_ip(timeout_ms: u64) -> Option<String> {
    // Prioritas 0: Cek PARALEL semua server yang dikenal secara bersamaan dan pilih yang TERCEPAT merespons.
    // Strategi ini memungkinkan deteksi instan (< 400ms) saat Server Guru aktif di LAN.
    let mut known_targets = vec![
        "127.0.0.1:1916".to_string(),
        "192.168.20.70:1916".to_string(),
        "192.168.20.100:1916".to_string(),
    ];

    if let Some(local_ip) = get_local_lan_ip() {
        let oct = local_ip.octets();
        let gw = format!("{}.{}.{}.1:1916", oct[0], oct[1], oct[2]);
        if !known_targets.contains(&gw) {
            known_targets.push(gw);
        }
        let own = format!("{}.{}.{}.{}:1916", oct[0], oct[1], oct[2], oct[3]);
        if !known_targets.contains(&own) {
            known_targets.push(own);
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(known_targets.len());
    for target in known_targets {
        let tx = tx.clone();
        tokio::spawn(async move {
            let res = tokio::time::timeout(
                Duration::from_millis(350),
                tokio::net::TcpStream::connect(&target),
            )
            .await;
            if let Ok(Ok(_)) = res {
                // Kirim hasilnya; channel sudah bounded jadi tidak ada blocking
                let _ = tx.send(target).await;
            }
        });
    }
    // Drop sender induk agar channel tertutup jika tak ada yang berhasil konek
    drop(tx);

    // Ambil respons pertama (server tercepat) dalam batas waktu 400ms
    match tokio::time::timeout(Duration::from_millis(400), rx.recv()).await {
        Ok(Some(fastest)) => {
            println!("[DISCOVERY] ✅ Server LAN tercepat merespons: {}", fastest);
            return Some(fastest);
        }
        _ => {} // Tidak ada yang merespons, lanjut ke strategi berikutnya
    }

    // Strategi 1: UDP Broadcast (cepat, tapi kadang diblokir firewall/router)
    if let Some(ip) = udp_broadcast_discover(timeout_ms / 2).await {
        return Some(ip);
    }
    // Strategi 2: TCP Port Scan seluruh subnet /24 secara paralel
    if let Some(ip) = tcp_subnet_scan(1916, timeout_ms / 2).await {
        return Some(ip);
    }
    None
}

async fn udp_broadcast_discover(timeout_ms: u64) -> Option<String> {
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let _ = socket.set_broadcast(true);
    let mut broadcast_addrs: Vec<std::net::SocketAddr> = vec![
        "127.0.0.1:1916".parse().ok()?,
        "255.255.255.255:1916".parse().ok()?,
        "192.168.20.255:1916".parse().ok()?,
        "192.168.20.70:1916".parse().ok()?,
        "192.168.20.100:1916".parse().ok()?,
    ];

    if let Some(local_ip) = get_local_lan_ip() {
        let oct = local_ip.octets();
        if let Ok(subnet_bc) = format!("{}.{}.{}.255:1916", oct[0], oct[1], oct[2]).parse() {
            if !broadcast_addrs.contains(&subnet_bc) {
                broadcast_addrs.push(subnet_bc);
            }
        }
    }

    for addr in &broadcast_addrs {
        let _ = socket.send_to(b"MOLAY_DISCOVER", addr).await;
    }

    let mut buf = [0u8; 512];
    let res = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        socket.recv_from(&mut buf),
    )
    .await;

    if let Ok(Ok((size, peer_addr))) = res {
        let msg = String::from_utf8_lossy(&buf[..size]);
        let trimmed = msg.trim();
        if trimmed.starts_with("MOLAY_SERVER:") || trimmed.starts_with("MOLAY_ANNOUNCE:") {
            let port_str = trimmed.split(':').nth(1).unwrap_or("1916");
            let port = port_str.parse::<u16>().unwrap_or(1916);
            let peer_ip = peer_addr.ip().to_string();
            if !peer_ip.starts_with("127.") && peer_ip != "0.0.0.0" {
                return Some(format!("{}:{}", peer_ip, port));
            }
        }
    }
    None
}

/// Deteksi IP LAN lokal komputer ini (non-loopback)
pub fn get_local_lan_ip() -> Option<std::net::Ipv4Addr> {
    // Probe routing via UDP dummy ke gateway atau DNS lokal (tidak kirim paket nyata)
    let targets = [
        "192.168.20.70:80",
        "192.168.20.100:80",
        "192.168.20.1:80",
        "8.8.8.8:80",
        "1.1.1.1:80",
        "192.168.1.1:80",
        "192.168.0.1:80",
        "192.168.100.1:80",
        "192.168.43.1:80",
        "10.0.0.1:80",
        "172.16.0.1:80",
    ];
    for t in targets {
        if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(t).is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    if let std::net::IpAddr::V4(v4) = addr.ip() {
                        if !v4.is_loopback() && !v4.is_unspecified() {
                            return Some(v4);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Scan TCP port 1916 ke seluruh /24 subnet secara paralel menggunakan mpsc channel
pub async fn tcp_subnet_scan(port: u16, total_timeout_ms: u64) -> Option<String> {
    let local_ip = get_local_lan_ip()?;
    let octets = local_ip.octets();

    // Hanya scan /24 subnet yang sama (ganti octet terakhir 1-254)
    let base = format!("{}.{}.{}.", octets[0], octets[1], octets[2]);
    let own_last = octets[3];

    println!(
        "[SCAN] Memindai Server Molay di subnet {}.0/24 port {}...",
        base.trim_end_matches('.'),
        port
    );

    let (tx, mut rx) = tokio::sync::mpsc::channel(1);

    for last in 1u8..=254 {
        if last == own_last {
            continue; // lewati diri sendiri
        }
        let ip_str = format!("{}{}", base, last);
        let addr = format!("{}:{}", ip_str, port);
        let tx = tx.clone();
        tokio::spawn(async move {
            let res = tokio::time::timeout(
                Duration::from_millis(500),
                tokio::net::TcpStream::connect(&addr),
            )
            .await;
            if let Ok(Ok(_)) = res {
                let _ = tx.send(format!("{}:{}", ip_str, port)).await;
            }
        });
    }
    // Drop sender induk agar channel tertutup jika tak ada yang connect
    drop(tx);

    let result = tokio::time::timeout(Duration::from_millis(total_timeout_ms), rx.recv()).await;
    match result {
        Ok(Some(ip)) => {
            println!("[SCAN] ✅ Server Molay ditemukan: {}", ip);
            Some(ip)
        }
        _ => {
            println!("[SCAN] Server tidak ditemukan di subnet {}.0/24", base.trim_end_matches('.'));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lan_ip_detection() {
        let ep = ServerEndpoint::from_raw("192.168.1.100:1916").unwrap();
        assert_eq!(ep.mode, ConnectionMode::Lan);
        assert_eq!(ep.ws_url, "ws://192.168.1.100:1916/ws/client");

        let ep2 = ServerEndpoint::from_raw("127.0.0.1:1916").unwrap();
        assert_eq!(ep2.mode, ConnectionMode::Lan);
        assert_eq!(ep2.ws_url, "ws://127.0.0.1:1916/ws/client");
    }

    #[test]
    fn test_lan_ip_auto_port_1916() {
        // Tanpa menyebutkan port :1916, sistem harus otomatis menyematkan :1916
        let ep = ServerEndpoint::from_raw("192.168.20.100").unwrap();
        assert_eq!(ep.mode, ConnectionMode::Lan);
        assert_eq!(ep.ws_url, "ws://192.168.20.100:1916/ws/client");

        let ep_http = ServerEndpoint::from_raw("http://192.168.20.100").unwrap();
        assert_eq!(ep_http.mode, ConnectionMode::Lan);
        assert_eq!(ep_http.ws_url, "ws://192.168.20.100:1916/ws/client");

        let ep_local = ServerEndpoint::from_raw("localhost").unwrap();
        assert_eq!(ep_local.mode, ConnectionMode::Lan);
        assert_eq!(ep_local.ws_url, "ws://localhost:1916/ws/client");
    }

    #[test]
    fn test_tunnel_domain_detection() {
        let ep = ServerEndpoint::from_raw("lab.sekolahkartini7.sch.id").unwrap();
        assert_eq!(ep.mode, ConnectionMode::Tunnel);
        assert_eq!(ep.ws_url, "wss://lab.sekolahkartini7.sch.id/ws/client");

        let ep2 = ServerEndpoint::from_raw("https://molay.example.com").unwrap();
        assert_eq!(ep2.mode, ConnectionMode::Tunnel);
        assert_eq!(ep2.ws_url, "wss://molay.example.com/ws/client");
    }

    #[test]
    fn test_dual_mode_parsing() {
        let config = "192.168.1.50:1916\nlab.sekolahkartini7.sch.id";
        let endpoints = parse_endpoints(config);
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].mode, ConnectionMode::Lan);
        assert_eq!(endpoints[1].mode, ConnectionMode::Tunnel);
    }

    #[test]
    fn test_empty_config_fallback() {
        let endpoints = parse_endpoints("");
        // Kosong => fallback: Lan (Superadmin 192.168.20.70) + Tunnel + Lan (127.0.0.1)
        assert!(endpoints.len() >= 2);
        assert!(endpoints.iter().any(|e| e.mode == ConnectionMode::Tunnel));
        assert!(endpoints.iter().any(|e| e.mode == ConnectionMode::Lan));
    }

    #[test]
    fn test_get_local_lan_ip() {
        // Hanya verifikasi bahwa fungsi tidak panik dan mengembalikan nilai atau None
        let ip = get_local_lan_ip();
        if let Some(v4) = ip {
            assert!(!v4.is_loopback());
            assert!(!v4.is_unspecified());
        }
        // Jika None pun tidak masalah (mesin mungkin tidak punya koneksi jaringan)
    }

    #[test]
    fn test_http_base_url() {
        let ep = ServerEndpoint::from_raw("molay.sekolahkartini7.web.id").unwrap();
        assert_eq!(ep.http_base_url(), "https://molay.sekolahkartini7.web.id");

        let ep_lan = ServerEndpoint::from_raw("192.168.1.100:1916").unwrap();
        assert_eq!(ep_lan.http_base_url(), "http://192.168.1.100:1916");
    }
}
