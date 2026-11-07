; ========================================================
; Inno Setup Script: Molay Sekar7 - All In One Setup (Server + Client)
; ========================================================

#define MyAppName "Molay Sekar7 (All-in-One)"
#define MyAppVersion "1.0.2"
#define MyAppPublisher "Sekar7 Lab"

[Setup]
AppId={{E14A89BF-3C0B-4D78-9B9F-D7E5F81D64B2}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\Molay Sekar7
DefaultGroupName=Molay Sekar7
OutputDir=..\dist\Installers
OutputBaseFilename=Molay-Sekar7-Setup-Lengkap
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Types]
Name: "full"; Description: "Instalasi Lengkap (Server Guru + Klien Siswa dalam 1 PC)"; Flags: iscustom
Name: "serveronly"; Description: "Komputer Guru (Server Lab & Web Dashboard)"
Name: "clientonly"; Description: "Komputer Siswa (Klien Pengunci Layar & Monitoring Lab)"

[Dirs]
Name: "{app}"; Permissions: users-full
Name: "{app}\Client"; Permissions: users-full
Name: "{app}\Server"; Permissions: users-full

[Components]
Name: "client"; Description: "Program Klien Siswa (MolayClient-Sekar7.exe)"; Types: clientonly full
Name: "server"; Description: "Program Server Guru & Cloudflare Tunnel"; Types: serveronly full

[Tasks]
Name: "desktopicon_client"; Description: "Buat pintasan Klien di Desktop"; Components: client; Flags: unchecked
Name: "startupicon_client"; Description: "Kunci layar otomatis saat Windows menyala (Auto Startup Siswa)"; Components: client; Flags: unchecked
Name: "desktopicon_server"; Description: "Buat pintasan Dashboard Guru di Desktop"; Components: server
Name: "desktopicon_tunnel"; Description: "Buat pintasan Cloudflare Tunnel di Desktop"; Components: server; Flags: unchecked
Name: "autostart_server"; Description: "Jalankan Server Otomatis saat Windows Menyala (Direkomendasikan)"; Components: server
Name: "autostart_tunnel"; Description: "Aktifkan Cloudflare Tunnel sebagai Windows Service (Otomatis)"; Components: server

[Files]
; File Klien Siswa
Source: "..\dist\Molay-Client\MolayClient-Sekar7.exe"; DestDir: "{app}\Client"; Components: client; Flags: ignoreversion
Source: "..\dist\Molay-Client\server_ip.txt"; DestDir: "{app}\Client"; Components: client; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\update_key.txt"; DestDir: "{app}\Client"; Components: client; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\client_auth_mode.json"; DestDir: "{app}\Client"; Components: client; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\users_cache.json"; DestDir: "{app}\Client"; Components: client; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\browser_redirect.json"; DestDir: "{app}\Client"; Components: client; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Client\Pasang-Otomatis-Saat-Boot.bat"; DestDir: "{app}\Client"; Components: client; Flags: ignoreversion
Source: "..\dist\Molay-Client\Matikan-Otomatis-Saat-Boot.bat"; DestDir: "{app}\Client"; Components: client; Flags: ignoreversion
Source: "..\dist\Molay-Client\Jalankan-Molay-Client.bat"; DestDir: "{app}\Client"; Components: client; Flags: ignoreversion
Source: "..\dist\Molay-Client\Jalankan-Mode-Teknisi.bat"; DestDir: "{app}\Client"; Components: client; Flags: ignoreversion
Source: "..\dist\Molay-Client\PANDUAN_KLIEN.txt"; DestDir: "{app}\Client"; Components: client; Flags: ignoreversion

; File Server Guru
Source: "..\dist\Molay-Server\MolayServer-Sekar7.exe"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\cloudflared.exe"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\tunnel_token.txt"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\users.json"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\activities.jsonl"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\Jalankan-Molay-Server.bat"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\Jalankan-Cloudflare-Tunnel.bat"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\Pasang-Cloudflare-Service.bat"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\Izinkan-Firewall-Server.bat"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\PANDUAN_SERVER.txt"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Client\MolayClient-Sekar7.exe"; DestDir: "{app}\Server"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Client\MolayClient-Sekar7.exe"; DestDir: "{app}\Server\updates"; Components: server; Flags: ignoreversion
Source: "..\dist\Molay-Server\update_config.json"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\app_policy.json"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\client_auth_mode.json"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\service_keys.json"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\.env"; DestDir: "{app}\Server"; Components: server; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\analisis\*"; DestDir: "{app}\Server\analisis"; Components: server; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
; Icons Klien
Name: "{group}\Molay Sekar7 Client"; Filename: "{app}\Client\MolayClient-Sekar7.exe"; WorkingDir: "{app}\Client"; Components: client
Name: "{group}\Mode Teknisi Klien (Lihat Status & Log CMD)"; Filename: "{app}\Client\Jalankan-Mode-Teknisi.bat"; WorkingDir: "{app}\Client"; Components: client
Name: "{group}\Edit Alamat Server (server_ip.txt)"; Filename: "notepad.exe"; Parameters: """{app}\Client\server_ip.txt"""; Components: client
Name: "{autodesktop}\Molay Client Siswa"; Filename: "{app}\Client\MolayClient-Sekar7.exe"; WorkingDir: "{app}\Client"; Components: client; Tasks: desktopicon_client
Name: "{userstartup}\Molay Client Siswa"; Filename: "{app}\Client\MolayClient-Sekar7.exe"; WorkingDir: "{app}\Client"; Components: client; Tasks: startupicon_client

