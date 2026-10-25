#if defined(TEST_BUILD)
  #define PayloadRoot "..\target\inno-fixture"
  #define KB_HASH "0000000000000000000000000000000000000000000000000000000000000000"
  #define KB_SIZE 1
  #define UBUNTU_HASH "0000000000000000000000000000000000000000000000000000000000000000"
  #define UBUNTU_SIZE 1
  #define DESKTOP_HASH "0000000000000000000000000000000000000000000000000000000000000000"
  #define DESKTOP_SIZE 1
  #define WSL_HASH "0000000000000000000000000000000000000000000000000000000000000000"
  #define WSL_SIZE 1
  #define MANAGED_HASH "0000000000000000000000000000000000000000000000000000000000000000"
  #define MANAGED_SIZE 1
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
Source: "{src}\opencode_desktop_win64.exe"; DestDir: "{app}"; Hash: "{#DESKTOP_HASH}"; ExternalSize: {#DESKTOP_SIZE}; Flags: external ignoreversion
Source: "{#PayloadRoot}\manifest.json"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadRoot}\manifest.sig"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\setup-runtime.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\setup-schedule.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\start-kb.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\cleanup.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\scripts\verify-desktop.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "{src}\managed-runtime.tar.gz"; DestDir: "{app}"; Hash: "{#MANAGED_HASH}"; ExternalSize: {#MANAGED_SIZE}; Flags: external ignoreversion
Source: "{#PayloadRoot}\updater.json"; DestDir: "{localappdata}\SI OpenCode"; Flags: ignoreversion onlyifdoesntexist
Source: "{src}\kb-runtime.tar.gz"; DestDir: "{app}"; Hash: "{#KB_HASH}"; ExternalSize: {#KB_SIZE}; Flags: external ignoreversion; Check: IsFpga
Source: "{src}\ubuntu-resolute-amd64.wsl"; DestDir: "{app}"; Hash: "{#UBUNTU_HASH}"; ExternalSize: {#UBUNTU_SIZE}; Flags: external ignoreversion; Check: IsFpga
Source: "..\scripts\wsl-preflight.ps1"; Flags: dontcopy

[Icons]
Name: "{group}\SI OpenCode"; Filename: "{app}\si_opencode_updater.exe"; Parameters: "--launch"
Name: "{group}\SI OpenCode 업데이트 확인"; Filename: "{app}\si_opencode_updater.exe"; Parameters: "--update"

[UninstallRun]
Filename: "{app}\si_opencode_updater.exe"; Parameters: "--uninstall-managed"; Flags: runhidden; RunOnceId: "RemoveManaged"
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\cleanup.ps1"""; Flags: runhidden; RunOnceId: "RemoveTasks"

[Code]
var
  SecretsPage: TInputQueryWizardPage;
  FpgaPage: TWizardPage;
  YesButton, NoButton: TNewRadioButton;

function IsFpga: Boolean;
begin
  Result := YesButton.Checked;
end;

function IsNotFpga: Boolean;
begin
  Result := not IsFpga;
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
  SecretsPage.Add('Bitbucket (code) PAT (선택):', True);

  FpgaPage := CreateCustomPage(SecretsPage.ID, 'FPGA 업무', 'FPGA Digital Logic 설계 업무 담당자입니까?');
  YesButton := TNewRadioButton.Create(FpgaPage);
  YesButton.Parent := FpgaPage.Surface;
  YesButton.Left := ScaleX(16);
  YesButton.Top := ScaleY(16);
  YesButton.Width := FpgaPage.SurfaceWidth - ScaleX(16);
  YesButton.Caption := '예';
  NoButton := TNewRadioButton.Create(FpgaPage);
  NoButton.Parent := FpgaPage.Surface;
  NoButton.Left := ScaleX(16);
  NoButton.Top := ScaleY(48);
  NoButton.Width := FpgaPage.SurfaceWidth - ScaleX(16);
  NoButton.Caption := '아니요';
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
  if (CurPageID = FpgaPage.ID) and not (YesButton.Checked or NoButton.Checked) then begin
    MsgBox('FPGA 업무 담당 여부에 예 또는 아니요를 선택하세요.', mbError, MB_OK);
    Result := False;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
  StatusPath: String;
begin
  Result := '';
  NeedsRestart := False;
  if not IsFpga then Exit;
  ExtractTemporaryFile('wsl-preflight.ps1');
  if not FileExists(ExpandConstant('{src}\wsl.msi')) then begin
    Result := 'FPGA 설치에는 설치기와 같은 폴더의 wsl.msi가 필요합니다.';
    Exit;
  end;
  StatusPath := ExpandConstant('{tmp}\wsl-preflight.status');
  DeleteFile(StatusPath);
  WizardForm.Caption := 'SI OpenCode - WSL2 준비 중';
  WizardForm.StatusLabel.Caption := 'WSL2를 준비합니다. 관리자 권한 창이 나타나면 승인해 주세요.';
  WizardForm.Refresh;
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{tmp}\wsl-preflight.ps1') +
      '" -Msi "' + ExpandConstant('{src}\wsl.msi') + '" -StatusPath "' + StatusPath + '"',
      ExpandConstant('{tmp}'), SW_SHOW, ewWaitUntilTerminated, Code) then begin
    Result := 'WSL 사전 준비를 시작할 수 없습니다.';
    Exit;
  end;
  if Code = 3010 then begin
    NeedsRestart := True;
    if not RegWriteStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\RunOnce',
       'SI OpenCode Resume', '"' + ExpandConstant('{srcexe}') + '"') then
      Result := '재부팅 후 설치를 재개할 RunOnce 등록에 실패했습니다.'
    else Result := 'WSL2를 활성화했습니다. 재부팅 동의 후 같은 설치기를 다시 실행합니다.';
  end else if Code <> 0 then begin
    ForceDirectories(ExpandConstant('{localappdata}\SI OpenCode\logs'));
    SaveStringToFile(ExpandConstant('{localappdata}\SI OpenCode\logs\wsl-preflight.log'),
      'Installer observed wsl-preflight exit code ' + IntToStr(Code) + #13#10, True);
    Exec(ExpandConstant('{sys}\notepad.exe'),
      '"' + ExpandConstant('{localappdata}\SI OpenCode\logs\wsl-preflight.log') + '"',
      ExpandConstant('{tmp}'), SW_SHOW, ewNoWait, Code);
    Result := 'WSL2 준비/복구가 실패했습니다 (exit code ' + IntToStr(Code) + '). 진단 로그를 메모장으로 열었습니다.';
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Data: TArrayOfString;
  InputFile, FpgaValue, FpgaArgument: String;
  Code: Integer;
begin
  if CurStep <> ssPostInstall then Exit;
  if IsFpga then begin FpgaValue := 'true'; FpgaArgument := '1'; end
  else begin FpgaValue := 'false'; FpgaArgument := '0'; end;
  InputFile := ExpandConstant('{tmp}\si-opencode-input.json');
  SetArrayLength(Data, 1);
  Data[0] := '{"api_key":' + JsonString(SecretsPage.Values[0]) +
    ',"wiki_pat":' + JsonString(SecretsPage.Values[1]) +
    ',"work_pat":' + JsonString(SecretsPage.Values[2]) +
    ',"code_pat":' + JsonString(SecretsPage.Values[3]) +
    ',"fpga":' + FpgaValue + '}';
  if not SaveStringsToUTF8FileWithoutBOM(InputFile, Data, False) then
    RaiseException('설치 입력을 안전하게 준비할 수 없습니다.');
  if not Exec(ExpandConstant('{app}\si_opencode_updater.exe'),
      '--provision "' + InputFile + '"', ExpandConstant('{app}'), SW_SHOW,
      ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('서비스 연결 검증 또는 OpenCode 구성에 실패했습니다. 설치가 완료되지 않았습니다.');
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\setup-runtime.ps1') +
      '" -AppDir "' + ExpandConstant('{app}') + '" -ManagedRuntime "' + ExpandConstant('{app}\managed-runtime.tar.gz') + '" -Fpga ' + FpgaArgument,
      ExpandConstant('{app}'), SW_SHOW, ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('Desktop/CLI/KB 구성이 실패했습니다. 재실행하여 복구하세요.');
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
      '-NoProfile -ExecutionPolicy Bypass -File "' + ExpandConstant('{app}\setup-schedule.ps1') +
      '" -AppDir "' + ExpandConstant('{app}') + '" -Fpga ' + FpgaArgument,
      ExpandConstant('{app}'), SW_SHOW, ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException('업데이트 확인 일정 구성이 실패했습니다.');
end;
