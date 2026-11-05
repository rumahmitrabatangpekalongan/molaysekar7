use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use tokio::time::sleep;

pub fn get_local_lan_ip() -> String {
    // 1. Coba probe routing via UDP (tidak mengirim paket nyata ke internet)
    let probe_targets = [
        "192.168.20.1:80",
        "192.168.20.70:80",
        "192.168.20.100:80",
        "8.8.8.8:80",
        "1.1.1.1:80",
        "192.168.1.1:80",
        "192.168.0.1:80",
        "192.168.100.1:80",
        "10.0.0.1:80",
    ];
    for target in &probe_targets {
        if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
            if socket.connect(target).is_ok() {
                if let Ok(addr) = socket.local_addr() {
                    let ip = addr.ip().to_string();
                    if ip != "0.0.0.0" && !ip.starts_with("127.") {
                        return ip;
                    }
                }
            }
        }
    }

    "127.0.0.1".to_string()
}

pub async fn start_udp_discovery_responder(port: u16) {
    let bind_addr = format!("0.0.0.0:{}", port);
    let socket = match tokio::net::UdpSocket::bind(&bind_addr).await {
        Ok(s) => {
            let _ = s.set_broadcast(true);
            println!("[DISCOVERY] Layanan Auto-Discovery LAN aktif di UDP port {}", port);
            s
        }
        Err(e) => {
            eprintln!("[DISCOVERY WARNING] Tidak dapat membuka UDP discovery di port {}: {}", port, e);
            return;
        }
    };

    let arc_socket = std::sync::Arc::new(socket);

    // Task 1: Responder untuk permintaan "MOLAY_DISCOVER" dari klien
    let s_rx = arc_socket.clone();
    tokio::spawn(async move {
        let mut buf = [0u8; 512];
        loop {
            match s_rx.recv_from(&mut buf).await {
                Ok((size, peer_addr)) => {
                    let msg = String::from_utf8_lossy(&buf[..size]);
                    if msg.trim().starts_with("MOLAY_DISCOVER") {
                        let response = format!("MOLAY_SERVER:{}", port);
                        let _ = s_rx.send_to(response.as_bytes(), peer_addr).await;
                    }
                }
                Err(_) => {
                    sleep(Duration::from_millis(500)).await;
                }
            }
        }
    });

    // Task 2: Broadcast periodik ke subnet LAN agar klien langsung mengenali server
    let s_tx = arc_socket.clone();
    let current_ip = get_local_lan_ip();
    tokio::spawn(async move {
        let mut broadcast_targets: Vec<SocketAddr> = vec![
            "255.255.255.255:1916".parse().unwrap(),
            "192.168.20.255:1916".parse().unwrap(),
        ];
        if let Ok(ip) = current_ip.parse::<std::net::Ipv4Addr>() {
            let oct = ip.octets();
            if let Ok(sub) = format!("{}.{}.{}.255:{}", oct[0], oct[1], oct[2], port).parse() {
                if !broadcast_targets.contains(&sub) {
                    broadcast_targets.push(sub);
                }
            }
        }
        let announce_msg = format!("MOLAY_ANNOUNCE:{}", port);
        loop {
            sleep(Duration::from_secs(5)).await;
            for target in &broadcast_targets {
                let _ = s_tx.send_to(announce_msg.as_bytes(), target).await;
            }
        }
    });
}
