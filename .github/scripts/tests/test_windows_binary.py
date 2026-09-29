"""Reject packaging changes that alter executable code or loader metadata."""

import copy
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "strip-windows-binary.py"
SPEC = importlib.util.spec_from_file_location("strip_windows_binary", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
sys.dont_write_bytecode = True
SPEC.loader.exec_module(MODULE)


def executable(with_symbols, machine=0x8664):
    # Minimal PE32+ fixture with one mapped executable section and a file-only
    # COFF symbol. The loader metadata and machine code are identical in both.
    data = bytearray(1024)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 60, 128)
    data[128:132] = b"PE\0\0"
    struct.pack_into("<HHIIIHH", data, 132, machine, 1, 0,
                     1024 if with_symbols else 0, 1 if with_symbols else 0, 240, 0x22)
    optional = 152
    struct.pack_into("<H", data, optional, 0x20B)
    struct.pack_into("<II", data, optional + 16, 4096, 4096)
    struct.pack_into("<Q", data, optional + 24, 0x140000000)
    struct.pack_into("<II", data, optional + 32, 4096, 512)
    struct.pack_into("<II", data, optional + 56, 8192, 512)
    struct.pack_into("<I", data, optional + 108, 16)
    section = optional + 240
    data[section:section + 8] = b".text\0\0\0"
    struct.pack_into("<IIII", data, section + 8, 3, 4096, 512, 512)
    struct.pack_into("<I", data, section + 36, 0x60000020)
    data[512:515] = b"\x31\xc0\xc3"
    if with_symbols:
        data.extend(bytes(18) + struct.pack("<I", 4))
    return data


def executable_with_dwarf():
    data = executable(False)
    struct.pack_into("<H", data, 134, 2)
    struct.pack_into("<II", data, 140, 1536, 1)
    struct.pack_into("<I", data, 152 + 56, 12288)
    section = 152 + 240 + 40
    data[section:section + 8] = b"/4\0\0\0\0\0\0"
    struct.pack_into("<IIII", data, section + 8, 3, 8192, 512, 1024)
    struct.pack_into("<I", data, section + 36, 0x42000040)
    data.extend(b"dbg" + bytes(509) + bytes(18))
    name = b".debug_info\0"
    data.extend(struct.pack("<I", 4 + len(name)) + name)
    return data


