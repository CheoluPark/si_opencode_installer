#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        eprintln!("Update check failed: {error}");
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
    use si_opencode_updater::{
        apply, config as oc_config, credentials, load_state,
        nas::{Nas, NasError},
        plan, verify_manifest, Action, NasConfig,
    };
    use std::{env, fs, process::Command};

    let action = env::args().nth(1);
    if action.as_deref() == Some("--launch") {
        let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
            .join("Programs/@opencodedesktop/OpenCode.exe");
        if !desktop.is_file() {
            return Err("OpenCode Desktop executable not found".into());
        }
        Command::new(desktop).spawn()?;
        Command::new(env::current_exe()?).arg("--update").spawn()?;
        return Ok(());
    }
    if action.as_deref() == Some("--show-whats-new") {
        let version = env::args()
            .nth(2)
            .ok_or("Expected release-notes version")?
            .parse()?;
        return show_whats_new(version);
    }
    if action.as_deref() == Some("--verify-kb") {
        return si_opencode_updater::validate::local_kb().map_err(Into::into);
    }
    if action.as_deref() == Some("--enable-kb") {
        return si_opencode_updater::provision::enable_kb();
    }
    if action.as_deref() == Some("--uninstall-managed") {
        return si_opencode_updater::provision::uninstall_managed();
    }
    if action.as_deref() == Some("--install-managed") {
        let app = env::args().nth(2).ok_or("Expected application directory")?;
        return si_opencode_updater::provision::install_managed(std::path::Path::new(&app));
    }
    if action.as_deref() == Some("--provision") {
        let path = env::args()
            .nth(2)
            .ok_or("Expected temporary installer input file")?;
        return si_opencode_updater::provision::install(std::path::Path::new(&path));
    }
    if !matches!(action.as_deref(), Some("--check" | "--update")) {
        return Err(
            "Usage: si_opencode_updater --check | --update | --provision <input-file>".into(),
        );
    }
    let base = std::path::PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode");
    let config: NasConfig = serde_json::from_slice(&fs::read(base.join("updater.json"))?)?;
    config.validate()?;
    let key = option_env!("SI_PUBLISHER_PUBLIC_KEY")
        .ok_or("Build with SI_PUBLISHER_PUBLIC_KEY set to the base64 Ed25519 public key")?;
    let key = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key)?;
    if key.len() != 32 {
        return Err("Invalid publisher public key".into());
    }

    let mut account = match credentials::load()? {
        Some(account) => account,
        None => credentials::prompt()?.ok_or("NAS sign-in cancelled")?,
    };
    let nas = loop {
        match Nas::login(&config, &account.0, &account.1) {
            Ok(nas) => break nas,
            Err(NasError::Authentication) => {
                account = credentials::prompt()?.ok_or("NAS sign-in cancelled")?;
            }
            Err(other) => return Err(other.into()),
        }
    };
    credentials::save(&account.0, &account.1)?;
    let session = (|| -> Result<_, Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let manifest = temp.path().join("manifest.json");
        let signature = temp.path().join("manifest.sig");
        nas.download(&config.manifest_path, &manifest, 16 * 1024 * 1024, None)?;
        let sig_path = config
            .manifest_path
            .strip_suffix(".json")
            .ok_or("Expected .json manifest")?
            .to_string()
            + ".sig";
        nas.download(&sig_path, &signature, 16 * 1024, None)?;
        let bytes = fs::read(&manifest)?;
        let sig = fs::read_to_string(&signature)?;
        let release = verify_manifest(&bytes, &sig, &key)?;
        let notes_entry = release.files.get("release-notes.json").cloned();
        let root = std::path::PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
        fs::create_dir_all(&root)?;
        let mut state = load_state(&root)?;
        if release.version < state.version {
            return Err("Refusing NAS release downgrade".into());
        }
        let fpga: bool = serde_json::from_slice(&fs::read(base.join("fpga.json"))?)?;
        let mut managed = release.clone();
        let desktop = managed.files.remove("desktop/opencode_desktop_win64.exe");
        managed.files.remove("release-notes.json");
        managed
            .files
            .retain(|name, _| !name.starts_with("release-notes/"));
        let changes = plan(&root, &state, &managed, fpga)?;
        let config_file = si_opencode_updater::provision::config_path(&root);
        let original_config = fs::read(&config_file)?;
        let mut desired_config = oc_config::parse_jsonc(std::str::from_utf8(&original_config)?)?;
        let before_config = desired_config.clone();
        let mut published = state.published_config.clone();
        let app_path = env::current_exe()?
            .parent()
            .ok_or("Updater location unknown")?
            .to_string_lossy()
            .into_owned();
        let managed_path = root.to_string_lossy().into_owned();
        let settings = managed
            .settings
            .iter()
            .map(|(key, value)| (key.clone(), expand_paths(value, &app_path, &managed_path)))
            .collect();
        let config_conflicts =
            oc_config::reconcile(&mut desired_config, &mut published, &settings)?;
        let config_change = desired_config != before_config;
        let desktop_change = desktop
            .as_ref()
            .is_some_and(|entry| state.desktop_sha256.as_deref() != Some(&entry.sha256));
        println!(
            "Installed release: {}; verified NAS release: {}; {} file changes; desktop change: {}; settings change: {}",
            state.version,
            release.version,
            changes.len(),
            desktop_change,
            config_change
        );
        if action.as_deref() == Some("--check") {
            return Ok(());
        }
        if changes.is_empty() && !desktop_change && !config_change {
            if release.version > state.version {
                state.version = release.version;
                si_opencode_updater::save_state(&root, &state)?;
            }
            return Ok(());
        }
        if !approve(&format!(
            "Update SI OpenCode to release {}? OpenCode Desktop must be closed before applying.",
            release.version
        )) {
            return Ok(());
        }
        let staging = tempfile::Builder::new()
            .prefix(".si-update-stage-")
            .tempdir_in(&root)?;
        for change in &changes {
            if let Action::Install(name) = change {
                let entry = &managed.files[name];
                let path = staging.path().join(name);
                fs::create_dir_all(path.parent().unwrap())?;
                let remote = format!(
                    "{}/{}",
                    config.manifest_path.rsplit_once('/').unwrap().0,
                    name
                );
                nas.download(&remote, &path, entry.bytes, Some(entry))?;
            }
        }
        if desktop_change {
            let entry = desktop.as_ref().unwrap();
            let path = staging.path().join("opencode_desktop_win64.exe");
            let remote = format!(
                "{}/desktop/opencode_desktop_win64.exe",
                config.manifest_path.rsplit_once('/').unwrap().0
            );
            nas.download(&remote, &path, entry.bytes, Some(entry))?;
            let app = env::current_exe()?
                .parent()
                .ok_or("Updater location unknown")?
                .to_path_buf();
            let signature = Command::new("powershell.exe")
                .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
                .arg(app.join("verify-desktop.ps1"))
                .arg("-Path")
                .arg(&path)
                .status()?;
            if !signature.success() {
                return Err(
                    "Downloaded Desktop executable failed publisher signature check".into(),
                );
            }
            close_desktop()?;
            let status = Command::new(&path).status()?;
            if !status.success() {
                return Err("OpenCode Desktop installer failed; old release state retained".into());
            }
            let installed_cli = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
                .join("Programs/@opencodedesktop/resources/opencode-cli.exe");
            if !installed_cli.is_file() {
                return Err("Desktop installer did not provide its bundled CLI".into());
            }
            let app = env::current_exe()?
                .parent()
                .ok_or("Updater location unknown")?
                .to_path_buf();
            fs::copy(&installed_cli, app.join("opencode.exe"))?;
            state.desktop_sha256 = Some(entry.sha256.clone());
        }
        let conflicts = apply(&root, staging.path(), &mut state, &managed, fpga)?;
        if config_change {
            fs::write(&config_file, serde_json::to_vec_pretty(&desired_config)?)?;
            state.published_config = published;
            if let Err(error) = si_opencode_updater::save_state(&root, &state) {
                fs::write(&config_file, original_config)?;
                return Err(error.into());
            }
        }
        if !conflicts.is_empty() {
            eprintln!(
                "User-modified entries left untouched: {}",
                conflicts.join(", ")
            );
        }
        if !config_conflicts.is_empty() {
            eprintln!(
                "User-edited MCP settings kept: {}",
                config_conflicts.join(", ")
            );
        }
        if let Some(ref entry) = notes_entry {
            let notes_root = base.join("release-notes").join(release.version.to_string());
            fs::create_dir_all(&notes_root)?;
            let notes_path = notes_root.join("release-notes.json");
            let remote = format!(
                "{}/release-notes.json",
                config.manifest_path.rsplit_once('/').unwrap().0
            );
            nas.download(&remote, &notes_path, entry.bytes, Some(entry))?;
            let notes = si_opencode_updater::release_notes::ReleaseNotes::parse(
                &fs::read(&notes_path)?,
                release.version,
            )?;
            let mut total = 0u64;
            for item in notes.items {
                if let Some(image) = item.image {
                    let image_entry = release
                        .files
                        .get(&image)
                        .ok_or("Release note image missing from signed manifest")?;
                    total += image_entry.bytes;
                    if image_entry.bytes > 5 * 1024 * 1024 || total > 20 * 1024 * 1024 {
                        return Err("Release note image size limit exceeded".into());
                    }
                    let target = notes_root.join(image.trim_start_matches("release-notes/"));
                    fs::create_dir_all(target.parent().ok_or("Invalid release note image path")?)?;
                    let remote = format!(
                        "{}/{}",
                        config.manifest_path.rsplit_once('/').unwrap().0,
                        image
                    );
                    nas.download(&remote, &target, image_entry.bytes, Some(image_entry))?;
                }
            }
        }
        let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
            .join("Programs/@opencodedesktop/OpenCode.exe");
        let mut restarted = Command::new(desktop).spawn()?;
        std::thread::sleep(std::time::Duration::from_secs(2));
        if restarted.try_wait()?.is_some() {
            return Err("OpenCode Desktop exited during restart".into());
        }
        if notes_entry.is_some() {
            Command::new(env::current_exe()?)
                .args(["--show-whats-new", &release.version.to_string()])
                .spawn()?;
        }
        println!("Applied release {}", state.version);
        Ok(())
    })();
    let _ = nas.logout();
    session
}

