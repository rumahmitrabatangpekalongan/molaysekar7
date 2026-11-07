#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
review_zero_regression.py
=========================
Engine Review Kode Otomatis dengan Standar Zero Regression.
Terhubung langsung ke SumoPod AI (https://ai.sumopod.com/v1) menggunakan model glm-5.3-flash.

Fitur:
- Menganalisis git diff lokal terhadap HEAD.
- Menerapkan 5 Aturan Wajib Zero Regression:
  1. No Silent Breaking Change
  2. Isolated New Feature
  3. Dependency & Call Stack Check
  4. Verification Steps
  5. Diff + Impact Report
- Menghasilkan laporan markdown lengkap (hasil_review_lokal.md).
"""

import os
import sys
import json
import subprocess
import urllib.request
import urllib.error

SUMOPOD_URL = "https://ai.sumopod.com/v1/chat/completions"
MODEL_NAME = "glm-5.3-flash"
API_KEY = os.environ.get("OPENAI_KEY") or os.environ.get("OPENAI_API_KEY") or "sk-C0epKR02pvy95P4etwAe2A"

SYSTEM_PROMPT = """Kamu adalah Senior Software Engineer dan Gatekeeper Kode dengan standar ZERO REGRESSION mutlak.
Tugasmu adalah menganalisis setiap perubahan kode (git diff) secara mendalam, kritis, dan teliti sebelum kode diizinkan masuk ke branch utama.

PRINSIP WAJIB YANG HARUS DIAUDIT:
1. No Silent Breaking Change:
   - Dilarang mengubah/menghapus logic bisnis, nama fungsi/method publik, signature parameter, tipe data, atau skema DB yang sudah ada.
   - Periksa apakah ada perubahan behavior yang bisa memecah ekspektasi modul lain.

2. Isolated New Feature:
   - Setiap fitur baru harus dibuat dalam modul/fungsi terisolasi baru, bukan menumpuk baris kode liar di file yang sudah padat.

3. Dependency & Call Stack Check:
   - Analisis fungsi mana saja yang menjadi pemanggil (callers) dari kode yang diubah.
   - Apakah perubahan ini backward-compatible bagi seluruh pemanggilnya?

4. Verification Steps:
   - Apakah perubahan ini sudah disertai test otomatis / langkah verifikasi yang jelas untuk membuktikan fitur lama tidak rusak?

FORMAT LAPORAN REVIEW:
## 1. Ringkasan Perubahan
(Jelaskan secara ringkas file apa yang diubah dan tujuannya)

## 2. Skor Risiko Regresi (Risk Score: 0 - 100)
(Berikan skor risiko. 0 = Sangat Aman, 100 = Sangat Berbahaya/Pasti Merusak Fitur Lama)

## 3. Temuan Kritis & Potensi Breaking Changes
(Detailkan jika ada potensi breaking change, unwrap/panic di Rust, atau memory/thread issue)

## 4. Evaluasi Call Stack & Modul Pemanggil
(Analisis dampak perubahan terhadap modul lain)

## 5. Saran Perbaikan & Langkah Verifikasi
(Langkah konkret yang harus dilakukan programmer sebelum menggabungkan kode)

## 6. Keputusan (Status: DISETUJUI / BUTUH PERBAIKAN / DITOLAK)
"""

def get_git_diff():
    try:
        targets = sys.argv[1:] if len(sys.argv) > 1 else ["*.rs"]
        # Prioritaskan audit pada target yang ditentukan atau file logic Rust (.rs)
        result_rs = subprocess.run(
            ["git", "diff", "HEAD", "--"] + targets,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace"
        )
        diff = result_rs.stdout
        if not diff.strip():
            # Coba ambil diff keseluruhan working tree terhadap HEAD
            result = subprocess.run(
                ["git", "diff", "HEAD"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace"
            )
            diff = result.stdout
        if not diff.strip():
            # Jika tidak ada diff terhadap HEAD, periksa diff staged
            result_staged = subprocess.run(
                ["git", "diff", "--staged"],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace"
            )
            diff = result_staged.stdout
        if len(diff) > 4000:
            diff = diff[:4000] + "\n\n... [DIFF DIPERSINGKAT AGAR RESPON CEPAT DARI SUMOPOD AI]"
        return diff
    except Exception as e:
        print(f"[ERROR] Gagal menjalankan git diff: {e}")
        return ""

def analyze_with_sumopod(diff_content):
    prompt = f"Berikut adalah perubahan kode (git diff) yang akan diaudit:\n\n```diff\n{diff_content}\n```\n\nLakukan analisis audit Zero Regression secara to-the-point dan ringkas."

    payload = {
        "model": MODEL_NAME,
        "messages": [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.2,
        "max_tokens": 3000
    }

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}"
    }

    req = urllib.request.Request(
        SUMOPOD_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers=headers
    )

    print(f"[*] Menghubungi SumoPod AI ({MODEL_NAME})...")
    with urllib.request.urlopen(req, timeout=120) as response:
        result = json.loads(response.read().decode("utf-8"))
        msg = result["choices"][0]["message"]
        content = msg.get("content") or msg.get("reasoning_content") or ""
        return content

def main():
    if hasattr(sys.stdout, 'reconfigure'):
        try:
            sys.stdout.reconfigure(encoding='utf-8')
        except Exception:
            pass

    print("=" * 70)
    print("   ZERO REGRESSION CODE REVIEWER (SumoPod AI Engine)")
    print("=" * 70)

    diff = get_git_diff()
    if not diff or not diff.strip():
        print("\n[INFO] Tidak ada perubahan kode (git diff kosong).")
        print("Silakan ubah/edit kode Anda terlebih dahulu, lalu jalankan script ini.\n")
        return

    print(f"[*] Perubahan terdeteksi: {len(diff)} karakter.")
    try:
        review_result = analyze_with_sumopod(diff)
        output_file = "hasil_review_lokal.md"
        from datetime import datetime
        tanggal_sekarang = datetime.now().strftime("%d/%m/%Y")
        with open(output_file, "w", encoding="utf-8") as f:
            f.write("# Hasil Audit Kode - Zero Regression Review\n\n")
            f.write(f"> **Engine:** SumoPod AI (`{MODEL_NAME}`) | **Tanggal:** {tanggal_sekarang}\n\n")
            f.write(review_result)

        print("\n" + "=" * 70)
        print(f"[SUKSES] Laporan audit berhasil disimpan ke: {output_file}")
        print("=" * 70)
        print("\nPratinjau Hasil Review:\n")
        print(review_result[:600] + ("\n... [buka file hasil_review_lokal.md untuk selengkapnya]" if len(review_result) > 600 else ""))

    except Exception as e:
        print(f"\n[ERROR] Gagal melakukan review: {e}")

if __name__ == "__main__":
    main()
