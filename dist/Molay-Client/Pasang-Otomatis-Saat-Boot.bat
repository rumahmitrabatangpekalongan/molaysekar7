@echo off
title Pasang Molay Sekar7 Saat Windows Hidup
color 0b
echo ========================================================
echo   PASANG OTOMATIS SAAT WINDOWS MENYALA (AUTORUN)
echo ========================================================
echo.
echo Script ini akan membuat shortcut di folder Startup Windows.
echo Molay Client akan otomatis berjalan saat Windows baru dinyalakan.
echo (Sistem akan mengunci layar jika mode Akun Siswa aktif,
echo  atau langsung membuka desktop jika mode Bebas Masuk aktif).
echo.

set SCRIPT_DIR=%~dp0
set TARGET_EXE=%SCRIPT_DIR%MolayClient-Sekar7.exe
set SHORTCUT_PATH=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\MolayClient-Sekar7.lnk

powershell -Command "$s=(New-Object -COM WScript.Shell).CreateShortcut('%SHORTCUT_PATH%');$s.TargetPath='%TARGET_EXE%';$s.WorkingDirectory='%SCRIPT_DIR%';$s.Save()"

echo [OK] MolayClient-Sekar7 berhasil dipasang ke Windows Startup!
echo.
pause
