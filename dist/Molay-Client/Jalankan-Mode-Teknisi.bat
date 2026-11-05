@echo off
title Molay Sekar7 - Mode Teknisi (Konsol Live Status)
color 0e
cd /d "%~dp0"

echo ====================================================================
echo      MOLAY SEKAR7 - MODE TEKNISI / DEBUG (LIVE COMMAND PROMPT)
echo ====================================================================
echo  Jendela Command Prompt ini sengaja dimunculkan di paling depan
echo  agar Anda dapat melihat secara langsung kondisi software:
echo   - Status pencarian Server di jaringan LAN (Auto-Discovery)
echo   - Percobaan koneksi WebSocket ^& Smart Failover ke Tunnel
echo   - Penerapan Mode Autentikasi (Bebas / Password / Akun Siswa)
echo   - Laju streaming layar (FPS ^& Encoding JPEG)
echo   - Status deteksi pembaruan versi (OTA Updater)
echo ====================================================================
echo.

if exist "server_ip.txt" (
    echo Alamat Target Server di server_ip.txt:
    type "server_ip.txt"
    echo.
) else (
    echo [INFO] server_ip.txt tidak ditemukan, menggunakan auto-discovery LAN.
)

echo Menjalankan MolayClient-Sekar7.exe dalam Mode Teknisi...
echo (Tekan Ctrl+C di jendela ini jika ingin menutup program kapan saja)
echo.

MolayClient-Sekar7.exe --debug
set "EXIT_CODE=%errorlevel%"

echo.
echo ====================================================================
echo Program telah berhenti (Exit Code: %EXIT_CODE%).
echo ====================================================================
pause
