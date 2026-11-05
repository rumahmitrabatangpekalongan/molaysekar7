@echo off
chcp 65001 >nul
title Analisis Log Aktivitas Lab
cd /d "%~dp0"

set "SILENT=0"
if /i "%~1"=="--silent" set "SILENT=1"

set "PY="
where python >nul 2>&1 && set "PY=python"
if not defined PY (where py >nul 2>&1 && set "PY=py")

if not defined PY (
  if "%SILENT%"=="0" (
    echo.
    echo Python belum terpasang di komputer ini.
    echo Unduh di https://www.python.org/downloads/ lalu centang
    echo "Add Python to PATH" saat instalasi.
    echo.
    pause
  )
  exit /b 1
)

echo Menganalisis activities.jsonl ...
echo.

if exist "..\activities.jsonl" (
  %PY% analyze_activities.py --file "..\activities.jsonl" --drive auto
) else if exist "activities.jsonl" (
  %PY% analyze_activities.py --file "activities.jsonl" --drive auto
) else if exist "activities_demo_SIMULASI.jsonl" (
  echo [INFO] Log activities.jsonl belum terbentuk (belum ada siswa login).
  echo Menampilkan contoh simulasi laporan aktivitas lab...
  echo.
  %PY% analyze_activities.py --file "activities_demo_SIMULASI.jsonl" --label "CONTOH SIMULASI LAB" --drive auto
) else (
  %PY% analyze_activities.py --drive auto
)

if "%SILENT%"=="1" exit /b 0

if exist "laporan_aktivitas_lab.html" (
  echo Membuka laporan di browser ...
  start "" "laporan_aktivitas_lab.html"
) else if exist "laporan_CONTOH_SIMULASI.html" (
  echo Membuka contoh laporan di browser ...
  start "" "laporan_CONTOH_SIMULASI.html"
)

echo.
echo ========================================================
echo Dashboard Monitoring Server Guru: http://localhost:1916
echo ========================================================

echo.
pause
