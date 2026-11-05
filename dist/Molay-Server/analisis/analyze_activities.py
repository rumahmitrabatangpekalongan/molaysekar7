#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
analyze_activities.py
=====================
Penganalisa log aktivitas lab komputer dari Molay Server (activities.jsonl).

Input  : activities.jsonl  (JSON Lines, 1 objek per baris)
Skema  : client_id, username, app_name, window_title, timestamp,
         category (APP|INSTALLER|BLOCKED|SYSTEM|KENDALA), severity, details

Output : ringkasan.json, laporan_aktivitas_lab.md, laporan_aktivitas_lab.html

Pemakaian:
    python analyze_activities.py                      # cari activities.jsonl otomatis
    python analyze_activities.py --file data.jsonl
    python analyze_activities.py --file x.jsonl --label "DATA SIMULASI"
"""

import argparse
import json
import os
import sys
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone

WIB = timezone(timedelta(hours=7))

# ----------------------------------------------------------------------------
# KAMUS KLASIFIKASI
# ----------------------------------------------------------------------------

BELAJAR_APPS = {
    "code.exe", "code - insiders.exe", "notepad.exe", "notepad++.exe",
    "winword.exe", "excel.exe", "powerpnt.exe", "acrobat.exe", "acrord32.exe",
    "foxitreader.exe", "sumatrapdf.exe", "scratch.exe", "scratch 3.exe",
    "geogebra.exe", "python.exe", "pythonw.exe", "idle.exe", "thonny.exe",
    "cmd.exe", "powershell.exe", "windowsterminal.exe", "explorer.exe",
    "mspaint.exe", "paint.exe", "canva.exe", "figma.exe", "zoom.exe",
    "teams.exe", "ms-teams.exe", "notion.exe", "obsidian.exe",
    "xampp-control.exe", "mysqlworkbench.exe", "arduinoide.exe",
    "dev-c++.exe", "eclipse.exe", "netbeans.exe", "blender.exe",
    "sketchup.exe", "autocad.exe", "coreldraw.exe", "photoshop.exe",
    "illustrator.exe", "typingmaster.exe", "winrar.exe", "7zfm.exe",
}

HIBURAN_APPS = {
    "steam.exe", "steamwebhelper.exe", "epicgameslauncher.exe",
    "battle.net.exe", "riotclientservices.exe", "valorant.exe",
    "minecraft.exe", "robloxplayerbeta.exe", "gta5.exe", "gta_sa.exe",
    "freefire.exe", "mobilelegends.exe", "spotify.exe", "vlc.exe",
    "wmplayer.exe", "netflix.exe", "discord.exe", "whatsapp.exe",
    "telegram.exe", "tiktok.exe", "instagram.exe", "cs2.exe", "dota2.exe",
    "pubg.exe", "amongus.exe", "stumbleguys.exe", "osu!.exe",
    "genshinimpact.exe", "ldplayer.exe", "bluestacks.exe", "nox.exe",
    "mpc-hc64.exe", "kmplayer.exe",
}

BROWSERS = {
    "chrome.exe", "msedge.exe", "firefox.exe", "opera.exe",
    "brave.exe", "chromium.exe", "iexplore.exe",
}

INSTALLER_KW = [
    "setup", "install", "msiexec", "uninstall", "_setup", "installer",
    "patch", "crack", "keygen", "activator", "unpack",
]

HIBURAN_TITLE_KW = [
    "youtube", "tiktok", "instagram", "facebook", "netflix", "spotify",
    "discord", "twitter", "twitch", "9gag", "snapchat", "pinterest",
    "game", "free fire", "mobile legends", "roblox", "steam", "nonton",
    "film", "movie", "dorama", "anime", "idn", "viu", "wetv", "iqiyi",
    "shopee", "tokopedia", "lazada", "bermain", "mlbb", "pubg",
]

BELAJAR_TITLE_KW = [
    "classroom", "google docs", "google sheets", "google slides", "docs.google",
    "modul", "materi", "tugas", "latihan", "soal", "ujian", "quiz", "quizizz",
    "kahoot", "wordwall", "geogebra", "w3schools", "stackoverflow", "github",
    "scratch", "python", "tutorial", "belajar", "buku", "pdf", "canva",
    "figma", "chatgpt", "gemini", "khan academy", "ruangguru", "zenius",
    "elearning", "lms", "sekolah", "praktikum", "laporan", "presentasi",
    "spreadsheet", "excel", "word", "slide", "makalah", "jurnal", "repo",
]

GROUP_BELAJAR = "Belajar / Tugas"
GROUP_HIBURAN = "Hiburan / Distraksi"
GROUP_NETRAL = "Netral / Tak Terklasifikasi"
GROUP_PELANGGARAN = "Pelanggaran (BLOCKED/INSTALLER)"
GROUP_SISTEM = "Sistem / Kendala"

GROUP_ORDER = [GROUP_BELAJAR, GROUP_HIBURAN, GROUP_NETRAL,
               GROUP_PELANGGARAN, GROUP_SISTEM]


# ----------------------------------------------------------------------------
# UTILITAS
# ----------------------------------------------------------------------------

def parse_ts(raw):
    """Terima ISO-8601 (dengan Z / offset), epoch detik, atau epoch milidetik."""
    if isinstance(raw, (int, float)):
        v = float(raw)
        if v > 1e11:      # milidetik
            v /= 1000.0
        return datetime.fromtimestamp(v, tz=timezone.utc).astimezone(WIB)
    s = str(raw).strip()
    if s.endswith("Z"):
        s = s[:-1] + "+00:00"
    try:
        dt = datetime.fromisoformat(s)
    except ValueError:
        for fmt in ("%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S", "%d/%m/%Y %H:%M:%S"):
            try:
                dt = datetime.strptime(s, fmt)
                break
            except ValueError:
                continue
        else:
            raise
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(WIB)


def fmt_dur(minutes):
    """Menit -> '1j 23m'."""
    m = int(round(minutes))
    if m < 60:
        return "%dm" % m
    return "%dj %02dm" % (m // 60, m % 60)


def classify(entry):
    """Kembalikan (grup, alasan)."""
    cat = (entry.get("category") or "APP").upper()
    app = (entry.get("app_name") or "").lower().strip()
    title = (entry.get("window_title") or "").lower()

    if cat in ("BLOCKED", "INSTALLER"):
        return GROUP_PELANGGARAN, "kategori %s" % cat
    if cat in ("SYSTEM", "KENDALA"):
        return GROUP_SISTEM, "kategori %s" % cat

    if app in HIBURAN_APPS:
        return GROUP_HIBURAN, "aplikasi hiburan"
    if app in BELAJAR_APPS:
        return GROUP_BELAJAR, "aplikasi produktif"
    if app in BROWSERS:
        h = sum(1 for k in HIBURAN_TITLE_KW if k in title)
        b = sum(1 for k in BELAJAR_TITLE_KW if k in title)
        if h > b:
            return GROUP_HIBURAN, "browser: judul hiburan"
        if b > h:
            return GROUP_BELAJAR, "browser: judul materi"
        return GROUP_NETRAL, "browser: judul netral"

    h = sum(1 for k in HIBURAN_TITLE_KW if k in title)
    b = sum(1 for k in BELAJAR_TITLE_KW if k in title)
    if h > b:
        return GROUP_HIBURAN, "judul jendela hiburan"
    if b > h:
        return GROUP_BELAJAR, "judul jendela materi"
    return GROUP_NETRAL, "tidak dikenali"


def looks_installer(entry):
    app = (entry.get("app_name") or "").lower()
    return any(k in app for k in INSTALLER_KW)


# ----------------------------------------------------------------------------
# PEMUATAN DATA
# ----------------------------------------------------------------------------

def find_default_file():
    here = os.path.dirname(os.path.abspath(__file__))
    for base in (os.getcwd(), here, os.path.dirname(here)):
        p = os.path.join(base, "activities.jsonl")
        if os.path.exists(p):
            return p
    return None


def load_entries(path):
    entries, bad, total = [], 0, 0
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for lineno, line in enumerate(fh, 1):
            line = line.strip()
            if not line:
                continue
            total += 1
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                bad += 1
                continue
            if not isinstance(obj, dict) or "timestamp" not in obj:
                bad += 1
                continue
            try:
                obj["_ts"] = parse_ts(obj["timestamp"])
            except Exception:
                bad += 1
                continue
            obj["_client"] = (obj.get("client_id") or "TANPA-PC").strip() or "TANPA-PC"
            obj["_user"] = (obj.get("username") or "(belum login)").strip() or "(belum login)"
            obj["_app"] = (obj.get("app_name") or "(tidak diketahui)").strip()
            obj["_title"] = (obj.get("window_title") or "").strip()
            obj["_cat"] = (obj.get("category") or "APP").upper()
            obj["_sev"] = (obj.get("severity") or "info").lower()
            g, why = classify(obj)
            obj["_group"] = g
            obj["_why"] = why
            entries.append(obj)
    entries.sort(key=lambda e: e["_ts"])
    return entries, bad, total


# ----------------------------------------------------------------------------
# ANALISIS
# ----------------------------------------------------------------------------

def build_sessions(entries, gap_minutes):
    """Sesi = rangkaian event per (PC, user) dengan jeda antar-event <= gap."""
    gap = timedelta(minutes=gap_minutes)
    buckets = defaultdict(list)
    for e in entries:
        buckets[(e["_client"], e["_user"])].append(e)

    sessions = []
    for (client, user), evs in buckets.items():
        evs.sort(key=lambda e: e["_ts"])
        cur = [evs[0]]
        for prev, nxt in zip(evs, evs[1:]):
            if nxt["_ts"] - prev["_ts"] <= gap:
                cur.append(nxt)
            else:
                sessions.append((client, user, cur))
                cur = [nxt]
        sessions.append((client, user, cur))

    out = []
    for client, user, evs in sessions:
        start, end = evs[0]["_ts"], evs[-1]["_ts"]
        span = (end - start).total_seconds() / 60.0
        # beri tenggang 2 menit untuk aktivitas terakhir yang tidak terpotret
        dur = span + 2.0 if span > 0 else 2.0
        out.append({
            "client": client, "user": user, "start": start, "end": end,
            "duration": dur, "events": len(evs),
        })
    out.sort(key=lambda s: s["start"])
    return out


def app_attribution(entries, gap_minutes):
    """Estimasi durasi per aplikasi: waktu antar-event diberikan ke app aktif."""
    gap = timedelta(minutes=gap_minutes)
    cap = timedelta(minutes=10)
    buckets = defaultdict(list)
    for e in entries:
        buckets[(e["_client"], e["_user"])].append(e)

    dur = defaultdict(float)
    for _, evs in buckets.items():
        evs.sort(key=lambda e: e["_ts"])
        for prev, nxt in zip(evs, evs[1:]):
            delta = nxt["_ts"] - prev["_ts"]
            if delta > gap or delta <= timedelta(0):
                continue
            dur[prev["_app"]] += min(delta, cap).total_seconds() / 60.0
    return dur


def group_attribution(entries, gap_minutes):
    """Durasi per kategori, diatribusikan per-event (bukan per-aplikasi),
    supaya jendela hiburan di dalam browser tetap terhitung hiburan."""
    gap = timedelta(minutes=gap_minutes)
    cap = timedelta(minutes=10)
    buckets = defaultdict(list)
    for e in entries:
        buckets[(e["_client"], e["_user"])].append(e)

    dur = defaultdict(float)
    for _, evs in buckets.items():
        evs.sort(key=lambda e: e["_ts"])
        for prev, nxt in zip(evs, evs[1:]):
            delta = nxt["_ts"] - prev["_ts"]
            if delta > gap or delta <= timedelta(0):
                continue
            dur[prev["_group"]] += min(delta, cap).total_seconds() / 60.0
    return dur


def analyze(entries, gap_minutes):
    r = {}

    # --- 1. SISWA & DURASI -------------------------------------------------
    sessions = build_sessions(entries, gap_minutes)
    per_user = defaultdict(lambda: {"dur": 0.0, "sessions": 0, "events": 0,
                                    "pcs": set(), "first": None, "last": None})
    for s in sessions:
        u = per_user[s["user"]]
        u["dur"] += s["duration"]
        u["sessions"] += 1
        u["events"] += s["events"]
        u["pcs"].add(s["client"])
        u["first"] = s["start"] if u["first"] is None or s["start"] < u["first"] else u["first"]
        u["last"] = s["end"] if u["last"] is None or s["end"] > u["last"] else u["last"]

    students = []
    for user, d in per_user.items():
        students.append({
            "user": user,
            "duration": d["dur"],
            "sessions": d["sessions"],
            "events": d["events"],
            "pcs": sorted(d["pcs"]),
            "first": d["first"],
            "last": d["last"],
        })
    students.sort(key=lambda s: s["duration"], reverse=True)

    total_dur = sum(s["duration"] for s in students) or 1.0
    for s in students:
        s["share"] = 100.0 * s["duration"] / total_dur
        s["avg_session"] = s["duration"] / s["sessions"] if s["sessions"] else 0

    # aktivitas per jam
    by_hour = Counter(e["_ts"].hour for e in entries)
    peak_hour = by_hour.most_common(1)[0][0] if by_hour else None

    # --- 2. APLIKASI -------------------------------------------------------
    app_count = Counter(e["_app"] for e in entries)
    app_dur = app_attribution(entries, gap_minutes)
    app_group = {}
    for e in entries:
        app_group.setdefault(e["_app"], Counter())[e["_group"]] += 1

    apps = []
    for app, cnt in app_count.items():
        groups = app_group.get(app, Counter())
        top_group = groups.most_common(1)[0][0] if groups else GROUP_NETRAL
        apps.append({
            "app": app, "count": cnt, "duration": app_dur.get(app, 0.0),
            "group": top_group,
            "share": 100.0 * cnt / len(entries) if entries else 0,
        })
    apps.sort(key=lambda a: (-a["count"], -a["duration"]))

    titles = Counter()
    title_group = {}
    for e in entries:
        if e["_title"]:
            key = (e["_app"], e["_title"][:120])
            titles[key] += 1
            title_group[key] = e["_group"]
    top_titles = [{"app": k[0], "title": k[1], "count": c,
                   "group": title_group.get(k, GROUP_NETRAL)}
                  for k, c in titles.most_common(15)]

    group_count = Counter(e["_group"] for e in entries)
    group_dur = group_attribution(entries, gap_minutes)

    # --- 3. PELANGGARAN ----------------------------------------------------
    viol = [e for e in entries if e["_cat"] in ("BLOCKED", "INSTALLER")]
    viol_by_user = defaultdict(lambda: {"BLOCKED": 0, "INSTALLER": 0,
                                        "events": [], "pcs": set()})
    for e in viol:
        v = viol_by_user[e["_user"]]
        v[e["_cat"]] += 1
        v["pcs"].add(e["_client"])
        v["events"].append(e)

    viol_summary = []
    for user, v in viol_by_user.items():
        viol_summary.append({
            "user": user,
            "blocked": v["BLOCKED"],
            "installer": v["INSTALLER"],
            "total": v["BLOCKED"] + v["INSTALLER"],
            "pcs": sorted(v["pcs"]),
            "events": sorted(v["events"], key=lambda e: e["_ts"]),
        })
    viol_summary.sort(key=lambda v: (-v["total"], v["user"]))

    installer_apps = Counter(e["_app"] for e in viol if e["_cat"] == "INSTALLER")
    blocked_apps = Counter(e["_app"] for e in viol if e["_cat"] == "BLOCKED")

    # --- 4. POLA TAMBAHAN --------------------------------------------------
    # siswa dengan rasio hiburan tinggi
    user_group = defaultdict(Counter)
    for e in entries:
        user_group[e["_user"]][e["_group"]] += 1
    distract = []
    for user, c in user_group.items():
        total = sum(c.values())
        if total < 5:
            continue
        h = c.get(GROUP_HIBURAN, 0)
        b = c.get(GROUP_BELAJAR, 0)
        distract.append({
            "user": user,
            "hiburan_pct": 100.0 * h / total,
            "belajar_pct": 100.0 * b / total,
            "total": total,
        })
    distract.sort(key=lambda d: -d["hiburan_pct"])

    r.update({
        "sessions": sessions,
        "students": students,
        "total_duration": total_dur,
        "by_hour": dict(sorted(by_hour.items())),
        "peak_hour": peak_hour,
        "apps": apps,
        "top_titles": top_titles,
        "group_count": dict(group_count),
        "group_dur": dict(group_dur),
        "viol_summary": viol_summary,
        "installer_apps": dict(installer_apps),
        "blocked_apps": dict(blocked_apps),
        "distract": distract,
        "counts": {
            "events": len(entries),
            "users": len(students),
            "pcs": len({e["_client"] for e in entries}),
            "apps": len(apps),
            "violations": len(viol),
        },
    })
    return r


# ----------------------------------------------------------------------------
# KESIMPULAN OTOMATIS
# ----------------------------------------------------------------------------

def build_conclusions(res, gap_minutes):
    c = res["counts"]
    st = res["students"]
    lines = []

    lines.append(
        "Terekam **%d aktivitas** dari **%d siswa** pada **%d PC** laboratorium, "
        "meliputi **%d aplikasi/jendela** berbeda. Total waktu penggunaan terpantau "
        "sekitar **%s** bila diakumulasi (metode: sesi dipisah bila jeda antar-aktivitas > %d menit)."
        % (c["events"], c["users"], c["pcs"], c["apps"],
           fmt_dur(res["total_duration"]), gap_minutes))

    if st:
        top = st[0]
        low = st[-1]
        lines.append(
            "Pengguna paling lama adalah **%s** (%s, %d sesi, rata-rata %s per sesi), "
            "sedangkan yang paling singkat adalah **%s** (%s). Selisih keduanya "
            "mencapai **%s** — perlu dicek apakah karena siswa tersebut memang lebih "
            "dulu selesai, atau karena kendala teknis/login."
            % (top["user"], fmt_dur(top["duration"]), top["sessions"],
               fmt_dur(top["avg_session"]), low["user"],
               fmt_dur(low["duration"]),
               fmt_dur(max(0, top["duration"] - low["duration"]))))

    gc = res["group_count"]
    tot = sum(gc.values()) or 1
    belajar_pct = 100.0 * gc.get(GROUP_BELAJAR, 0) / tot
    hiburan_pct = 100.0 * gc.get(GROUP_HIBURAN, 0) / tot
    if hiburan_pct >= 35:
        verdict = ("Porsi aktivitas hiburan/distraksi tergolong **tinggi** "
                   "(%.1f%%), sehingga pengawasan kelas perlu diperketat." % hiburan_pct)
    elif hiburan_pct >= 15:
        verdict = ("Porsi aktivitas hiburan **moderat** (%.1f%%) — masih dalam batas "
                   "wajar untuk jeda istirahat, namun perlu diingatkan." % hiburan_pct)
    else:
        verdict = ("Porsi aktivitas hiburan **rendah** (%.1f%%) — fokus belajar "
                   "siswa tergolong baik." % hiburan_pct)
    lines.append(
        "Komposisi aktivitas: **%.1f%% belajar/tugas**, **%.1f%% hiburan/distraksi**. %s"
        % (belajar_pct, hiburan_pct, verdict))

    if res["viol_summary"]:
        n_students = len(res["viol_summary"])
        n_blocked = sum(v["blocked"] for v in res["viol_summary"])
        n_install = sum(v["installer"] for v in res["viol_summary"])
        worst = res["viol_summary"][0]
        lines.append(
            "Ditemukan **%d siswa** dengan percobaan pelanggaran: **%d percobaan "
            "membuka aplikasi terlarang (BLOCKED)** dan **%d percobaan instalasi "
            "program (INSTALLER)**. Pelanggaran terbanyak oleh **%s** (%d kejadian)."
            % (n_students, n_blocked, n_install, worst["user"], worst["total"]))
    else:
        lines.append(
            "**Tidak ditemukan** percobaan membuka aplikasi terlarang (BLOCKED) "
            "maupun instalasi program baru (INSTALLER) pada periode ini.")

    if res["peak_hour"] is not None:
        lines.append(
            "Puncak aktivitas terjadi pada pukul **%02d:00–%02d:00 WIB**, yang dapat "
            "dijadikan acuan penempatan sesi materi inti."
            % (res["peak_hour"], (res["peak_hour"] + 1) % 24))

    return lines


def build_recommendations(res):
    recs = []
    gc = res["group_count"]
    tot = sum(gc.values()) or 1
    hiburan_pct = 100.0 * gc.get(GROUP_HIBURAN, 0) / tot

    recs.append(
        "**Tetapkan kontrak belajar di awal sesi.** Sampaikan daftar aplikasi yang "
        "diizinkan dan target tugas per pertemuan, sehingga siswa tahu konsekuensi "
        "sebelum membuka aplikasi di luar daftar.")

    if res["viol_summary"]:
        recs.append(
            "**Tindak lanjuti pelanggaran secara bertahap, bukan langsung sanksi.** "
            "Percobaan pertama: teguran lisan + penjelasan alasan pemblokiran. "
            "Percobaan berulang (>2 kali): panggil siswa dan catat di jurnal kelas. "
            "Data BLOCKED/INSTALLER adalah *percobaan*, bukan bukti program berhasil "
            "dipasang — verifikasi dulu ke siswa sebelum menyimpulkan.")

    if hiburan_pct >= 15:
        recs.append(
            "**Jadwalkan jeda terstruktur.** Karena distraksi muncul terutama di sela "
            "pengerjaan tugas, sediakan 5–10 menit istirahat resmi di tengah sesi agar "
            "siswa tidak mencari hiburan sendiri saat bosan.")

    if res["students"]:
        durs = [s["duration"] for s in res["students"]]
        avg = sum(durs) / len(durs)
        spread = max(durs) - min(durs)
        if spread > avg * 0.5:
            recs.append(
                "**Periksa kesenjangan durasi antar-siswa.** Rentang durasi mencapai "
                "%s (rata-rata %s). Bila penyebabnya siswa yang login belakangan atau "
                "PC bermasalah, siapkan prosedur check-in PC sebelum sesi dimulai."
                % (fmt_dur(spread), fmt_dur(avg)))

    belajar_pct = 100.0 * gc.get(GROUP_BELAJAR, 0) / tot
    recs.append(
        "**Gunakan data ini sebagai umpan balik, bukan penghakiman.** Sampaikan "
        "ringkasan ke siswa (mis. \"kelas hari ini %.0f%% aktivitas ada di aplikasi "
        "tugas\") agar tercipta kesadaran bersama, bukan rasa diawasi." % belajar_pct)

    recs.append(
        "**Arsipkan log per pertemuan.** Simpan `activities.jsonl` dengan nama "
        "bertanggal (mis. `activities_2026-10-27.jsonl`) supaya tren antar-pertemuan "
        "bisa dibandingkan dan file tidak terus membengkak.")

    recs.append(
        "**Tinjau ulang daftar blokir di `app_policy.json`.** Jika ada aplikasi "
        "edukasi yang sering muncul sebagai BLOCKED, tambahkan ke daftar izin; "
        "sebaliknya, tambahkan aplikasi hiburan baru yang lolos ke daftar blokir.")

    return recs


# ----------------------------------------------------------------------------
# RENDER MARKDOWN
# ----------------------------------------------------------------------------

def render_markdown(res, concl, recs, label, gap_minutes, meta):
    L = []
    A = L.append
    c = res["counts"]

    A("# Laporan Analisis Aktivitas Pembelajaran Lab Komputer\n")
    if label:
        A("> **%s** — angka pada laporan ini BUKAN data siswa sungguhan.\n" % label)
    A("| Keterangan | Nilai |")
    A("|---|---|")
    A("| Sumber data | `%s` |" % meta["file"])
    A("| Periode | %s s.d. %s WIB |" % (meta["start"], meta["end"]))
    A("| Total aktivitas | %d baris |" % c["events"])
    A("| Siswa terpantau | %d |" % c["users"])
    A("| PC terpakai | %d |" % c["pcs"])
    A("| Aplikasi berbeda | %d |" % c["apps"])
    A("| Baris gagal dibaca | %d |" % meta["bad"])
    A("")

    # 1
    A("## 1. Siswa Aktif dan Durasi Penggunaan\n")
    A("| # | Siswa | Durasi | Sesi | Rata-rata/sesi | PC | Login pertama | Terakhir | Porsi |")
    A("|---|---|---|---|---|---|---|---|---|")
    for i, s in enumerate(res["students"], 1):
        A("| %d | %s | %s | %d | %s | %s | %s | %s | %.1f%% |" % (
            i, s["user"], fmt_dur(s["duration"]), s["sessions"],
            fmt_dur(s["avg_session"]), ", ".join(s["pcs"]),
            s["first"].strftime("%H:%M"), s["last"].strftime("%H:%M"), s["share"]))
    A("")
    A("Total waktu penggunaan terakumulasi seluruh siswa: **%s**.\n"
      % fmt_dur(res["total_duration"]))

    # 2
    A("## 2. Aplikasi dan Jendela Paling Sering Dibuka\n")
    A("### 2a. Komposisi per kategori\n")
    A("| Kategori | Jumlah aktivitas | Porsi | Estimasi durasi |")
    A("|---|---|---|---|")
    tot = sum(res["group_count"].values()) or 1
    for g in GROUP_ORDER:
        n = res["group_count"].get(g, 0)
        if not n:
            continue
        A("| %s | %d | %.1f%% | %s |" % (
            g, n, 100.0 * n / tot, fmt_dur(res["group_dur"].get(g, 0))))
    A("")
    A("### 2b. Aplikasi teratas\n")
    A("| # | Aplikasi | Kategori | Frekuensi | Estimasi durasi |")
    A("|---|---|---|---|---|")
    for i, a in enumerate(res["apps"][:20], 1):
        A("| %d | `%s` | %s | %d | %s |" % (
            i, a["app"], a["group"], a["count"], fmt_dur(a["duration"])))
    A("")
    A("### 2c. Judul jendela teratas\n")
    A("| # | Aplikasi | Judul jendela | Kategori | Frekuensi |")
    A("|---|---|---|---|---|")
    for i, t in enumerate(res["top_titles"][:15], 1):
        A("| %d | `%s` | %s | %s | %d |" % (
            i, t["app"], t["title"], t["group"], t["count"]))
    A("")

    # 3
    A("## 3. Percobaan Aplikasi Terlarang dan Instalasi\n")
    if res["viol_summary"]:
        A("| # | Siswa | BLOCKED | INSTALLER | Total | PC |")
        A("|---|---|---|---|---|---|")
        for i, v in enumerate(res["viol_summary"], 1):
            A("| %d | %s | %d | %d | %d | %s |" % (
                i, v["user"], v["blocked"], v["installer"], v["total"],
                ", ".join(v["pcs"])))
        A("")
        A("**Rincian kejadian:**\n")
        A("| Waktu | Siswa | PC | Kategori | Aplikasi | Keterangan |")
        A("|---|---|---|---|---|---|")
        allv = [e for v in res["viol_summary"] for e in v["events"]]
        for e in sorted(allv, key=lambda x: x["_ts"]):
            A("| %s | %s | %s | %s | `%s` | %s |" % (
                e["_ts"].strftime("%d/%m %H:%M"), e["_user"], e["_client"],
                e["_cat"], e["_app"], (e.get("details") or "-")))
        A("")
    else:
        A("Tidak ada aktivitas berkategori BLOCKED maupun INSTALLER.\n")

    # 4
    A("## 4. Kesimpulan\n")
    for line in concl:
        A("- " + line)
    A("")
    A("## 5. Saran Evaluasi Pembelajaran untuk Guru\n")
    for i, r in enumerate(recs, 1):
        A("%d. %s" % (i, r))
    A("")
    A("---")
    A("*Dibuat otomatis oleh `analyze_activities.py`. Metode sesi: jeda antar-"
      "aktivitas > %d menit dianggap sesi baru; durasi aplikasi diestimasi dari "
      "selisih waktu antar-aktivitas (maks. 10 menit per potongan).*" % gap_minutes)
    return "\n".join(L)


# ----------------------------------------------------------------------------
# RENDER HTML
# ----------------------------------------------------------------------------

CSS = """
:root{
  --bg:#0f1115; --panel:#171a21; --panel2:#1e222b; --line:#2a2f3a;
  --txt:#e6e9ef; --dim:#9aa4b2; --accent:#4f9cf9;
  --good:#2ecc71; --warn:#f0b429; --bad:#e74c3c; --neutral:#7f8c9b;
}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--txt);
  font-family:'Segoe UI',system-ui,-apple-system,sans-serif;
  font-size:14px;line-height:1.6}
