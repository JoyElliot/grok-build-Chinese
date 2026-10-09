"""Strip a staged Windows EXE and verify that its runtime image is unchanged."""

import argparse
import array
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tempfile


def inspect_image(path):
    data = Path(path).read_bytes()
    if data[:2] != b"MZ" or len(data) < 64:
        raise ValueError("not a Windows executable")
    pe = struct.unpack_from("<I", data, 60)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError("missing PE signature")
    machine, count, _, symbols, symbol_count, optional_size, flags = struct.unpack_from(
        "<HHIIIHH", data, pe + 4,
    )
    optional = pe + 24
    if machine not in (0x8664, 0xAA64) or struct.unpack_from("<H", data, optional)[0] != 0x20B:
        raise ValueError("expected Windows x64 or ARM64 PE32+")
    header = data[optional:optional + optional_size]
    if len(header) != optional_size or optional_size < 112:
        raise ValueError("truncated optional header")
    directory_count = struct.unpack_from("<I", header, 108)[0]
    if 112 + directory_count * 8 > optional_size:
        raise ValueError("truncated data directories")
    directories = [struct.unpack_from("<II", header, 112 + i * 8)
                   for i in range(directory_count)]
    if len(directories) > 4 and directories[4] != (0, 0):
        raise ValueError("strip before signing; refusing a signed executable")
    # Removing discardable DWARF sections can shrink SizeOfImage/SizeOfHeaders.
    # Validate those capacities below, rather than requiring equal file layout.
    # Code, data, imports, unwind tables, entry point and mitigations must match.
    image_header = header[16:56] + header[68:112]
    runtime_directories = [(i, d) for i, d in enumerate(directories) if i != 6]
    section_alignment, file_alignment = struct.unpack_from("<II", header, 32)
    image_size, headers_size = struct.unpack_from("<II", header, 56)
    if not section_alignment or not file_alignment:
        raise ValueError("invalid PE alignment")
    section_table_end = optional + optional_size + 40 * count
    if (headers_size < section_table_end or headers_size > len(data)
            or headers_size % file_alignment):
        raise ValueError("invalid PE header capacity")
    image_end = headers_size
    debug_sections = []
    sections = []
    for i in range(count):
        offset = optional + optional_size + 40 * i
        if offset + 40 > len(data):
            raise ValueError("truncated section table")
        name = data[offset:offset + 8].rstrip(b"\0").decode("ascii")
        if name.startswith("/"):
            # PE long section names refer to the COFF string table. GNU C
            # dependencies can retain DWARF even when Cargo debug=0 is set.
            strings = symbols + symbol_count * 18
            if not symbols or strings + 4 > len(data):
                raise ValueError("missing COFF string table")
            strings_size = struct.unpack_from("<I", data, strings)[0]
            name_offset = int(name[1:])
            if strings + strings_size > len(data) or not 4 <= name_offset < strings_size:
                raise ValueError("invalid COFF section name")
            name_start = strings + name_offset
            name_end = data.find(b"\0", name_start, strings + strings_size)
            if name_end < 0:
                raise ValueError("unterminated COFF section name")
            name = data[name_start:name_end].decode("ascii")
        virtual_size, address, size, raw = struct.unpack_from("<IIII", data, offset + 8)
        characteristics = struct.unpack_from("<I", data, offset + 36)[0]
        if size and (raw == 0 or raw + size > len(data)):
            raise ValueError("truncated section contents")
        if size and raw < headers_size:
            raise ValueError("section overlaps PE headers")
        image_end = max(image_end, address + max(virtual_size, size))
        if (name.startswith((".debug_", ".zdebug_"))
                and characteristics & 0x02000000  # discardable
                and not characteristics & 0xA0000000):  # not executable/writable
            debug_sections.append((address, address + max(virtual_size, size)))
        elif characteristics & 0xE0000000:  # IMAGE_SCN_MEM_EXECUTE/READ/WRITE
            sections.append((name, virtual_size, address, size, characteristics,
                             hashlib.sha256(data[raw:raw + size]).hexdigest()))
    if image_size != (image_end + section_alignment - 1) // section_alignment * section_alignment:
        raise ValueError("invalid PE image capacity")
    for start, end in debug_sections:
        if start <= struct.unpack_from("<I", header, 16)[0] < end:
            raise ValueError("entry point references a debug section")
        for _, (address, size) in runtime_directories:
            if address and address < end and address + max(size, 1) > start:
                raise ValueError("runtime directory references a debug section")
    if not sections:
        raise ValueError("executable has no mapped sections")
    return {
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "symbol_table": symbols,
        "symbol_count": symbol_count,
        "image": (machine, flags & ~0x20C, image_header.hex(), runtime_directories, sections),
    }


def verify_stripped(before, after):
    if before["image"] != after["image"]:
        raise ValueError("stripping changed the executable's runtime image")
    if after["symbol_table"] or after["symbol_count"]:
        raise ValueError("COFF symbols remain after stripping")
    if after["bytes"] > before["bytes"]:
        raise ValueError("stripping increased the executable size")


