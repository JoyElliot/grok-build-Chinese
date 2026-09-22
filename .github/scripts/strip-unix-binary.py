"""Strip a staged Unix executable, preserving and checking its runtime image."""

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile


def digest(data):
    return hashlib.sha256(data).hexdigest()


def region(data, offset, size):
    if offset < 0 or size < 0 or offset + size > len(data):
        raise ValueError("truncated binary region")
    return data[offset:offset + size]


def unpack(fmt, data, offset=0):
    return struct.unpack(fmt, region(data, offset, struct.calcsize(fmt)))


def cstring(data, offset):
    if not 0 <= offset < len(data):
        raise ValueError("invalid string offset")
    end = data.find(b"\0", offset)
    if end < 0:
        raise ValueError("unterminated binary string")
    return data[offset:end].hex()


def adhoc_identity(signature):
    magic, length, count = unpack(">III", signature)
    if magic != 0xFADE0CC0 or length > len(signature) or not 1 <= count <= 16:
        raise ValueError("expected ad-hoc signature container")
    identities, seen = [], set()
    for i in range(count):
        slot, offset = unpack(">II", signature, 12 + i * 8)
        if slot in seen or offset < 12 + count * 8:
            raise ValueError("invalid signature slot")
        seen.add(slot)
        blob_magic, blob_length = unpack(">II", signature, offset)
        blob = region(signature[:length], offset, blob_length)
        if slot == 0 or 0x1000 <= slot <= 0x1005:
            _, _, version, flags, hash_offset, identifier_offset = unpack(">6I", blob)
            if blob_magic != 0xFADE0C02 or not flags & 2 or flags & ~0x20002:
                raise ValueError("expected ad-hoc code directory without runtime restrictions")
            identities.append(bytes.fromhex(cstring(blob, identifier_offset)).decode("utf-8"))
        elif slot == 2 and blob == struct.pack(">III", 0xFADE0C01, 12, 0):
            pass  # codesign may add an empty requirements set.
        else:
            raise ValueError("signature has entitlements, requirements or CMS that must be preserved")
    if 0 not in seen or not identities or len(set(identities)) != 1:
        raise ValueError("inconsistent ad-hoc signing identifier")
    return identities[0]


def elf_image(data):
    header = unpack("<16sHHIQQQIHHHHHH", data)
    ident, kind, machine, version, entry, phoff, shoff, flags, ehsize, phsize, phnum, shsize, shnum, names_index = header
    if (ident[:7] != b"\x7fELF\x02\x01\x01" or kind != 3 or machine != 62
            or version != 1 or ehsize != 64 or phsize != 56 or shsize != 64
            or not phnum or not shnum or names_index >= shnum):
        raise ValueError("expected ELF64 little-endian x86_64 PIE")
    programs = [unpack("<IIQQQQQQ", data, phoff + i * phsize) for i in range(phnum)]
    for p in programs:
        region(data, p[2], p[5])
    sections = [unpack("<IIQQQQIIQQ", data, shoff + i * shsize) for i in range(shnum)]
    names = region(data, sections[names_index][4], sections[names_index][5])
    mapped = []
    symbols = 0
    for section in sections:
        name, typ, attributes, addr, off, size, link, info, align, entsize = section
        if typ == 2:  # SHT_SYMTAB; never SHT_DYNSYM.
            symbols += size
        if not attributes & 2:  # SHF_ALLOC
            continue
        if typ in (14, 15, 16):  # SHT_INIT_ARRAY / FINI_ARRAY / PREINIT_ARRAY
            # ELF64 x86_64 arrays contain 8-byte function addresses. LLVM may
            # leave sh_entsize unset; GNU strip fills it without changing data.
            if entsize not in (0, 8) or size % 8:
                raise ValueError("invalid ELF64 initialization/finalization array size")
            entsize = 8
        content = b"" if typ == 8 else region(data, off, size)  # SHT_NOBITS
        linked_name = cstring(names, sections[link][0]) if link else None
        mapped.append((cstring(names, name), typ, attributes, addr, off, size,
                       linked_name, info, align, entsize, digest(content)))
    if not mapped:
        raise ValueError("ELF has no allocated sections")
    return {"image": (ident.hex(), kind, machine, version, entry, flags, programs, mapped),
            "static_symbol_bytes": symbols}


