fn main() {
    for key in ["MEGALODON_UPDATE_REPO", "MEGALODON_BUILD_COMMIT"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    // Project metadata keys: environment first, then the git-ignored
    // .local/providers.env (KEY=value lines). Absent keys simply disable that
    // source; the build never fails and nothing is written to the repository.
    println!("cargo:rerun-if-changed=.local/providers.env");
    let file = std::fs::read_to_string(".local/providers.env").unwrap_or_default();
    for key in ["MEGALODON_TMDB_TOKEN", "MEGALODON_FANART_KEY"] {
        println!("cargo:rerun-if-env-changed={key}");
        let value = std::env::var(key)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                file.lines()
                    .filter_map(|line| line.split_once('='))
                    .find(|(k, _)| k.trim() == key)
                    .map(|(_, v)| v.trim().trim_matches('"').to_owned())
            });
        if let Some(value) = value.filter(|v| !v.is_empty() && !v.contains(['\r', '\n'])) {
            println!("cargo:rustc-env={key}={value}");
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    println!("cargo:rustc-env=MEGALODON_BUILD_TIMESTAMP={stamp}");
    println!("cargo:rerun-if-changed=static/windows.rc");
    println!("cargo:rerun-if-changed=static/megalodon.ico");
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_none()
        || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
    {
        return;
    }
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("megalodon.res");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = format!("{},0", version.split('-').next().unwrap().replace('.', ","));
    let template = std::fs::read_to_string("static/windows.rc").unwrap();
    let resource = output.with_extension("rc");
    std::fs::write(
        &resource,
        template
            .replace("MEGALODON_VERSION_NUM", &numbers)
            .replace("MEGALODON_VERSION_TEXT", &version),
    )
    .unwrap();
    let gnu = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu");
    let mut command = std::process::Command::new(if gnu { "windres" } else { "rc" });
    if gnu {
        command
            .arg("-i")
            .arg(&resource)
            .args(["-O", "res", "-o"])
            .arg(&output);
    } else {
        command
            .arg("/nologo")
            .arg(format!("/fo{}", output.display()))
            .arg(&resource);
    }
    assert!(
        command
            .status()
            .expect("Windows resource compiler (windres or rc) is required")
            .success(),
        "Windows resource compilation failed"
    );
    for binary in ["megalodon-desktop", "megalodon-setup"] {
        println!("cargo:rustc-link-arg-bin={binary}={}", output.display());
    }
}
