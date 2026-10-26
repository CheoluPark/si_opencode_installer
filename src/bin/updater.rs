#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        eprintln!("SI OpenCode: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
    std::process::exit(1);
}

#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, process::Command};

    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("--provision") => {
            let input = args.next().ok_or("Expected installer input file")?;
            let stage = args.next().ok_or("Expected release staging directory")?;
            let confirmed = args.next().as_deref() == Some("--overwrite-confirmed");
            si_opencode_updater::provision::install(input.as_ref(), stage.as_ref(), confirmed)
        }
        Some("--sync-extensions") => si_opencode_updater::provision::sync_extensions(),
        Some("--fetch-current-release") => {
            let stage = args.next().ok_or("Expected release staging directory")?;
            si_opencode_updater::release_download::fetch_from_local_config(stage.as_ref())
        }
        Some("--check-update") => {
            let _ = check_update();
            Ok(())
        }
        Some("--apply-update") => apply_update(),
        Some("--launch") => {
            let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
                .join("Programs/@opencodedesktop/OpenCode.exe");
            if !desktop.is_file() {
                return Err("OpenCode Desktop executable not found".into());
            }
            Command::new(desktop).spawn()?;
            Ok(())
        }
        _ => Err("Usage: si_opencode_updater --provision <input-file> | --sync-extensions | --fetch-current-release <staging-dir> | --check-update | --apply-update | --launch".into()),
    }
}

#[cfg(windows)]
fn check_update() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs};
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO};
    let state = std::path::PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode/installed-release.json");
    let installed = fs::read(&state).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()).and_then(|value| value["version"].as_str().map(str::to_owned)).unwrap_or_default();
    let latest = si_opencode_updater::release_download::current_version_from_local_config()?;
    if !version_newer(&latest, &installed) { return Ok(()); }
    let text: Vec<_> = format!("새 SI OpenCode release {latest}가 있습니다. 최신 installer를 실행해 업데이트하시겠습니까?").encode_utf16().chain([0]).collect();
    let caption: Vec<_> = "SI OpenCode 업데이트".encode_utf16().chain([0]).collect();
    let accepted = unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION | MB_YESNO) == IDYES };
    if accepted { apply_update()?; }
    Ok(())
}

#[cfg(windows)]
fn version_newer(latest: &str, installed: &str) -> bool {
    if installed.is_empty() { return true; }
    let parse = |value: &str| value.split(|ch: char| !ch.is_ascii_digit()).filter(|part| !part.is_empty()).map(|part| part.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let latest = parse(latest);
    let installed = parse(installed);
    latest > installed
}

#[cfg(windows)]
fn apply_update() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, process::Command};
    use windows_sys::{Win32::{Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError}, System::Threading::CreateMutexW, UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO}}};
    let mutex_name: Vec<_> = "Local\\SI_OpenCode_Update".encode_utf16().chain([0]).collect();
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_name.as_ptr()) };
    if mutex.is_null() { return Err("Cannot acquire update lock".into()); }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS { unsafe { CloseHandle(mutex); }; return Ok(()); }
    let temp = tempfile::tempdir()?;
    let stage = temp.path().join("release");
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        si_opencode_updater::release_download::fetch_from_local_config(&stage)?;
        let app = env::current_exe()?.parent().ok_or("Updater location unknown")?.to_path_buf();
        let runtime = app.join("setup-runtime.ps1");
        let status = Command::new("powershell.exe").args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(runtime).arg("-AppDir").arg(&app).arg("-ReleaseDir").arg(&stage).status()?;
        if !status.success() { return Err("Release tool update failed".into()); }
        si_opencode_updater::provision::apply_update(&stage)
    })();
    unsafe { CloseHandle(mutex); }
    let version = result?;
    let text: Vec<_> = format!("SI OpenCode release {version}를 적용했습니다. 기존 API Key와 PAT는 유지됩니다.").encode_utf16().chain([0]).collect();
    let caption: Vec<_> = "SI OpenCode 업데이트".encode_utf16().chain([0]).collect();
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION); }
    let restart: Vec<_> = "변경된 plugin과 MCP를 반영하려면 OpenCode를 재시작해야 합니다. 지금 재시작하시겠습니까?".encode_utf16().chain([0]).collect();
    if unsafe { MessageBoxW(std::ptr::null_mut(), restart.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION | MB_YESNO) } == IDYES {
        let _ = Command::new("taskkill.exe").args(["/IM", "OpenCode.exe", "/F"]).status();
        let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?).join("Programs/@opencodedesktop/OpenCode.exe");
        Command::new(desktop).spawn()?;
    }
    Ok(())
}
