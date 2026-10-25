"""Offline, bounded Windows MSVC bundle format shared by release tooling.

A GNU PE is followed by one complete native ZIP and a 64-byte little-endian
footer. The outer authenticated package and the compiled launcher pin the ZIP's
size and SHA-256; the footer is a locator, never an independent trust root.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import tempfile
import zipfile

MAGIC = b"GROKMSVCBUNDLE01"
FOOTER = struct.Struct("<16sQQ32s")
MAX_ARCHIVE_BYTES = 512 * 1024 * 1024
MAX_EXPANDED_BYTES = 768 * 1024 * 1024
MAX_ENTRIES = 128
TARGET = "x86_64-pc-windows-msvc"
FORMAT = "grok-msvc-bundle-v1"
MIGRATION_ID = "windows-x64-gnu-to-msvc-embedded-v1"
VERSION_RE = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
CHUNK = 1024 * 1024


def canonical_version(version):
    if not isinstance(version, str) or len(version) > 64 or not re.fullmatch(VERSION_RE, version):
        raise ValueError("embedded bundles require a canonical stable version")
    if any(int(part) > 2**64 - 1 for part in version.split(".")):
        raise ValueError("version component exceeds unsigned 64-bit bound")
    return version


def digest_file(path):
    with open(path, "rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def no_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON property")
        result[key] = value
    return result


def protocol_from_text(text, version, *, outer=False, direct=False):
    canonical_version(version)
    lines = text.replace("\r\n", "\n").splitlines()
    begin, end = "GROK-UPDATE-PROTOCOL-BEGIN", "GROK-UPDATE-PROTOCOL-END"
    if lines.count(begin) != 1 or lines.count(end) != 1 or lines.index(begin) >= lines.index(end):
        raise ValueError("missing or duplicate package protocol")
    protocol = json.loads("\n".join(lines[lines.index(begin) + 1:lines.index(end)]), object_pairs_hook=no_duplicates)
    expected = dict(schema=1, version=version, platform=TARGET, mode="executable-only",
                    manifest="SHA256SUMS.txt", executable="grok-zh.exe", installer="Install-GrokZh.ps1")
    if direct:
        expected["platform"] = "x86_64-pc-windows-gnu"
        expected["direct_runtime"] = dict(schema=1, version=version, platform=TARGET)
    elif outer:
        expected["platform"] = "x86_64-pc-windows-gnu"
        metadata = protocol.get("embedded_runtime")
        validate_runtime_metadata(metadata, version)
        expected["embedded_runtime"] = metadata
    if (protocol != expected or type(protocol.get("schema")) is not int or
            (direct and type(protocol.get("direct_runtime", {}).get("schema")) is not int)):
        raise ValueError("native package protocol/version/target mismatch")
    if not re.search(r"^Target:[ \t]*" + expected["platform"] + r"[ \t]*$", text, re.M):
        raise ValueError("native package BUILD-INFO target mismatch")
    return protocol


def validate_runtime_metadata(metadata, version):
    if not isinstance(metadata, dict) or set(metadata) != {"schema", "format", "version", "platform", "archive_size", "archive_sha256"}:
        raise ValueError("invalid embedded runtime descriptor")
    if (type(metadata["schema"]) is not int or metadata["schema"] != 1 or metadata["format"] != FORMAT or
            metadata["version"] != version or metadata["platform"] != TARGET or
            type(metadata["archive_size"]) is not int or not 0 < metadata["archive_size"] <= MAX_ARCHIVE_BYTES or
            not isinstance(metadata["archive_sha256"], str) or not re.fullmatch(r"[0-9a-f]{64}", metadata["archive_sha256"])):
        raise ValueError("embedded runtime descriptor identity/size/hash mismatch")
    return metadata


def safe_relative(name):
    if not name or name != name.strip() or re.search(r'[\\:<>"|?*\x00-\x1f\x7f]', name):
        raise ValueError("unsafe package member")
    for part in name.split("/"):
        if (part in ("", ".", "..") or part.endswith((".", " ")) or
                re.match(r"^(CON|PRN|AUX|NUL|COM[1-9¹²³]|LPT[1-9¹²³])(?:\.|$)", part, re.I)):
            raise ValueError("unsafe package member")
    return name


def parse_manifest(text):
    hashes, seen = {}, set()
    for line in text.splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]{64})  (.+)", line)
        if not match:
            raise ValueError("invalid native package manifest")
        digest, name = match.groups()
        safe_relative(name)
        if name.casefold() in seen or name.casefold() == "sha256sums.txt":
            raise ValueError("duplicate native manifest member")
        hashes[name] = digest.lower()
        seen.add(name.casefold())
    if not {"BUILD-INFO.txt", "grok-zh.exe", "Install-GrokZh.ps1"} <= hashes.keys():
        raise ValueError("native package manifest missing required entries")
    return hashes


def _validate_package_archive(archive, version, *, outer=False, direct=False):
    """Fully validate ZIP structure, metadata and every manifest digest."""
    canonical_version(version)
    if not 0 < Path(archive).stat().st_size <= MAX_ARCHIVE_BYTES:
        raise ValueError("native archive exceeds size limit")
    root = f"grok-zh-{version}-windows-x86_64-" + ("gnu" if outer or direct else "msvc")
    with zipfile.ZipFile(archive) as zipped:
        if not 0 < len(zipped.infolist()) <= MAX_ENTRIES:
            raise ValueError("native archive entry limit exceeded")
        files, directories, seen, total = {}, set(), set(), 0
        for entry in zipped.infolist():
            name = entry.filename
            if "\\" in name or "//" in name or ":" in name:
                raise ValueError("unsafe native archive path")
            directory = name.endswith("/")
            unix_type = (entry.external_attr >> 16) & 0xF000
            if (entry.flag_bits & 1 or unix_type not in (0, 0x8000, 0x4000) or
                    entry.external_attr & 0x400 or (unix_type == 0x8000 and directory) or
                    (unix_type == 0x4000 and not directory) or entry.compress_type not in (0, 8)):
                raise ValueError("unsupported native archive member type")
            if not name.startswith(root + "/"):
                raise ValueError("native archive must have one canonical package root")
            relative = name[len(root) + 1:]
            logical = relative.rstrip("/")
            if not logical and directory:
                key = root + "/"
            else:
                safe_relative(logical)
                key = logical.casefold()
            if key in seen:
                raise ValueError("duplicate native archive member")
            seen.add(key)
            if directory:
                if entry.file_size:
                    raise ValueError("nonempty directory member")
                if logical:
                    directories.add(logical)
            else:
                if not 0 <= entry.file_size <= MAX_ARCHIVE_BYTES:
                    raise ValueError("native member exceeds size limit")
                total += entry.file_size
                if total > MAX_EXPANDED_BYTES:
                    raise ValueError("native expanded size limit exceeded")
                files[relative] = entry
        for name in ("BUILD-INFO.txt", "SHA256SUMS.txt"):
            if name not in files or files[name].file_size > 65536:
                raise ValueError("missing or oversized native metadata")
        info = zipped.read(files["BUILD-INFO.txt"]).decode("utf-8-sig").replace("\r\n", "\n")
        protocol = protocol_from_text(info, version, outer=outer, direct=direct)
        hashes = parse_manifest(zipped.read(files["SHA256SUMS.txt"]).decode("utf-8-sig"))
        if set(files) != set(hashes) | {"SHA256SUMS.txt"}:
            raise ValueError("native manifest does not cover exact file set")
        allowed_dirs = set()
        folded_files = {name.casefold() for name in files}
        for name in files:
            parts = name.split("/")
            for index in range(1, len(parts)):
                parent = "/".join(parts[:index])
                if parent.casefold() in folded_files:
                    raise ValueError("native file/directory collision")
                allowed_dirs.add(parent)
        if not directories <= allowed_dirs:
            raise ValueError("unlisted native archive directory")
        if not outer:
            with zipped.open(files["grok-zh.exe"]) as executable:
                validate_pe_prefix(executable, files["grok-zh.exe"].file_size)
        for name, digest in hashes.items():
            hasher, count = hashlib.sha256(), 0
            with zipped.open(files[name]) as member:
                while chunk := member.read(CHUNK):
                    count += len(chunk)
                    if count > files[name].file_size:
                        raise ValueError("native member exceeds declared size")
                    hasher.update(chunk)
            if count != files[name].file_size or hasher.hexdigest() != digest:
                raise ValueError("native package member SHA-256 mismatch")
    return dict(protocol=protocol, hashes=hashes, root=root, expanded_size=total,
                archive_size=Path(archive).stat().st_size, archive_sha256=digest_file(archive))


def validate_native_archive(archive, version):
    return _validate_package_archive(archive, version)


def validate_compat_archive(archive, version):
    """Validate the exact legacy transport, footer and complete native payload."""
    outer = _validate_package_archive(archive, version, outer=True)
    legacy_names = {
        "grok-zh.exe", "agent-zh.cmd", "rg.exe", "一键安装.cmd", "[可选]替换原始启动方式.cmd",
        "Install-GrokZh.ps1", "INSTALL-WINDOWS.md", "LICENSE-grok-build.txt", "BUILD-INFO.txt",
        "licenses/ripgrep/COPYING", "licenses/ripgrep/LICENSE-MIT", "licenses/ripgrep/UNLICENSE",
        "licenses/project/THIRD-PARTY-NOTICES", "licenses/project/THIRD_PARTY_NOTICES.md", "licenses/project/NOTICE",
    }
    if set(outer["hashes"]) != legacy_names:
        raise ValueError("outer transport must retain exact legacy file set")
    metadata = outer["protocol"]["embedded_runtime"]
    with tempfile.TemporaryDirectory(prefix="grok-compat-verify-") as directory:
        launcher = Path(directory) / "grok-zh.exe"
        with zipfile.ZipFile(archive) as zipped, zipped.open(outer["root"] + "/grok-zh.exe") as source, launcher.open("xb") as output:
            shutil.copyfileobj(source, output, CHUNK)
        native = validate_bundle(launcher, version, expected_size=metadata["archive_size"], expected_sha256=metadata["archive_sha256"])
    return dict(embedded_runtime=metadata, native=native, outer=outer)


def runtime_metadata(archive, version):
    native = validate_native_archive(archive, version)
    return dict(schema=1, format=FORMAT, version=version, platform=TARGET,
                archive_size=native["archive_size"], archive_sha256=native["archive_sha256"])


def validate_pe_prefix(source, prefix_size):
    """Check bounded PE32+ x64 headers without trusting overlay bytes as PE."""
    if prefix_size < 256:
        raise ValueError("embedded prefix is too short for PE headers")
    source.seek(0)
    dos = source.read(64)
    if len(dos) != 64 or dos[:2] != b"MZ":
        raise ValueError("embedded prefix lacks DOS header")
    pe_offset = struct.unpack_from("<I", dos, 0x3C)[0]
    if pe_offset < 64 or pe_offset + 24 > prefix_size:
        raise ValueError("PE header lies outside executable prefix")
    source.seek(pe_offset)
    coff = source.read(24)
    if len(coff) != 24 or coff[:4] != b"PE\0\0" or struct.unpack_from("<H", coff, 4)[0] != 0x8664:
        raise ValueError("embedded prefix is not an x64 PE executable")
    optional_size = struct.unpack_from("<H", coff, 20)[0]
    sections = struct.unpack_from("<H", coff, 6)[0]
    if optional_size < 112 or sections < 1 or pe_offset + 24 + optional_size + sections * 40 > prefix_size or source.read(2) != b"\x0b\x02":
        raise ValueError("invalid or truncated PE32+ optional header")


def read_footer(launcher, *, expected_size=None, expected_sha256=None, verify=True):
    with open(launcher, "rb") as source:
        length = os.fstat(source.fileno()).st_size
        if not FOOTER.size + 2 <= length <= MAX_ARCHIVE_BYTES or source.read(2) != b"MZ":
            raise ValueError("bundle prefix is not a Windows executable")
        source.seek(-FOOTER.size, os.SEEK_END)
        magic, offset, size, digest = FOOTER.unpack(source.read(FOOTER.size))
        if (magic != MAGIC or not 0 < size <= MAX_ARCHIVE_BYTES or offset < 256 or
                offset > length - FOOTER.size or size != length - FOOTER.size - offset):
            raise ValueError("invalid embedded bundle footer/bounds")
        validate_pe_prefix(source, offset)
        if ((expected_size is not None and size != expected_size) or
                (expected_sha256 is not None and digest.hex() != expected_sha256)):
            raise ValueError("embedded bundle does not match pinned payload")
        if verify:
            source.seek(offset)
            hasher, remaining = hashlib.sha256(), size
            while remaining:
                chunk = source.read(min(CHUNK, remaining))
                if not chunk:
                    raise ValueError("truncated embedded bundle")
                hasher.update(chunk)
                remaining -= len(chunk)
            if hasher.digest() != digest:
                raise ValueError("embedded payload SHA-256 mismatch")
    return dict(offset=offset, size=size, sha256=digest.hex())


def append_bundle(prefix, archive, destination, version):
    metadata = runtime_metadata(archive, version)
    prefix, archive, destination = map(Path, (prefix, archive, destination))
    if destination in (prefix, archive):
        raise ValueError("bundle output must be distinct from its inputs")
    if prefix.stat().st_size + metadata["archive_size"] + FOOTER.size > MAX_ARCHIVE_BYTES:
        raise ValueError("complete GNU executable exceeds historical updater 512 MiB limit")
    created = False
    try:
        with prefix.open("rb") as source:
            validate_pe_prefix(source, prefix.stat().st_size)
            source.seek(0)
            with destination.open("xb") as output:
                created = True
                shutil.copyfileobj(source, output, CHUNK)
                offset = output.tell()
                with archive.open("rb") as payload:
                    shutil.copyfileobj(payload, output, CHUNK)
                output.write(FOOTER.pack(MAGIC, offset, metadata["archive_size"], bytes.fromhex(metadata["archive_sha256"])))
                output.flush()
                os.fsync(output.fileno())
        read_footer(destination, expected_size=metadata["archive_size"], expected_sha256=metadata["archive_sha256"])
    except BaseException:
        if created:
            destination.unlink(missing_ok=True)
        raise
    return metadata


def extract_archive(launcher, destination, *, expected_size=None, expected_sha256=None):
    footer = read_footer(launcher, expected_size=expected_size, expected_sha256=expected_sha256, verify=False)
    destination = Path(destination)
    created = False
    try:
        with open(launcher, "rb") as source, destination.open("xb") as output:
            created = True
            source.seek(footer["offset"])
            hasher, remaining = hashlib.sha256(), footer["size"]
            while remaining:
                chunk = source.read(min(CHUNK, remaining))
                if not chunk:
                    raise ValueError("truncated embedded bundle")
                hasher.update(chunk)
                output.write(chunk)
                remaining -= len(chunk)
            if hasher.hexdigest() != footer["sha256"]:
                raise ValueError("embedded payload SHA-256 mismatch")
        return destination
    except BaseException:
        if created:
            destination.unlink(missing_ok=True)
        raise


def validate_bundle(launcher, version, *, expected_size=None, expected_sha256=None):
    with tempfile.TemporaryDirectory(prefix="grok-embedded-verify-") as directory:
        archive = extract_archive(launcher, Path(directory) / "native.zip", expected_size=expected_size,
                                  expected_sha256=expected_sha256)
        return validate_native_archive(archive, version)
