@echo off
title Molay Sekar7 - Server Lab Guru ^& Cloudflare Tunnel
color 0b
cd /d "%~dp0"

echo ========================================================
echo       MOLAY SEKAR7 - SERVER MONITORING ^& CLOUDFLARE
echo ========================================================
echo.

echo [1/4] Memeriksa proses server lama...
taskkill /F /IM MolayServer-Sekar7.exe >nul 2>&1

if not exist "MolayServer-Sekar7.exe" goto :not_found

echo.
echo [2/4] Memeriksa Windows Defender Firewall (Port 1916)...
netsh advfirewall firewall show rule name="Molay Sekar7 Server (TCP 1916)" >nul 2>&1
if errorlevel 1 (
    echo       [PERHATIAN] Port 1916 belum diizinkan di Firewall Windows.
    echo       Membuka izin firewall otomatis - klik YES bila muncul konfirmasi Admin...
    if exist "Izinkan-Firewall-Server.bat" (
        start "" "Izinkan-Firewall-Server.bat"
    )
) else (
    echo       [OK] Port 1916 sudah diizinkan di Windows Firewall.
)

set "CF_TOKEN=eyJhIjoiYTMyNzMwNDkyYzQ3YWE0MDkzY2Y4Y2FlNmQyMTU0OGEiLCJ0IjoiNjhjYTNkZDAtYzM4YS00NWZjLWFiZTItZWFjYzI3OGUyOWRkIiwicyI6IlpUVTRZbVJoTldFdE5UQmpOeTAwTnpWbUxUazNOek10TmpSa01UQTJOMlZrWkRneiJ9"
if exist "tunnel_token.txt" set /p CF_TOKEN=<tunnel_token.txt

echo.
echo [3/4] Memeriksa koneksi Cloudflare Zero Trust Tunnel...
tasklist /FI "IMAGENAME eq cloudflared.exe" 2>nul | find /I "cloudflared.exe" >nul
if errorlevel 1 (
    if exist "cloudflared.exe" (
        start "Molay Cloudflare Tunnel" /min cloudflared.exe tunnel run --url http://localhost:1916 --token %CF_TOKEN%
        echo       [OK] Cloudflare Tunnel otomatis dinyalakan di background!
        set "SPAWNED_TUNNEL=1"
    ) else (
        echo       [INFO] cloudflared.exe tidak ditemukan, berjalan dalam mode LAN saja.
        set "SPAWNED_TUNNEL=0"
    )
) else (
    echo       [OK] Cloudflare Tunnel sudah aktif berjalan di background.
    set "SPAWNED_TUNNEL=0"
)

echo.
echo [4/4] Menjalankan Server Molay di port 1916...
echo.
echo IP Komputer Guru di Jaringan LAN:
ipconfig | findstr /i "IPv4"
echo.
echo ========================================================
echo  Dashboard Web Guru:
echo  - Akses Lokal / LAN : http://localhost:1916
echo  - Akses Internet    : https://molay.sekolahkartini7.web.id
echo  - Analisa Aktivitas : http://localhost:1916/analisaaktivitas
echo                      : https://molay.sekolahkartini7.web.id/analisaaktivitas
echo ========================================================
echo.

echo Membuka Dashboard Web Guru di browser...
net session >nul 2>&1
if %errorlevel% == 0 (
    start "" explorer "http://localhost:1916"
) else (
    start "" "http://localhost:1916"
)

MolayServer-Sekar7.exe
set "EXIT_CODE=%errorlevel%"

if "%SPAWNED_TUNNEL%"=="1" (
    echo.
    echo Menutup Cloudflare Tunnel...
    taskkill /F /IM cloudflared.exe >nul 2>&1
)

if %EXIT_CODE% neq 0 goto :server_err
goto :eof

:not_found
echo [ERROR] File MolayServer-Sekar7.exe tidak ditemukan!
pause
exit /b 1

:server_err
echo.
echo Server terhenti atau terjadi kendala.
pause
