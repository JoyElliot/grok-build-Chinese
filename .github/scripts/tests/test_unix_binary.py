"""Exercise the ELF/Mach-O invariants used before publishing stripped programs."""

import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location(
    "strip_unix", Path(__file__).resolve().parents[1] / "strip-unix-binary.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def elf(symbols=True):
    data = bytearray(512)
    names = b"\0.text\0.shstrtab\0.symtab\0"
    data[256:260] = b"code"
    data[300:300 + len(names)] = names
    header = (b"\x7fELF\x02\x01\x01" + bytes(9), 3, 62, 1, 0x100100,
              64, 512, 0, 64, 56, 1, 64, 4 if symbols else 3, 2)
    struct.pack_into("<16sHHIQQQIHHHHHH", data, 0, *header)
    struct.pack_into("<IIQQQQQQ", data, 64, 1, 5, 0, 0x100000, 0x100000, 260, 260, 4096)
    records = [(0,) * 10, (1, 1, 6, 0x100100, 256, 4, 0, 0, 16, 0),
               (7, 3, 0, 0, 300, len(names), 0, 0, 1, 0)]
    if symbols:
        records.append((17, 2, 0, 0, 400, 24, 2, 0, 8, 24))
    data.extend(b"".join(struct.pack("<IIQQQQIIQQ", *s) for s in records))
    return data


def macho(locals_present=True):
    def command(cmd, data):
        return struct.pack("<II", cmd, len(data) + 8) + data

    count = 3 if locals_present else 2
    names = b"\0local\0entry\0_external\0" if locals_present else b"\0entry\0_external\0"
    entry_name = 7 if locals_present else 1
    external_name = 13 if locals_present else 7
    symbol_entries = []
    if locals_present:
        symbol_entries.append(struct.pack("<IBBHQ", 1, 0xE, 1, 0, 0x100000300))
    symbol_entries.extend([struct.pack("<IBBHQ", entry_name, 0xF, 1, 0, 0x100000300),
                           struct.pack("<IBBHQ", external_name, 1, 0, 0, 0)])
    linkedit = bytearray(b"REBASE!!EXPORT!!SPLIT!!!DRS!!!!!")
    symoff = 1024 + len(linkedit)
    linkedit.extend(b"".join(symbol_entries))
    indirectoff = 1024 + len(linkedit)
    linkedit.extend(struct.pack("<I", count - 1))
    stroff = 1024 + len(linkedit)
    linkedit.extend(names)
    linkedit.extend(bytes((-len(linkedit)) % 16))
    sigoff = 1024 + len(linkedit)
    signature = struct.pack(">IIIII", 0xFADE0CC0, 64, 1, 0, 20)
    signature += struct.pack(">6I", 0xFADE0C02, 44, 0x20400, 2, 0, 24) + b"fixture\0" + bytes(12)
    linkedit.extend(signature)
    text = struct.pack("<16sQQQQiiII", b"__TEXT", 0x100000000, 4096, 0, 1024, 5, 5, 1, 0)
    text += struct.pack("<16s16sQQ8I", b"__text", b"__TEXT", 0x100000300,
                        4, 768, 2, 0, 0, 0x80000400, 0, 0, 0)
    commands = [command(0x19, text), command(0x19, struct.pack(
        "<16sQQQQiiII", b"__LINKEDIT", 0x100001000, 4096, 1024, len(linkedit), 1, 1, 0, 0))]
    commands.append(command(0x80000022, struct.pack("<10I", 1024, 8, 0, 0, 0, 0, 0, 0, 1032, 8)))
    commands.append(command(0x1E, struct.pack("<II", 1040, 8)))
    commands.append(command(0x2B, struct.pack("<II", 1048, 8)))
    commands.append(command(2, struct.pack("<4I", symoff, count, stroff, len(names))))
    dynamic = [0, int(locals_present), int(locals_present), 1, count - 1, 1] + [0] * 12
    dynamic[12:14] = [indirectoff, 1]
    commands.append(command(0xB, struct.pack("<18I", *dynamic)))
    commands.append(command(0x1B, bytes(range(16))))
    commands.append(command(0x80000028, struct.pack("<QQ", 768, 0)))
    commands.append(command(0x1D, struct.pack("<II", sigoff, len(signature))))
    header = struct.pack("<8I", 0xFEEDFACF, 0x100000C, 0, 2, len(commands),
                         sum(map(len, commands)), 0x200085, 0)
    data = bytearray(header + b"".join(commands))
    data.extend(bytes(1024 - len(data)))
    data[768:772] = b"code"
    data.extend(linkedit)
    return data


class UnixBinaryTests(unittest.TestCase):
    def inspect(self, data, platform):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "binary"
            path.write_bytes(data)
            return MODULE.inspect_image(path, platform)

    def test_elf_removes_only_nonallocated_static_symbols(self):
        MODULE.verify_stripped(self.inspect(elf(), "linux"), self.inspect(elf(False), "linux"), "linux")

    def test_elf_rejects_code_entry_and_loader_changes(self):
        before = self.inspect(elf(), "linux")
        for offset in (256, 24, 64 + 4, 64 + 40):
            with self.subTest(offset=offset):
                after = elf(False)
                after[offset] ^= 1
                with self.assertRaisesRegex(ValueError, "runtime image"):
                    MODULE.verify_stripped(before, self.inspect(after, "linux"), "linux")

    def test_elf_requires_static_symbol_removal(self):
        before = self.inspect(elf(), "linux")
        with self.assertRaisesRegex(ValueError, "symbols remain"):
            MODULE.verify_stripped(before, before, "linux")

    def test_elf_address_array_entry_size_normalization_preserves_runtime_checks(self):
        def array(symbols, kind, entsize):
            data = elf(symbols)
            # Convert the allocated fixture section to an array of two pointers.
            struct.pack_into("<IIQQQQIIQQ", data, 512 + 64,
                             1, kind, 3, 0x100100, 256, 16, 0, 0, 8, entsize)
            struct.pack_into("<QQ", data, 64 + 32, 272, 272)
            return data

        for kind in (14, 15, 16):
            with self.subTest(kind=kind):
                before = self.inspect(array(True, kind, 0), "linux")
                after = array(False, kind, 8)
                MODULE.verify_stripped(before, self.inspect(after, "linux"), "linux")
                for entry_size in (4, 16):
                    with self.assertRaisesRegex(ValueError, "array size"):
                        self.inspect(array(False, kind, entry_size), "linux")
                for offset in (256, 512 + 64 + 8, 512 + 64 + 16, 512 + 64 + 24, 512 + 64 + 32):
                    changed = bytearray(after)
                    changed[offset] ^= 8
                    with self.assertRaisesRegex(ValueError, "runtime image"):
                        MODULE.verify_stripped(before, self.inspect(changed, "linux"), "linux")
                changed = bytearray(after)
                changed[512 + 64 + 32] ^= 1
                with self.assertRaisesRegex(ValueError, "array size"):
                    self.inspect(changed, "linux")

    def test_elf_other_entry_sizes_and_error_details_are_preserved(self):
        before = self.inspect(elf(), "linux")
        changed = elf(False)
        struct.pack_into("<Q", changed, 512 + 64 + 56, 8)
        with self.assertRaisesRegex(ValueError, r"image.sections\[0\]\[9\]"):
            MODULE.verify_stripped(before, self.inspect(changed, "linux"), "linux")
        changed = elf(False)
        changed[24] ^= 1
        with self.assertRaisesRegex(ValueError, "image.entry"):
            MODULE.verify_stripped(before, self.inspect(changed, "linux"), "linux")

    def test_macho_reindexed_indirect_symbols_and_relocated_linkedit_are_equivalent(self):
        before = self.inspect(macho(), "macos")
        after = self.inspect(macho(False), "macos")
        MODULE.verify_stripped(before, after, "macos")
        self.assertEqual(after["local_symbols"], 0)

    def test_macho_rejects_code_dyld_export_symbol_and_uuid_changes(self):
        before = self.inspect(macho(), "macos")
        original = macho(False)
        uuid = original.index(bytes(range(16)))
        name = original.index(b"entry\0")
        for offset in (768, 1024, 1032, 1040, 1048, uuid, name):
            with self.subTest(offset=offset):
                after = bytearray(original)
                after[offset] ^= 1
                with self.assertRaisesRegex(ValueError, "runtime image"):
                    MODULE.verify_stripped(before, self.inspect(after, "macos"), "macos")

    def test_macho_rejects_entitlements_cms_and_restricted_signatures(self):
        for offset, value in ((-56, 2), (-32, 0x10002), (-32, 0)):
            with self.subTest(offset=offset, value=value):
                data = macho()
                struct.pack_into(">I", data, len(data) + offset, value)
                with self.assertRaisesRegex(ValueError, "signature|code directory"):
                    self.inspect(data, "macos")

    def test_truncated_or_wrong_architecture_input_is_rejected(self):
        for platform, fixture, machine_offset in (("linux", elf(), 18), ("macos", macho(), 4)):
            with self.subTest(platform=platform):
                with self.assertRaises(ValueError):
                    self.inspect(fixture[:100], platform)
                fixture[machine_offset] ^= 1
                with self.assertRaises(ValueError):
                    self.inspect(fixture, platform)


if __name__ == "__main__":
    unittest.main()
