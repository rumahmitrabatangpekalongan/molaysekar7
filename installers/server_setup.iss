; ========================================================
; Inno Setup Script: Molay Sekar7 Server (PC Guru)
; ========================================================

#define MyAppName "Molay Sekar7 Server"
#define MyAppVersion "1.0.2"
#define MyAppPublisher "Sekar7 Lab"
#define MyAppExeName "MolayServer-Sekar7.exe"

[Setup]
AppId={{D8F41A3B-8C42-43E8-9B5A-64D0F8E32D9A}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\Molay Sekar7 Server
DefaultGroupName=Molay Sekar7
OutputDir=..\dist\Installers
OutputBaseFilename=MolayServer-Sekar7-Setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Dirs]
Name: "{app}"; Permissions: users-full

[Tasks]
Name: "desktopicon"; Description: "Buat pintasan di Desktop"; GroupDescription: "Pintasan:"
Name: "autostart_server"; Description: "Jalankan Server Otomatis saat Windows Menyala (Direkomendasikan)"; GroupDescription: "Auto-Start:"
Name: "autostart_tunnel"; Description: "Aktifkan Cloudflare Tunnel sebagai Windows Service (Otomatis)"; GroupDescription: "Auto-Start:"

[Files]
Source: "..\dist\Molay-Server\MolayServer-Sekar7.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\MolayClient-Sekar7.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Client\MolayClient-Sekar7.exe"; DestDir: "{app}\updates"; Flags: ignoreversion
Source: "..\dist\Molay-Server\cloudflared.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\tunnel_token.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\users.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\activities.jsonl"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\Jalankan-Molay-Server.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\Jalankan-Cloudflare-Tunnel.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\Pasang-Cloudflare-Service.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\Izinkan-Firewall-Server.bat"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\PANDUAN_SERVER.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\Molay-Server\update_config.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\app_policy.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\client_auth_mode.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\service_keys.json"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\.env"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\dist\Molay-Server\analisis\*"; DestDir: "{app}\analisis"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Dashboard Molay Server (Web Guru)"; Filename: "{app}\Jalankan-Molay-Server.bat"; WorkingDir: "{app}"
Name: "{group}\Jalankan Cloudflare Tunnel"; Filename: "{app}\Jalankan-Cloudflare-Tunnel.bat"; WorkingDir: "{app}"
Name: "{group}\Izinkan Firewall Server (Port 1916)"; Filename: "{app}\Izinkan-Firewall-Server.bat"; WorkingDir: "{app}"
Name: "{group}\Analisis Log Aktivitas Lab"; Filename: "{app}\analisis\Analisis-Log-Lab.bat"; WorkingDir: "{app}\analisis"
Name: "{group}\Uninstall Molay Server"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Dashboard Molay Server"; Filename: "{app}\Jalankan-Molay-Server.bat"; WorkingDir: "{app}"; Tasks: desktopicon
Name: "{autodesktop}\Jalankan Cloudflare Tunnel"; Filename: "{app}\Jalankan-Cloudflare-Tunnel.bat"; WorkingDir: "{app}"; Tasks: desktopicon
Name: "{autodesktop}\Analisis Log Aktivitas Lab"; Filename: "{app}\analisis\Analisis-Log-Lab.bat"; WorkingDir: "{app}\analisis"; Tasks: desktopicon

[Run]
; === Izinkan port 1916 di Firewall Windows ===
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (TCP 1916)"""; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (UDP 1916)"""; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall add rule name=""Molay Sekar7 Server (TCP 1916)"" dir=in action=allow protocol=TCP localport=1916 profile=any"; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall add rule name=""Molay Sekar7 Server (UDP 1916)"" dir=in action=allow protocol=UDP localport=1916 profile=any"; Flags: runhidden

; === Daftarkan MolayServer sebagai Task Scheduler (auto-start saat login) ===
Filename: "schtasks.exe"; Parameters: "/delete /tn ""MolayServer-Sekar7"" /f"; Flags: runhidden
Filename: "schtasks.exe"; Parameters: "/create /tn ""MolayServer-Sekar7"" /tr """"""{app}\MolayServer-Sekar7.exe"""""" /sc onlogon /rl HIGHEST /f"; Flags: runhidden; Tasks: autostart_server
Filename: "schtasks.exe"; Parameters: "/run /tn ""MolayServer-Sekar7"""; Flags: runhidden; Tasks: autostart_server

; === Pasang Cloudflare Tunnel sebagai Windows Service permanen ===
Filename: "sc.exe"; Parameters: "stop cloudflared"; Flags: runhidden
Filename: "{app}\cloudflared.exe"; Parameters: "service uninstall"; Flags: runhidden
Filename: "{app}\cloudflared.exe"; Parameters: "service install {code:GetTunnelToken}"; Flags: runhidden; Tasks: autostart_tunnel
Filename: "sc.exe"; Parameters: "start cloudflared"; Flags: runhidden; Tasks: autostart_tunnel

; === Buka Dashboard setelah install ===
Filename: "{app}\Jalankan-Molay-Server.bat"; Description: "Jalankan Molay Server & Buka Dashboard (http://localhost:1916)"; Flags: shellexec nowait postinstall skipifsilent runasoriginaluser; WorkingDir: "{app}"

[UninstallRun]
; === Bersihkan saat Uninstall ===
Filename: "schtasks.exe"; Parameters: "/delete /tn ""MolayServer-Sekar7"" /f"; Flags: runhidden
Filename: "sc.exe"; Parameters: "stop cloudflared"; Flags: runhidden
Filename: "{app}\cloudflared.exe"; Parameters: "service uninstall"; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (TCP 1916)"""; Flags: runhidden
Filename: "netsh"; Parameters: "advfirewall firewall delete rule name=""Molay Sekar7 Server (UDP 1916)"""; Flags: runhidden
Filename: "taskkill.exe"; Parameters: "/F /IM MolayServer-Sekar7.exe"; Flags: runhidden

[Code]
{ Baca token dari tunnel_token.txt agar tidak hard-code di installer }
function GetTunnelToken(Param: String): String;
var
  TokenFile: String;
  Lines: TArrayOfString;
begin
  TokenFile := ExpandConstant('{app}\tunnel_token.txt');
  if LoadStringsFromFile(TokenFile, Lines) and (GetArrayLength(Lines) > 0) then
    Result := Trim(Lines[0])
  else
    Result := '';
end;
