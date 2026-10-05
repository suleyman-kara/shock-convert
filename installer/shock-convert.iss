; Inno Setup betiği. Derleme: iscc /DAppVersion=0.1.0 installer\shock-convert.iss
; Önce `cargo build --release` çalıştırılmış olmalı.

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppId={{6F1B7C52-3D0A-4E8B-9A57-5C0D2E94B1A3}
AppName=Shock Convert
AppVersion={#AppVersion}
AppPublisher=Shock Convert
; Yönetici hakkı gerekmez: her şey kullanıcı profiline (HKCU / LocalAppData) kurulur.
PrivilegesRequired=lowest
DefaultDirName={localappdata}\Programs\ShockConvert
DisableProgramGroupPage=yes
DisableDirPage=yes
UninstallDisplayIcon={app}\shock-convert.exe
OutputDir=..\dist
OutputBaseFilename=shock-convert-setup-{#AppVersion}
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\target\release\shock-convert.exe"; DestDir: "{app}"; Flags: ignoreversion

[Run]
; Menüyü (eklentiler dahil) kayıt defterine yazar.
Filename: "{app}\shock-convert.exe"; Parameters: "register"; Flags: runhidden

[UninstallRun]
; Dosyalar silinmeden ÖNCE menü anahtarlarını temizler.
Filename: "{app}\shock-convert.exe"; Parameters: "unregister"; Flags: runhidden; RunOnceId: "ShockConvertUnregister"

[UninstallDelete]
; Kullanıcının sonradan eklediği eklentiler dahil klasör tamamen kalksın.
Type: filesandordirs; Name: "{app}"
; Günlük dosyası. Kullanıcı preset'leri ({userappdata}\ShockConvert\presets.toml) kullanıcı verisidir, silinmez.
Type: filesandordirs; Name: "{localappdata}\ShockConvert"