def macho_image(data):
    magic, cpu, subtype, kind, count, command_bytes, flags, reserved = unpack("<8I", data)
    if magic != 0xFEEDFACF or cpu != 0x100000C or kind != 2:
        raise ValueError("expected thin Mach-O arm64 executable")
    end = 32 + command_bytes
    region(data, 32, command_bytes)
    commands, segments, sections, payloads = [], [], [], []
    symbols = None
    dynamic = None
    signature = None
    off = 32
    for _ in range(count):
        cmd, size = unpack("<II", data, off)
        if size < 8 or size % 8 or off + size > end:
            raise ValueError("invalid Mach-O load command")
        raw = region(data, off, size)
        if cmd == 0x19:  # LC_SEGMENT_64
            seg = unpack("<16sQQQQiiII", raw, 8)
            name, vmaddr, vmsize, fileoff, filesize, maxprot, initprot, nsects, segflags = seg
            if size != 72 + 80 * nsects:
                raise ValueError("invalid Mach-O section table")
            region(data, fileoff, filesize)
            if name.rstrip(b"\0") == b"__LINKEDIT":
                # Offsets/capacity change; every runtime payload below is checked.
                segments.append((name.hex(), vmaddr, fileoff, maxprot, initprot, segflags))
            else:
                segments.append(tuple(x.hex() if isinstance(x, bytes) else x for x in seg))
            for i in range(nsects):
                s = unpack("<16s16sQQ8I", raw, 72 + i * 80)
                sname, owner, addr, length, start, align, reloff, nreloc, sflags, r1, r2, r3 = s
                if nreloc:
                    raise ValueError("unexpected section relocations in linked Mach-O")
                zero_fill = sflags & 0xFF in (1, 0xC, 0x12)
                content = b"" if zero_fill else region(data, start, length)
                sections.append((sname.hex(), owner.hex(), addr, length, start, align,
                                 sflags, r1, r2, r3, digest(content)))
        elif cmd == 2:  # LC_SYMTAB
            symbols = unpack("<4I", raw, 8)
        elif cmd == 0xB:  # LC_DYSYMTAB
            dynamic = unpack("<18I", raw, 8)
        elif cmd in (0x22, 0x80000022):  # LC_DYLD_INFO[_ONLY]
            values = unpack("<10I", raw, 8)
            payloads.append((cmd, [(length, digest(region(data, start, length)))
                                   for start, length in zip(values[::2], values[1::2])]))
        elif cmd in (0x1E, 0x26, 0x29, 0x2B, 0x2E, 0x80000033, 0x80000034):
            # linkedit_data_command: data may move, but its contents must not.
            start, length = unpack("<II", raw, 8)
            payloads.append((cmd, length, digest(region(data, start, length))))
        elif cmd == 0x1D:  # LC_CODE_SIGNATURE; only ordinary ad-hoc inputs allowed.
            start, length = unpack("<II", raw, 8)
            signature = region(data, start, length)
        else:
            # Entry point, UUID, dependencies, build version, stack size, etc.
            commands.append(raw.hex())
        off += size
    if off != end or symbols is None or dynamic is None or signature is None:
        raise ValueError("incomplete Mach-O loader metadata")
    symoff, nsyms, stroff, strsize = symbols
    strings = region(data, stroff, strsize)
    table = []
    for i in range(nsyms):
        name, typ, sect, desc, value = unpack("<IBBHQ", data, symoff + 16 * i)
        table.append((cstring(strings, name), typ, sect, desc, value))
    ilocal, nlocal, iext, nextdef, iundef, nundef = dynamic[:6]
    if any(dynamic[i] for i in (7, 9, 11, 15, 17)):
        raise ValueError("unsupported Mach-O dynamic symbol tables/relocations")
    for first, number in ((ilocal, nlocal), (iext, nextdef), (iundef, nundef)):
        if first + number > nsyms:
            raise ValueError("invalid Mach-O symbol range")
    indirectoff, nindirect = dynamic[12:14]
    indirect = []
    for i in range(nindirect):
        index, = unpack("<I", data, indirectoff + 4 * i)
        if index & 0xC0000000:
            indirect.append(index)
        elif index < len(table):
            indirect.append(table[index])
        else:
            raise ValueError("invalid indirect symbol index")
    # Refuse Developer ID, entitlements, hardened-runtime or other metadata that
    # this unsigned community packaging path is not equipped to preserve.
    identity = adhoc_identity(signature)
    return {
        "image": ((magic, cpu, subtype, kind, flags, reserved), commands, segments,
                  sections, payloads, table[iext:iext + nextdef],
                  table[iundef:iundef + nundef], indirect, identity),
        "local_symbols": nlocal,
        "signing_identifier": identity,
    }


def inspect_image(path, platform):
    data = Path(path).read_bytes()
    details = elf_image(data) if platform == "linux" else macho_image(data)
    return details | {"bytes": len(data), "sha256": digest(data)}


