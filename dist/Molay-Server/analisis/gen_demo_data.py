#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
gen_demo_data.py
================
Pembuat DATA SIMULASI activities.jsonl untuk menguji analyze_activities.py.

Data yang dihasilkan SEPENUHNYA FIKTIF — dibuat agar bentuk laporan bisa
diperiksa sebelum sesi lab sungguhan berjalan. Format baris mengikuti skema
ActivityEntry pada Molay Server (timestamp dalam UTC, seperti yang ditulis
server).

Pemakaian:
    python gen_demo_data.py            # -> activities_demo_SIMULASI.jsonl
"""

import json
import random
from datetime import datetime, timedelta, timezone

WIB = timezone(timedelta(hours=7))
random.seed(20261027)

TANGGAL = datetime(2026, 10, 27, 7, 30, tzinfo=WIB)


# app, judul, menit, kategori
RENCANA = {
    "siswa01": ("PC-LAB-01", "Ahmad Fauzi", 7 * 60 + 30, [
        ("chrome.exe", "Google Classroom - Tugas Pemrograman Dasar", 12, "APP"),
        ("chrome.exe", "Modul Praktikum Algoritma - PDF", 14, "APP"),
        ("code.exe", "main.py - Visual Studio Code", 35, "APP"),
        ("chrome.exe", "stackoverflow.com - Python list comprehension", 6, "APP"),
        ("code.exe", "latihan_loop.py - Visual Studio Code", 28, "APP"),
        ("chrome.exe", "YouTube - Musik Belajar", 9, "APP"),
        ("code.exe", "tugas_akhir.py - Visual Studio Code", 40, "APP"),
        ("chrome.exe", "Google Docs - Laporan Praktikum", 22, "APP"),
        ("explorer.exe", "Documents", 4, "APP"),
        ("code.exe", "main.py - Visual Studio Code", 25, "APP"),
    ]),
    "siswa02": ("PC-LAB-02", "Budi Santoso", 7 * 60 + 34, [
        ("chrome.exe", "Google Classroom - Tugas Pemrograman Dasar", 8, "APP"),
        ("steam.exe", "Steam - Library", 4, "APP"),
        ("steam.exe", "Steam - Toko", 3, "APP"),
        ("game.exe", "Minecraft 1.20", 6, "BLOCKED"),
        ("chrome.exe", "YouTube - Gameplay Minecraft", 18, "APP"),
        ("notepad.exe", "catatan.txt - Notepad", 5, "APP"),
        ("steam.exe", "Steam - Library", 2, "APP"),
        ("steam.exe", "Steam - Update", 3, "BLOCKED"),
        ("setup_game.exe", "Installer - Setup Wizard", 2, "INSTALLER"),
        ("chrome.exe", "TikTok - FYP", 21, "APP"),
        ("chrome.exe", "Google Classroom - Tugas Pemrograman Dasar", 11, "APP"),
        ("discord.exe", "Discord", 14, "APP"),
        ("chrome.exe", "Mobile Legends - Download", 3, "BLOCKED"),
        ("notepad.exe", "tugas.txt - Notepad", 9, "APP"),
        ("chrome.exe", "YouTube - Highlight MLBB", 16, "APP"),
    ]),
    "siswa03": ("PC-LAB-03", "Citra Lestari", 7 * 60 + 41, [
        ("chrome.exe", "Google Classroom - Tugas Pemrograman Dasar", 15, "APP"),
        ("chrome.exe", "YouTube - Tutorial Python", 13, "APP"),
        ("code.exe", "tugas1.py - Visual Studio Code", 24, "APP"),
        ("chrome.exe", "Instagram - Reels", 17, "APP"),
        ("chrome.exe", "w3schools.com - Python Functions", 12, "APP"),
        ("code.exe", "tugas1.py - Visual Studio Code", 30, "APP"),
        ("chrome.exe", "YouTube - Musik", 22, "APP"),
        ("chrome.exe", "Google Docs - Laporan", 18, "APP"),
        ("chrome.exe", "TikTok - FYP", 14, "APP"),
        ("code.exe", "tugas1.py - Visual Studio Code", 20, "APP"),
    ]),
    "siswa04": ("PC-LAB-04", "Dewi Anggraini", 7 * 60 + 28, [
        ("chrome.exe", "Google Classroom - Tugas Matematika", 11, "APP"),
        ("geogebra.exe", "GeoGebra Classic - Fungsi Kuadrat", 33, "APP"),
        ("excel.exe", "data_nilai.xlsx - Excel", 27, "APP"),
        ("chrome.exe", "Khan Academy - Latihan Aljabar", 19, "APP"),
        ("geogebra.exe", "GeoGebra Classic - Grafik", 24, "APP"),
        ("winword.exe", "Laporan Praktikum.docx - Word", 31, "APP"),
        ("chrome.exe", "Google Classroom - Kuis", 15, "APP"),
        ("excel.exe", "data_nilai.xlsx - Excel", 21, "APP"),
        ("chrome.exe", "YouTube - Video Pembelajaran Matematika", 12, "APP"),
        ("geogebra.exe", "GeoGebra Classic - Fungsi Kuadrat", 18, "APP"),
    ]),
    "siswa05": ("PC-LAB-05", "Eko Prasetyo", 9 * 60 + 12, [
        ("chrome.exe", "Google Classroom - Tugas Pemrograman Dasar", 9, "APP"),
        ("chrome.exe", "YouTube - Highlight Bola", 25, "APP"),
        ("chrome.exe", "TikTok - FYP", 19, "APP"),
        ("notepad.exe", "tugas.txt - Notepad", 7, "APP"),
        ("vlc.exe", "VLC media player", 16, "APP"),
        ("setup_ff.exe", "Free Fire Installer", 3, "INSTALLER"),
        ("chrome.exe", "Facebook - Beranda", 18, "APP"),
        ("code.exe", "tugas.py - Visual Studio Code", 14, "APP"),
        ("chrome.exe", "YouTube - Musik", 11, "APP"),
    ]),
    "siswa06": ("PC-LAB-06", "Fitri Handayani", 7 * 60 + 52, [
        ("chrome.exe", "Google Classroom - Tugas Pemrograman Dasar", 13, "APP"),
        ("chrome.exe", "Modul Praktikum - PDF", 17, "APP"),
        ("code.exe", "latihan.py - Visual Studio Code", 29, "APP"),
        ("chrome.exe", "ChatGPT - Bantuan Error Python", 9, "APP"),
        ("code.exe", "latihan.py - Visual Studio Code", 34, "APP"),
        ("chrome.exe", "Google Docs - Laporan Praktikum", 20, "APP"),
        ("chrome.exe", "YouTube - Tutorial VS Code", 8, "APP"),
        ("code.exe", "latihan.py - Visual Studio Code", 26, "APP"),
        ("chrome.exe", "Google Classroom - Kumpulkan Tugas", 7, "APP"),
    ]),
}

DETAIL_BLOCKED = {
    "game.exe": "Aplikasi ditutup paksa: terdeteksi sebagai aplikasi hiburan",
    "steam.exe": "Aplikasi ditutup paksa: klien game terdeteksi",
    "setup_game.exe": "Proses instalasi diblokir oleh kebijakan lab",
    "setup_ff.exe": "Proses instalasi diblokir oleh kebijakan lab",
}


def main():
    rows = []
    for user, (pc, nama, menit_mulai, segmen) in RENCANA.items():
        t = TANGGAL.replace(hour=menit_mulai // 60, minute=menit_mulai % 60)
        for app, title, durasi, cat in segmen:
            sisa = durasi
            while sisa > 0:
                step = min(sisa, random.choice([1, 2, 2, 3, 3, 4]))
                sev = "info"
                det = None
                if cat == "BLOCKED":
                    sev = "danger"
                    det = DETAIL_BLOCKED.get(app, "Aplikasi ditutup paksa")
                elif cat == "INSTALLER":
                    sev = "danger"
                    det = DETAIL_BLOCKED.get(app, "Proses instalasi diblokir")
                rows.append({
                    "client_id": pc,
                    "username": user,
                    "app_name": app,
                    "window_title": title,
                    "timestamp": t.astimezone(timezone.utc).strftime(
                        "%Y-%m-%dT%H:%M:%SZ"),
                    "category": cat,
                    "severity": sev,
                    "details": det,
                })
                t += timedelta(minutes=step, seconds=random.randint(0, 45))
                sisa -= step
            t += timedelta(minutes=random.randint(0, 2))

    # satu kendala teknis
    rows.append({
        "client_id": "PC-LAB-05",
        "username": "siswa05",
        "app_name": "PENGATURAN AKSES",
        "window_title": "Kendala: klien terputus sementara",
        "timestamp": (TANGGAL.replace(hour=10, minute=5)).astimezone(
            timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "category": "KENDALA",
        "severity": "warning",
        "details": "Koneksi klien terputus 40 detik, tersambung kembali otomatis",
    })

    rows.sort(key=lambda r: r["timestamp"])
    out = "activities_demo_SIMULASI.jsonl"
    with open(out, "w", encoding="utf-8") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    print("Dibuat: %s (%d baris, data fiktif)" % (out, len(rows)))


if __name__ == "__main__":
    main()
