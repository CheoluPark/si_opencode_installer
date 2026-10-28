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
Source: "..\scripts\setup-schedule.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\config\release.example.json"; DestDir: "{localappdata}\SI OpenCode"; DestName: "release.json"; Flags: ignoreversion onlyifdoesntexist

[Icons]
Name: "{group}\SI OpenCode"; Filename: "{app}\si_opencode_updater.exe"; Parameters: "--launch"

[Code]
var
  SecretsPage: TInputQueryWizardPage;
  ReleaseStage: String;
  RebootRequired: Boolean;

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
var
  Code: Integer;
begin
  Result := True;
  if CurPageID = SecretsPage.ID then begin
    if (Trim(SecretsPage.Values[0]) = '') or (Trim(SecretsPage.Values[1]) = '') or
       (Trim(SecretsPage.Values[2]) = '') then begin
      MsgBox('LLM API Key, wiki PAT, work PAT을 모두 입력해야 합니다.', mbError, MB_OK);
      Result := False;
      Exit;
    end;
    ExtractTemporaryFile('si_opencode_updater.exe');
    ReleaseStage := ExpandConstant('{tmp}\si-opencode-current-release');
    if not Exec(ExpandConstant('{tmp}\si_opencode_updater.exe'),
        '--fetch-current-release "' + ReleaseStage + '"', ExpandConstant('{tmp}'), SW_SHOW,
        ewWaitUntilTerminated, Code) or (Code <> 0) then begin
      MsgBox('서명된 현재 릴리스를 확인하지 못했습니다. 설치를 취소합니다.', mbError, MB_OK);
      Result := False;
      Exit;
    end;
    if DirExists(ExpandConstant('{userprofile}\.config\opencode')) and
       (MsgBox('기존 OpenCode 설정을 백업한 뒤 새 설정 번들로 교체합니다. 계속하시겠습니까?', mbConfirmation, MB_YESNO) <> IDYES) then
      Result := False;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Data: TArrayOfString;
  InputFile: String;
  WarningSummary: String;
  I: Integer;
  Code: Integer;
begin
  if CurStep = ssDone then begin
    InputFile := ExpandConstant('{tmp}\si-opencode-optional-tool-warnings.txt');
    if FileExists(InputFile) then begin
      LoadStringsFromFile(InputFile, Data);
      for I := 0 to GetArrayLength(Data) - 1 do WarningSummary := WarningSummary + Data[I] + #13#10;
      if WarningSummary <> '' then
        MsgBox('설치는 완료되었지만 선택 도구 일부를 건너뛰거나 설치하지 못했습니다:' + #13#10 + #13#10 + WarningSummary, mbInformation, MB_OK);
    end;
    if RebootRequired and (MsgBox('설치를 완료하려면 Windows 재부팅이 필요합니다. 지금 재부팅하시겠습니까?', mbConfirmation, MB_YESNO) = IDYES) then
      Exec(ExpandConstant('{sys}\shutdown.exe'), '/r /t 0', '', SW_HIDE, ewNoWait, Code);
    Exit;
  end;
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
       ExpandConstant('{tmp}\si-opencode-current-release') + '" -WarningFile "' + ExpandConstant('{tmp}\si-opencode-optional-tool-warnings.txt') + '"',
      ExpandConstant('{app}'), SW_SHOW, ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('필수 OpenCode Desktop 설치 또는 검증에 실패했습니다.');
  if not Exec(ExpandConstant('{app}\si_opencode_updater.exe'),
      '--provision "' + InputFile + '" "' + ReleaseStage + '" --overwrite-confirmed', ExpandConstant('{app}'), SW_SHOW,
      ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('서비스 연결 검증 또는 OpenCode 구성에 실패했습니다. 설치가 완료되지 않았습니다.');
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\setup-schedule.ps1') + '" -AppDir "' + ExpandConstant('{app}') + '"',
      ExpandConstant('{app}'), SW_SHOW, ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('업데이트 확인 일정 구성에 실패했습니다.');
  RebootRequired := FileExists(ExpandConstant('{tmp}\si-opencode-reboot-required'));
end;
