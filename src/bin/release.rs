use base64::{engine::general_purpose::STANDARD, Engine};
use ring::{rand::SystemRandom, signature::{Ed25519KeyPair, KeyPair}};
use si_opencode_updater::{hash_file, safe_name, Artifact, Release};
use std::{collections::BTreeMap, env, fs, io, path::Path};

fn main() {
    if let Err(error) = run() {
        eprintln!("release: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    match args.as_slice() {
        [_, command, private, public] if command == "keygen" => {
            if Path::new(private).exists() || Path::new(public).exists() { return Err("Key already exists".into()); }
            let key = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).map_err(|_| "Cannot generate signing key")?;
            let pair = Ed25519KeyPair::from_pkcs8(key.as_ref()).map_err(|_| "Cannot parse signing key")?;
            fs::write(private, key.as_ref())?;
            fs::write(public, pair.public_key().as_ref())?;
        }
        [_, command, private, folder, version, manifest, signature] if command == "sign" => {
            let key = fs::read(private)?;
            let pair = Ed25519KeyPair::from_pkcs8(&key).map_err(|_| "Invalid private key")?;
            let mut files = BTreeMap::new();
            collect(Path::new(folder), Path::new(folder), &mut files)?;
            if files.is_empty() { return Err("Empty direct-copy payload".into()); }
            let bytes = serde_json::to_vec_pretty(&Release { schema: 1, version: version.parse()?, files })?;
            fs::write(manifest, &bytes)?;
            fs::write(signature, STANDARD.encode(pair.sign(&bytes).as_ref()))?;
        }
        [_, command, public, manifest, signature] if command == "verify" => {
            si_opencode_updater::verify_manifest(&fs::read(manifest)?, &fs::read_to_string(signature)?, &fs::read(public)?)?;
        }
        _ => return Err("Usage: release keygen <private.pk8> <public.bin> | release sign <private.pk8> <direct-payload-dir> <version> <manifest.json> <manifest.sig> | release verify <public.bin> <manifest.json> <manifest.sig>".into()),
    }
    Ok(())
}

fn collect(root: &Path, at: &Path, files: &mut BTreeMap<String, Artifact>) -> io::Result<()> {
    for item in fs::read_dir(at)? {
        let item = item?;
        let kind = item.file_type()?;
        if kind.is_symlink() { return Err(io::Error::other("Symlinks are not allowed in direct payload")); }
        if kind.is_dir() { collect(root, &item.path(), files)?; continue; }
        if !kind.is_file() { return Err(io::Error::other("Unsupported direct payload item")); }
        let name = item.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
        safe_name(&name).map_err(io::Error::other)?;
        files.insert(name, Artifact { sha256: hash_file(&item.path())?, bytes: item.metadata()?.len() });
    }
    Ok(())
}
