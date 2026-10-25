"""Build an offline legacy-named ZIP containing the unchanged native MSVC EXE.

Only transport metadata changes. No executable launcher, extraction-on-start,
network request, installation, or publication is performed by this tool.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import tempfile
import zipfile


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


SCRIPTS = Path(__file__).parent
bundle = load_module("direct_package_validation", SCRIPTS / "embedded-msvc-bundle.py")
legacy = load_module("direct_legacy_contract", SCRIPTS / "build-windows-migration-bootstrap.py")
LEGACY_FILES = legacy.LEGACY_FILES


def validate_direct_archive(archive, version):
    result = bundle._validate_package_archive(archive, version, direct=True)
    if set(result["hashes"]) != set(LEGACY_FILES):
        raise ValueError("direct transport must retain exact legacy file set")
    with zipfile.ZipFile(archive) as zipped:
        entry = zipped.getinfo(result["root"] + "/grok-zh.exe")
        with zipped.open(entry) as executable:
            # Refuse the known self-extracting format, even if mislabeled.
            if entry.file_size >= bundle.FOOTER.size:
                executable.seek(entry.file_size - bundle.FOOTER.size)
                if executable.read(16) == bundle.MAGIC:
                    raise ValueError("direct payload must not be an embedded launcher")
    return result


def build_package(output, archive, version):
    output, archive = Path(output), Path(archive)
    native = bundle.validate_native_archive(archive, version)
    if not set(LEGACY_FILES) <= native["hashes"].keys():
        raise ValueError("native archive missing legacy transport support files")
    # Reject files that an EXE-only update cannot deploy. The native package
    # currently uses exactly this set; new runtime dependencies need review.
    if set(native["hashes"]) != set(LEGACY_FILES):
        raise ValueError("native package adds files unsupported by legacy EXE-only update")
    if output.exists():
        raise FileExistsError("refusing to overwrite an existing output directory")
    output.parent.mkdir(parents=True, exist_ok=True)
    name = f"grok-zh-{version}-windows-x86_64-gnu"
    # Stage adjacent to output, validate the resulting archive, then atomically
    # expose the completed directory. Failed validation leaves no partial output.
    with tempfile.TemporaryDirectory(prefix=".direct-msvc-", dir=output.parent) as temporary:
        stage = Path(temporary) / "ready"
        package = stage / name
        package.mkdir(parents=True)
        copied_total = 0
        with zipfile.ZipFile(archive) as zipped:
            for member in LEGACY_FILES:
                target = package / member
                target.parent.mkdir(parents=True, exist_ok=True)
                with zipped.open(native["root"] + "/" + member) as source, target.open("xb") as destination:
                    digest, count = hashlib.sha256(), 0
                    while chunk := source.read(bundle.CHUNK):
                        count += len(chunk)
                        copied_total += len(chunk)
                        if count > bundle.MAX_ARCHIVE_BYTES or copied_total > bundle.MAX_EXPANDED_BYTES:
                            raise ValueError("source archive changed beyond validated size bounds")
                        digest.update(chunk)
                        destination.write(chunk)
                    if digest.hexdigest() != native["hashes"][member]:
                        raise ValueError("source archive changed after validation")
        info = (package / "BUILD-INFO.txt").read_text(encoding="utf-8-sig").replace("\r\n", "\n")
        before = info.split("GROK-UPDATE-PROTOCOL-BEGIN", 1)[0].rstrip()
        before = re.sub(r"^Target:.*$", "Target: x86_64-pc-windows-gnu", before, flags=re.M)
        before += "\nRuntime-Target: x86_64-pc-windows-msvc\nRole: legacy GNU transport; unchanged native MSVC executable\n"
        protocol = dict(native["protocol"], platform="x86_64-pc-windows-gnu",
                        direct_runtime=dict(schema=1, version=version, platform=bundle.TARGET))
        (package / "BUILD-INFO.txt").write_text(before + "GROK-UPDATE-PROTOCOL-BEGIN\n" +
            json.dumps(protocol, indent=2) + "\nGROK-UPDATE-PROTOCOL-END\n", encoding="utf-8", newline="\n")
        (package / "SHA256SUMS.txt").write_text("".join(
            f"{bundle.digest_file(package / member)}  {member}\n" for member in sorted(LEGACY_FILES)),
            encoding="utf-8", newline="\n")
        result = stage / (name + ".zip")
        with zipfile.ZipFile(result, "x", compression=zipfile.ZIP_DEFLATED) as zipped:
            for member in sorted((*LEGACY_FILES, "SHA256SUMS.txt")):
                entry = zipfile.ZipInfo(name + "/" + member, date_time=(2026, 1, 1, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = 0o100644 << 16
                with (package / member).open("rb") as source, zipped.open(entry, "w", force_zip64=True) as destination:
                    shutil.copyfileobj(source, destination, bundle.CHUNK)
        checked = validate_direct_archive(result, version)
        if checked["hashes"]["grok-zh.exe"] != native["hashes"]["grok-zh.exe"]:
            raise ValueError("direct package must preserve exact native executable")
        result.with_suffix(".zip.sha256").write_text(
            f"{bundle.digest_file(result)}  {result.name}\n", encoding="ascii")
        # rename cannot replace a nonempty directory; explicitly guard empty
        # output too, retaining the no-overwrite contract.
        if output.exists():
            raise FileExistsError("output appeared during packaging")
        stage.rename(output)
    return output / (name + ".zip")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--payload-archive", type=Path, required=True)
    args = parser.parse_args()
    result = build_package(args.output, args.payload_archive, args.version)
    print(json.dumps(dict(archive=str(result), sha256=bundle.digest_file(result)), indent=2))


if __name__ == "__main__":
    main()
