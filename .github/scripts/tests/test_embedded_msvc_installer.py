"""Source-contract tests; Windows PowerShell behavior is exercised separately.

These checks are intentionally not represented as PS5.1/7 execution evidence.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[3]
WINDOWS = ROOT / "packaging/windows"
FULL = (WINDOWS / "Install-GrokZh.ps1").read_text(encoding="utf-8-sig")
ONLINE = (WINDOWS / "Install-GrokZhOnline.ps1").read_text(encoding="utf-8-sig")
FIXTURE = (WINDOWS / "tests/Test-EmbeddedMsvcInstaller.ps1").read_text(encoding="utf-8-sig")
INSTALL_FIXTURE = (WINDOWS / "tests/Test-Install-GrokZh.ps1").read_text(encoding="utf-8-sig")


def function(source, name):
    match = re.search(rf"^function {re.escape(name)} \{{\n(.*?)(?=^function |\Z)", source, re.M | re.S)
    if not match:
        raise AssertionError(f"missing function {name}")
    return match.group(1)


class EmbeddedInstallerContractTests(unittest.TestCase):
    def test_windows_scripts_preserve_ps51_utf8_bom(self):
        for name in ("Install-GrokZh.ps1", "Install-GrokZhOnline.ps1", "tests/Test-EmbeddedMsvcInstaller.ps1"):
            self.assertTrue((WINDOWS / name).read_bytes().startswith(b"\xef\xbb\xbf"), name)

    def test_library_only_returns_before_any_defaults_or_install_action(self):
        guard = FULL.index("if ($LibraryOnly) { return }")
        self.assertLess(guard, FULL.index("if ([string]::IsNullOrWhiteSpace($PackageDir))"))
        self.assertLess(guard, FULL.index("$PackageDir = Resolve-FullPath $PackageDir"))
        self.assertIn("[switch]$LibraryOnly", FULL)
        self.assertIn("[switch]$ExtractOnly", FULL)

    def test_canonical_offline_extractor_has_no_network_path(self):
        for name in ("Expand-GrokEmbeddedArchive", "Expand-GrokEmbeddedPackage", "Expand-VerifiedGrokBundlePackage"):
            body = function(FULL, name)
            for forbidden in ("Invoke-WebRequest", "HttpClient", "Receive-OnlineFile", "Get-ExactOnlineRelease", "Start-Process"):
                self.assertNotIn(forbidden, body)
        self.assertIn("Read-AndVerifyManifest $Package", function(FULL, "Expand-GrokEmbeddedPackage"))
        self.assertIn("Assert-GrokPe64Prefix -Path (Join-Path $package 'grok-zh.exe')", function(FULL, "Expand-VerifiedGrokBundlePackage"))

    def test_footer_bytes_pins_bounds_and_source_lock(self):
        body = function(FULL, "Expand-GrokEmbeddedArchive")
        for required in ("ReadBytes(16)", "ReadUInt64()", "ReadBytes(32)", "GROKMSVCBUNDLE01", "$offset -lt 256", "$input.Length -gt 536870912", "$ExpectedSha256", "TransformFinalBlock", "'Open', 'Read', 'Read'", "Assert-GrokFreeSpace"):
            self.assertIn(required, body)
        self.assertIn("$size -ne [uint64]($input.Length - 64) - $offset", body)

    def test_same_pe_header_validation_for_outer_and_inner(self):
        body = function(FULL, "Assert-GrokPe64Prefix")
        for required in ("0x5a4d", "0x8664", "0x20b", "$optionalSize -lt 112", "$sections -lt 1", "40L * $sections", "$peOffset -lt 64"):
            self.assertIn(required, body)
        self.assertIn("Assert-GrokPe64Prefix -Path $Launcher", function(FULL, "Expand-GrokEmbeddedArchive"))

    def test_online_version_selection_precedes_asset_selection(self):
        body = function(ONLINE, "Get-LatestOnlineRelease")
        self.assertIn("Get-OnlineReleaseIdentity $release", body)
        self.assertIn("return Get-OnlineReleaseContract $bestRelease", body)
        self.assertNotIn("$candidate = Get-OnlineReleaseContract $release", body)
        contract = function(ONLINE, "Get-OnlineReleaseContract")
        self.assertIn("@('windows-x86_64-msvc', 'windows-x86_64-gnu')", contract)
        self.assertIn("RequireEmbedded", contract)

    def test_online_extracts_before_executing_candidate_and_requires_exact_downgrade(self):
        body = function(ONLINE, "Invoke-GrokZhOnline")
        self.assertLess(body.index("Expand-VerifiedOnlinePackage"), body.index("-ExtractOnly -ExtractionDirectory"))
        self.assertLess(body.index("-ExtractOnly -ExtractionDirectory"), body.index("$candidate = Get-OnlineExecutableVersion"))
        self.assertIn("if ($AllowDowngrade -and !$Version)", body)
        self.assertIn("if (!$AllowDowngrade -or !$Version)", body)
        self.assertIn("elseif ($release.RequireEmbedded)", body)

    def test_historical_installer_does_not_receive_unsupported_switch(self):
        body = function(ONLINE, "Invoke-OnlinePackageInstaller")
        self.assertIn("Language.Parser]::ParseFile", body)
        self.assertIn("UserPath -ceq 'AllowDowngrade'", body)
        self.assertIn("$arguments += '-OverrideOfficialCommands'", body)

    def test_historical_fixture_removes_one_parameter_with_either_line_ending(self):
        start = FIXTURE.index("function ConvertTo-EmbeddedHistoricalInstaller(")
        body = FIXTURE[start:FIXTURE.index("\nfunction ", start + 1)]
        pattern = re.search(r"\$parameterPattern = '([^']+)'", body).group(1)
        self.assertIn("[regex]::Matches($Source, $parameterPattern).Count -eq 1", body)
        removal = body.index("[regex]::Replace($Source, $parameterPattern, '')")
        absent = body.index("$_.Name.VariablePath.UserPath -ieq 'AllowDowngrade'")
        references = body.index("$result.Replace('$AllowDowngrade', '$true')")
        self.assertLess(removal, absent)
        self.assertLess(absent, references)
        self.assertIn(".Count -eq 0)", body[absent:references])
        self.assertIn("Parser]::ParseInput", body[removal:absent])
        self.assertIn("Parser]::ParseInput", body[references:])
        self.assertEqual(body.count("Assert-Embedded ($parseErrors.Count -eq 0)"), 2)
        transformed = []
        for newline in ("\n", "\r\n"):
            with self.subTest(newline=repr(newline)):
                source = FULL.replace("\n", newline)
                self.assertEqual(len(re.findall(pattern, source)), 1)
                historical, count = re.subn(pattern, "", source)
                self.assertEqual(count, 1)
                self.assertNotRegex(historical, r"\[switch\]\s*\$AllowDowngrade\b")
                self.assertIn("$AllowDowngrade", historical)
                historical = historical.replace("$AllowDowngrade", "$true")
                self.assertNotIn("[switch]$true", historical)
                transformed.append(historical.replace("\r\n", "\n"))
        self.assertEqual(transformed[0], transformed[1])
        self.assertIn("ConvertTo-EmbeddedHistoricalInstaller ($oldInstallerSource.Replace(\"`n\", \"`r`n\"))", FIXTURE)
        self.assertIn("$oldInstaller -ceq $oldInstallerCrlf.Replace(\"`r`n\", \"`n\")", FIXTURE)

    def test_native_schema1_accepts_manifest_resources_outer_stays_exact(self):
        body = function(FULL, "Read-AndVerifyManifest")
        self.assertIn("$nativeProtocol -and $hashes.Count -ge $completeRequired.Count", body)
        self.assertIn("if (!$nativeProtocol -and $completeRequired -cnotcontains $name)", body)
        self.assertIn("if ($manifest.Count -ne 15)", function(FULL, "Expand-GrokEmbeddedPackage"))
        self.assertNotIn("$nativeManifest.Count -ne 15", function(FULL, "Expand-VerifiedGrokBundlePackage"))

    def test_reinstall_preserves_commands_only_without_an_explicit_choice(self):
        start = FULL.index("# Preserve the user's command choice across reinstall")
        preserve = FULL[start:FULL.index("$operationParts =", start)]
        self.assertRegex(preserve, r"if \(!\$PSBoundParameters\.ContainsKey\('OverrideOfficialCommands'\) -and\s+!\$InteractiveCommandSetup\.IsPresent -and")
        self.assertIn("$existingMarker.PSObject.Properties['commands']", preserve)
        self.assertIn("@('grok', 'agent')", preserve)
        self.assertIn("$provideOfficialNames = $true", preserve)
        self.assertLess(FULL.index("$requestedOverrideOfficialCommands = $interactiveChoice.OverrideOfficial"), start)
        self.assertLess(FULL.index("$requestedUninstallOfficial = $interactiveChoice.RemoveOfficial"), start)

    def test_windows_reinstall_fixture_covers_choice_precedence_and_invariants(self):
        start = INSTALL_FIXTURE.index("# Omitting command setup preserves")
        choices = INSTALL_FIXTURE[start:INSTALL_FIXTURE.index("$whatIfInstall =", start)]
        self.assertRegex(choices, r"-NoPathUpdate -Confirm:\$false\s+& \$assertCommandChoice \$true")
        self.assertRegex(choices, r"-OverrideOfficialCommands:\$false -NoPathUpdate -Confirm:\$false\s+& \$assertCommandChoice \$false")
        self.assertRegex(choices, r"-NoPathUpdate -Confirm:\$false\s+& \$assertCommandChoice \$false")
        self.assertIn("-OverrideOfficialCommands:$false -InteractiveCommandSetup -ScriptedCommandSetupAnswers '1'", choices)
        self.assertIn("-OverrideOfficialCommands:$false -InteractiveCommandSetup -ScriptedCommandSetupAnswers '3'", choices)
        self.assertIn("$beforeCancelMarkerHash", choices)
        self.assertIn("$keptOfficialHash", choices)
        invariant_start = INSTALL_FIXTURE.index("$assertCommandChoice = {")
        invariants = INSTALL_FIXTURE[invariant_start:INSTALL_FIXTURE.index("$takeoverOutput =", invariant_start)]
        for required in ("'grok.cmd', 'agent.cmd'", "$marker.commands", "$marker.product", "$marker.install_dir", "$marker.official_command_home", "$commandChoiceDataHashes", "$commandChoiceProcessPath", "$commandChoiceUserPath"):
            self.assertIn(required, invariants)
        self.assertIn("@('auth.json', 'config.toml', 'sessions/keep.json')", INSTALL_FIXTURE)

    def test_windows_matrix_exercises_round_trip_and_recovery(self):
        for scenario in ("/platform:x64", "future-resource.dat", "1.0.43-zh.ci.12345", "1.0.43-zh.ci.12344", "-AllowDowngrade", "-WindowsX64Runtime Msvc", ".grok-zh-msvc-ready.json", "模拟激活失败", "模拟磁盘空间不足", "'magic', 'offset', 'size', 'digest', 'content', 'trailing'", "keep-auth", "keep-config"):
            self.assertIn(scenario, FIXTURE)


if __name__ == "__main__":
    unittest.main()
