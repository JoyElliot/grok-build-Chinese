"""TEST ONLY: compile the real C launcher with an offline HttpClient transport.

The production builder has no runtime alternate-URL, script or transport hook.
This test injects a fake handler into a temporary compiled fixture only.
"""
import argparse
import importlib.util
import json
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("migration_builder", SCRIPTS / "build-windows-migration-bootstrap.py")
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)

TRANSPORT = r'''
# TEST FIXTURE ONLY; never emitted by the production build command.
Add-Type -AssemblyName System.Net.Http
if (!('GrokBootstrapFixtureHandler' -as [type])) {
    Add-Type -ReferencedAssemblies System.Net.Http -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
public sealed class GrokBootstrapFixtureHandler : HttpMessageHandler {
    public readonly Dictionary<string,string> Files=new Dictionary<string,string>();
    public string Log;
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request,CancellationToken token) {
        string url=request.RequestUri.AbsoluteUri;
        File.AppendAllText(Log,url+Environment.NewLine);
        var response=new HttpResponseMessage(Files.ContainsKey(url)?HttpStatusCode.OK:HttpStatusCode.NotFound);
        response.Content=Files.ContainsKey(url)?new StreamContent(File.OpenRead(Files[url])):(HttpContent)new ByteArrayContent(new byte[0]);
        return Task.FromResult(response);
    }
}
'@
}
function New-OnlineHttpClient {
    $config=Get-Content -LiteralPath MANIFEST_PATH -Raw -Encoding UTF8 | ConvertFrom-Json
    $handler=[GrokBootstrapFixtureHandler]::new()
    $handler.Log=$config.log
    foreach ($property in $config.files.PSObject.Properties) { $handler.Files[$property.Name]=[string]$property.Value }
    return [Net.Http.HttpClient]::new($handler)
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--pin", required=True, type=Path)
    parser.add_argument("--transport", required=True, type=Path)
    parser.add_argument("--version", default="1.0.99")
    parser.add_argument("--cc", required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    source_reader = builder.script_bytes
    transport = TRANSPORT.replace("MANIFEST_PATH", "'" + str(args.transport.resolve()).replace("'", "''") + "'")

    def fixture_source(path):
        original = source_reader(path)
        return original + transport.encode("utf-8") if path.name == "Install-GrokZhOnline.ps1" else original

    builder.script_bytes = fixture_source
    builder.build_launcher(args.output, args.version, json.loads(args.pin.read_text(encoding="utf-8")), args.cc)


if __name__ == "__main__":
    main()
