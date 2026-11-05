# Laporan Analisis Aktivitas Pembelajaran Lab Komputer

> **DATA SIMULASI - BUKAN DATA SISWA SUNGGUHAN** — angka pada laporan ini BUKAN data siswa sungguhan.

| Keterangan | Nilai |
|---|---|
| Sumber data | `activities_demo_SIMULASI.jsonl` |
| Periode | 27 October 2026 07:28 s.d. 27 October 2026 11:40 WIB |
| Total aktivitas | 423 baris |
| Siswa terpantau | 6 |
| PC terpakai | 6 |
| Aplikasi berbeda | 14 |
| Baris gagal dibaca | 0 |

## 1. Siswa Aktif dan Durasi Penggunaan

| # | Siswa | Durasi | Sesi | Rata-rata/sesi | PC | Login pertama | Terakhir | Porsi |
|---|---|---|---|---|---|---|---|---|
| 1 | siswa04 | 4j 12m | 1 | 4j 12m | PC-LAB-04 | 07:28 | 11:37 | 20.8% |
| 2 | siswa01 | 3j 58m | 1 | 3j 58m | PC-LAB-01 | 07:30 | 11:26 | 19.7% |
| 3 | siswa03 | 3j 40m | 1 | 3j 40m | PC-LAB-03 | 07:41 | 11:18 | 18.1% |
| 4 | siswa06 | 3j 18m | 1 | 3j 18m | PC-LAB-06 | 07:52 | 11:08 | 16.4% |
| 5 | siswa02 | 2j 33m | 1 | 2j 33m | PC-LAB-02 | 07:34 | 10:04 | 12.6% |
| 6 | siswa05 | 2j 31m | 1 | 2j 31m | PC-LAB-05 | 09:12 | 11:40 | 12.4% |

Total waktu penggunaan terakumulasi seluruh siswa: **20j 12m**.

## 2. Aplikasi dan Jendela Paling Sering Dibuka

### 2a. Komposisi per kategori

| Kategori | Jumlah aktivitas | Porsi | Estimasi durasi |
|---|---|---|---|
| Belajar / Tugas | 309 | 73.0% | 14j 31m |
| Hiburan / Distraksi | 91 | 21.5% | 4j 22m |
| Netral / Tak Terklasifikasi | 14 | 3.3% | 38m |
| Pelanggaran (BLOCKED/INSTALLER) | 8 | 1.9% | 26m |
| Sistem / Kendala | 1 | 0.2% | 1m |

### 2b. Aplikasi teratas

| # | Aplikasi | Kategori | Frekuensi | Estimasi durasi |
|---|---|---|---|---|
| 1 | `chrome.exe` | Belajar / Tugas | 194 | 9j 11m |
| 2 | `code.exe` | Belajar / Tugas | 125 | 6j 01m |
| 3 | `geogebra.exe` | Belajar / Tugas | 33 | 1j 26m |
| 4 | `excel.exe` | Belajar / Tugas | 19 | 59m |
| 5 | `winword.exe` | Belajar / Tugas | 13 | 35m |
| 6 | `notepad.exe` | Belajar / Tugas | 10 | 26m |
| 7 | `steam.exe` | Hiburan / Distraksi | 7 | 21m |
| 8 | `vlc.exe` | Hiburan / Distraksi | 7 | 19m |
| 9 | `discord.exe` | Hiburan / Distraksi | 7 | 17m |
| 10 | `explorer.exe` | Belajar / Tugas | 2 | 7m |
| 11 | `game.exe` | Pelanggaran (BLOCKED/INSTALLER) | 2 | 7m |
| 12 | `setup_ff.exe` | Pelanggaran (BLOCKED/INSTALLER) | 2 | 6m |
| 13 | `setup_game.exe` | Pelanggaran (BLOCKED/INSTALLER) | 1 | 2m |
| 14 | `PENGATURAN AKSES` | Sistem / Kendala | 1 | 1m |

### 2c. Judul jendela teratas

| # | Aplikasi | Judul jendela | Kategori | Frekuensi |
|---|---|---|---|---|
| 1 | `code.exe` | latihan.py - Visual Studio Code | Belajar / Tugas | 38 |
| 2 | `code.exe` | tugas1.py - Visual Studio Code | Belajar / Tugas | 29 |
| 3 | `chrome.exe` | Google Classroom - Tugas Pemrograman Dasar | Belajar / Tugas | 28 |
| 4 | `code.exe` | main.py - Visual Studio Code | Belajar / Tugas | 25 |
| 5 | `geogebra.exe` | GeoGebra Classic - Fungsi Kuadrat | Belajar / Tugas | 21 |
| 6 | `chrome.exe` | TikTok - FYP | Hiburan / Distraksi | 21 |
| 7 | `excel.exe` | data_nilai.xlsx - Excel | Belajar / Tugas | 19 |
| 8 | `chrome.exe` | Google Docs - Laporan Praktikum | Belajar / Tugas | 18 |
| 9 | `code.exe` | tugas_akhir.py - Visual Studio Code | Belajar / Tugas | 16 |
| 10 | `winword.exe` | Laporan Praktikum.docx - Word | Belajar / Tugas | 13 |
| 11 | `chrome.exe` | YouTube - Musik | Hiburan / Distraksi | 13 |
| 12 | `geogebra.exe` | GeoGebra Classic - Grafik | Belajar / Tugas | 12 |
| 13 | `code.exe` | latihan_loop.py - Visual Studio Code | Belajar / Tugas | 11 |
| 14 | `chrome.exe` | YouTube - Gameplay Minecraft | Hiburan / Distraksi | 9 |
| 15 | `chrome.exe` | YouTube - Highlight Bola | Hiburan / Distraksi | 9 |

