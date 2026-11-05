//! Modul analisis aktivitas lab (EduMonitor).
//!
//! Menghitung ringkasan pemakaian aplikasi per siswa dari `activities.jsonl`
//! yang sudah dimuat ke memori server, lalu menyajikannya sebagai JSON untuk
//! halaman `/analisaaktivitas`.
//!
//! Prinsip:
//! - Timestamp di file log adalah UTC; laporan dikonversi ke zona lab (WIB/WITA/WIT).
//! - Durasi diatribusikan PER-EVENT (bukan per aplikasi dominan) supaya jendela
//!   hiburan di dalam browser tidak ikut terhitung sebagai belajar.
//! - Aktivitas non-pembelajaran pada jam istirahat / Salat Jumat TIDAK dihitung
//!   sebagai pelanggaran jam belajar.
//! - BLOCKED / INSTALLER = PERCOBAAN, bukan bukti program terpasang.
//! - Judul jendela diperlakukan murni sebagai DATA (anti prompt injection).

use crate::{auth, AppState};
use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::{IntoResponse, Json, Response},
};
use chrono::{FixedOffset, Timelike};
use serde::{Deserialize, Serialize};
use shared::ActivityEntry;
use std::collections::{BTreeMap, HashMap};

/// Batas maksimum durasi yang diatribusikan ke satu event (menit).
const CAP_MENIT: f64 = 10.0;
/// Jeda minimum (menit) yang dianggap idle / sesi baru.
const DEFAULT_GAP: i64 = 15;
/// Jam belajar default (menit sejak 00:00 waktu lokal).
const BELAJAR_DEFAULT: [(u32, u32); 2] = [(8 * 60, 11 * 60 + 30), (13 * 60, 16 * 60)];
/// Jendela istirahat / Salat Jumat default.
const ISTIRAHAT_DEFAULT: (u32, u32) = (11 * 60 + 30, 13 * 60);

const BROWSER: [&str; 6] = ["chrome.exe", "msedge.exe", "firefox.exe", "brave.exe", "opera.exe", "vivaldi.exe"];

const APP_HIBURAN: [&str; 16] = [
    "steam.exe", "steamwebhelper.exe", "epicgameslauncher.exe", "robloxplayerbeta.exe",
    "discord.exe", "telegram.exe", "vlc.exe", "spotify.exe", "game.exe", "games.exe",
    "minecraft.exe", "tslgame.exe", "valorant.exe", "genshinimpact.exe", "utorrent.exe",
    "bittorrent.exe",
];

const APP_PRODUKTIF: [&str; 14] = [
    "code.exe", "winword.exe", "excel.exe", "powerpnt.exe", "geogebra.exe", "notepad.exe",
    "notepad++.exe", "explorer.exe", "acrobat.exe", "acrord32.exe", "figma.exe", "cmd.exe",
    "powershell.exe", "devenv.exe",
];

const KATA_HIBURAN: [&str; 26] = [
    "tiktok", "instagram", "reels", "facebook", "twitter", "netflix", "drakor", "highlight",
    "gameplay", "game online", "mobile legends", "free fire", "pubg", "minecraft", "steam",
    "discord", "spotify", "musik", "music", "lagu", "film", "movie", "shopee", "tokopedia",
    "9gag", "reddit",
];

const KATA_BELAJAR: [&str; 32] = [
    "classroom", "google docs", "google drive", "google sheets", "google slides", "scholar",
    "github", "stack overflow", "stackoverflow", "w3schools", "tutorial", "modul", "materi",
    "tugas", "latihan", "kuis", "ujian", "belajar", "pembelajaran", "praktikum", "silabus",
    "rumus", "kalkulator", "docs.google", "drive.google", "classroom.google", "canva", "figma",
    "wikipedia", "quizizz", "kahoot", "edmodo",
];

const KATA_INJEKSI: [&str; 10] = [
    "ignore previous", "abaikan instruksi", "abaikan perintah", "lupakan instruksi",
    "system prompt", "you are now", "as an ai", "hapus catatan", "hapus absensi", "prompt:",
];

#[derive(Deserialize)]
pub struct AnalisaQuery {
    pub token: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    pub tanggal: Option<String>,
    pub zona: Option<String>,
    pub gap: Option<i64>,
    /// Contoh: "08:00-11:30,13:00-16:00"
    pub belajar: Option<String>,
    /// Contoh: "11:30-13:00"
    pub istirahat: Option<String>,
}