def image_differences(before, after, platform):
    names = (("identity", "type", "machine", "version", "entry", "flags", "programs", "sections")
             if platform == "linux" else
             ("header", "commands", "segments", "sections", "payloads", "exports", "undefined", "indirect", "identity"))
    differences = []

    def compare(left, right, path):
        if left == right or len(differences) >= 8:
            return
        if isinstance(left, (tuple, list)) and isinstance(right, (tuple, list)):
            if len(left) != len(right):
                differences.append({"path": path + ".length", "before": len(left), "after": len(right)})
            for index, (a, b) in enumerate(zip(left, right)):
                compare(a, b, f"{path}[{index}]")
                if len(differences) >= 8:
                    break
        else:
            differences.append({"path": path, "before": str(left)[:160], "after": str(right)[:160]})

    for name, left, right in zip(names, before["image"], after["image"]):
        compare(left, right, "image." + name)
    return differences


def verify_stripped(before, after, platform):
    if before["image"] != after["image"]:
        details = json.dumps(image_differences(before, after, platform))
        raise ValueError("stripping changed the executable runtime image: " + details)
    if after["bytes"] > before["bytes"]:
        raise ValueError("stripping increased the executable size")
    if platform == "linux" and after["static_symbol_bytes"]:
        raise ValueError("static ELF symbols remain after stripping")
    if platform == "macos" and after["local_symbols"] > before["local_symbols"]:
        raise ValueError("Mach-O local symbol count increased")


def smoke_pair(before, after, version):
    checks = []
    with tempfile.TemporaryDirectory(prefix="grok-unix-smoke-") as home:
        env = os.environ | {"GROK_HOME": home}
        for args in (["--version"], ["--help"], ["agent", "--help"], ["update", "--help"]):
            outputs = [subprocess.run([str(p.resolve()), *args], env=env, check=True,
                                      capture_output=True, timeout=30).stdout for p in (before, after)]
            if not outputs[0].strip() or outputs[0] != outputs[1]:
                raise ValueError(f"CLI output changed after stripping: {args}")
            if args == ["--version"] and not re.match(
                    rf"^grok-zh {re.escape(version)} \(", outputs[1].decode("utf-8")):
                raise ValueError("unexpected executable version")
            checks.append({"args": args, "stdout_sha256": digest(outputs[1])})
    return checks


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", choices=("linux", "macos"), required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--diagnostics", type=Path, required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()
    if args.input.resolve() == args.output.resolve():
        parser.error("input and staged output must differ")
    if args.diagnostics.resolve().is_relative_to(args.output.resolve().parent):
        parser.error("symbol diagnostics must stay outside the installation package")
    before = inspect_image(args.input, args.platform)
    args.diagnostics.mkdir(parents=True, exist_ok=True)
    symbols = args.diagnostics / "symbols.nm.gz"
    with tempfile.TemporaryFile() as raw:
        subprocess.run(["nm", "-an", str(args.input)], stdout=raw, check=True, timeout=120,
                       env=os.environ | {"LC_ALL": "C"})
        if not raw.tell():
            raise ValueError("empty original symbol diagnostics")
        raw.seek(0)
        with symbols.open("wb") as output, gzip.GzipFile(fileobj=output, mode="wb", mtime=0) as zipped:
            shutil.copyfileobj(raw, zipped)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    if args.platform == "linux":
        subprocess.run(["strip", "--strip-unneeded", "-o", str(args.output), str(args.input)], check=True)
    else:
        subprocess.run(["strip", "-x", "-o", str(args.output), str(args.input)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", "--identifier",
                        before["signing_identifier"],
                        "--preserve-metadata=identifier,entitlements,flags,runtime,requirements",
                        str(args.output)], check=True)
        subprocess.run(["codesign", "--verify", "--strict", str(args.output)], check=True)
    args.output.chmod(0o755)
    after = inspect_image(args.output, args.platform)
    verify_stripped(before, after, args.platform)
    checks = smoke_pair(args.input, args.output, args.version)
    report = {
        "platform": args.platform, "version": args.version,
        "source_commit": os.environ.get("GITHUB_SHA"),
        "input_bytes": before["bytes"], "output_bytes": after["bytes"],
        "saved_bytes": before["bytes"] - after["bytes"],
        "input_sha256": before["sha256"], "output_sha256": after["sha256"],
        "runtime_image_unchanged": True, "cli_checks": checks,
        "symbols_file": symbols.name, "symbols_sha256": digest(symbols.read_bytes()),
        "symbols_bytes": symbols.stat().st_size, "symbols_command": ["nm", "-an"],
        "symbols_locale": "C", "symbols_format": "native nm numeric address/type/name listing",
        "diagnostic_boundary": "Original nm address/name map retained for offline symbol lookup; live crash names may be reduced. No DWARF line tables in this diagnostic map.",
    }
    (args.diagnostics / "manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
