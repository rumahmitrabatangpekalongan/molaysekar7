@echo off
title MOLAY SEKAR7 - PASANG CLOUDFLARE SERVICE (AUTO BOOT)
cd /d "%~dp0"

echo ====================================================================
echo        MOLAY SEKAR7 - PASANG CLOUDFLARE TUNNEL SEBAGAI WINDOWS SERVICE
echo ====================================================================
echo PERHATIAN: Skrip ini membutuhkan hak akses Administrator.
echo.

if not exist cloudflared.exe (
    echo [ERROR] File cloudflared.exe tidak ditemukan di folder ini!
    pause
    exit /b 1
)

cloudflared.exe service install eyJhIjoiYTMyNzMwNDkyYzQ3YWE0MDkzY2Y4Y2FlNmQyMTU0OGEiLCJ0IjoiZGQxNDhmZDMtNGNhYy00YmIxLTkwYzUtZWNkZDg3Zjc0YmVhIiwicyI6Ik9HVTFaR0kyTURFdE5qTXpaUzAwTVRRMkxXSTVNbVV0TkdFd1l6RXpPRFJpTUdGbCJ9

echo.
echo Jika berhasil, Tunnel akan otomatis aktif setiap kali Windows dinyalakan!
pause