#[derive(Serialize)]
pub struct Meta {
    pub tanggal: String,
    pub zona: String,
    pub offset_jam: i32,
    pub gap_menit: i64,
    pub total_event: usize,
    pub event_terpakai: usize,
    pub jumlah_siswa: usize,
    pub jumlah_pc: usize,
    pub mulai: Option<String>,
    pub selesai: Option<String>,
    pub tanggal_tersedia: Vec<String>,
    pub jendela_belajar: Vec<String>,
    pub jendela_istirahat: String,
    pub sumber: String,
    pub catatan_durasi: String,
    pub total_keystrokes: u64,
    pub total_mouse_clicks: u64,
}

#[derive(Serialize)]
pub struct Komposisi {
    pub kategori: String,
    pub menit: f64,
    pub persen: f64,
}

#[derive(Serialize)]
pub struct AppRingkas {
    pub app: String,
    pub kategori: String,
    pub menit: f64,
    pub event: usize,
    pub persen: f64,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
}

#[derive(Serialize)]
pub struct SiswaRingkas {
    pub username: String,
    pub pc: Vec<String>,
    pub menit: f64,
    pub produktif: f64,
    pub nonproduktif: f64,
    pub netral: f64,
    pub pelanggaran: f64,
    pub sistem: f64,
    pub persen_produktif: f64,
    pub persen_nonproduktif: f64,
    pub nonproduktif_jam_belajar: f64,
    pub nonproduktif_istirahat: f64,
    pub event: usize,
    pub mulai: String,
    pub selesai: String,
    pub idle_menit: f64,
    pub percobaan_blokir: usize,
    pub keystrokes: u64,
    pub mouse_clicks: u64,
    pub mouse_distance_px: u64,
    pub keystrokes_per_menit: f64,
    pub aplikasi_teratas: Vec<AppRingkas>,
}

#[derive(Serialize)]
pub struct PelanggaranItem {
    pub username: String,
    pub pc: String,
    pub waktu: String,
    pub app: String,
    pub judul: String,
    pub jenis: String,
    pub detail: String,
}

#[derive(Serialize)]
pub struct IdleItem {
    pub username: String,
    pub mulai: String,
    pub selesai: String,
    pub menit: f64,
}

#[derive(Serialize)]
pub struct Validasi {
    pub status_kebijakan: String,
    pub konteks_waktu: String,
    pub analisis_risiko: String,
    pub keamanan_data: String,
    pub total_nonproduktif_jam_belajar: f64,
    pub total_nonproduktif_istirahat: f64,
    pub siswa_terindikasi: Vec<String>,
    pub siswa_bersih: Vec<String>,
    pub sinyal_injeksi: Vec<String>,
    pub catatan: Vec<String>,
}

#[derive(Serialize)]
pub struct AnalisaResponse {
    pub meta: Meta,
    pub komposisi: Vec<Komposisi>,
    pub siswa: Vec<SiswaRingkas>,
    pub aplikasi: Vec<AppRingkas>,
    pub pelanggaran: Vec<PelanggaranItem>,
    pub idle: Vec<IdleItem>,
    pub validasi: Validasi,
}

fn parse_zona(zona: &str) -> (String, i32) {
    match zona.trim().to_uppercase().as_str() {
        "WITA" => ("WITA".to_string(), 8),
        "WIT" => ("WIT".to_string(), 9),
        "UTC" => ("UTC".to_string(), 0),
        other => {
            // Dukung format "UTC+7" / "+7"
            if let Some(rest) = other.strip_prefix("UTC") {
                if let Ok(v) = rest.trim_start_matches('+').parse::<i32>() {
                    return (format!("UTC{:+}", v), v);
                }
            }
            ("WIB".to_string(), 7)
        }
    }
}

fn parse_jendela(spec: &str, fallback: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for bagian in spec.split(',') {
        let mut it = bagian.split('-');
        let (a, b) = match (it.next(), it.next()) {
            (Some(a), Some(b)) => (a.trim(), b.trim()),
            _ => continue,
        };
        let to_menit = |s: &str| -> Option<u32> {
            let mut p = s.split(':');
            let h: u32 = p.next()?.trim().parse().ok()?;
            let m: u32 = p.next().unwrap_or("0").trim().parse().ok()?;
            Some(h * 60 + m)
        };
        if let (Some(x), Some(y)) = (to_menit(a), to_menit(b)) {
            if y > x {
                out.push((x, y));
            }
        }
    }
    if out.is_empty() {
        fallback.to_vec()
    } else {
        out
    }
}