class WindowsBinaryTests(unittest.TestCase):
    def inspect(self, data):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fixture.exe"
            path.write_bytes(data)
            return MODULE.inspect_image(path)

    def test_file_only_symbols_can_be_removed_without_changing_runtime_image(self):
        before = self.inspect(executable(True))
        after = self.inspect(executable(False))
        MODULE.verify_stripped(before, after)
        self.assertGreater(before["bytes"], after["bytes"])

    def test_arm64_pe_is_accepted_and_must_keep_its_machine_type(self):
        before = self.inspect(executable(True, machine=0xAA64))
        after = self.inspect(executable(False, machine=0xAA64))
        MODULE.verify_stripped(before, after)
        with self.assertRaisesRegex(ValueError, "runtime image"):
            MODULE.verify_stripped(before, self.inspect(executable(False)))

    def test_code_entry_point_imports_and_mitigations_must_not_change(self):
        before = self.inspect(executable(True))
        for offset in (512, 152 + 16, 152 + 112 + 8, 152 + 70):
            with self.subTest(offset=offset):
                changed = executable(False)
                changed[offset] ^= 1
                with self.assertRaisesRegex(ValueError, "runtime image"):
                    MODULE.verify_stripped(before, self.inspect(changed))

    def test_symbols_left_in_output_are_rejected(self):
        before = self.inspect(executable(True))
        with self.assertRaisesRegex(ValueError, "symbols remain"):
            MODULE.verify_stripped(before, copy.deepcopy(before))

    def test_import_and_crt_write_permissions_must_not_change(self):
        section = 152 + 240 + 40
        for name in (b".idata", b".CRT"):
            with self.subTest(section=name):
                original = executable_with_dwarf()
                original[section:section + 8] = name.ljust(8, b"\0")
                struct.pack_into("<I", original, section + 36, 0xC0000040)
                stripped = original[:1536]
                struct.pack_into("<II", stripped, 140, 0, 0)
                before = self.inspect(original)
                MODULE.verify_stripped(before, self.inspect(stripped))
                struct.pack_into("<I", stripped, section + 36, 0x40000040)
                with self.assertRaisesRegex(ValueError, "runtime image"):
                    MODULE.verify_stripped(before, self.inspect(stripped))

    def test_known_write_loss_is_restored_and_other_changes_are_rejected(self):
        section = 152 + 240 + 40
        for name in (b".idata", b".CRT"):
            original = executable_with_dwarf()
            original[section:section + 8] = name.ljust(8, b"\0")
            struct.pack_into("<I", original, section + 36, 0xC0000040)
            stripped = original[:1536]
            struct.pack_into("<II", stripped, 140, 0, 0)
            struct.pack_into("<I", stripped, section + 36, 0x40000040)
            before = self.inspect(original)
            for extra_change in (None, 512, 152 + 16, section + 36):
                with self.subTest(section=name, extra_change=extra_change), tempfile.TemporaryDirectory() as directory:
                    candidate = bytearray(stripped)
                    if extra_change is not None:
                        candidate[extra_change] ^= 1
                    path = Path(directory) / "stripped.exe"
                    path.write_bytes(candidate)
                    after = MODULE.inspect_image(path)
                    if extra_change is None:
                        self.assertEqual(MODULE.restore_mingw_write_permissions(path, before, after),
                                         [name.decode("ascii")])
                        MODULE.verify_stripped(before, MODULE.inspect_image(path))
                    else:
                        with self.assertRaisesRegex(ValueError, "runtime image"):
                            MODULE.restore_mingw_write_permissions(path, before, after)
                        self.assertEqual(path.read_bytes(), candidate)

    def test_permission_restoration_refuses_changed_staged_file(self):
        original = executable_with_dwarf()
        section = 152 + 240 + 40
        original[section:section + 8] = b".idata\0\0"
        struct.pack_into("<I", original, section + 36, 0xC0000040)
        stripped = original[:1536]
        struct.pack_into("<II", stripped, 140, 0, 0)
        struct.pack_into("<I", stripped, section + 36, 0x40000040)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "stripped.exe"
            path.write_bytes(stripped)
            after = MODULE.inspect_image(path)
            stripped[512] ^= 1
            path.write_bytes(stripped)
            with self.assertRaisesRegex(ValueError, "changed before"):
                MODULE.restore_mingw_write_permissions(path, self.inspect(original), after)
            self.assertEqual(path.read_bytes(), stripped)

    @unittest.skipUnless(sys.platform == "win32", "requires the native Windows PE checksum API")
    def test_permission_restoration_checksum_matches_windows_for_even_and_odd_lengths(self):
        import ctypes
        checksum = ctypes.WinDLL("imagehlp").CheckSumMappedFile
        checksum.argtypes = [ctypes.c_void_p, ctypes.c_uint32,
                             ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint32)]
        checksum.restype = ctypes.c_void_p
        original = executable_with_dwarf()
        section = 152 + 240 + 40
        original[section:section + 8] = b".idata\0\0"
        struct.pack_into("<I", original, section + 36, 0xC0000040)
        for overlay in (b"", b"\x81", b"\x81\xff\x17"):
            with self.subTest(overlay=overlay), tempfile.TemporaryDirectory() as directory:
                stripped = original[:1536] + overlay
                struct.pack_into("<II", stripped, 140, 0, 0)
                struct.pack_into("<I", stripped, section + 36, 0x40000040)
                path = Path(directory) / "stripped.exe"
                path.write_bytes(stripped)
                MODULE.restore_mingw_write_permissions(path, self.inspect(original), MODULE.inspect_image(path))
                data = bytearray(path.read_bytes())
                buffer = (ctypes.c_ubyte * len(data)).from_buffer(data)
                header, calculated = ctypes.c_uint32(), ctypes.c_uint32()
                self.assertTrue(checksum(buffer, len(data), ctypes.byref(header), ctypes.byref(calculated)))
                self.assertEqual(header.value, calculated.value)

    def test_discardable_dwarf_can_shrink_image_capacity(self):
        before = self.inspect(executable_with_dwarf())
        after = self.inspect(executable(False))
        MODULE.verify_stripped(before, after)

    def test_executable_debug_named_section_must_not_be_removed(self):
        data = executable_with_dwarf()
        struct.pack_into("<I", data, 152 + 240 + 40 + 36, 0x62000040)
        with self.assertRaisesRegex(ValueError, "runtime image"):
            MODULE.verify_stripped(self.inspect(data), self.inspect(executable(False)))

    def test_runtime_references_to_discardable_debug_are_rejected(self):
        for offset in (152 + 16, 152 + 112 + 8):
            with self.subTest(offset=offset):
                data = executable_with_dwarf()
                struct.pack_into("<I", data, offset, 8192)
                with self.assertRaisesRegex(ValueError, "references a debug section"):
                    self.inspect(data)

    def test_invalid_image_and_header_capacities_are_rejected(self):
        for offset in (152 + 56, 152 + 60):
            with self.subTest(offset=offset):
                data = executable_with_dwarf()
                struct.pack_into("<I", data, offset, 512)
                if offset == 152 + 60:
                    struct.pack_into("<I", data, offset, 4096)
                with self.assertRaisesRegex(ValueError, "capacity"):
                    self.inspect(data)

    def test_truncated_section_or_signed_input_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "truncated section contents"):
            self.inspect(executable(False)[:-1])
        signed = executable(False)
        struct.pack_into("<II", signed, 152 + 112 + 4 * 8, 1024, 8)
        with self.assertRaisesRegex(ValueError, "before signing"):
            self.inspect(signed)


if __name__ == "__main__":
    unittest.main()
