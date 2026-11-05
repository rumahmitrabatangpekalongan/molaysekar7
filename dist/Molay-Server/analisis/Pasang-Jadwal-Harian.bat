@echo off
chcp 65001 >nul
title Pasang Jadwal Analisis Harian
cd /d "%~dp0"

set "TASK_NAME=Analisis Log Lab Harian"
set "TARGET=\"%~dp0Analisis-Log-Lab.bat\" --silent"

echo ============================================================
echo  PASANG JADWAL ANALISIS OTOMATIS
echo ============================================================
echo.
echo  Laporan akan dibuat dan dikirim ke Google Drive setiap hari
echo  pada jam yang Anda tentukan, tanpa perlu klik apa pun.
echo.
echo  Ketik jam (format 24 jam, contoh 15:30), atau ketik HAPUS
echo  untuk membatalkan jadwal yang sudah ada.
echo.

set "JAM="
set /p "JAM=Jam berapa? "

if /i "%JAM%"=="HAPUS" goto :hapus
if "%JAM%"=="" goto :batal

schtasks /Create /TN "%TASK_NAME%" /TR "%TARGET%" /SC DAILY /ST %JAM% /F
if errorlevel 1 (
  echo.
  echo GAGAL membuat jadwal. Coba jalankan file ini dengan
  echo klik kanan - "Run as administrator".
  echo.
  pause
  exit /b 1
)

echo.
echo Jadwal terpasang: setiap hari pukul %JAM%.
echo Catatan: komputer harus menyala pada jam tersebut.
echo.
pause
exit /b 0

:hapus
schtasks /Delete /TN "%TASK_NAME%" /F
echo.
echo Jadwal dihapus.
echo.
pause
exit /b 0

:batal
echo.
echo Dibatalkan.
echo.
pause
exit /b 0
