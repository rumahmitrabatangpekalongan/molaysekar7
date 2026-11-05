@echo off
title Izinkan Windows Defender Firewall - Molay Sekar7
color 0a

:: Memeriksa hak akses Administrator (UAC)
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo ====================================================================
    echo    MEMINTA IZIN ADMINISTRATOR (UAC) UNTUK MENGUBAH FIREWALL...
    echo ====================================================================
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Start-Process -FilePath '%~f0' -Verb RunAs -WorkingDirectory '%~dp0'"
    exit /b
)

cd /d "%~dp0"
echo ====================================================================
echo    MENGIZINKAN PORT 1916 DI WINDOWS DEFENDER FIREWALL (PC GURU)
echo ====================================================================
echo.
echo [1/3] Menghapus aturan lama yang mungkin duplikat...
netsh advfirewall firewall delete rule name="Molay Sekar7 Server (TCP 1916)" >nul 2>&1
netsh advfirewall firewall delete rule name="Molay Sekar7 Server (UDP 1916)" >nul 2>&1
netsh advfirewall firewall delete rule name="Molay Server Program" >nul 2>&1

echo [2/3] Mendaftarkan aturan TCP & UDP port 1916 untuk SEMUA profil (Domain, Private, Public)...
netsh advfirewall firewall add rule name="Molay Sekar7 Server (TCP 1916)" dir=in action=allow protocol=TCP localport=1916 profile=any
netsh advfirewall firewall add rule name="Molay Sekar7 Server (UDP 1916)" dir=in action=allow protocol=UDP localport=1916 profile=any

echo [3/3] Mendaftarkan izin program MolayServer-Sekar7.exe...
if exist "%~dp0MolayServer-Sekar7.exe" (
    netsh advfirewall firewall add rule name="Molay Server Program" dir=in action=allow program="%~dp0MolayServer-Sekar7.exe" profile=any
)

echo.
echo ====================================================================
echo [SUKSES] Port 1916 & Program Server BERHASIL diizinkan di Windows Firewall!
echo Komputer Siswa di lab (LAN) sekarang BISA terhubung langsung ke Server Guru.
echo ====================================================================
echo.
pause
