#!/usr/bin/env python3
import importlib.util,pathlib,unittest,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3]
DIRECTORY=ROOT/'.github/scripts/transaction-acceptance'
SPEC=importlib.util.spec_from_file_location('transaction_prepare',DIRECTORY/'prepare.py')
prepare=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(prepare)
class TransactionAcceptance(unittest.TestCase):
 def test_branch_and_readonly_permissions(self):
  s=(ROOT/'.github/workflows/zh-windows-transaction-acceptance.yml').read_text()
  self.assertIn("if: github.ref == 'refs/heads/test/windows-msvc-transaction-acceptance'",s)
  self.assertIn('contents: read\n  actions: read',s)
  self.assertNotIn('write',s)
  self.assertIn('target-feature=+crt-static',s)
  self.assertIn('direct-transaction-validation/Cargo.toml',s)
  self.assertNotIn('build-msvc-x64',s)
  self.assertIn('WindowsPowerShell/v1.0/powershell.exe',s)
  self.assertIn("Join-Path $env:RUNNER_TEMP 'portable-migration-acceptance'",s)
  self.assertIn('path: transaction-evidence/',s)
 def test_pinned_inputs(self):
  self.assertEqual(prepare.RUN,40156800532);self.assertEqual(prepare.ARTIFACT,12584849702)
  self.assertEqual(prepare.SOURCE,'0eb78dd950ad376ea8a1f1df3d2e8719f14c42e7')
  self.assertEqual(prepare.PINS['product-artifact.zip'],'102a9a1e6871c8f77701d37deb596d5bacc7cc78f1cd7fbc83490b598fcc18fc')
 def test_download_url_policy(self):
  prepare.verify_url('https://github.com/test')
  for url in ['http://github.com/test','https://evil.example/x','https://github.com:8080/x']:
   with self.assertRaises(ValueError):prepare.verify_url(url)
 def test_local_only_runner(self):
  s=(DIRECTORY/'Run-MigrationTransaction.ps1').read_text(encoding='utf-8-sig')
  self.assertIn('EnvironmentVariables.Clear()',s)
  self.assertIn('Get-FileHash',s);self.assertIn('Stop-TestTree',s)
  for name in ['historical-gnu.zip','native-msvc.zip','direct-msvc.zip']:self.assertIn(prepare.PINS[name],s)
  self.assertIn('$killer.ExitCode -ne 0',s)
  self.assertIn("$evidence.status -cne 'PASSED'",s)
  self.assertIn("@('direct-reinstall','activation-rollback')",s)
  self.assertIn('WaitForExit($TimeoutSeconds * 1000)',s)
  self.assertIn("'legacy-transaction' 'community_release::transaction:: --test-threads=1 --nocapture'",s)
  self.assertIn("'current-transaction' 'community_release::transaction:: --test-threads=1 --nocapture'",s)
  for forbidden in ['Invoke-WebRequest','Invoke-RestMethod','Set-ExecutionPolicy','-ExecutionPolicy','cargo ','python ','setx ','update --']:
   self.assertNotIn(forbidden,s)
 def test_non_pe_test_binary_rejected(self):
  with tempfile.TemporaryDirectory() as directory:
   p=pathlib.Path(directory)/"fake.exe";p.write_bytes(b"not an executable")
   with self.assertRaises(ValueError):prepare.check_portable_pe(p)
 def test_honest_limitations(self):
  s=(DIRECTORY/'Windows迁移验收说明.txt').read_text(encoding='utf-8-sig')
  self.assertIn('同版本重装',s);self.assertIn('模拟传输',s);self.assertIn('不是未修改旧 GNU EXE 在线',s)
if __name__=='__main__':unittest.main()