## 3. Percobaan Aplikasi Terlarang dan Instalasi

| # | Siswa | BLOCKED | INSTALLER | Total | PC |
|---|---|---|---|---|---|
| 1 | siswa02 | 5 | 1 | 6 | PC-LAB-02 |
| 2 | siswa05 | 0 | 2 | 2 | PC-LAB-05 |

**Rincian kejadian:**

| Waktu | Siswa | PC | Kategori | Aplikasi | Keterangan |
|---|---|---|---|---|---|
| 27/10 07:54 | siswa02 | PC-LAB-02 | BLOCKED | `game.exe` | Aplikasi ditutup paksa: terdeteksi sebagai aplikasi hiburan |
| 27/10 07:59 | siswa02 | PC-LAB-02 | BLOCKED | `game.exe` | Aplikasi ditutup paksa: terdeteksi sebagai aplikasi hiburan |
| 27/10 08:33 | siswa02 | PC-LAB-02 | BLOCKED | `steam.exe` | Aplikasi ditutup paksa: klien game terdeteksi |
| 27/10 08:35 | siswa02 | PC-LAB-02 | BLOCKED | `steam.exe` | Aplikasi ditutup paksa: klien game terdeteksi |
| 27/10 08:38 | siswa02 | PC-LAB-02 | INSTALLER | `setup_game.exe` | Proses instalasi diblokir oleh kebijakan lab |
| 27/10 09:33 | siswa02 | PC-LAB-02 | BLOCKED | `chrome.exe` | Aplikasi ditutup paksa |
| 27/10 10:43 | siswa05 | PC-LAB-05 | INSTALLER | `setup_ff.exe` | Proses instalasi diblokir oleh kebijakan lab |
| 27/10 10:44 | siswa05 | PC-LAB-05 | INSTALLER | `setup_ff.exe` | Proses instalasi diblokir oleh kebijakan lab |

## 4. Kesimpulan

- Terekam **423 aktivitas** dari **6 siswa** pada **6 PC** laboratorium, meliputi **14 aplikasi/jendela** berbeda. Total waktu penggunaan terpantau sekitar **20j 12m** bila diakumulasi (metode: sesi dipisah bila jeda antar-aktivitas > 15 menit).
- Pengguna paling lama adalah **siswa04** (4j 12m, 1 sesi, rata-rata 4j 12m per sesi), sedangkan yang paling singkat adalah **siswa05** (2j 31m). Selisih keduanya mencapai **1j 41m** — perlu dicek apakah karena siswa tersebut memang lebih dulu selesai, atau karena kendala teknis/login.
- Komposisi aktivitas: **73.0% belajar/tugas**, **21.5% hiburan/distraksi**. Porsi aktivitas hiburan **moderat** (21.5%) — masih dalam batas wajar untuk jeda istirahat, namun perlu diingatkan.
- Ditemukan **2 siswa** dengan percobaan pelanggaran: **5 percobaan membuka aplikasi terlarang (BLOCKED)** dan **3 percobaan instalasi program (INSTALLER)**. Pelanggaran terbanyak oleh **siswa02** (6 kejadian).
- Puncak aktivitas terjadi pada pukul **09:00–10:00 WIB**, yang dapat dijadikan acuan penempatan sesi materi inti.

## 5. Saran Evaluasi Pembelajaran untuk Guru

1. **Tetapkan kontrak belajar di awal sesi.** Sampaikan daftar aplikasi yang diizinkan dan target tugas per pertemuan, sehingga siswa tahu konsekuensi sebelum membuka aplikasi di luar daftar.
2. **Tindak lanjuti pelanggaran secara bertahap, bukan langsung sanksi.** Percobaan pertama: teguran lisan + penjelasan alasan pemblokiran. Percobaan berulang (>2 kali): panggil siswa dan catat di jurnal kelas. Data BLOCKED/INSTALLER adalah *percobaan*, bukan bukti program berhasil dipasang — verifikasi dulu ke siswa sebelum menyimpulkan.
3. **Jadwalkan jeda terstruktur.** Karena distraksi muncul terutama di sela pengerjaan tugas, sediakan 5–10 menit istirahat resmi di tengah sesi agar siswa tidak mencari hiburan sendiri saat bosan.
4. **Periksa kesenjangan durasi antar-siswa.** Rentang durasi mencapai 1j 41m (rata-rata 3j 22m). Bila penyebabnya siswa yang login belakangan atau PC bermasalah, siapkan prosedur check-in PC sebelum sesi dimulai.
5. **Gunakan data ini sebagai umpan balik, bukan penghakiman.** Sampaikan ringkasan ke siswa (mis. "kelas hari ini 73% aktivitas ada di aplikasi tugas") agar tercipta kesadaran bersama, bukan rasa diawasi.
6. **Arsipkan log per pertemuan.** Simpan `activities.jsonl` dengan nama bertanggal (mis. `activities_2026-10-27.jsonl`) supaya tren antar-pertemuan bisa dibandingkan dan file tidak terus membengkak.
7. **Tinjau ulang daftar blokir di `app_policy.json`.** Jika ada aplikasi edukasi yang sering muncul sebagai BLOCKED, tambahkan ke daftar izin; sebaliknya, tambahkan aplikasi hiburan baru yang lolos ke daftar blokir.

---
*Dibuat otomatis oleh `analyze_activities.py`. Metode sesi: jeda antar-aktivitas > 15 menit dianggap sesi baru; durasi aplikasi diestimasi dari selisih waktu antar-aktivitas (maks. 10 menit per potongan).*