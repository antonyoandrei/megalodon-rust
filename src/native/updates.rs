//! Release metadata is embedded by CI. No hard-coded owner or unrelated update feed.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone)]
pub struct Release {
    pub name: String,
    pub version: String,
    pub url: String,
    pub checksum_url: String,
    pub filename: String,
    pub tag: String,
    pub repository: String,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    name: Option<String>,
    draft: bool,
    published_at: Option<String>,
    assets: Vec<Asset>,
}
pub fn repository(input: &str) -> Result<String, String> {
    let input = input.trim();
    let pieces: Vec<_> = input.split('/').collect();
    if pieces.len() != 2
        || pieces.iter().any(|s| {
            s.is_empty()
                || *s == "."
                || *s == ".."
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
    {
        return Err("Indica un repositorio como usuario/megalodon.".into());
    }
    Ok(input.into())
}
fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(concat!("Megalodon/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_download(attempt.url()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "No se pudo preparar la actualización.".into())
}
fn trusted_download(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "api.github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}
fn asset_url(url: &str, repo: &str, tag: &str, name: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("github.com")
            && u.path() == format!("/{repo}/releases/download/{tag}/{name}")
            && u.query().is_none()
            && u.fragment().is_none()
    })
}
fn version(v: &str) -> Option<(u64, u64, u64)> {
    let mut p = v.trim_start_matches('v').split('.');
    let version = (
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
    );
    p.next().is_none().then_some(version)
}
pub fn check(repo: &str) -> Result<Option<Release>, String> {
    let repo = repository(repo)?;
    let response = client()?
        .get(format!(
            "https://api.github.com/repos/{repo}/releases?per_page=20"
        ))
        .timeout(Duration::from_secs(15))
        .send()
        .map_err(|_| "No se pudo comprobar si hay actualizaciones.")?;
    let response = response
        .error_for_status()
        .map_err(|_| "No se encuentran versiones públicas en este repositorio.")?;
    let mut bytes = vec![];
    response
        .take(2_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "No se pudieron leer las versiones.")?;
    if bytes.len() > 2_000_000 {
        return Err("La respuesta de versiones es demasiado grande.".into());
    }
    let releases: Vec<GithubRelease> =
        serde_json::from_slice(&bytes).map_err(|_| "Respuesta de versiones no válida.")?;
    let current = version(env!("CARGO_PKG_VERSION")).unwrap();
    let build = env!("MEGALODON_BUILD_TIMESTAMP")
        .parse::<i64>()
        .unwrap_or(i64::MAX);
    for release in releases.into_iter().filter(|r| !r.draft) {
        let Some(installer) = release.assets.iter().find(|a| {
            a.name.starts_with("Megalodon-") && a.name.ends_with("-windows-x64-setup.exe")
        }) else {
            continue;
        };
        let Some(checksum) = release.assets.iter().find(|a| a.name == "SHA256SUMS.txt") else {
            continue;
        };
        let ver = installer
            .name
            .trim_start_matches("Megalodon-")
            .trim_end_matches("-windows-x64-setup.exe");
        let Some(next) = version(ver) else { continue };
        let published = release
            .published_at
            .as_deref()
            .and_then(|date| {
                time::OffsetDateTime::parse(date, &time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .map_or(0, |d| d.unix_timestamp());
        let same_commit = option_env!("MEGALODON_BUILD_COMMIT")
            .is_some_and(|commit| release.tag_name == format!("build-{commit}"));
        if next < current || (next == current && (published <= build || same_commit)) {
            continue;
        }
        if !asset_url(
            &installer.browser_download_url,
            &repo,
            &release.tag_name,
            &installer.name,
        ) || !asset_url(
            &checksum.browser_download_url,
            &repo,
            &release.tag_name,
            &checksum.name,
        ) {
            continue;
        }
        return Ok(Some(Release {
            name: release.name.unwrap_or_else(|| format!("Megalodon {ver}")),
            version: ver.into(),
            url: installer.browser_download_url.clone(),
            checksum_url: checksum.browser_download_url.clone(),
            filename: installer.name.clone(),
            tag: release.tag_name,
            repository: repo,
        }));
    }
    Ok(None)
}
pub fn download(
    release: &Release,
    dir: &Path,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf, String> {
    let repo = repository(&release.repository)?;
    if !asset_url(&release.url, &repo, &release.tag, &release.filename)
        || !asset_url(&release.checksum_url, &repo, &release.tag, "SHA256SUMS.txt")
    {
        return Err("La actualización no pertenece al repositorio configurado.".into());
    }
    std::fs::create_dir_all(dir).map_err(|_| "No se puede guardar la actualización.")?;
    let client = client()?;
    let response = client
        .get(&release.checksum_url)
        .send()
        .map_err(|_| "No se pudo descargar la comprobación del paquete.")?
        .error_for_status()
        .map_err(|_| "No se encuentra la comprobación del paquete.")?;
    let mut checksums = String::new();
    response
        .take(32_769)
        .read_to_string(&mut checksums)
        .map_err(|_| "Comprobación no válida.")?;
    let expected = checksum_for(&checksums, &release.filename)
        .ok_or("Falta la suma SHA-256 de este instalador.")?;
    let target = dir.join(&release.filename);
    let temporary = target.with_extension("download");
    let result = (|| {
        let mut response = client
            .get(&release.url)
            .send()
            .map_err(|_| "No se pudo descargar la actualización.")?
            .error_for_status()
            .map_err(|_| "El paquete no está disponible.")?;
        let total = response.content_length().unwrap_or(0);
        if total > 600_000_000 {
            return Err("El paquete supera el tamaño permitido.".into());
        }
        let mut file = std::fs::File::create(&temporary)
            .map_err(|_| "No se pudo guardar la actualización.")?;
        copy_verified(&mut response, &mut file, &expected, total, &mut progress)?;
        file.sync_all()
            .map_err(|_| "No se pudo guardar la actualización.")?;
        drop(file);
        std::fs::rename(&temporary, &target).map_err(|_| "No se pudo preparar el instalador.")?;
        Ok(target.clone())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}
fn copy_verified(
    reader: &mut impl Read,
    writer: &mut impl Write,
    expected: &str,
    total: u64,
    progress: &mut impl FnMut(u64, u64),
) -> Result<(), String> {
    let mut hash = Sha256::new();
    let mut read = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| "La descarga se interrumpió. Puedes volver a intentarlo.")?;
        if count == 0 {
            break;
        }
        read += count as u64;
        if read > 600_000_000 {
            return Err("Paquete demasiado grande.".into());
        }
        hash.update(&buffer[..count]);
        writer
            .write_all(&buffer[..count])
            .map_err(|_| "No se pudo guardar la actualización.")?;
        progress(read, total);
    }
    if (total > 0 && read != total) || format!("{:x}", hash.finalize()) != expected {
        return Err(
            "El paquete descargado no supera la comprobación de integridad. Vuelve a descargarlo."
                .into(),
        );
    }
    Ok(())
}
fn checksum_for(text: &str, filename: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let file = parts.next()?.trim_start_matches('*');
        (file == filename && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}
#[derive(Default)]
pub struct State {
    pub checking: bool,
    pub checked: bool,
    pub release: Option<Release>,
    pub downloading: bool,
    pub progress: (u64, u64),
    pub ready: Option<PathBuf>,
    pub error: Option<String>,
    pub dismissed: bool,
    pub request: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_rejects_truncation_tampering_and_failed_writes() {
        let package = b"test-package-for-integrity";
        let expected = format!("{:x}", Sha256::digest(package));
        assert!(
            copy_verified(
                &mut &package[..],
                &mut vec![],
                &expected,
                package.len() as u64,
                &mut |_, _| {}
            )
            .is_ok()
        );
        assert!(
            copy_verified(
                &mut &package[..10],
                &mut vec![],
                &expected,
                package.len() as u64,
                &mut |_, _| {}
            )
            .is_err()
        );
        assert!(
            copy_verified(
                &mut &b"tampered-package"[..],
                &mut vec![],
                &expected,
                0,
                &mut |_, _| {}
            )
            .is_err()
        );
        assert!(
            copy_verified(
                &mut &package[..],
                &mut [0u8; 2].as_mut_slice(),
                &expected,
                0,
                &mut |_, _| {}
            )
            .is_err()
        );
    }
    #[test]
    fn updater_rejects_other_hosts_and_path_injection() {
        assert!(repository("owner/repo").is_ok());
        assert!(repository("../x").is_err());
        assert!(repository("https://host/repo").is_err());
        assert!(asset_url(
            "https://github.com/owner/repo/releases/download/v1/Megalodon.exe",
            "owner/repo",
            "v1",
            "Megalodon.exe"
        ));
        assert!(!asset_url(
            "https://github.com.evil/owner/repo/releases/download/v1/Megalodon.exe",
            "owner/repo",
            "v1",
            "Megalodon.exe"
        ));
        assert!(!trusted_download(
            &reqwest::Url::parse("http://github.com/x").unwrap()
        ));
        assert!(
            checksum_for(
                &format!("{}  different.exe", "a".repeat(64)),
                "Megalodon.exe"
            )
            .is_none()
        );
        assert_eq!(
            checksum_for(
                &format!("{}  Megalodon.exe", "A".repeat(64)),
                "Megalodon.exe"
            ),
            Some("a".repeat(64))
        );
        assert!(version("0.10.0") > version("0.3.9"));
    }
}
