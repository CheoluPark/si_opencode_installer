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
        Some("--check-update") => check_update(),
        Some("--launch") => {
            let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
                .join("Programs/@opencodedesktop/OpenCode.exe");
            if !desktop.is_file() {
                return Err("OpenCode Desktop executable not found".into());
            }
            Command::new(desktop).spawn()?;
            Ok(())
        }
        _ => Err("Usage: si_opencode_updater --provision <input-file> | --sync-extensions | --fetch-current-release <staging-dir> | --check-update | --launch".into()),
    }
}

#[cfg(windows)]
fn check_update() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, fs};
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONINFORMATION, MB_YESNO};
    let state = std::path::PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode/installed-release.json");
    let installed = fs::read(&state).ok().and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok()).and_then(|value| value["version"].as_str().map(str::to_owned)).unwrap_or_default();
    let latest = si_opencode_updater::release_download::current_version_from_local_config()?;
    if latest == installed { return Ok(()); }
    let text: Vec<_> = format!("새 SI OpenCode release {latest}가 있습니다. 최신 installer를 실행해 업데이트하시겠습니까?").encode_utf16().chain([0]).collect();
    let caption: Vec<_> = "SI OpenCode 업데이트".encode_utf16().chain([0]).collect();
    let accepted = unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION | MB_YESNO) == IDYES };
    if accepted {
        let message: Vec<_> = "회사 공유 폴더의 최신 si_opencode_installer.exe를 실행해 업데이트를 진행하세요.".encode_utf16().chain([0]).collect();
        unsafe { MessageBoxW(std::ptr::null_mut(), message.as_ptr(), caption.as_ptr(), MB_ICONINFORMATION); }
    }
    Ok(())
}
