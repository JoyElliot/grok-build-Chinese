//! A declarative protocol carried in BUILD-INFO.txt, which legacy packages
//! already hash. SHA256SUMS.txt remains the standard, extensible file manifest.

use super::*;

pub(super) const BUILD_INFO: &str = "BUILD-INFO.txt";
const BEGIN: &str = "GROK-UPDATE-PROTOCOL-BEGIN";
const END: &str = "GROK-UPDATE-PROTOCOL-END";

#[derive(Debug, Deserialize)]
pub(super) struct PackageProtocol {
    schema: u32,
    version: String,
    platform: String,
    mode: String,
    manifest: String,
    pub executable: String,
    pub installer: String,
}

impl PackageProtocol {
    pub fn parse(bytes: &[u8], version: &str, platform: &str) -> Result<Option<Self>> {
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            anyhow::bail!("community BUILD-INFO.txt exceeds the size limit");
        }
        let text = std::str::from_utf8(bytes).context("community BUILD-INFO.txt is not UTF-8")?;
        if !text.contains(BEGIN) && !text.contains(END) {
            return Ok(None);
        }
        let lines: Vec<_> = text.lines().collect();
        let starts: Vec<_> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| **line == BEGIN)
            .collect();
        let ends: Vec<_> = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| **line == END)
            .collect();
        if starts.len() != 1 || ends.len() != 1 || starts[0].0 >= ends[0].0 {
            anyhow::bail!("community package protocol block is malformed");
        }
        let json = lines[starts[0].0 + 1..ends[0].0].join("\n");
        let protocol: Self =
            serde_json::from_str(&json).context("parsing community package protocol")?;
        if protocol.schema != 1
            || protocol.mode != "executable-only"
            || protocol.manifest != INNER_MANIFEST
        {
            anyhow::bail!(
                "unsupported community package protocol; update the installer or client first"
            );
        }
        if protocol.version != version || protocol.platform != platform {
            anyhow::bail!(
                "community package protocol version or platform does not match its release"
            );
        }
        if !safe_path(&protocol.executable)
            || !safe_path(&protocol.installer)
            || [&protocol.executable, &protocol.installer]
                .iter()
                .any(|path| {
                    path.eq_ignore_ascii_case(BUILD_INFO)
                        || path.eq_ignore_ascii_case(INNER_MANIFEST)
                })
            || protocol.executable.to_lowercase() == protocol.installer.to_lowercase()
        {
            anyhow::bail!("community package protocol has invalid or overlapping entry points");
        }
        Ok(Some(protocol))
    }

    pub fn check_manifest(&self, hashes: &HashMap<String, String>) -> Result<()> {
        if !hashes.contains_key(&self.executable)
            || !hashes.contains_key(&self.installer)
            || !hashes.contains_key(BUILD_INFO)
        {
            anyhow::bail!(
                "community package manifest must cover its executable and BUILD-INFO.txt"
            );
        }
        Ok(())
    }
}