.wrap{max-width:1120px;margin:0 auto;padding:32px 24px 64px}
header{border-bottom:1px solid var(--line);padding-bottom:20px;margin-bottom:28px}
h1{font-size:24px;margin:0 0 6px;letter-spacing:-.3px}
h2{font-size:18px;margin:36px 0 14px;padding-left:10px;
  border-left:3px solid var(--accent)}
h3{font-size:14px;color:var(--dim);margin:22px 0 10px;
  text-transform:uppercase;letter-spacing:.6px;font-weight:600}
.sub{color:var(--dim);font-size:13px}
.banner{background:#3a2a0e;border:1px solid #6b4a12;color:#ffd98a;
  padding:10px 14px;border-radius:8px;margin:14px 0;font-size:13px}
.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));
  gap:12px;margin:20px 0}
.card{background:var(--panel);border:1px solid var(--line);
  border-radius:10px;padding:14px 16px}
.card .n{font-size:22px;font-weight:600;color:var(--accent)}
.card .l{font-size:11px;color:var(--dim);text-transform:uppercase;
  letter-spacing:.5px;margin-top:2px}
table{width:100%;border-collapse:collapse;margin:10px 0 18px;
  background:var(--panel);border-radius:8px;overflow:hidden;font-size:13px}
th{background:var(--panel2);text-align:left;padding:9px 11px;
  font-size:11px;text-transform:uppercase;letter-spacing:.5px;
  color:var(--dim);font-weight:600;border-bottom:1px solid var(--line)}
