; ========================================================
; Inno Setup Script: Molay Sekar7 Client (PC Siswa)
; ========================================================

#define MyAppName "Molay Sekar7 Client"
#define MyAppVersion "1.0.2"
#define MyAppPublisher "Sekar7 Lab"
#define MyAppExeName "MolayClient-Sekar7.exe"

[Setup]
AppId={{C6D298E1-267A-4D3B-A8FE-1D0A5774E91B}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\Molay Sekar7 Client
DefaultGroupName=Molay Sekar7
OutputDir=..\dist\Installers
OutputBaseFilename=MolayClient-Sekar7-Setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Dirs]
Name: "{app}"; Permissions: users-full

[Tasks]
Name: "startupicon"; Description: "Jalankan otomatis saat Windows menyala (Auto Startup Client)"; GroupDescription: "Opsi Startup:"; Flags: unchecked
Name: "desktopicon"; Description: "Buat pintasan di Desktop"; GroupDescription: "Pintasan Tambahan:"; Flags: unchecked

[Files]
Source: "..\dist\Molay-Client\MolayClient-Sekar7.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\server_ip.txt"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\update_key.txt"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\client_auth_mode.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\users_cache.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\browser_redirect.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\Pasang-Otomatis-Saat-Boot.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\Matikan-Otomatis-Saat-Boot.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\Jalankan-Molay-Client.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\Jalankan-Mode-Teknisi.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\PANDUAN_KLIEN.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Molay Sekar7 Client"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"
Name: "{group}\Mode Teknisi (Lihat Status & Log CMD)"; Filename: "{app}\Jalankan-Mode-Teknisi.bat"; WorkingDir: "{app}"
Name: "{group}\Edit Alamat Server (server_ip.txt)"; Filename: "notepad.exe"; Parameters: """{app}\server_ip.txt"""
Name: "{group}\Aktifkan Startup Otomatis"; Filename: "{app}\Pasang-Otomatis-Saat-Boot.bat"; WorkingDir: "{app}"
Name: "{group}\Matikan Startup Otomatis"; Filename: "{app}\Matikan-Otomatis-Saat-Boot.bat"; WorkingDir: "{app}"
Name: "{group}\Uninstall Molay Client"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Molay Sekar7 Client"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Tasks: desktopicon
Name: "{userstartup}\Molay Sekar7 Client"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Tasks: startupicon

[Run]
Filename: "icacls.exe"; Parameters: """{app}"" /grant *S-1-5-32-545:(OI)(CI)F /T /C /Q"; Flags: runhidden
Filename: "{app}\{#MyAppExeName}"; Description: "Jalankan Molay Sekar7 Client sekarang"; Flags: nowait postinstall skipifsilent
