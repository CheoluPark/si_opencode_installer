#if defined(TEST_BUILD)
  #define PayloadRoot "..\target\inno-fixture"
#else
  #define PayloadRoot "..\payload"
#endif

[Setup]
AppId={{AD4EA35D-919A-4B77-A932-E79BFAC12007}
AppName=SI OpenCode
AppVersion=0.1.0
DefaultDirName={localappdata}\Programs\SI OpenCode
DefaultGroupName=SI OpenCode
PrivilegesRequired=lowest
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
#if defined(TEST_BUILD)
  OutputDir=..\target\inno-syntax-check
  OutputBaseFilename=NOT_FOR_DISTRIBUTION
#else
  OutputDir=..\out
  OutputBaseFilename=si_opencode_installer
#endif
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\si_opencode_updater.exe

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[Files]
Source: "{#PayloadRoot}\si_opencode_updater.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadRoot}\si_opencode_updater.exe"; Flags: dontcopy
Source: "..\scripts\setup-runtime.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\verify-desktop.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\config\extensions.example.json"; DestDir: "{localappdata}\SI OpenCode"; DestName: "extensions.json"; Flags: ignoreversion onlyifdoesntexist

[Icons]
Name: "{group}\SI OpenCode"; Filename: "{app}\si_opencode_updater.exe"; Parameters: "--launch"

[Code]
var
  SecretsPage: TInputQueryWizardPage;

function InitializeSetup: Boolean;
var
  Code: Integer;
  StagePath: String;
begin
  Result := False;
  ExtractTemporaryFile('si_opencode_updater.exe');
  StagePath := ExpandConstant('{tmp}\si-opencode-current-release');
  if not Exec(ExpandConstant('{tmp}\si_opencode_updater.exe'),
      '--fetch-current-release "' + StagePath + '"', ExpandConstant('{tmp}'), SW_SHOW,
      ewWaitUntilTerminated, Code) or (Code <> 0) then begin
    MsgBox('서명된 현재 릴리스를 확인하지 못했습니다. 설치를 취소합니다.', mbError, MB_OK);
    Exit;
  end;
  Result := True;
end;

function JsonString(const Text: String): String;
var
  I: Integer;
begin
  Result := '"';
  for I := 1 to Length(Text) do begin
    if Text[I] = '"' then Result := Result + '\"'
    else if Text[I] = '\' then Result := Result + '\\'
    else if Ord(Text[I]) < 32 then Result := Result + ' '
    else Result := Result + Text[I];
  end;
  Result := Result + '"';
end;

procedure InitializeWizard;
begin
  SecretsPage := CreateInputQueryPage(wpWelcome, '사내 서비스 연결',
    'API Key 및 PAT 입력', 'wiki와 work의 PAT는 필수이며 code는 선택 사항입니다.');
  SecretsPage.Add('사내 LLM API Key:', True);
  SecretsPage.Add('Confluence (wiki) PAT:', True);
  SecretsPage.Add('Jira (work) PAT:', True);
  SecretsPage.Add('Bitbucket (code) PAT (선택, Optional):', True);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if CurPageID = SecretsPage.ID then begin
    if (Trim(SecretsPage.Values[0]) = '') or (Trim(SecretsPage.Values[1]) = '') or
       (Trim(SecretsPage.Values[2]) = '') then begin
      MsgBox('LLM API Key, wiki PAT, work PAT을 모두 입력해야 합니다.', mbError, MB_OK);
      Result := False;
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Data: TArrayOfString;
  InputFile: String;
  Code: Integer;
begin
  if CurStep <> ssPostInstall then Exit;
  InputFile := ExpandConstant('{tmp}\si-opencode-input.json');
  SetArrayLength(Data, 1);
  Data[0] := '{"api_key":' + JsonString(SecretsPage.Values[0]) +
    ',"wiki_pat":' + JsonString(SecretsPage.Values[1]) +
    ',"work_pat":' + JsonString(SecretsPage.Values[2]) +
    ',"code_pat":' + JsonString(SecretsPage.Values[3]) + '}';
  if not SaveStringsToUTF8FileWithoutBOM(InputFile, Data, False) then
    RaiseException('설치 입력을 안전하게 준비할 수 없습니다.');
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\setup-runtime.ps1') +
      '" -AppDir "' + ExpandConstant('{app}') + '" -ReleaseDir "' +
      ExpandConstant('{tmp}\si-opencode-current-release') + '"',
      ExpandConstant('{app}'), SW_SHOW, ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('필수 OpenCode Desktop 설치 또는 검증에 실패했습니다.');
  if not Exec(ExpandConstant('{app}\si_opencode_updater.exe'),
      '--provision "' + InputFile + '"', ExpandConstant('{app}'), SW_SHOW,
      ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('서비스 연결 검증 또는 OpenCode 구성에 실패했습니다. 설치가 완료되지 않았습니다.');
end;
