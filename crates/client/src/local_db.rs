//! Modul Penyimpanan Database Lokal Klien (Offline-First Store & Forward)
//!
//! Bertanggung jawab menampung seluruh log aktivitas, input metrics, dan pelanggaran
//! ke file database lokal (`client_activities_db.jsonl`) saat klien belum tersambung
//! ke Server Guru (localhost:1916 / LAN) maupun Cloud (monitoringmolay.sekolahkartini7.web.id).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared::ActivityEntry;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const LOCAL_DB_FILE: &str = "client_activities_db.jsonl";
pub const MAX_LOCAL_RECORDS: usize = 10_000;
pub const MAX_SYNC_BATCH_SIZE: usize = 100;

static DB_MUTEX: Mutex<()> = Mutex::new(());
static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalActivityRecord {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default)]
    pub synced: bool,
    pub activity: ActivityEntry,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalDbStats {
    pub total_records: usize,
    pub unsynced_records: usize,
    pub file_size_bytes: u64,
}

fn get_default_db_path() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            return parent.join(LOCAL_DB_FILE);
        }
    }
    PathBuf::from(LOCAL_DB_FILE)
}

/// Menghasilkan ID unik untuk entri log lokal
fn generate_record_id() -> String {
    let now_ms = Utc::now().timestamp_millis();
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("loc_{}_{}", now_ms, counter)
}

/// Menyimpan entri aktivitas ke database lokal klien
pub fn save_activity(entry: ActivityEntry) -> String {
    save_activity_to_path(&get_default_db_path(), entry)
}

/// Menyimpan entri aktivitas ke path spesifik (digunakan untuk production dan unit test)
pub fn save_activity_to_path(path: &Path, entry: ActivityEntry) -> String {
    let _lock = DB_MUTEX.lock().unwrap();

    let id = generate_record_id();
    let record = LocalActivityRecord {
        id: id.clone(),
        timestamp: entry.timestamp,
        synced: false,
        activity: entry,
    };

    if let Ok(json) = serde_json::to_string(&record) {
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{}", json);
        }
    }

    id
}

/// Membaca entri yang belum tersinkronisasi (synced == false) hingga batas `limit`
pub fn get_unsynced_activities(limit: usize) -> Vec<LocalActivityRecord> {
    get_unsynced_activities_from_path(&get_default_db_path(), limit)
}

pub fn get_unsynced_activities_from_path(path: &Path, limit: usize) -> Vec<LocalActivityRecord> {
    let _lock = DB_MUTEX.lock().unwrap();
    if !path.exists() {
        return Vec::new();
    }

    let mut unsynced = Vec::new();
    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<LocalActivityRecord>(trimmed) {
                if !rec.synced {
                    unsynced.push(rec);
                    if unsynced.len() >= limit {
                        break;
                    }
                }
            }
        }
    }

    unsynced
}

/// Menandai daftar ID aktivitas sebagai telah tersinkronisasi (synced: true)
pub fn mark_as_synced(ids: &[String]) -> usize {
    mark_as_synced_in_path(&get_default_db_path(), ids)
}

pub fn mark_as_synced_in_path(path: &Path, ids: &[String]) -> usize {
    if ids.is_empty() {
        return 0;
    }
    let _lock = DB_MUTEX.lock().unwrap();
    if !path.exists() {
        return 0;
    }

    let id_set: std::collections::HashSet<&str> = ids.iter().map(|s| s.as_str()).collect();
    let mut all_records = Vec::new();
    let mut updated_count = 0;

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(mut rec) = serde_json::from_str::<LocalActivityRecord>(trimmed) {
                if !rec.synced && id_set.contains(rec.id.as_str()) {
                    rec.synced = true;
                    updated_count += 1;
                }
                all_records.push(rec);
            }
        }
    }

    // Jika total rekaman melebihi batas, lakukan rotasi/pembersihan rekaman lama yang sudah tersinkronisasi
    if all_records.len() > MAX_LOCAL_RECORDS {
        // Pertahankan SEMUA rekaman yang belum disinkronkan, pangkas yang sudah disinkronkan tertua
        let (unsynced_items, mut synced_items): (Vec<_>, Vec<_>) =
            all_records.into_iter().partition(|r| !r.synced);
        let max_synced_keep = MAX_LOCAL_RECORDS.saturating_sub(unsynced_items.len());
        if synced_items.len() > max_synced_keep {
            let drain_start = synced_items.len() - max_synced_keep;
            synced_items.drain(0..drain_start);
        }
        all_records = synced_items;
        all_records.extend(unsynced_items);
        all_records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    // Tulis ulang file secara atomic
    let temp_path = path.with_extension("tmp");
    if let Ok(mut temp_file) = File::create(&temp_path) {
        for rec in &all_records {
            if let Ok(json) = serde_json::to_string(rec) {
                let _ = writeln!(temp_file, "{}", json);
            }
        }
        let _ = temp_file.flush();
        drop(temp_file);
        let _ = std::fs::rename(&temp_path, path);
    }

    updated_count
}