td{padding:8px 11px;border-bottom:1px solid var(--line);vertical-align:top}
tr:last-child td{border-bottom:none}
tr:hover td{background:#1b1f27}
code{background:#252a34;padding:1px 5px;border-radius:4px;
  font-family:Consolas,monospace;font-size:12px;color:#9ad1ff}
.bar{height:7px;background:#252a34;border-radius:4px;overflow:hidden;
  min-width:90px;margin-top:4px}
.bar span{display:block;height:100%;border-radius:4px}
.g-Belajar,.g-BelajarTugas{background:var(--good)}
.pill{display:inline-block;padding:2px 8px;border-radius:20px;
  font-size:11px;font-weight:600;white-space:nowrap}
.p-good{background:rgba(46,204,113,.15);color:#5ce08d;
  border:1px solid rgba(46,204,113,.35)}
.p-bad{background:rgba(231,76,60,.15);color:#ff7b6b;
  border:1px solid rgba(231,76,60,.35)}
.p-warn{background:rgba(240,180,41,.15);color:#f5c451;
  border:1px solid rgba(240,180,41,.35)}
.p-neu{background:rgba(127,140,155,.15);color:#a9b4c2;
  border:1px solid rgba(127,140,155,.35)}
ul,ol{padding-left:22px;margin:10px 0}
li{margin:8px 0}
.callout{background:var(--panel);border-left:3px solid var(--accent);
  padding:12px 16px;border-radius:0 8px 8px 0;margin:14px 0}
footer{margin-top:48px;padding-top:16px;border-top:1px solid var(--line);
  color:var(--dim);font-size:12px}
.num{color:var(--dim);font-variant-numeric:tabular-nums}
@media print{body{background:#fff;color:#111}.card,table{background:#fff}
  th{background:#eee}code{background:#eee;color:#111}}
"""


def esc(s):
    return (str(s).replace("&", "&amp;").replace("<", "&lt;")
            .replace(">", "&gt;"))


def group_pill(g):
    cls = {GROUP_BELAJAR: "p-good", GROUP_HIBURAN: "p-bad",
           GROUP_PELANGGARAN: "p-bad", GROUP_SISTEM: "p-warn"}.get(g, "p-neu")
    return f'<span class="pill {cls}">{esc(g)}</span>'


def bar(pct, color):
    w = max(2.0, min(100.0, pct))
    return f'<div class="bar"><span style="width:{w:.1f}%;background:{color}"></span></div>'


def render_html(res, concl, recs, label, gap_minutes, meta):
    c = res["counts"]
    P = []
    A = P.append

    A('<!DOCTYPE html><html lang="id"><head><meta charset="utf-8">')
    A('<meta name="viewport" content="width=device-width,initial-scale=1">')
    A('<title>Laporan Aktivitas Lab Komputer</title>')
    A(f'<style>{CSS}</style></head><body><div class="wrap">')

    A('<header><h1>Laporan Analisis Aktivitas Pembelajaran Lab Komputer</h1>')
    A(f'<div class="sub">Periode {meta["start"]} &ndash; {meta["end"]} WIB '
      f'&middot; sumber <code>{esc(meta["file"])}</code></div></header>')

    if label:
        A(f'<div class="banner"><strong>{esc(label)}</strong> &mdash; seluruh angka '
          'pada laporan ini dihasilkan dari data simulasi, bukan rekaman siswa '
          'sungguhan. Gunakan untuk melihat bentuk laporan dan memvalidasi logika '
          'analisis.</div>')

    A('<div class="cards">')
    for n, l in [(c["events"], "Aktivitas"), (c["users"], "Siswa"),
                 (c["pcs"], "PC"), (c["apps"], "Aplikasi"),
                 (fmt_dur(res["total_duration"]), "Total durasi"),
                 (c["violations"], "Pelanggaran")]:
        A(f'<div class="card"><div class="n">{n}</div>'
          f'<div class="l">{l}</div></div>')
    A('</div>')

    # ---- 1
    A('<h2>1. Siswa Aktif dan Durasi Penggunaan</h2>')
    A('<table><thead><tr><th>#</th><th>Siswa</th><th>Durasi</th>'
      '<th>Porsi</th><th>Sesi</th><th>Rata-rata / sesi</th><th>PC</th>'
      '<th>Login pertama</th><th>Terakhir</th></tr></thead><tbody>')
    for i, s in enumerate(res["students"], 1):
        col = "#4f9cf9" if s["share"] >= 15 else "#7f8c9b"
        A(f'<tr><td class="num">{i}</td><td><strong>{esc(s["user"])}</strong></td>'
          f'<td class="num">{fmt_dur(s["duration"])}</td>'
          f'<td style="min-width:120px"><span class="num">{s["share"]:.1f}%</span>'
          f'{bar(s["share"], col)}</td>'
          f'<td class="num">{s["sessions"]}</td>'
          f'<td class="num">{fmt_dur(s["avg_session"])}</td>'
          f'<td class="num">{esc(", ".join(s["pcs"]))}</td>'
          f'<td class="num">{s["first"].strftime("%H:%M")}</td>'
          f'<td class="num">{s["last"].strftime("%H:%M")}</td></tr>')
    A('</tbody></table>')

    # ---- 2
    A('<h2>2. Aplikasi dan Jendela Paling Sering Dibuka</h2>')
    A('<h3>2a. Komposisi kategori</h3>')
    tot = sum(res["group_count"].values()) or 1
    colors = {GROUP_BELAJAR: "#2ecc71", GROUP_HIBURAN: "#e74c3c",
              GROUP_NETRAL: "#7f8c9b", GROUP_PELANGGARAN: "#f0b429",
              GROUP_SISTEM: "#4f9cf9"}
    A('<table><thead><tr><th>Kategori</th><th>Aktivitas</th><th>Porsi</th>'
      '<th>Estimasi durasi</th></tr></thead><tbody>')
    for g in GROUP_ORDER:
        n = res["group_count"].get(g, 0)
        if not n:
            continue
        pct = 100.0 * n / tot
        A(f'<tr><td>{group_pill(g)}</td><td class="num">{n}</td>'
          f'<td style="min-width:140px"><span class="num">{pct:.1f}%</span>'
          f'{bar(pct, colors[g])}</td>'
          f'<td class="num">{fmt_dur(res["group_dur"].get(g, 0))}</td></tr>')
    A('</tbody></table>')

    A('<h3>2b. Aplikasi teratas</h3>')
    A('<table><thead><tr><th>#</th><th>Aplikasi</th><th>Kategori</th>'
      '<th>Frekuensi</th><th>Estimasi durasi</th></tr></thead><tbody>')
    for i, a in enumerate(res["apps"][:20], 1):
        A(f'<tr><td class="num">{i}</td><td><code>{esc(a["app"])}</code></td>'
          f'<td>{group_pill(a["group"])}</td>'
          f'<td class="num">{a["count"]}</td>'
          f'<td class="num">{fmt_dur(a["duration"])}</td></tr>')
    A('</tbody></table>')

    A('<h3>2c. Judul jendela teratas</h3>')
    A('<table><thead><tr><th>#</th><th>Aplikasi</th><th>Judul jendela</th>'
      '<th>Kategori</th><th>Frekuensi</th></tr></thead><tbody>')
    for i, t in enumerate(res["top_titles"][:15], 1):
        A(f'<tr><td class="num">{i}</td><td><code>{esc(t["app"])}</code></td>'
          f'<td>{esc(t["title"])}</td><td>{group_pill(t["group"])}</td>'
          f'<td class="num">{t["count"]}</td></tr>')
    A('</tbody></table>')

    # ---- 3
    A('<h2>3. Percobaan Aplikasi Terlarang dan Instalasi Program</h2>')
    if res["viol_summary"]:
        A('<table><thead><tr><th>#</th><th>Siswa</th><th>BLOCKED</th>'
          '<th>INSTALLER</th><th>Total</th><th>PC</th></tr></thead><tbody>')
        for i, v in enumerate(res["viol_summary"], 1):
            A(f'<tr><td class="num">{i}</td><td><strong>{esc(v["user"])}</strong></td>'
              f'<td class="num">{v["blocked"]}</td>'
              f'<td class="num">{v["installer"]}</td>'
              f'<td class="num"><strong>{v["total"]}</strong></td>'
              f'<td class="num">{esc(", ".join(v["pcs"]))}</td></tr>')
        A('</tbody></table>')

        A('<h3>Rincian kejadian</h3>')
        A('<table><thead><tr><th>Waktu</th><th>Siswa</th><th>PC</th>'
          '<th>Kategori</th><th>Aplikasi</th><th>Keterangan</th></tr></thead><tbody>')
        allv = sorted([e for v in res["viol_summary"] for e in v["events"]],
                      key=lambda x: x["_ts"])
        for e in allv:
            cat_cls = "p-bad" if e["_cat"] == "BLOCKED" else "p-warn"
            A(f'<tr><td class="num">{e["_ts"].strftime("%d/%m %H:%M")}</td>'
              f'<td>{esc(e["_user"])}</td><td class="num">{esc(e["_client"])}</td>'
              f'<td><span class="pill {cat_cls}">{esc(e["_cat"])}</span></td>'
              f'<td><code>{esc(e["_app"])}</code></td>'
              f'<td>{esc(e.get("details") or "-")}</td></tr>')
        A('</tbody></table>')
    else:
        A('<div class="callout">Tidak ada aktivitas berkategori '
          '<strong>BLOCKED</strong> maupun <strong>INSTALLER</strong> pada periode '
          'ini.</div>')

    # ---- 4
    A('<h2>4. Kesimpulan</h2><ul>')
    for line in concl:
        A(f'<li>{line}</li>')
    A('</ul>')

    A('<h2>5. Saran Evaluasi Pembelajaran untuk Guru</h2><ol>')
    for r in recs:
        A(f'<li>{r}</li>')
    A('</ol>')

    A(f'<footer>Dibuat otomatis oleh <code>analyze_activities.py</code>. '
      f'Metode sesi: jeda antar-aktivitas &gt; {gap_minutes} menit dianggap sesi '
      f'baru. Durasi aplikasi diestimasi dari selisih waktu antar-aktivitas '
      f'(maksimal 10 menit per potongan), sehingga bersifat perkiraan.</footer>')
    A('</div></body></html>')
    return "".join(P)


# ----------------------------------------------------------------------------
# MAIN
# ----------------------------------------------------------------------------

def main():
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

    ap = argparse.ArgumentParser(description="Analisis activities.jsonl lab komputer")
    ap.add_argument("--file", help="path activities.jsonl")
    ap.add_argument("--out", default=os.path.dirname(os.path.abspath(__file__)),
                    help="folder output")
    ap.add_argument("--gap", type=int, default=15,
                    help="menit jeda pemisah sesi (default 15)")
    ap.add_argument("--label", default="", help="label data, mis. 'DATA SIMULASI'")
    ap.add_argument("--top", type=int, default=20, help="jumlah app teratas di konsol")
    ap.add_argument("--prefix", default="laporan_aktivitas_lab",
                    help="awalan nama file output")
    ap.add_argument("--drive", nargs="?", const="auto", default=None,
                    help="salin hasil ke Google Drive; tanpa nilai = deteksi otomatis")
    args = ap.parse_args()

    path = args.file or find_default_file()
    if not path or not os.path.exists(path):
        print("=" * 68)
        print("FILE activities.jsonl TIDAK DITEMUKAN")
        print("=" * 68)
        print("File ini dibuat otomatis oleh Molay Server saat ada aktivitas")
        print("siswa yang tercatat. Selama server belum menerima laporan")
        print("aktivitas dari klien, file belum ada.")
        print("")
        print("Langkah:")
        print("  1. Jalankan MolayServer-Sekar7.exe di folder ini.")
        print("  2. Jalankan MolayClient-Sekar7.exe di PC siswa, login, lalu")
        print("     gunakan komputer seperti biasa (10-15 menit cukup).")
        print("  3. Ulangi perintah ini.")
        return 2

    entries, bad, total = load_entries(path)
    if not entries:
        print("File ada tetapi tidak berisi baris valid: %s" % path)
        print("Baris terbaca: %d, gagal: %d" % (total, bad))
        return 2

    res = analyze(entries, args.gap)
    concl = build_conclusions(res, args.gap)
    recs = build_recommendations(res)

    ts = [e["_ts"] for e in entries]
    meta = {
        "file": os.path.basename(path),
        "start": min(ts).strftime("%d %B %Y %H:%M"),
        "end": max(ts).strftime("%d %B %Y %H:%M"),
        "bad": bad,
    }

    os.makedirs(args.out, exist_ok=True)
    md = render_markdown(res, concl, recs, args.label, args.gap, meta)
    html = render_html(res, concl, recs, args.label, args.gap, meta)

    p_md = os.path.join(args.out, args.prefix + ".md")
    p_html = os.path.join(args.out, args.prefix + ".html")
    p_json = os.path.join(args.out, args.prefix + "_ringkasan.json")

    with open(p_md, "w", encoding="utf-8") as f:
        f.write(md)
    with open(p_html, "w", encoding="utf-8") as f:
        f.write(html)

    summary = {
        "sumber": path,
        "label": args.label,
        "periode": {"mulai": meta["start"], "selesai": meta["end"]},
        "ringkasan": res["counts"],
        "total_durasi_menit": round(res["total_duration"], 1),
        "siswa": [{"user": s["user"], "durasi_menit": round(s["duration"], 1),
                   "sesi": s["sessions"], "pc": s["pcs"]}
                  for s in res["students"]],
        "komposisi": {g: res["group_count"].get(g, 0) for g in GROUP_ORDER},
        "aplikasi_teratas": [{"app": a["app"], "kategori": a["group"],
                              "frekuensi": a["count"],
                              "durasi_menit": round(a["duration"], 1)}
                             for a in res["apps"][:20]],
        "pelanggaran": [{"user": v["user"], "blocked": v["blocked"],
                         "installer": v["installer"], "total": v["total"]}
                        for v in res["viol_summary"]],
    }
    with open(p_json, "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2, ensure_ascii=False)

    # ---- sinkronisasi Google Drive
    drive_result = None
    if args.drive is not None:
        try:
            import drive_sync
            cfg = drive_sync.load_config()
            if args.drive != "auto":
                cfg["folder"] = args.drive
            drive_result = drive_sync.sync(
                prefix=args.prefix,
                tanggal=min(ts).strftime("%Y-%m-%d"),
                log_file=path,
                cfg=cfg,
                out_dir=args.out,
            )
        except Exception as exc:
            print("[DRIVE] Gagal sinkronisasi: %s" % exc)

    # ---- konsol
    print("=" * 68)
    print("ANALISIS AKTIVITAS LAB  |  %s" % os.path.basename(path))
    print("=" * 68)
    if args.label:
        print("!! %s !!" % args.label)
    print("Periode   : %s s.d. %s WIB" % (meta["start"], meta["end"]))
    print("Aktivitas : %d baris (%d gagal dibaca)" % (len(entries), bad))
    print("Siswa/PC  : %d siswa, %d PC" % (res["counts"]["users"],
                                           res["counts"]["pcs"]))
    print("Durasi    : %s akumulasi semua siswa" % fmt_dur(res["total_duration"]))
    print("")

    print("-- 1. SISWA AKTIF " + "-" * 50)
    for s in res["students"]:
        print("  %-14s %8s  %2d sesi  %-22s %.1f%%"
              % (s["user"], fmt_dur(s["duration"]), s["sessions"],
                 ",".join(s["pcs"]), s["share"]))
    print("")

    print("-- 2. KOMPOSISI " + "-" * 50)
    tot = sum(res["group_count"].values()) or 1
    for g in GROUP_ORDER:
        n = res["group_count"].get(g, 0)
        if n:
            print("  %-34s %5d  %5.1f%%  ~%s" % (
                g, n, 100.0 * n / tot, fmt_dur(res["group_dur"].get(g, 0))))
    print("")

    print("-- 2b. APLIKASI TERATAS " + "-" * 42)
    for a in res["apps"][:args.top]:
        print("  %-28s %5d  ~%-9s %s"
              % (a["app"][:28], a["count"], fmt_dur(a["duration"]), a["group"]))
    print("")

    print("-- 3. PELANGGARAN " + "-" * 48)
    if res["viol_summary"]:
        for v in res["viol_summary"]:
            print("  %-14s BLOCKED=%d  INSTALLER=%d  total=%d  (%s)"
                  % (v["user"], v["blocked"], v["installer"], v["total"],
                     ",".join(v["pcs"])))
    else:
        print("  Tidak ada aktivitas BLOCKED / INSTALLER.")
    print("")

    print("-- OUTPUT " + "-" * 56)
    print("  %s" % p_html)
    print("  %s" % p_md)
    print("  %s" % p_json)
    if drive_result is not None:
        if drive_result["ok"]:
            print("  Drive: %s (%d file)" % (drive_result["target"],
                                             len(drive_result["copied"])))
        else:
            print("  Drive: tidak tersinkron - %s" % drive_result["message"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