#[cfg(windows)]
fn show_whats_new(version: u64) -> Result<(), Box<dyn std::error::Error>> {
    use si_opencode_updater::{load_state, release_notes::ReleaseNotes, save_state};
    use std::{env, fs};

    let base = std::path::PathBuf::from(env::var("LOCALAPPDATA")?).join("SI OpenCode");
    let mut notes: ReleaseNotes = ReleaseNotes::parse(
        &fs::read(
            base.join("release-notes")
                .join(version.to_string())
                .join("release-notes.json"),
        )?,
        version,
    )?;
    for item in &mut notes.items {
        if let Some(image) = &item.image {
            let bytes = fs::read(
                base.join("release-notes")
                    .join(version.to_string())
                    .join(image.trim_start_matches("release-notes/")),
            )?;
            let mime = if image.ends_with(".png") {
                "image/png"
            } else if image.ends_with(".webp") {
                "image/webp"
            } else {
                "image/jpeg"
            };
            item.image = Some(format!(
                "data:{mime};base64,{}",
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)
            ));
        }
    }
    let root = std::path::PathBuf::from(env::var("USERPROFILE")?).join(".config/opencode");
    let mut state = load_state(&root)?;
    if state.shown_release_notes == Some(version) {
        return Ok(());
    }
    tauri::Builder::default()
        .manage(notes)
        .invoke_handler(tauri::generate_handler![release_notes])
        .setup(|app| {
            tauri::WebviewWindowBuilder::new(
                app,
                "release-notes",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("SI OpenCode 업데이트")
            .inner_size(820.0, 680.0)
            .resizable(true)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())?;
    state.shown_release_notes = Some(version);
    save_state(&root, &state)?;
    Ok(())
}

#[cfg(windows)]
#[tauri::command]
fn release_notes(
    notes: tauri::State<'_, si_opencode_updater::release_notes::ReleaseNotes>,
) -> si_opencode_updater::release_notes::ReleaseNotes {
    notes.inner().clone()
}

#[cfg(windows)]
fn approve(message: &str) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, IDYES, MB_ICONQUESTION, MB_YESNO,
    };
    let text: Vec<_> = message.encode_utf16().chain([0]).collect();
    let caption: Vec<_> = "SI OpenCode Update".encode_utf16().chain([0]).collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_ICONQUESTION | MB_YESNO,
        ) == IDYES
    }
}

