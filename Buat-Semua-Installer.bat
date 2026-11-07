@echo off
title MOLAY SEKAR7 - BUILD RELEASE & INSTALLER OTOMATIS
cd /d "%~dp0"

echo ========================================================
echo   MOLAY SEKAR7 - BUILD OTOMATIS BINARY & INSTALLER (.EXE)
echo ========================================================
echo.

set ISCC="C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
if not exist %ISCC% (
    echo [ERROR] Inno Setup Compiler tidak ditemukan di: %ISCC%
    pause
    exit /b 1
)

if not exist dist\Installers mkdir dist\Installers

echo [1/4] Mengompilasi Binary Rust Release (Server & Klien)...
cargo build --release
if errorlevel 1 (
    echo [GAGAL] Gagal mengompilasi program Rust.
    pause
    exit /b 1
)

echo.
echo [2/4] Memperbarui file binary di folder dist...
if exist target\release\MolayServer-Sekar7.exe (
    copy /y target\release\MolayServer-Sekar7.exe dist\Molay-Server\ >nul
    echo       [OK] dist\Molay-Server\MolayServer-Sekar7.exe diperbarui.
)
if exist target\release\MolayClient-Sekar7.exe (
    copy /y target\release\MolayClient-Sekar7.exe dist\Molay-Client\ >nul
    echo       [OK] dist\Molay-Client\MolayClient-Sekar7.exe diperbarui.
)

echo.
echo [3/4] Membuat Installer Klien Siswa (MolayClient-Sekar7-Setup.exe)...
%ISCC% installers\client_setup.iss >nul
if errorlevel 1 (
    echo [GAGAL] Gagal membuat installer klien.
    pause
    exit /b 1
)
echo       [OK] dist\Installers\MolayClient-Sekar7-Setup.exe siap!

echo.
echo [4/4] Membuat Installer Server Guru & Paket Lengkap...
%ISCC% installers\server_setup.iss >nul
if errorlevel 1 (
    echo [GAGAL] Gagal membuat installer server.
    pause
    exit /b 1
)
echo       [OK] dist\Installers\MolayServer-Sekar7-Setup.exe siap!

%ISCC% installers\master_setup.iss >nul
if errorlevel 1 (
    echo [GAGAL] Gagal membuat installer paket lengkap.
    pause
    exit /b 1
)
echo       [OK] dist\Installers\Molay-Sekar7-Setup-Lengkap.exe siap!

echo.
echo ========================================================
echo   SUKSES! Seluruh binary dan installer terbaru siap di:
echo   dist\Installers\
echo ========================================================
pause