/// Mengambil ringkasan statistik database lokal klien
#[allow(dead_code)]
pub fn get_stats() -> LocalDbStats {
    get_stats_from_path(&get_default_db_path())
}

pub fn get_stats_from_path(path: &Path) -> LocalDbStats {
    let _lock = DB_MUTEX.lock().unwrap();
    if !path.exists() {
        return LocalDbStats::default();
    }

    let mut total = 0;
    let mut unsynced = 0;
    let file_size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    if let Ok(file) = File::open(path) {
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(rec) = serde_json::from_str::<LocalActivityRecord>(trimmed) {
                total += 1;
                if !rec.synced {
                    unsynced += 1;
                }
            }
        }
    }

    LocalDbStats {
        total_records: total,
        unsynced_records: unsynced,
        file_size_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_activity(app: &str, title: &str) -> ActivityEntry {
        ActivityEntry {
            client_id: "PC-TEST-01".to_string(),
            username: Some("siswa01".to_string()),
            app_name: app.to_string(),
            window_title: title.to_string(),
            timestamp: Utc::now(),
            category: Some("APP".to_string()),
            severity: Some("info".to_string()),
            details: None,
            keystrokes: Some(25),
            mouse_clicks: Some(5),
            mouse_distance_px: Some(150),
            idle_seconds: Some(2),
            is_afk: Some(false),
        }
    }

    #[test]
    fn test_save_and_get_unsynced_activities() {
        let test_db = PathBuf::from("test_local_db_1.jsonl");
        let _ = std::fs::remove_file(&test_db);

        let id1 = save_activity_to_path(&test_db, sample_activity("notepad.exe", "Catatan.txt"));
        let id2 = save_activity_to_path(&test_db, sample_activity("calc.exe", "Kalkulator"));

        let unsynced = get_unsynced_activities_from_path(&test_db, 10);
        assert_eq!(unsynced.len(), 2);
        assert_eq!(unsynced[0].id, id1);
        assert_eq!(unsynced[0].activity.app_name, "notepad.exe");
        assert_eq!(unsynced[1].id, id2);
        assert_eq!(unsynced[1].activity.app_name, "calc.exe");

        let stats = get_stats_from_path(&test_db);
        assert_eq!(stats.total_records, 2);
        assert_eq!(stats.unsynced_records, 2);

        let _ = std::fs::remove_file(&test_db);
    }

    #[test]
    fn test_mark_as_synced() {
        let test_db = PathBuf::from("test_local_db_2.jsonl");
        let _ = std::fs::remove_file(&test_db);

        let id1 = save_activity_to_path(&test_db, sample_activity("code.exe", "main.rs"));
        let id2 = save_activity_to_path(&test_db, sample_activity("chrome.exe", "Google"));

        let updated = mark_as_synced_in_path(&test_db, &[id1.clone()]);
        assert_eq!(updated, 1);

        let unsynced = get_unsynced_activities_from_path(&test_db, 10);
        assert_eq!(unsynced.len(), 1);
        assert_eq!(unsynced[0].id, id2);

        let stats = get_stats_from_path(&test_db);
        assert_eq!(stats.total_records, 2);
        assert_eq!(stats.unsynced_records, 1);

        let _ = std::fs::remove_file(&test_db);
    }

    #[test]
    fn test_offline_to_online_batch_sync_flow() {
        let test_db = PathBuf::from("test_local_db_offline_sync.jsonl");
        let _ = std::fs::remove_file(&test_db);

        // 1. Siswa belajar offline: beberapa aplikasi dibuka
        let offline_apps = vec![
            ("word.exe", "Tugas_Sekolah.docx - Microsoft Word"),
            ("excel.exe", "Nilai_Siswa.xlsx - Microsoft Excel"),
            ("robloxplayerbeta.exe", "Roblox Game"),
        ];

        let mut saved_ids = Vec::new();
        for (app, title) in &offline_apps {
            let id = save_activity_to_path(&test_db, sample_activity(app, title));
            saved_ids.push(id);
        }

        // 2. Verifikasi status saat offline
        let unsynced_offline = get_unsynced_activities_from_path(&test_db, 100);
        assert_eq!(unsynced_offline.len(), 3);
        let stats_offline = get_stats_from_path(&test_db);
        assert_eq!(stats_offline.unsynced_records, 3);

        // 3. Server online: klien mengambil batch dan menandai telah tersinkron
        let batch_ids: Vec<String> = unsynced_offline.into_iter().map(|r| r.id).collect();
        let synced_count = mark_as_synced_in_path(&test_db, &batch_ids);
        assert_eq!(synced_count, 3);

        // 4. Verifikasi status setelah sync online
        let unsynced_online = get_unsynced_activities_from_path(&test_db, 100);
        assert_eq!(unsynced_online.len(), 0);
        let stats_online = get_stats_from_path(&test_db);
        assert_eq!(stats_online.total_records, 3);
        assert_eq!(stats_online.unsynced_records, 0);

        let _ = std::fs::remove_file(&test_db);
    }
}