; Icons Server
Name: "{group}\Dashboard Server (Web Guru)"; Filename: "{app}\Server\Jalankan-Molay-Server.bat"; WorkingDir: "{app}\Server"; Components: server
Name: "{group}\Jalankan Cloudflare Tunnel"; Filename: "{app}\Server\Jalankan-Cloudflare-Tunnel.bat"; WorkingDir: "{app}\Server"; Components: server
Name: "{group}\Izinkan Firewall Server (Port 1916)"; Filename: "{app}\Server\Izinkan-Firewall-Server.bat"; WorkingDir: "{app}\Server"; Components: server
Name: "{group}\Analisis Log Aktivitas Lab"; Filename: "{app}\Server\analisis\Analisis-Log-Lab.bat"; WorkingDir: "{app}\Server\analisis"; Components: server
Name: "{autodesktop}\Dashboard Server Guru"; Filename: "{app}\Server\Jalankan-Molay-Server.bat"; WorkingDir: "{app}\Server"; Components: server; Tasks: desktopicon_server
Name: "{autodesktop}\Jalankan Cloudflare Tunnel"; Filename: "{app}\Server\Jalankan-Cloudflare-Tunnel.bat"; WorkingDir: "{app}\Server"; Components: server; Tasks: desktopicon_tunnel
Name: "{autodesktop}\Analisis Log Aktivitas Lab"; Filename: "{app}\Server\analisis\Analisis-Log-Lab.bat"; WorkingDir: "{app}\Server\analisis"; Components: server; Tasks: desktopicon_server

Name: "{group}\Uninstall Molay Sekar7"; Filename: "{uninstallexe}"

[Run]
Filename: "icacls.exe"; Parameters: """{app}"" /grant *S-1-5-32-545:(OI)(CI)F /T /C /Q"; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (TCP 1916)"""; Components: server; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (UDP 1916)"""; Components: server; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall add rule name=""Molay Sekar7 Server (TCP 1916)"" dir=in action=allow protocol=TCP localport=1916 profile=any"; Components: server; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall add rule name=""Molay Sekar7 Server (UDP 1916)"" dir=in action=allow protocol=UDP localport=1916 profile=any"; Components: server; Flags: runhidden

; === Daftarkan MolayServer sebagai Task Scheduler (auto-start saat login) ===
Filename: "schtasks.exe"; Parameters: "/delete /tn ""MolayServer-Sekar7"" /f"; Components: server; Flags: runhidden
Filename: "schtasks.exe"; Parameters: "/create /tn ""MolayServer-Sekar7"" /tr """"""{app}\Server\MolayServer-Sekar7.exe"""""" /sc onlogon /rl HIGHEST /f"; Components: server; Flags: runhidden; Tasks: autostart_server
Filename: "schtasks.exe"; Parameters: "/run /tn ""MolayServer-Sekar7"""; Components: server; Flags: runhidden; Tasks: autostart_server

; === Pasang Cloudflare Tunnel sebagai Windows Service permanen ===
Filename: "sc.exe"; Parameters: "stop cloudflared"; Components: server; Flags: runhidden
Filename: "{app}\Server\cloudflared.exe"; Parameters: "service uninstall"; Components: server; Flags: runhidden
Filename: "{app}\Server\cloudflared.exe"; Parameters: "service install {code:GetTunnelToken}"; Components: server; Flags: runhidden; Tasks: autostart_tunnel
Filename: "sc.exe"; Parameters: "start cloudflared"; Components: server; Flags: runhidden; Tasks: autostart_tunnel

; === Buka Dashboard setelah install ===
Filename: "{app}\Server\Jalankan-Molay-Server.bat"; Description: "Jalankan Dashboard Server Guru (http://localhost:1916)"; Components: server; Flags: shellexec nowait postinstall skipifsilent runasoriginaluser; WorkingDir: "{app}\Server"
Filename: "{app}\Client\MolayClient-Sekar7.exe"; Description: "Jalankan Molay Client Siswa sekarang"; Components: client; Flags: nowait postinstall skipifsilent; WorkingDir: "{app}\Client"

[UninstallRun]
Filename: "schtasks.exe"; Parameters: "/delete /tn ""MolayServer-Sekar7"" /f"; Components: server; Flags: runhidden
Filename: "sc.exe"; Parameters: "stop cloudflared"; Components: server; Flags: runhidden
Filename: "{app}\Server\cloudflared.exe"; Parameters: "service uninstall"; Components: server; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (TCP 1916)"""; Components: server; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (UDP 1916)"""; Components: server; Flags: runhidden
Filename: "taskkill.exe"; Parameters: "/F /IM MolayServer-Sekar7.exe"; Components: server; Flags: runhidden

[Code]
function GetTunnelToken(Param: String): String;
var
  TokenFile: String;
  Lines: TArrayOfString;
begin
  TokenFile := ExpandConstant('{app}\Server\tunnel_token.txt');
  if LoadStringsFromFile(TokenFile, Lines) and (GetArrayLength(Lines) > 0) then
    Result := Trim(Lines[0])
  else
    Result := '';
end;