def restore_mingw_write_permissions(path, before, after):
    """Restore only the two known BFD permission losses, then recheck everything."""
    sections = []
    restored = []
    for section in after["image"][4]:
        original = [item for item in before["image"][4] if item[0] == section[0]]
        if (section[0] in (".idata", ".CRT") and len(original) == 1
                and original[0][4] == 0xC0000040 and section[4] == 0x40000040):
            # Original READ|WRITE initialized data, with only WRITE lost by BFD.
            sections.append((*section[:4], original[0][4], section[5]))
            restored.append(section[0])
        else:
            sections.append(section)
    candidate = after | {"image": (*after["image"][:4], sections)}
    # Reject changed bytes, addresses, other flags, headers, or residual symbols
    # before touching the file. The final check also inspects the written result.
    verify_stripped(before, candidate)
    if not restored:
        return []
    data = bytearray(Path(path).read_bytes())
    if hashlib.sha256(data).hexdigest() != after["sha256"]:
        raise ValueError("staged executable changed before permission restoration")
    pe = struct.unpack_from("<I", data, 60)[0]
    count = struct.unpack_from("<H", data, pe + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    offsets = []
    for name in restored:
        matches = [pe + 24 + optional_size + 40 * i for i in range(count)
                   if data[pe + 24 + optional_size + 40 * i:
                           pe + 32 + optional_size + 40 * i].rstrip(b"\0") == name.encode("ascii")]
        if len(matches) != 1:
            raise ValueError("ambiguous section permission restoration")
        offsets.append(matches[0] + 36)
    for offset in offsets:
        struct.pack_into("<I", data, offset, 0xC0000040)
    # Keep the PE checksum valid after updating section-table fields.
    checksum_offset = pe + 24 + 64
    struct.pack_into("<I", data, checksum_offset, 0)
    words = array.array("H", data + (b"\0" if len(data) % 2 else b""))
    if sys.byteorder != "little":
        words.byteswap()
    checksum = sum(words)
    while checksum >> 16:
        checksum = (checksum & 0xFFFF) + (checksum >> 16)
    struct.pack_into("<I", data, checksum_offset, checksum + len(data))
    Path(path).write_bytes(data)
    verify_stripped(before, inspect_image(path))
    return restored


def smoke_binary(path, version):
    with tempfile.TemporaryDirectory(prefix="grok-zh-binary-smoke-") as home:
        env = os.environ | {"GROK_HOME": home}
        for arguments in (["--version"], ["--help"], ["agent", "--help"], ["update", "--help"]):
            result = subprocess.run(
                [str(Path(path).resolve()), *arguments], env=env,
                capture_output=True, encoding="utf-8", errors="replace", timeout=30, check=True,
            )
            if not result.stdout.strip():
                raise ValueError(f"empty output from {arguments}")
            if arguments == ["--version"] and not re.match(
                rf"^grok-zh {re.escape(version)} \(", result.stdout,
            ):
                raise ValueError(f"unexpected executable version: {result.stdout!r}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--strip", required=True)
    parser.add_argument("--preserve-mingw-write-permissions", action="store_true",
                        help="restore only BFD's known .idata/.CRT write-bit loss before strict verification")
    parser.add_argument("--version", required=True, help="expected version for isolated CLI smoke checks")
    parser.add_argument("--symbols-dir", required=True, type=Path,
                        help="diagnostic output directory outside the installation package")
    args = parser.parse_args()
    if args.input.resolve() == args.output.resolve():
        parser.error("input and staged output must differ")
    if args.symbols_dir.resolve().is_relative_to(args.output.resolve().parent):
        parser.error("diagnostic symbols must stay outside the installation package")
    before = inspect_image(args.input)
    args.symbols_dir.mkdir(parents=True, exist_ok=True)
    symbols = args.symbols_dir / (args.output.name + ".debug")
    subprocess.run([args.strip, "--only-keep-debug", "-o", str(symbols), str(args.input)], check=True)
    subprocess.run([args.strip, "--strip-all", "-o", str(args.output), str(args.input)], check=True)
    after = inspect_image(args.output)
    restored = []
    if args.preserve_mingw_write_permissions:
        restored = restore_mingw_write_permissions(args.output, before, after)
        after = inspect_image(args.output)
    verify_stripped(before, after)
    smoke_binary(args.output, args.version)
    report = {
        "input_bytes": before["bytes"], "output_bytes": after["bytes"],
        "saved_bytes": before["bytes"] - after["bytes"],
        "input_sha256": before["sha256"],
        "output_sha256": after["sha256"], "runtime_image_unchanged": True,
        "restored_write_permissions": restored,
        "cli_smoke_passed": True,
        "version": args.version, "source_commit": os.environ.get("GITHUB_SHA"),
        "symbols_file": symbols.name,
        "symbols_sha256": hashlib.sha256(symbols.read_bytes()).hexdigest(),
    }
    (args.symbols_dir / "manifest.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
