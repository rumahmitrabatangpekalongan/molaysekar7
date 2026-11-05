@echo off
title Molay Sekar7 - Client Agent Siswa
color 0a
cd /d "%~dp0"

echo ========================================================
echo        MOLAY SEKAR7 - CLIENT AGENT (KOMPUTER SISWA)
echo ========================================================
echo.

if exist "server_ip.txt" (
    echo Alamat Target Server di server_ip.txt:
    type "server_ip.txt"
) else (
    echo [INFO] server_ip.txt tidak ditemukan, menggunakan auto-discovery LAN.
)

echo.
echo ========================================================
echo Menghubungkan ke Server Guru & Menampilkan Layar Kunci...
echo (Tips: Untuk melihat log teknisi / Command Prompt,
echo        jalankan Jalankan-Mode-Teknisi.bat)
echo ========================================================
echo.

MolayClient-Sekar7.exe
pause
