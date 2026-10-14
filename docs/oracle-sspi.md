# Wine security DLL diagnostics

If Rhino startup reports that `AcquireCredentialsHandleW` is missing from
`sspicli.dll`, check the Windows loader separately from Rhino:

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh exec \
  python3 -m tools.rhino_oracle.wine_sspi --output /tmp/sspi-x64.json
```

The tool builds a small Windows executable with LLVM-MinGW and runs it through
the configured Wine launcher in the private Xvfb display. It checks both DLLs
with public `LoadLibrary` and `GetProcAddress` calls, records their loader errors,
and releases each loaded module. It resolves three SSPI entry points without
calling them. A successful result establishes symbol availability; it does not
establish successful authentication or a working license-server connection.

Supply `--compiler /path/to/x86_64-w64-mingw32-clang` if the compiler is outside
PATH and the local `~/wines/build/toolchains/llvm-mingw-*` installations.
`--launcher` selects a Wine launcher with the existing `wine` subcommand.
The compiled PE machine type must match the requested architecture. Builds and
execution have a bounded `--timeout`, defaulting to 30 seconds each, and their
temporary files are removed. Exit codes are 0 for resolved symbols, 1 for an
observed loader/resolution failure, and 2 for a build or diagnostic error.

For an ARM64 Wine installation, compare the native executable path:

```sh
WINEDLLOVERRIDES='sspicli,secur32,schannel=b' \
  tools/rhino_oracle/run_headless.sh exec \
  python3 -m tools.rhino_oracle.wine_sspi --architecture aarch64 \
  --output /tmp/sspi-arm64.json
```

The 2026-10-14 local x64 probe loads `sspicli.dll`, but all three forwarded
symbols fail with error 127. Loading `secur32.dll` separately fails with error
998. A module/exception trace shows an access violation during its provider
initialization. The native ARM64 executable resolves all six symbols under the
same built-in DLL override. This comparison isolates an architecture-dependent
Wine initialization failure; it does not identify a particular Wine/FEX source
defect. Neither the existing override nor a temporary overlay of Wine's x64
security DLLs repaired the x64 probe. The diagnostic uses process-local overrides
and temporary files; it does not install DLLs or update Wine registry settings.

The initial isolated Project run timed out before its worker started. The
second run with built-in security DLLs showed the same startup problem and was
terminated after diagnosing it. These runs produced no fresh geometry
observations. Loose projection still needs reference behavior for trimmed and
multiple-hit targets before its implementation can be qualified.

Retained diagnostic responses and source hashes are in the
[qualification record](oracle-sspi-qualification.json). Tests cover incomplete
and inconsistent responses, PE architecture mismatch, the private-display
boundary, and argument paths containing spaces:

```sh
python3 -m unittest tools.rhino_oracle.test_wine_sspi
```

Public API references: [GetProcAddress](https://learn.microsoft.com/en-us/windows/win32/api/libloaderapi/nf-libloaderapi-getprocaddress)
and [Windows loader error codes](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--500-999-).