#[cfg(windows)]
fn expand_paths(value: &serde_json::Value, app: &str, managed: &str) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => {
            serde_json::Value::String(text.replace("{app}", app).replace("{managed}", managed))
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .iter()
                .map(|item| expand_paths(item, app, managed))
                .collect(),
        ),
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, item)| (key.clone(), expand_paths(item, app, managed)))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(windows)]
fn desktop_running() -> Result<bool, Box<dyn std::error::Error>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut process = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = false;
    let mut ok = unsafe { Process32FirstW(snapshot, &mut process) };
    while ok != 0 {
        let len = process
            .szExeFile
            .iter()
            .position(|&v| v == 0)
            .unwrap_or(process.szExeFile.len());
        if String::from_utf16_lossy(&process.szExeFile[..len]).eq_ignore_ascii_case("OpenCode.exe")
        {
            found = true;
            break;
        }
        ok = unsafe { Process32NextW(snapshot, &mut process) };
    }
    unsafe {
        CloseHandle(snapshot);
    }
    Ok(found)
}

#[cfg(windows)]
fn close_desktop() -> Result<(), Box<dyn std::error::Error>> {
    if !desktop_running()? {
        return Ok(());
    }
    let status = std::process::Command::new("taskkill.exe")
        .args(["/IM", "OpenCode.exe", "/F"])
        .status()?;
    if !status.success() {
        return Err("Could not close OpenCode Desktop for the approved update".into());
    }
    for _ in 0..30 {
        if !desktop_running()? {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Err("OpenCode Desktop did not exit in time".into())
}
