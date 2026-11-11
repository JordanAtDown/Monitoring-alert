; Monitoring Alert - installateur Windows (Inno Setup 6)
;
; Installation par utilisateur, sans droits administrateur : %LOCALAPPDATA%\Programs\MonitoringAlert.
; L'installateur ne fait que copier le fichier ; tout le reste (nettoyage d'une version 1.x, migration
; de config.toml, copie sans fenetre, demarrage automatique, demarrage) est fait par
; "monitoring-alert.exe install --in-place", qui est teste. Idem a la desinstallation.
;
; Compilation (depuis la racine du depot) :
;   iscc /DAppVersion=2.0.0 /DExePath=..\backend\target\release\monitoring-alert.exe installer\monitoring-alert.iss

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef ExePath
  #define ExePath "..\backend\target\release\monitoring-alert.exe"
#endif
#define AppName "Monitoring Alert"
#define DashboardUrl "http://127.0.0.1:8787/"

[Setup]
AppId={{62E21ACE-53A6-48D0-9065-D303F3479C13}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=monitoring-alert
AppPublisherURL=https://github.com/JordanAtDown/monitoring-alert
DefaultDirName={autopf}\MonitoringAlert
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=commandline
OutputDir=..\dist
OutputBaseFilename=monitoring-alert-setup-v{#AppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\monitoring-alert.exe
CloseApplications=no
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "french"; MessagesFile: "compiler:Languages\French.isl"

[Files]
Source: "{#ExePath}"; DestDir: "{app}"; DestName: "monitoring-alert.exe"; Flags: ignoreversion
; Modele de configuration : jamais ecrase, jamais supprime a la desinstallation.
Source: "config.toml"; DestDir: "{app}"; Flags: onlyifdoesntexist uninsneveruninstall
; Extrait au debut de l'assistant pour retrouver la base de donnees actuelle.
Source: "{#ExePath}"; Flags: dontcopy

[INI]
Filename: "{app}\Tableau de bord.url"; Section: "InternetShortcut"; Key: "URL"; String: "{#DashboardUrl}"

[Icons]
Name: "{autoprograms}\{#AppName} - Tableau de bord"; Filename: "{app}\Tableau de bord.url"

[Run]
Filename: "{#DashboardUrl}"; Description: "Ouvrir le tableau de bord"; Flags: shellexec postinstall skipifsilent nowait

[UninstallRun]
Filename: "{app}\monitoring-alert.exe"; Parameters: "uninstall"; Flags: runhidden waituntilterminated; RunOnceId: "StopAndUnregister"

[UninstallDelete]
; Crees par "install", pas par cet installateur.
Type: files; Name: "{app}\monitoring-daemon.exe"
Type: files; Name: "{app}\Tableau de bord.url"

[Code]
const
  NL = #13#10;

var
  DbPage: TInputFileWizardPage;
  DefaultDb: String;

{ Lance une commande en capturant sa sortie dans un fichier. }
function RunCapture(const Exe, Params, OutFile: String; var ResultCode: Integer): Boolean;
begin
  Result := Exec(ExpandConstant('{cmd}'),
    '/C ""' + Exe + '" ' + Params + ' > "' + OutFile + '" 2>&1"',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

{ Valeur d'une ligne "cle=valeur". }
function ReadKey(const Lines: TArrayOfString; const Key: String): String;
var
  I: Integer;
begin
  Result := '';
  for I := 0 to GetArrayLength(Lines) - 1 do
    if Pos(Key + '=', Lines[I]) = 1 then
    begin
      Result := Copy(Lines[I], Length(Key) + 2, MaxInt);
      Exit;
    end;
end;

{ Une ancienne version 1.x (service Windows) ne peut etre supprimee qu'en administrateur. }
function InitializeSetup(): Boolean;
var
  Code: Integer;
begin
  Result := True;
  if Exec(ExpandConstant('{sys}\sc.exe'), 'query MonitoringAlert', '', SW_HIDE, ewWaitUntilTerminated, Code)
     and (Code = 0) and (not IsAdmin) then
  begin
    if MsgBox('Une ancienne version de Monitoring Alert (service Windows) est installée.' + NL +
              'Pour la remplacer, l''installation doit s''exécuter en administrateur.' + NL + NL +
              'Relancer en administrateur maintenant ?', mbConfirmation, MB_YESNO) = IDYES then
      ShellExec('runas', ExpandConstant('{srcexe}'), '', '', SW_SHOWNORMAL, ewNoWait, Code);
    Result := False;
  end;
end;

{ Page "Base de donnees" : preremplie avec la base de l'installation actuelle. }
procedure InitializeWizard;
var
  Lines: TArrayOfString;
  Found, Summary, Info: String;
  Code: Integer;
begin
  Found := '';
  DefaultDb := '';
  Summary := '';
  ExtractTemporaryFile('monitoring-alert.exe');
  if RunCapture(ExpandConstant('{tmp}\monitoring-alert.exe'), 'db locate --porcelain',
                ExpandConstant('{tmp}\db-locate.txt'), Code)
     and (Code = 0)
     and LoadStringsFromFile(ExpandConstant('{tmp}\db-locate.txt'), Lines) then
  begin
    Found := ReadKey(Lines, 'path');
    DefaultDb := ReadKey(Lines, 'default');
    Summary := ReadKey(Lines, 'summary');
  end;

  if Summary <> '' then
    Info := 'Base détectée : ' + Summary + '.' + NL +
            'Gardez ce fichier pour retrouver tout votre historique, ou choisissez-en un autre.'
  else
    Info := 'Choisissez l''emplacement de la base de données (elle sera créée si elle n''existe pas).';

  DbPage := CreateInputFilePage(wpSelectDir, 'Base de données', 'Historique des températures', Info);
  DbPage.Add('Fichier de la base de données :', 'Base SQLite (*.db)|*.db|Tous les fichiers (*.*)|*.*', '.db');
  if Found <> '' then
    DbPage.Values[0] := Found
  else
    DbPage.Values[0] := DefaultDb;
end;

{ Parametre --db-path, seulement si la base choisie n'est pas l'emplacement par defaut. }
function DbPathParam(Param: String): String;
var
  V: String;
begin
  V := Trim(DbPage.Values[0]);
  if (V = '') or (CompareText(V, DefaultDb) = 0) then
    Result := ''
  else
    Result := ' --db-path "' + V + '"';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Code: Integer;
begin
  if CurStep = ssPostInstall then
  begin
    WizardForm.StatusLabel.Caption := 'Configuration et démarrage...';
    if (not RunCapture(ExpandConstant('{app}\monitoring-alert.exe'),
                       'install --in-place' + DbPathParam(''),
                       ExpandConstant('{tmp}\install-output.txt'), Code)) or (Code <> 0) then
      MsgBox('La configuration automatique a échoué (code ' + IntToStr(Code) + ').' + NL + NL +
             'Ouvrez une invite de commandes et lancez :' + NL +
             '"' + ExpandConstant('{app}\monitoring-alert.exe') + '" install --in-place' + NL +
             'pour voir le message d''erreur.', mbError, MB_OK);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if (CurUninstallStep = usPostUninstall) and (not UninstallSilent) then
  begin
    DataDir := ExpandConstant('{localappdata}\Programs\MonitoringAlert');
    if DirExists(DataDir) then
      if MsgBox('Supprimer aussi la configuration, les journaux et la base de données de ce dossier ?' +
                NL + NL + DataDir + NL + NL +
                'Une base de données placée dans un autre dossier n''est jamais supprimée.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
