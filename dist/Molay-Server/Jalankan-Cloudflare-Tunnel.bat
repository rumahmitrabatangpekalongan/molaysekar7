@echo off
title MOLAY SEKAR7 - CLOUDFLARE TUNNEL CONNECTOR
cd /d "%~dp0"

echo ====================================================================
echo        MOLAY SEKAR7 - CLOUDFLARE ZERO TRUST TUNNEL CONNECTOR
echo ====================================================================
echo Menghubungkan Server Molay lokal (localhost:1916) ke Cloudflare Tunnel...
echo.

if not exist cloudflared.exe (
    echo [ERROR] File cloudflared.exe tidak ditemukan di folder ini!
    echo Silakan pastikan cloudflared.exe ada di folder yang sama.
    pause
    exit /b 1
)

cloudflared.exe tunnel run --url http://localhost:1916 --token eyJhIjoiYTMyNzMwNDkyYzQ3YWE0MDkzY2Y4Y2FlNmQyMTU0OGEiLCJ0IjoiNjhjYTNkZDAtYzM4YS00NWZjLWFiZTItZWFjYzI3OGUyOWRkIiwicyI6IlpUVTRZbVJoTldFdE5UQmpOeTAwTnpWbUxUazNOek10TmpSa01UQTJOMlZrWkRneiJ9

pause