pub(super) fn safe_path(name: &str) -> bool {
    is_safe_package_relative_path(name)
        && !name
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '<' | '>' | '"' | '|' | '?' | '*'))
        && name.split('/').all(|part| {
            if part.ends_with(['.', ' ']) {
                return false;
            }
            let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            let reserved_port = stem
                .strip_prefix("COM")
                .or_else(|| stem.strip_prefix("LPT"))
                .is_some_and(|suffix| {
                    let mut chars = suffix.chars();
                    matches!(
                        chars.next(),
                        Some('1'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}')
                    ) && chars.next().is_none()
                });
            !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") && !reserved_port
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_rejects_superscript_windows_device_names() {
        for prefix in ["COM", "LPT", "com", "lpt"] {
            for digit in ['\u{b9}', '\u{b2}', '\u{b3}'] {
                for path in [
                    format!("{prefix}{digit}"),
                    format!("docs/{prefix}{digit}.txt"),
                    format!("{prefix}{digit}/readme.md"),
                ] {
                    assert!(!safe_path(&path), "accepted device path: {path}");
                    let manifest = format!("{}  {path}\n", "a".repeat(64));
                    assert!(parse_manifest(manifest.as_bytes()).is_err());
                }
            }
        }
        for path in [
            "docs/中文说明.txt",
            "COM10.txt",
            "LPT10/readme.md",
            "COM¹notes.txt",
        ] {
            assert!(safe_path(path), "rejected ordinary path: {path}");
        }
    }
}

pub(super) fn parse_manifest(bytes: &[u8]) -> Result<HashMap<String, String>> {
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        anyhow::bail!("community package manifest exceeds the size limit");
    }
    let text = std::str::from_utf8(bytes).context("community package manifest is not UTF-8")?;
    let mut hashes = HashMap::new();
    let mut names = HashSet::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let (digest, path) = line
            .split_once("  ")
            .context("invalid community package manifest line")?;
        if digest.len() != 64
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !safe_path(path)
            || path.eq_ignore_ascii_case(INNER_MANIFEST)
            || !names.insert(path.to_lowercase())
        {
            anyhow::bail!("community package manifest contains an invalid or duplicate entry");
        }
        hashes.insert(path.to_string(), digest.to_ascii_lowercase());
    }
    if hashes.is_empty() || hashes.len() >= MAX_ARCHIVE_ENTRIES {
        anyhow::bail!("community package manifest has an invalid file count");
    }
    // A file cannot also be the parent directory of another file.
    for name in &names {
        for (index, _) in name.match_indices('/') {
            if names.contains(&name[..index]) {
                anyhow::bail!("community package manifest has a file/directory collision");
            }
        }
    }
    Ok(hashes)
}

pub(super) fn directories(files: &[&str]) -> HashSet<String> {
    files
        .iter()
        .flat_map(|name| {
            name.match_indices('/')
                .map(|(index, _)| name[..index].to_lowercase())
        })
        .collect()
}

pub(super) fn read_zip_metadata(
    archive: &mut zip::ZipArchive<File>,
    root: Option<&str>,
    name: &str,
) -> Result<Vec<u8>> {
    let path = root
        .map(|root| format!("{root}/{name}"))
        .unwrap_or_else(|| name.to_string());
    let entry = archive
        .by_name(&path)
        .with_context(|| format!("community package is missing {name}"))?;
    if !entry.is_file() || entry.is_symlink() || entry.size() > MAX_MANIFEST_BYTES {
        anyhow::bail!("community package metadata is not a bounded regular file");
    }
    let size = entry.size();
    let mut bytes = Vec::new();
    entry.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 != size {
        anyhow::bail!("community package metadata was truncated");
    }
    Ok(bytes)
}

// Read just the two bounded metadata files before the validating extraction
// pass. No package code runs, and no unverified paths are written to disk.
pub(super) fn read_tar_metadata(path: &Path, root: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let decoder = flate2::read::GzDecoder::new(File::open(path)?);
    let mut archive = tar::Archive::new(decoder);
    let mut build_info = None;
    let mut manifest = None;
    let mut total = 0u64;
    for (index, entry) in archive.entries()?.raw(true).enumerate() {
        if index >= MAX_ARCHIVE_ENTRIES {
            anyhow::bail!("community tar has too many entries");
        }
        let entry = entry?;
        total = total
            .checked_add(entry.size())
            .context("community tar size overflow")?;
        if total > MAX_UNCOMPRESSED_BYTES {
            anyhow::bail!("community tar exceeds size limit");
        }
        let raw = entry.header().path_bytes();
        let name = std::str::from_utf8(raw.as_ref()).context("community tar path is not UTF-8")?;
        let name = name.strip_prefix("./").unwrap_or(name);
        let target = if name == format!("{root}/{BUILD_INFO}") {
            &mut build_info
        } else if name == format!("{root}/{INNER_MANIFEST}") {
            &mut manifest
        } else {
            continue;
        };
        if target.is_some()
            || !entry.header().entry_type().is_file()
            || entry.link_name_bytes().is_some()
            || entry.size() > MAX_MANIFEST_BYTES
        {
            anyhow::bail!("community tar contains invalid or duplicate metadata");
        }
        let size = entry.size();
        let mut bytes = Vec::new();
        entry.take(MAX_MANIFEST_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size {
            anyhow::bail!("community tar metadata was truncated");
        }
        *target = Some(bytes);
    }
    Ok((
        build_info.context("community tar is missing BUILD-INFO.txt")?,
        manifest.context("community tar is missing SHA256SUMS.txt")?,
    ))
}
