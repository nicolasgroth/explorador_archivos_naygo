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
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "it"; MessagesFile: "compiler:Languages\Italian.isl"
Name: "pt"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"

[CustomMessages]
en.StartupWin=Start Naygo when Windows starts
es.StartupWin=Iniciar Naygo al arrancar Windows
en.OpenWithFolders=Register Naygo in 'Open with' for folders
es.OpenWithFolders=Registrar Naygo en 'Abrir con' para carpetas
en.CtxMenuFolders=Add 'Open in Naygo' to the folder context menu
es.CtxMenuFolders=Agregar 'Abrir en Naygo' al menú contextual de carpetas
en.AppLangPage=Naygo language
es.AppLangPage=Idioma de Naygo
en.AppLangPrompt=Choose the language Naygo will start in:
es.AppLangPrompt=Elige el idioma con el que Naygo se iniciará:

[Tasks]
; Acceso directo en el escritorio (marcado por defecto vía el grupo estándar).
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
; Iniciar con Windows (desmarcada por defecto).
Name: "startupwin"; Description: "{cm:StartupWin}"; Flags: unchecked
; Integraciones opcionales con el shell (desmarcadas por defecto).
Name: "openwith"; Description: "{cm:OpenWithFolders}"; Flags: unchecked
Name: "ctxmenu"; Description: "{cm:CtxMenuFolders}"; Flags: unchecked

[Files]
; Único ejecutable (CRT estático + assets embebidos), licencia y readme.
Source: "..\target\release\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\THIRD-PARTY-NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Menú Inicio siempre; escritorio si se marcó la tarea.
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Desinstalar {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; Iniciar con Windows (clave Run del usuario). --tray = arrancar minimizado en bandeja.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Naygo"; ValueData: """{app}\{#MyAppExe}"" --tray"; Flags: uninsdeletevalue; Tasks: startupwin
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

[Code]
var
  LangPage: TInputOptionWizardPage;

function NaygoLangId(Index: Integer): String;
begin
  case Index of
    0: Result := 'en';
    1: Result := 'es';
    2: Result := 'de';
    3: Result := 'fr';
    4: Result := 'it';
    5: Result := 'pt';
    6: Result := 'ja';
    7: Result := 'hi';
    8: Result := 'ko';
    9: Result := 'zh';
  else Result := 'en';
  end;
end;

// Mapea el LangID primario de Windows (GetUILanguage and $3FF) a un índice de NaygoLangId.
function DetectWindowsLangIndex(): Integer;
var prim: Integer;
begin
  prim := GetUILanguage() and $3FF;
  case prim of
    $09: Result := 0; // inglés
    $0A: Result := 1; // español
    $07: Result := 2; // alemán
    $0C: Result := 3; // francés
    $10: Result := 4; // italiano
    $16: Result := 5; // portugués
    $11: Result := 6; // japonés
    $39: Result := 7; // hindi
    $12: Result := 8; // coreano
    $04: Result := 9; // chino
  else Result := 0;
  end;
end;

procedure InitializeWizard();
begin
  LangPage := CreateInputOptionPage(wpSelectTasks,
    ExpandConstant('{cm:AppLangPage}'), '',
    ExpandConstant('{cm:AppLangPrompt}'), True, False);
  LangPage.Add('English');
  LangPage.Add('Español');
  LangPage.Add('Deutsch');
  LangPage.Add('Français');
  LangPage.Add('Italiano');
  LangPage.Add('Português');
  LangPage.Add('日本語');
  LangPage.Add('हिन्दी');
  LangPage.Add('한국어');
  LangPage.Add('中文');
  LangPage.SelectedValueIndex := DetectWindowsLangIndex();
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  path, content, lang: String;
begin
  if CurStep = ssPostInstall then
  begin
    path := ExpandConstant('{app}\settings.json');
    if not FileExists(path) then
    begin
      lang := NaygoLangId(LangPage.SelectedValueIndex);
      content := '{' + #13#10 + '  "version": 2,' + #13#10 +
                 '  "language": "' + lang + '"' + #13#10 + '}';
      SaveStringToFile(path, content, False);
    end;
  end;
end;
