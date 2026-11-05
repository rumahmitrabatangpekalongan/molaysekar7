@echo off
title MOLAY SEKAR7 - COMPILER INSTALLER SETUP WIZARD
cd /d "%~dp0"

echo ========================================================
echo       MOLAY SEKAR7 - MEMBUAT SOFTWARE INSTALLER (.EXE)
echo ========================================================
echo.

set ISCC="C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
if not exist %ISCC% (
    echo [ERROR] Inno Setup Compiler tidak ditemukan di: %ISCC%
    pause
    exit /b 1
)

if not exist dist\Installers mkdir dist\Installers

echo [1/2] Membuat Installer Klien Siswa (MolayClient-Sekar7-Setup.exe)...
%ISCC% installers\client_setup.iss
if errorlevel 1 (
    echo [GAGAL] Gagal membuat installer klien.
    pause
    exit /b 1
)

echo.
echo [2/3] Membuat Installer Server Guru (MolayServer-Sekar7-Setup.exe)...
%ISCC% installers\server_setup.iss
if errorlevel 1 (
    echo [GAGAL] Gagal membuat installer server.
    pause
    exit /b 1
)

echo.
echo [3/3] Membuat Installer ALL-IN-ONE Lengkap (Molay-Sekar7-Setup-Lengkap.exe)...
%ISCC% installers\master_setup.iss
if errorlevel 1 (
    echo [GAGAL] Gagal membuat installer lengkap.
    pause
    exit /b 1
)

echo.
echo ========================================================
echo   SUKSES! Semua software installer berhasil dibuat di:
echo   dist\Installers\
echo ========================================================
pause