fn fmt_menit(m: u32) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn kategori_event(e: &ActivityEntry) -> &'static str {
    let cat = e.category.as_deref().unwrap_or("APP").to_uppercase();
    if cat == "BLOCKED" || cat == "INSTALLER" {
        return "pelanggaran";
    }
    if cat == "KENDALA" || cat == "SYSTEM" {
        return "sistem";
    }

    let app = e.app_name.to_lowercase();
    let judul = e.window_title.to_lowercase();

    if app.contains("molayclient") || judul.contains("client locker") || app.contains("sentinel") || judul.contains("hard disk") || app.contains("lively") || judul.contains("monitoring layar lab") || judul.contains("molay sekar7") {
        return "sistem";
    }

    if APP_HIBURAN.iter().any(|a| app == *a) {
        return "nonproduktif";
    }

    if BROWSER.iter().any(|b| app == *b) {
        // Browser dinilai dari judul tab: tutorial tetap produktif, musik/hiburan tidak.
        if KATA_HIBURAN.iter().any(|k| judul.contains(k)) {
            return "nonproduktif";
        }
        if KATA_BELAJAR.iter().any(|k| judul.contains(k)) {
            return "produktif";
        }
        return "netral";
    }

    if APP_PRODUKTIF.iter().any(|a| app == *a) {
        return "produktif";
    }

    // Aplikasi tak dikenal: cek judul jendela sebagai petunjuk terakhir.
    if KATA_HIBURAN.iter().any(|k| judul.contains(k)) {
        return "nonproduktif";
    }
    if KATA_BELAJAR.iter().any(|k| judul.contains(k)) {
        return "produktif";
    }
    "netral"
}

fn jendela_waktu(menit: u32, belajar: &[(u32, u32)], istirahat: (u32, u32)) -> &'static str {
    if menit >= istirahat.0 && menit < istirahat.1 {
        return "istirahat";
    }
    if belajar.iter().any(|(a, b)| menit >= *a && menit < *b) {
        return "belajar";
    }
    "luar"
}

