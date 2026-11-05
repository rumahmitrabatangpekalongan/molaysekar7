# Molay Sekar7 - Monitoring Layar & Lab Access Control (Rust)

Aplikasi manajemen lab komputer berbasis Client-Server yang dibangun 100% menggunakan **Rust**, dirancang khusus untuk jaringan lokal (LAN) berkecepatan tinggi dengan nama resmi:
**Molay (Monitoring Layar) Sekar7**.

---

## 📦 Paket Siap Pakai & Installer Mandiri (.exe Setup Wizard)

Tersedia di folder **`dist/Installers/`** untuk instalasi standar Windows:

### 1. Software Installer (.exe Setup Wizard)
* **`dist/Installers/MolayServer-Sekar7-Setup.exe`**: Installer Server Guru (Otomatis memasang Server + Cloudflare Tunnel + Shortcut Desktop).
* **`dist/Installers/MolayClient-Sekar7-Setup.exe`**: Installer Klien Siswa (Otomatis memasang lock screen + opsi auto-startup saat Windows dinyalakan).

---

## 📂 Paket Portabel (Tanpa Install / Langsung Pakai)

Jika tidak ingin instalasi permanen, tersedia folder mandiri di `dist/`:

### 1. `dist/Molay-Server/` (Untuk Komputer Guru / Server)
* **`MolayServer-Sekar7.exe`**: Executable server utama (hanya ~2.1 MB).
* **`cloudflared.exe`**: Connector Cloudflare Zero Trust bawaan.
* **`users.json`**: Database akun siswa & guru.
* **`Jalankan-Molay-Server.bat`**: Script 1-klik terintegrasi untuk menjalankan Server Molay Sekaligus mengaktifkan Cloudflare Tunnel (`https://molay.sekolahkartini7.web.id`) dan langsung membuka Web Dashboard di browser (`http://localhost:1916`).
* **`Jalankan-Cloudflare-Tunnel.bat`**: Script terpisah opsional jika hanya ingin menjalankan tunnel saja.
* **`Pasang-Cloudflare-Service.bat`**: Mendaftarkan tunnel sebagai Windows Service otomatis.
* **`PANDUAN_SERVER.txt`**: Panduan singkat pengoperasian server.

### 2. `dist/Molay-Client/` (Untuk Komputer Siswa / Klien)
* **`MolayClient-Sekar7.exe`**: Executable agent pengunci layar & streamer (hanya ~2.8 MB, pure Rust TLS).
* **`server_ip.txt`**: File konfigurasi alamat server (Mendukung Dual Mode: LAN dan Cloudflare Tunnel).
* **`Jalankan-Molay-Client.bat`**: Script 1-klik untuk menjalankan lock screen.
* **`Pasang-Otomatis-Saat-Boot.bat`**: Script otomatis mendaftarkan Molay ke Windows Startup.
* **`PANDUAN_KLIEN.txt`**: Panduan pemasangan di PC lab siswa.

---

## 🚀 Fitur Utama Molay Sekar7

1. **Layar Kunci Otomatis (Lock Screen Kiosk)**:
   - Layar komputer siswa langsung terkunci penuh (*Always on Top*).
   - Siswa **wajib login dengan Username & Password** yang telah didaftarkan guru.
   - Bila koneksi LAN terputus atau guru menekan tombol "Kunci Layar", PC siswa otomatis terkunci kembali.

2. **Monitoring Layar Real-Time (Grid ala Veyon)**:
   - Guru dapat memantau seluruh layar siswa secara bersamaan dalam format tile/grid di browser (`http://localhost:1916`).
   - Sangat cepat dan responsif melalui kabel LAN berkecepatan tinggi.
   - Klik pada thumbnail mana saja untuk memperbesar ke tampilan live *fullscreen*.

3. **Pencatatan Log Aktivitas Siswa**:
   - Mencatat aplikasi aktif dan judul jendela (file Word yang dibuka, browser tab YouTube/Google, dll).
   - Log tercatat otomatis di server dengan timestamp dan nama siswa yang sedang login.

4. **Remote Control (Kunci / Buka Massal)**:
   - Guru dapat mengunci atau membuka semua PC siswa hanya dengan 1 klik tombol.

5. **Manajemen Akun Siswa Terpusat**:
   - Guru bisa menambah akun siswa baru langsung melalui Web Dashboard tanpa perlu menyentuh PC siswa.

6. **Pembaruan Sistem Mandiri (Self-Update & OTA LAN/Internet)**:
   - PC Siswa dapat mendeteksi pembaruan versi baru secara otomatis ke `molay.sekolahkartini7.web.id` atau IP LAN.
   - Mengunduh binary baru dan menggantikan dirinya sendiri tanpa instal ulang dari nol.
   - Dilindungi dengan **Private API Key (Cloudflare Bearer Token)** sehingga aman dari akses publik tidak berhak.
   - Guru dapat menyiarkan pembaruan langsung dari Web Dashboard (Menu **"Pembaruan Sistem"**).

---

## 🔒 Uji Coba Endpoint Update (Private Bearer Token)

Endpoint pembaruan dilindungi oleh token private:
```bash
# Periksa info rilis update terbaru:
curl -s "https://molay.sekolahkartini7.web.id/api/update/check?current_version=1.0.0" \
  -H "Authorization: Bearer cfut_EFE4TDO6DKe6tGvVKfgGgjW1Hldzn6ss43VLVg2J7b17463a"
```

---

## 👥 Akun Default Lab

Tersimpan di `dist/Molay-Server/users.json`:

| Username | Password | Nama Lengkap | Role |
| :--- | :--- | :--- | :--- |
| `admin` | `admin123` | Administrator Lab | Admin |
| `guru` | `guru123` | Instruktur / Guru Lab | Guru |
| `siswa01` | `lab123` | Ahmad Siswa 01 | Siswa |
| `siswa02` | `lab123` | Budi Siswa 02 | Siswa |
| `siswa03` | `lab123` | Citra Siswa 03 | Siswa |

*(Akun baru bisa ditambahkan kapan saja via menu **"Akun Login Lab"** di Dashboard Web)*
