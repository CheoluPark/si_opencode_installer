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
        Some("--launch") => {
            let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
                .join("Programs/@opencodedesktop/OpenCode.exe");
            if !desktop.is_file() {
                return Err("OpenCode Desktop executable not found".into());
            }
            Command::new(desktop).spawn()?;
            Ok(())
        }
        _ => Err("Usage: si_opencode_updater --provision <input-file> | --sync-extensions | --fetch-current-release <staging-dir> | --launch".into()),
    }
}
