//! Strict, bounded access to the one MSVC ZIP appended to a GNU PE launcher.
//! No executable scanning, generic self-extracting ZIP support, or package code
//! execution is involved in deciding which bytes are eligible for activation.

use std::io::{self, SeekFrom};

use super::*;
use package_protocol::EmbeddedRuntime;

const MAGIC: &[u8; 16] = b"GROKMSVCBUNDLE01";
const FOOTER_SIZE: u64 = 64;
const MIN_PE_PREFIX_SIZE: u64 = 256;

pub(super) fn extract_msvc_executable(
    version: &str,
    outer: &mut zip::ZipArchive<File>,
    destination: &Path,
) -> Result<()> {
    let launcher_path = destination.with_extension("embedded-launcher.tmp");
    if std::fs::symlink_metadata(&launcher_path).is_ok() {
        anyhow::bail!("refusing to overwrite an existing embedded launcher temporary file");
    }
    let result = (|| {
        let descriptor = extract_windows_package(
            version,
            CommunityPlatform::WindowsX86_64Gnu,
            outer,
            &launcher_path,
            true,
        )?
        .context("GNU package does not declare an embedded MSVC runtime")?;
        let inner = open_verified(&launcher_path, &descriptor, version)?;
        let mut archive =
            zip::ZipArchive::new(inner).context("opening the authenticated embedded MSVC ZIP")?;
        extract_windows_package(
            version,
            CommunityPlatform::WindowsX86_64Msvc,
            &mut archive,
            destination,
            false,
        )?;
        let mut executable = File::open(destination)?;
        let executable_size = executable.metadata()?.len();
        validate_x64_pe(&mut executable, executable_size)
            .context("embedded runtime executable is not a Windows x86_64 PE32+ image")?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&launcher_path);
    result
}

/// A seekable view that cannot read the PE prefix or EOF footer. ZIP offsets
/// are relative to the full native ZIP, exactly as they were before embedding.
struct ArchiveSlice {
    file: File,
    offset: u64,
    size: u64,
    position: u64,
}

impl Read for ArchiveSlice {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let remaining = self.size.saturating_sub(self.position);
        let count = remaining.min(buffer.len() as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        self.file
            .seek(SeekFrom::Start(self.offset + self.position))?;
        let read = self.file.read(&mut buffer[..count])?;
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for ArchiveSlice {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::Current(delta) => i128::from(self.position) + i128::from(delta),
            SeekFrom::End(delta) => i128::from(self.size) + i128::from(delta),
        };
        if position < 0 || position > i128::from(self.size) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "embedded ZIP seek is outside its authenticated range",
            ));
        }
        self.position = position as u64;
        Ok(self.position)
    }
}

fn open_verified(path: &Path, descriptor: &EmbeddedRuntime, version: &str) -> Result<ArchiveSlice> {
    descriptor.validate(version)?;
    let mut file = File::open(path).context("opening the validated GNU migration launcher")?;
    let file_size = file.metadata()?.len();
    if file_size < FOOTER_SIZE + MIN_PE_PREFIX_SIZE || file_size > MAX_ENTRY_BYTES {
        anyhow::bail!("embedded MSVC launcher size is outside the accepted range");
    }
    file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
    let mut footer = [0u8; FOOTER_SIZE as usize];
    file.read_exact(&mut footer)?;
    if &footer[..16] != MAGIC {
        anyhow::bail!("GNU launcher lacks the exact embedded MSVC footer");
    }
    let offset = u64::from_le_bytes(footer[16..24].try_into().unwrap());
    let size = u64::from_le_bytes(footer[24..32].try_into().unwrap());
    if offset < MIN_PE_PREFIX_SIZE
        || size == 0
        || size > MAX_ASSET_BYTES
        || size != descriptor.archive_size
        || offset
            .checked_add(size)
            .and_then(|end| end.checked_add(FOOTER_SIZE))
            != Some(file_size)
    {
        anyhow::bail!("embedded MSVC archive range does not match its footer and descriptor");
    }
    let footer_digest: String = footer[32..]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if !footer_digest.eq_ignore_ascii_case(&descriptor.archive_sha256) {
        anyhow::bail!("embedded MSVC footer digest does not match its package descriptor");
    }
    // Check the advertised prefix directly. Never search for MZ or PK inside
    // unrelated executable bytes or let the ZIP reader locate a second ZIP.
    validate_x64_pe(&mut file, offset).context("invalid GNU migration PE prefix")?;
    file.seek(SeekFrom::Start(offset))?;
    let mut signature = [0u8; 4];
    file.read_exact(&mut signature)?;
    if signature != *b"PK\x03\x04" {
        anyhow::bail!("embedded MSVC range does not start with the native ZIP");
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut remaining = size;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    while remaining > 0 {
        let count = remaining.min(buffer.len() as u64) as usize;
        file.read_exact(&mut buffer[..count])?;
        hasher.update(&buffer[..count]);
        remaining -= count as u64;
    }
    if hasher.finalize().as_slice() != &footer[32..] {
        anyhow::bail!("embedded MSVC archive SHA-256 mismatch");
    }
    Ok(ArchiveSlice {
        file,
        offset,
        size,
        position: 0,
    })
}

/// Validate the PE identity and complete header/table bounds without treating
/// payload bytes as part of a truncated image. The actual compiler ABI remains
/// authenticated by the package protocol, since GNU and MSVC share PE32+.
/// Direct transport must not relabel a known embedded launcher as its runtime.
pub(super) fn validate_direct_executable<R: Read + Seek>(reader: &mut R, size: u64) -> Result<()> {
    validate_x64_pe(reader, size)?;
    if size >= FOOTER_SIZE {
        reader.seek(SeekFrom::Start(size - FOOTER_SIZE))?;
        let mut magic = [0u8; 16];
        reader.read_exact(&mut magic)?;
        if &magic == MAGIC {
            anyhow::bail!("direct MSVC runtime must not be an embedded launcher");
        }
    }
    Ok(())
}

pub(super) fn validate_x64_pe<R: Read + Seek>(reader: &mut R, image_size: u64) -> Result<()> {
    if image_size < MIN_PE_PREFIX_SIZE {
        anyhow::bail!("Windows PE prefix is shorter than 256 bytes");
    }
    reader.seek(SeekFrom::Start(0))?;
    let mut dos = [0u8; 64];
    reader.read_exact(&mut dos)?;
    let pe_offset = u64::from(u32::from_le_bytes(dos[60..64].try_into().unwrap()));
    if &dos[..2] != b"MZ" || pe_offset < 64 || pe_offset + 26 > image_size {
        anyhow::bail!("Windows PE header is outside its image prefix");
    }
    reader.seek(SeekFrom::Start(pe_offset))?;
    let mut pe = [0u8; 26];
    reader.read_exact(&mut pe)?;
    let optional_size = u64::from(u16::from_le_bytes([pe[20], pe[21]]));
    let sections = u64::from(u16::from_le_bytes([pe[6], pe[7]]));
    if &pe[..4] != b"PE\0\0"
        || u16::from_le_bytes([pe[4], pe[5]]) != 0x8664
        || optional_size < 112
        || sections == 0
        || pe_offset + 24 + optional_size + sections * 40 > image_size
        || u16::from_le_bytes([pe[24], pe[25]]) != 0x20b
    {
        anyhow::bail!("Windows PE32+ header or section table is invalid");
    }
    Ok(())
}
