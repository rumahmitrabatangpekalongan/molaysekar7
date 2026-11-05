#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
drive_sync.py
=============
Menyalin hasil analisis (HTML/MD/JSON) dan arsip log mentah ke folder
Google Drive yang tersinkron di komputer ini.

Pendekatan ini memakai **folder sinkron Google Drive Desktop**, bukan API.
Artinya: tidak perlu OAuth, tidak perlu service account, tidak perlu
kredensial apa pun. Selama Google Drive Desktop berjalan dan folder
"My Drive" tersedia, file yang disalin akan otomatis terunggah.

Konfigurasi: drive_config.json (satu folder dengan skrip ini)
    {
      "folder": "",                        <- kosong = deteksi otomatis
      "subfolder": "Lab Komputer/Laporan",
      "arsip_log": true
    }

Pemakaian mandiri:
    python drive_sync.py --detect
    python drive_sync.py --sync --prefix laporan_aktivitas_lab
"""

import argparse
import json
import os
import shutil
import sys
import string
from datetime import datetime

HERE = os.path.dirname(os.path.abspath(__file__))
CONFIG_PATH = os.path.join(HERE, "drive_config.json")

DEFAULT_CONFIG = {
    "folder": "",
    "subfolder": "Lab Komputer/Laporan",
    "arsip_log": True,
}

CANDIDATE_NAMES = ["My Drive", "Drive Saya", "Google Drive"]


def load_config():
    cfg = dict(DEFAULT_CONFIG)
    if os.path.exists(CONFIG_PATH):
        try:
            with open(CONFIG_PATH, "r", encoding="utf-8") as f:
                cfg.update(json.load(f))
        except Exception:
            pass
    return cfg


def save_config(cfg):
    with open(CONFIG_PATH, "w", encoding="utf-8") as f:
        json.dump(cfg, f, indent=2, ensure_ascii=False)


def _looks_like_drive(path):
    """Folder Drive biasanya punya penanda ini."""
    if not os.path.isdir(path):
        return False
    markers = [".shortcut-targets-by-id", ".tmp", "desktop.ini"]
    return any(os.path.exists(os.path.join(path, m)) for m in markers)


def detect_drive_root():
    """Cari folder akar Google Drive. Kembalikan (path, sumber) atau (None, alasan)."""
    home = os.path.expanduser("~")

    # 1) lokasi paling umum
    for name in CANDIDATE_NAMES:
        for base in (home, os.path.join(home, "Google Drive")):
            p = os.path.join(base, name)
            if os.path.isdir(p):
                return p, "lokasi umum"

    # 2) drive letter (mode virtual/stream)
    for letter in string.ascii_uppercase:
        root = "%s:\\" % letter
        if not os.path.isdir(root):
            continue
        for name in CANDIDATE_NAMES:
            p = os.path.join(root, name)
            if os.path.isdir(p):
                return p, "drive %s:" % letter

    return None, "folder My Drive tidak ditemukan"


def resolve_target(cfg):
    """Tentukan folder tujuan akhir (root + subfolder)."""
    explicit = (cfg.get("folder") or "").strip()
    if explicit:
        if not os.path.isdir(explicit):
            return None, "folder di drive_config.json tidak ada: %s" % explicit
        root, src = explicit, "drive_config.json"
    else:
        root, src = detect_drive_root()
        if not root:
            return None, src

    sub = (cfg.get("subfolder") or "").strip()
    parts = [x for x in sub.replace("\\", "/").split("/") if x]
    target = os.path.join(root, *parts) if parts else root
    return target, src


def _same_size(a, b):
    try:
        return os.path.getsize(a) == os.path.getsize(b)
    except OSError:
        return False


def _copy(src, dst_dir):
    os.makedirs(dst_dir, exist_ok=True)
    dst = os.path.join(dst_dir, os.path.basename(src))
    shutil.copy2(src, dst)
    ok = _same_size(src, dst)
    return dst, ok


def sync(prefix="laporan_aktivitas_lab", tanggal=None, log_file=None,
         cfg=None, out_dir=HERE, verbose=True):
    """Salin laporan + arsip log ke Drive. Kembalikan dict hasil."""
    cfg = cfg or load_config()
    target, src_info = resolve_target(cfg)

    result = {"ok": False, "target": target, "sumber": src_info,
              "copied": [], "failed": [], "message": ""}

    if not target:
        result["message"] = src_info
        if verbose:
            print("[DRIVE] Lewati sinkronisasi: %s" % src_info)
            print("[DRIVE] Pasang Google Drive Desktop lalu login, atau isi "
                  "\"folder\" di drive_config.json")
        return result

    # 1) file laporan
    for ext in (".html", ".md", "_ringkasan.json"):
        p = os.path.join(out_dir, prefix + ext)
        if os.path.exists(p):
            dst, ok = _copy(p, target)
            (result["copied"] if ok else result["failed"]).append(dst)

    # 2) arsip log mentah (sekali per tanggal)
    if cfg.get("arsip_log", True) and log_file and os.path.exists(log_file):
        tgl = tanggal or datetime.now().strftime("%Y-%m-%d")
        arch_dir = os.path.join(target, "log-mentah")
        dst = os.path.join(arch_dir, "activities_%s.jsonl" % tgl)
        if os.path.exists(dst) and _same_size(log_file, dst):
            if verbose:
                print("[DRIVE] Arsip log %s sudah ada, dilewati" % tgl)
        else:
            d, ok = _copy(log_file, arch_dir)
            final = os.path.join(arch_dir, "activities_%s.jsonl" % tgl)
            if ok and d != final:
                os.replace(d, final)
                d = final
            (result["copied"] if ok else result["failed"]).append(d)

    result["ok"] = bool(result["copied"]) and not result["failed"]
    result["message"] = "tersalin %d file" % len(result["copied"])

    if verbose:
        print("[DRIVE] Tujuan : %s  (%s)" % (target, src_info))
        for c in result["copied"]:
            print("[DRIVE]   OK    %s" % os.path.basename(c))
        for f in result["failed"]:
            print("[DRIVE]   GAGAL %s" % os.path.basename(f))
        if result["ok"]:
            print("[DRIVE] Selesai. Google Drive Desktop akan mengunggah otomatis.")
    return result


def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    ap = argparse.ArgumentParser(description="Sinkronkan laporan lab ke Google Drive")
    ap.add_argument("--detect", action="store_true", help="deteksi folder Drive saja")
    ap.add_argument("--set-folder", help="simpan path folder Drive ke konfigurasi")
    ap.add_argument("--sync", action="store_true", help="jalankan sinkronisasi")
    ap.add_argument("--prefix", default="laporan_aktivitas_lab")
    ap.add_argument("--log", help="path activities.jsonl untuk diarsipkan")
    ap.add_argument("--out", default=HERE)
    args = ap.parse_args()

    cfg = load_config()

    if args.set_folder:
        cfg["folder"] = args.set_folder
        save_config(cfg)
        print("Folder Drive disimpan: %s" % args.set_folder)

    if args.detect or not args.sync:
        root, src = detect_drive_root()
        if root:
            print("Folder Drive terdeteksi: %s  (%s)" % (root, src))
            target, _ = resolve_target(cfg)
            print("Tujuan laporan        : %s" % target)
        else:
            print("Folder Drive TIDAK terdeteksi: %s" % src)
            print("")
            print("Kemungkinan penyebab:")
            print("  1. Google Drive Desktop belum terpasang")
            print("     -> https://www.google.com/drive/download/")
            print("  2. Sudah terpasang tetapi belum login / belum berjalan")
            print("     -> buka Google Drive, login, tunggu folder 'My Drive' muncul")
            print("  3. Folder berada di lokasi tidak umum")
            print("     -> jalankan: python drive_sync.py --set-folder \"X:\\path\\My Drive\"")
        if not args.sync:
            return 0

    if args.sync:
        r = sync(prefix=args.prefix, log_file=args.log, cfg=cfg, out_dir=args.out)
        return 0 if r["ok"] else 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
