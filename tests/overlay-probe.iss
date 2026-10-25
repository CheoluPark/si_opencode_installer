[Setup]
AppName=SI Overlay Probe
AppVersion=1.0
DefaultDirName={localappdata}\SIOverlayProbe
PrivilegesRequired=lowest
OutputDir=..\target\overlay-probe
OutputBaseFilename=overlay-probe
Uninstallable=no
DisableWelcomePage=yes

[Code]
function InitializeSetup(): Boolean;
begin
  Result := False;
end;
