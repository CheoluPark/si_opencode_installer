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
            si_opencode_updater::provision::install(input.as_ref())
        }
        Some("--install-managed") => {
            let app = args.next().ok_or("Expected application directory")?;
            si_opencode_updater::provision::install_managed(app.as_ref())
        }
        Some("--uninstall-managed") => si_opencode_updater::provision::uninstall_managed(),
        Some("--launch") => {
            let desktop = std::path::PathBuf::from(env::var("LOCALAPPDATA")?)
                .join("Programs/@opencodedesktop/OpenCode.exe");
            if !desktop.is_file() {
                return Err("OpenCode Desktop executable not found".into());
            }
            Command::new(desktop).spawn()?;
            Ok(())
        }
        _ => Err("Usage: si_opencode_updater --provision <input-file> | --install-managed <app-dir> | --uninstall-managed | --launch".into()),
    }
}