pub async fn get_analisa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AnalisaQuery>,
) -> Response {
    // ATURAN IZIN: hanya guru/admin atau Service API Key terdaftar yang boleh membaca analisis layar siswa.
    let effective_token = query.api_key.as_deref().or(query.token.as_deref());
    let sesi = auth::check_dashboard_auth(&headers, effective_token, &state.session_store).await;
    if sesi.is_none() {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response();
    }

    let (zona, offset_jam) = parse_zona(query.zona.as_deref().unwrap_or("WIB"));
    let tz = match FixedOffset::east_opt(offset_jam * 3600) {
        Some(t) => t,
        None => FixedOffset::east_opt(7 * 3600).unwrap(),
    };
    let gap_menit = query.gap.unwrap_or(DEFAULT_GAP).max(1);
    let belajar = match query.belajar.as_deref() {
        Some(s) => parse_jendela(s, &BELAJAR_DEFAULT),
        None => BELAJAR_DEFAULT.to_vec(),
    };
    let istirahat = match query.istirahat.as_deref() {
        Some(s) => {
            let v = parse_jendela(s, &[ISTIRAHAT_DEFAULT]);
            v[0]
        }
        None => ISTIRAHAT_DEFAULT,
    };

    // Jika di memori masih kosong, coba muat ulang dari file agar data selalu mutakhir
    {
        let acts_r = state.activities.read().await;
        if acts_r.is_empty() {
            drop(acts_r);
            let fresh = crate::load_persisted_activities();
            if !fresh.is_empty() {
                let mut acts_w = state.activities.write().await;
                *acts_w = fresh;
            }
        }
    }

    let acts = state.activities.read().await;

    // Daftar tanggal (waktu lokal) yang tersedia di log, terbaru dulu.
    let mut tanggal_set: Vec<String> = acts
        .iter()
        .map(|e| e.timestamp.with_timezone(&tz).format("%Y-%m-%d").to_string())
        .collect();
    tanggal_set.sort();
    tanggal_set.dedup();
    tanggal_set.reverse();
    tanggal_set.truncate(60);

    let is_all = match query.tanggal.as_deref() {
        Some(t) => {
            let lower = t.trim().to_lowercase();
            lower == "all" || lower == "semua" || lower == "*"
        }
        None => false,
    };

    let tanggal = if is_all {
        "all".to_string()
    } else {
        match query.tanggal.as_deref() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => tanggal_set.first().cloned().unwrap_or_else(|| {
                chrono::Utc::now().with_timezone(&tz).format("%Y-%m-%d").to_string()
            }),
        }
    };

    let enriched_acts: Vec<ActivityEntry> = acts.iter().map(|e| e.clone().enriched()).collect();

    let mut events: Vec<&ActivityEntry> = if is_all {
        enriched_acts.iter().collect()
    } else {
        enriched_acts
            .iter()
            .filter(|e| e.timestamp.with_timezone(&tz).format("%Y-%m-%d").to_string() == tanggal)
            .collect()
    };
    events.sort_by_key(|e| e.timestamp);

    let total_event = acts.len();
    let event_terpakai = events.len();

    // ---- pengelompokan per siswa ----
    let mut per_siswa: BTreeMap<String, Vec<&ActivityEntry>> = BTreeMap::new();
    for e in &events {
        let key = e
            .username
            .clone()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| e.client_id.clone());
        per_siswa.entry(key).or_default().push(e);
    }

    let mut siswa_out: Vec<SiswaRingkas> = Vec::new();
    let mut app_total: HashMap<String, (f64, usize, HashMap<&'static str, f64>, u64, u64)> = HashMap::new();
    let mut pelanggaran_out: Vec<PelanggaranItem> = Vec::new();
    let mut idle_out: Vec<IdleItem> = Vec::new();
    let mut komposisi: HashMap<&'static str, f64> = HashMap::new();

    let mut mulai_global: Option<String> = None;
    let mut selesai_global: Option<String> = None;
    let mut global_keys: u64 = 0;
    let mut global_clicks: u64 = 0;

    for (username, list) in &per_siswa {
        let mut menit = 0.0;
        let mut produktif = 0.0;
        let mut nonproduktif = 0.0;
        let mut netral = 0.0;
        let mut pelanggaran = 0.0;
        let mut sistem = 0.0;
        let mut nonprod_belajar = 0.0;
        let mut nonprod_istirahat = 0.0;
        let mut idle_menit = 0.0;
        let mut percobaan = 0usize;
        let mut total_keys: u64 = 0;
        let mut total_clicks: u64 = 0;
        let mut total_dist: u64 = 0;
        let mut app_siswa: HashMap<String, (f64, usize, HashMap<&'static str, f64>, u64, u64)> = HashMap::new();
        let mut pc_set: Vec<String> = Vec::new();

        for (i, e) in list.iter().enumerate() {
            let lokal = e.timestamp.with_timezone(&tz);
            let menit_hari = lokal.hour() * 60 + lokal.minute();

            let ev_keys = e.keystrokes.unwrap_or(0);
            let ev_clicks = e.mouse_clicks.unwrap_or(0);
            let ev_dist = e.mouse_distance_px.unwrap_or(0);
            total_keys += ev_keys;
            total_clicks += ev_clicks;
            total_dist += ev_dist;
            global_keys += ev_keys;
            global_clicks += ev_clicks;

            let durasi = if i + 1 < list.len() {
                let selisih = (list[i + 1].timestamp - e.timestamp).num_seconds() as f64 / 60.0;
                if selisih < 0.0 {
                    0.0
                } else if selisih > gap_menit as f64 {
                    // Jika selisih melebihi jeda idle/gap (mis. 15 menit), jangan anggap seluruh jeda
                    // sebagai pemakaian aktif aplikasi sebelumnya; batasi ke durasi wajar 1 menit
                    1.0
                } else {
                    // Berikan durasi minimum 0.2 menit (12 detik) per aksi jendela agar tidak terpotong jadi 0.0
                    selisih.max(0.2).min(CAP_MENIT)
                }
            } else {
                // Event terakhir: beri 1 menit sebagai penanda, bukan durasi sebenarnya.
                1.0
            };

            if i + 1 < list.len() {
                let jeda = (list[i + 1].timestamp - e.timestamp).num_seconds() as f64 / 60.0;
                if jeda > gap_menit as f64 {
                    idle_menit += jeda;
                    idle_out.push(IdleItem {
                        username: username.clone(),
                        mulai: lokal.format("%H:%M").to_string(),
                        selesai: list[i + 1].timestamp.with_timezone(&tz).format("%H:%M").to_string(),
                        menit: (jeda * 10.0).round() / 10.0,
                    });
                }
            }

            let kat = kategori_event(e);
            let jendela = jendela_waktu(menit_hari, &belajar, istirahat);

            menit += durasi;
            *komposisi.entry(kat).or_insert(0.0) += durasi;
            match kat {
                "produktif" => produktif += durasi,
                "nonproduktif" => {
                    nonproduktif += durasi;
                    match jendela {
                        "belajar" => nonprod_belajar += durasi,
                        "istirahat" => nonprod_istirahat += durasi,
                        _ => {}
                    }
                }
                "pelanggaran" => {
                    pelanggaran += durasi;
                    percobaan += 1;
                }
                "sistem" => sistem += durasi,
                _ => netral += durasi,
            }

            let entry_app = app_siswa
                .entry(e.app_name.clone())
                .or_insert_with(|| (0.0, 0, HashMap::new(), 0, 0));
            entry_app.0 += durasi;
            entry_app.1 += 1;
            *entry_app.2.entry(kat).or_insert(0.0) += durasi;
            entry_app.3 += ev_keys;
            entry_app.4 += ev_clicks;

            let entry_total = app_total
                .entry(e.app_name.clone())
                .or_insert_with(|| (0.0, 0, HashMap::new(), 0, 0));
            entry_total.0 += durasi;
            entry_total.1 += 1;
            *entry_total.2.entry(kat).or_insert(0.0) += durasi;
            entry_total.3 += ev_keys;
            entry_total.4 += ev_clicks;

            if !pc_set.contains(&e.client_id) {
                pc_set.push(e.client_id.clone());
            }

            if kat == "pelanggaran" {
                pelanggaran_out.push(PelanggaranItem {
                    username: username.clone(),
                    pc: e.client_id.clone(),
                    waktu: lokal.format("%H:%M").to_string(),
                    app: e.app_name.clone(),
                    judul: e.window_title.clone(),
                    jenis: e.category.clone().unwrap_or_else(|| "BLOCKED".to_string()),
                    detail: e.details.clone().unwrap_or_default(),
                });
            }
        }

        let pertama = list.first().map(|e| e.timestamp.with_timezone(&tz).format("%H:%M").to_string()).unwrap_or_default();
        let terakhir = list.last().map(|e| e.timestamp.with_timezone(&tz).format("%H:%M").to_string()).unwrap_or_default();

        if mulai_global.is_none() || pertama < *mulai_global.as_ref().unwrap() {
            mulai_global = Some(pertama.clone());
        }
        if selesai_global.is_none() || terakhir > *selesai_global.as_ref().unwrap() {
            selesai_global = Some(terakhir.clone());
        }

        let mut apps: Vec<AppRingkas> = app_siswa
            .into_iter()
            .map(|(app, (m, ev, per_kat, k, c))| {
                let (kat, _) = per_kat
                    .iter()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(k, v)| (*k, *v))
                    .unwrap_or(("netral", 0.0));
                AppRingkas {
                    app,
                    kategori: kat.to_string(),
                    menit: (m * 10.0).round() / 10.0,
                    event: ev,
                    persen: if menit > 0.0 { (m / menit * 1000.0).round() / 10.0 } else { 0.0 },
                    keystrokes: k,
                    mouse_clicks: c,
                }
            })
            .collect();
        apps.sort_by(|a, b| b.menit.partial_cmp(&a.menit).unwrap_or(std::cmp::Ordering::Equal));
        apps.truncate(6);

        siswa_out.push(SiswaRingkas {
            username: username.clone(),
            pc: pc_set,
            menit: (menit * 10.0).round() / 10.0,
            produktif: (produktif * 10.0).round() / 10.0,
            nonproduktif: (nonproduktif * 10.0).round() / 10.0,
            netral: (netral * 10.0).round() / 10.0,
            pelanggaran: (pelanggaran * 10.0).round() / 10.0,
            sistem: (sistem * 10.0).round() / 10.0,
            persen_produktif: if menit > 0.0 { (produktif / menit * 1000.0).round() / 10.0 } else { 0.0 },
            persen_nonproduktif: if menit > 0.0 { (nonproduktif / menit * 1000.0).round() / 10.0 } else { 0.0 },
            nonproduktif_jam_belajar: (nonprod_belajar * 10.0).round() / 10.0,
            nonproduktif_istirahat: (nonprod_istirahat * 10.0).round() / 10.0,
            event: list.len(),
            mulai: pertama,
            selesai: terakhir,
            idle_menit: (idle_menit * 10.0).round() / 10.0,
            percobaan_blokir: percobaan,
            keystrokes: total_keys,
            mouse_clicks: total_clicks,
            mouse_distance_px: total_dist,
            keystrokes_per_menit: if menit > 0.0 { (total_keys as f64 / menit * 10.0).round() / 10.0 } else { 0.0 },
            aplikasi_teratas: apps,
        });
    }

    siswa_out.sort_by(|a, b| b.menit.partial_cmp(&a.menit).unwrap_or(std::cmp::Ordering::Equal));

    let total_menit: f64 = komposisi.values().sum();
    let urutan = ["produktif", "nonproduktif", "netral", "pelanggaran", "sistem"];
    let komposisi_out: Vec<Komposisi> = urutan
        .iter()
        .filter_map(|k| komposisi.get(k).map(|v| (*k, *v)))
        .map(|(k, v)| Komposisi {
            kategori: k.to_string(),
            menit: (v * 10.0).round() / 10.0,
            persen: if total_menit > 0.0 { (v / total_menit * 1000.0).round() / 10.0 } else { 0.0 },
        })
        .collect();

    let mut aplikasi_out: Vec<AppRingkas> = app_total
        .into_iter()
        .map(|(app, (m, ev, per_kat, k, c))| {
            let (kat, _) = per_kat
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(k, v)| (*k, *v))
                .unwrap_or(("netral", 0.0));
            AppRingkas {
                app,
                kategori: kat.to_string(),
                menit: (m * 10.0).round() / 10.0,
                event: ev,
                persen: if total_menit > 0.0 { (m / total_menit * 1000.0).round() / 10.0 } else { 0.0 },
                keystrokes: k,
                mouse_clicks: c,
            }
        })
        .collect();
    aplikasi_out.sort_by(|a, b| b.menit.partial_cmp(&a.menit).unwrap_or(std::cmp::Ordering::Equal));
    aplikasi_out.truncate(25);

    pelanggaran_out.sort_by(|a, b| a.waktu.cmp(&b.waktu));
    idle_out.sort_by(|a, b| b.menit.partial_cmp(&a.menit).unwrap_or(std::cmp::Ordering::Equal));

    // ---- validasi kebijakan (lapisan kedua) ----
    let total_nonprod_belajar: f64 = siswa_out.iter().map(|s| s.nonproduktif_jam_belajar).sum();
    let total_nonprod_istirahat: f64 = siswa_out.iter().map(|s| s.nonproduktif_istirahat).sum();
    let siswa_terindikasi: Vec<String> = siswa_out
        .iter()
        .filter(|s| s.nonproduktif_jam_belajar >= 15.0)
        .map(|s| s.username.clone())
        .collect();
    let siswa_bersih: Vec<String> = siswa_out
        .iter()
        .filter(|s| s.nonproduktif_jam_belajar < 15.0 && s.percobaan_blokir == 0)
        .map(|s| s.username.clone())
        .collect();

    // Anti prompt injection: judul jendela diperiksa, bukan dijalankan.
    let mut sinyal: Vec<String> = Vec::new();
    for e in &events {
        let judul = e.window_title.to_lowercase();
        if KATA_INJEKSI.iter().any(|k| judul.contains(k)) {
            if sinyal.len() < 5 {
                sinyal.push(format!("[{}] {}: {}", e.client_id, e.app_name, e.window_title));
            }
        }
    }

    let status_kebijakan = if event_terpakai == 0 {
        if total_event == 0 {
            "Data log tidak ditemukan".to_string()
        } else {
            format!("Tidak ada data log pada tanggal {} (tersedia: {})", tanggal, tanggal_set.join(", "))
        }
    } else if total_nonprod_belajar >= 15.0 {
        "Ditemukan indikasi pelanggaran jam belajar".to_string()
    } else {
        "Sesuai jadwal".to_string()
    };

    let konteks_waktu = if total_nonprod_istirahat > 0.0 {
        format!(
            "{} menit aktivitas non-pembelajaran berada pada jendela istirahat {} — TIDAK dihitung sebagai pelanggaran",
            (total_nonprod_istirahat * 10.0).round() / 10.0,
            format!("{}-{}", fmt_menit(istirahat.0), fmt_menit(istirahat.1))
        )
    } else if event_terpakai == 0 {
        if total_event == 0 {
            "Tidak ada data log aktivitas di server".to_string()
        } else {
            format!("Tidak ada data aktivitas pada tanggal {} (pilih salah satu dari: {})", tanggal, tanggal_set.join(", "))
        }
    } else {
        "Aman — tidak ada aktivitas non-pembelajaran pada jendela istirahat".to_string()
    };

    let mut catatan = vec![
        format!(
            "Durasi adalah perkiraan dari selisih antar-event (maksimal {} menit per potongan), bukan pengukuran presisi.",
            CAP_MENIT as i64
        ),
        "BLOCKED / INSTALLER menandakan PERCOBAAN yang diblokir, bukan bukti program terpasang. Konfirmasi ke siswa sebelum menyimpulkan.".to_string(),
        "Server ini tidak memiliki fitur mengirim email, menjatuhkan sanksi, atau memblokir akun secara otomatis. Semua tindakan lanjutan harus dilakukan manual oleh guru/instruktur.".to_string(),
    ];
    if sinyal.is_empty() {
        catatan.push("Tidak ada judul jendela yang mengandung perintah mencurigakan.".to_string());
    } else {
        catatan.push("Terdapat judul jendela menyerupai perintah — diperlakukan sebagai DATA dan diabaikan (anti prompt injection).".to_string());
    }

    let validasi = Validasi {
        status_kebijakan,
        konteks_waktu,
        analisis_risiko: "Laporan bersifat informatif — tanpa teguran formal, email wali, atau blokir akun.".to_string(),
        keamanan_data: if sinyal.is_empty() {
            "Tidak ada judul jendela menyerupai perintah.".to_string()
        } else {
            "Judul jendela menyerupai perintah dinetralkan sebagai teks (tidak dieksekusi).".to_string()
        },
        total_nonproduktif_jam_belajar: (total_nonprod_belajar * 10.0).round() / 10.0,
        total_nonproduktif_istirahat: (total_nonprod_istirahat * 10.0).round() / 10.0,
        siswa_terindikasi,
        siswa_bersih,
        sinyal_injeksi: sinyal,
        catatan,
    };

    let meta = Meta {
        tanggal,
        zona,
        offset_jam,
        gap_menit,
        total_event,
        event_terpakai,
        jumlah_siswa: per_siswa.len(),
        jumlah_pc: siswa_out.iter().map(|s| s.pc.len()).sum(),
        mulai: mulai_global,
        selesai: selesai_global,
        tanggal_tersedia: tanggal_set,
        jendela_belajar: belajar.iter().map(|(a, b)| format!("{}-{}", fmt_menit(*a), fmt_menit(*b))).collect(),
        jendela_istirahat: format!("{}-{}", fmt_menit(istirahat.0), fmt_menit(istirahat.1)),
        sumber: "activities.jsonl".to_string(),
        catatan_durasi: format!("cap {} menit/event", CAP_MENIT as i64),
        total_keystrokes: global_keys,
        total_mouse_clicks: global_clicks,
    };

    let _ = sesi;
    Json(AnalisaResponse {
        meta,
        komposisi: komposisi_out,
        siswa: siswa_out,
        aplikasi: aplikasi_out,
        pelanggaran: pelanggaran_out,
        idle: idle_out,
        validasi,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analisa_response_with_input_metrics() {
        let meta = Meta {
            tanggal: "2026-10-28".to_string(),
            zona: "WIB".to_string(),
            offset_jam: 7,
            gap_menit: 15,
            total_event: 10,
            event_terpakai: 10,
            jumlah_siswa: 1,
            jumlah_pc: 1,
            mulai: Some("08:00".to_string()),
            selesai: Some("09:30".to_string()),
            tanggal_tersedia: vec!["2026-10-28".to_string()],
            jendela_belajar: vec!["08:00-11:30".to_string()],
            jendela_istirahat: "11:30-13:00".to_string(),
            sumber: "activities.jsonl".to_string(),
            catatan_durasi: "cap 10 menit".to_string(),
            total_keystrokes: 1850,
            total_mouse_clicks: 420,
        };

        let siswa = SiswaRingkas {
            username: "siswa01".to_string(),
            pc: vec!["PC-01".to_string()],
            menit: 45.0,
            produktif: 40.0,
            nonproduktif: 5.0,
            netral: 0.0,
            pelanggaran: 0.0,
            sistem: 0.0,
            persen_produktif: 88.9,
            persen_nonproduktif: 11.1,
            nonproduktif_jam_belajar: 5.0,
            nonproduktif_istirahat: 0.0,
            event: 10,
            mulai: "08:00".to_string(),
            selesai: "08:45".to_string(),
            idle_menit: 0.0,
            percobaan_blokir: 0,
            keystrokes: 1850,
            mouse_clicks: 420,
            mouse_distance_px: 35000,
            keystrokes_per_menit: 41.1,
            aplikasi_teratas: vec![AppRingkas {
                app: "code.exe".to_string(),
                kategori: "produktif".to_string(),
                menit: 35.0,
                event: 8,
                persen: 77.8,
                keystrokes: 1600,
                mouse_clicks: 300,
            }],
        };

        let resp = AnalisaResponse {
            meta,
            komposisi: vec![],
            siswa: vec![siswa],
            aplikasi: vec![],
            pelanggaran: vec![],
            idle: vec![],
            validasi: Validasi {
                status_kebijakan: "Aman".to_string(),
                konteks_waktu: "Aman".to_string(),
                analisis_risiko: "Rendah".to_string(),
                keamanan_data: "Aman".to_string(),
                total_nonproduktif_jam_belajar: 0.0,
                total_nonproduktif_istirahat: 0.0,
                siswa_terindikasi: vec![],
                siswa_bersih: vec!["siswa01".to_string()],
                sinyal_injeksi: vec![],
                catatan: vec![],
            },
        };

        let json = serde_json::to_string(&resp).expect("AnalisaResponse harus bisa di-serialize");
        assert!(json.contains("\"total_keystrokes\":1850"));
        assert!(json.contains("\"total_mouse_clicks\":420"));
        assert!(json.contains("\"keystrokes_per_menit\":41.1"));
    }

    #[test]
    fn test_kategori_event_system_and_learning() {
        use chrono::Utc;

        let ev_molay = ActivityEntry {
            client_id: "PC5LAB2".to_string(),
            username: None,
            app_name: "MolayClient-Sekar7.exe".to_string(),
            window_title: "Molay Sekar7 - Client Locker".to_string(),
            timestamp: Utc::now(),
            category: None,
            severity: None,
            details: None,
            keystrokes: None,
            mouse_clicks: None,
            mouse_distance_px: None,
            idle_seconds: None,
            is_afk: None,
        };
        assert_eq!(kategori_event(&ev_molay), "sistem");

        let ev_sentinel = ActivityEntry {
            client_id: "PC5LAB2".to_string(),
            username: None,
            app_name: "unknown".to_string(),
            window_title: "Disk: 0, ST1000DM010-2EP102 [Z9ARJA13] - Hard Disk Sentinel".to_string(),
            timestamp: Utc::now(),
            category: None,
            severity: None,
            details: None,
            keystrokes: None,
            mouse_clicks: None,
            mouse_distance_px: None,
            idle_seconds: None,
            is_afk: None,
        };
        assert_eq!(kategori_event(&ev_sentinel), "sistem");

        let ev_gdrive = ActivityEntry {
            client_id: "PC5LAB2".to_string(),
            username: Some("Ahmad Siswa 01".to_string()),
            app_name: "firefox.exe".to_string(),
            window_title: "kebutuhan file download - Google Drive — Mozilla Firefox".to_string(),
            timestamp: Utc::now(),
            category: None,
            severity: None,
            details: None,
            keystrokes: None,
            mouse_clicks: None,
            mouse_distance_px: None,
            idle_seconds: None,
            is_afk: None,
        };
        assert_eq!(kategori_event(&ev_gdrive), "produktif");
    }
}
