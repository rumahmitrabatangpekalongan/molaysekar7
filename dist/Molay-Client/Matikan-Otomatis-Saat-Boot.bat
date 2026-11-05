@echo off
title Matikan Autostart Molay Sekar7
color 0c
echo ========================================================
echo   HAPUS AUTOSTARTUP WINDOWS (NONAKTIFKAN RUN SAAT BOOT)
echo ========================================================
echo.
echo Script ini akan menghapus shortcut Molay dari folder Startup Windows.
echo Komputer TIDAK AKAN otomatis menjalankan Molay saat baru dinyalakan.
echo.

set SHORTCUT_PATH=%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\MolayClient-Sekar7.lnk

if exist "%SHORTCUT_PATH%" (
    del /f /q "%SHORTCUT_PATH%"
    echo [OK] Shortcut Startup berhasil dihapus!
    echo Molay Client tidak akan berjalan otomatis saat boot.
) else (
    echo [INFO] Shortcut Startup tidak ditemukan atau sudah nonaktif.
)

echo.
pause
