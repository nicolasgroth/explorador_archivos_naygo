; Naygo — script de Inno Setup. Genera el instalador (setup.exe).
; Copyright (c) 2026 Nicolás Groth / ISGroth. MIT License.
;
; La versión se inyecta desde scripts/build-release.ps1 con /DMyAppVersion=...
; (fuente única: el Cargo.toml del workspace). El default de abajo es solo para
; compilar el .iss a mano sin el script.

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#define MyAppName "Naygo"
#define MyAppPublisher "ISGroth"
#define MyAppURL "https://github.com/nicolasgroth/explorador_archivos_naygo"
#define MyAppExe "naygo.exe"

[Setup]
; AppId fijo: identifica el producto para upgrades/desinstalación (NO cambiar entre versiones).
AppId={{B7E6A4C2-1F3D-4E9B-9C2A-A1B2C3D4E5F6}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
; Modo elegible: el asistente pregunta "para mí" (sin admin) o "para todos" (admin).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=Naygo-{#MyAppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Imágenes del asistente (BMP generados desde logo_naygo.png por el script de build).
WizardImageFile=wizard-large.bmp
WizardSmallImageFile=wizard-small.bmp
LicenseFile=..\LICENSE
SetupIconFile=..\assets\icons\naygo_icon.ico
UninstallDisplayIcon={app}\{#MyAppExe}
; Si Naygo está corriendo durante un update, ofrecer cerrarlo antes de reemplazar
; el .exe (evita el error "archivo en uso"). No reiniciar la app automáticamente.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Acceso directo en el escritorio (marcado por defecto vía el grupo estándar).
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
; Integraciones opcionales con el shell (desmarcadas por defecto).
Name: "openwith"; Description: "Registrar Naygo en 'Abrir con' para carpetas"; Flags: unchecked
Name: "ctxmenu"; Description: "Agregar 'Abrir en Naygo' al menú contextual de carpetas"; Flags: unchecked

[Files]
; Único ejecutable (CRT estático + assets embebidos), licencia y readme.
Source: "..\target\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Menú Inicio siempre; escritorio si se marcó la tarea.
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; "Abrir con" (NO predeterminado): registra el ProgId y lo lista para carpetas.
Root: HKA; Subkey: "Software\Classes\Naygo.Folder"; ValueType: string; ValueData: "Carpeta en Naygo"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Naygo.Folder\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Directory\OpenWithProgids"; ValueType: string; ValueName: "Naygo.Folder"; ValueData: ""; Flags: uninsdeletevalue; Tasks: openwith
; Menú contextual "Abrir en Naygo" en carpetas y en el fondo de carpeta. %V = la carpeta.
Root: HKA; Subkey: "Software\Classes\Directory\shell\Naygo"; ValueType: string; ValueData: "Abrir en Naygo"; Flags: uninsdeletekey; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\Naygo"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#MyAppExe}"; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\shell\Naygo\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%V"""; Flags: uninsdeletekey; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Naygo"; ValueType: string; ValueData: "Abrir en Naygo"; Flags: uninsdeletekey; Tasks: ctxmenu
Root: HKA; Subkey: "Software\Classes\Directory\Background\shell\Naygo\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%V"""; Flags: uninsdeletekey; Tasks: ctxmenu

[Run]
; Página final: ofrecer ejecutar Naygo.
Filename: "{app}\{#MyAppExe}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
