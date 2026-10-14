"""Diagnose Wine security DLL loading through public Windows APIs."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

from .client import OracleClient, OracleError, _run_logged, posix_to_wine_path

LIBRARIES = ("sspicli.dll", "secur32.dll")
SYMBOLS = ("AcquireCredentialsHandleW", "FreeCredentialsHandle", "InitializeSecurityContextW")
ARCHITECTURES = ("x86_64", "aarch64")
PREFIX = "VIBOCEROS_SSPI "


def parse_response(output: str, returncode: int) -> dict:
    lines = [line[len(PREFIX):] for line in output.splitlines() if line.startswith(PREFIX)]
    if len(lines) != 1:
        raise OracleError("SSPI probe did not publish one diagnostic record")
    try:
        payload = json.loads(lines[0])
        rows = payload["libraries"]
        if set(payload) != {"libraries"} or len(rows) != len(LIBRARIES):
            raise ValueError("invalid library records")
        for row, name in zip(rows, LIBRARIES):
            if (set(row) != {"name", "loaded", "error", "symbols"} or row["name"] != name
                    or type(row["loaded"]) is not bool or len(row["symbols"]) != len(SYMBOLS)):
                raise ValueError("invalid library record")
            checks = [(row["error"], row["loaded"])]
            for symbol, expected in zip(row["symbols"], SYMBOLS):
                if (set(symbol) != {"name", "attempted", "resolved", "error"}
                        or symbol["name"] != expected
                        or type(symbol["attempted"]) is not bool
                        or type(symbol["resolved"]) is not bool
                        or symbol["attempted"] != row["loaded"]
                        or symbol["resolved"] and not symbol["attempted"]):
                    raise ValueError("invalid symbol record")
                checks.append((symbol["error"], symbol["resolved"] or not symbol["attempted"]))
            if any(type(error) is not int or not 0 <= error <= 0xffffffff
                   or success and error != 0 for error, success in checks):
                raise ValueError("invalid loader error")
    except (KeyError, TypeError, ValueError) as error:
        raise OracleError("malformed SSPI diagnostic record") from error
    passed = all(row["loaded"] and all(s["resolved"] for s in row["symbols"]) for row in rows)
    if returncode != (0 if passed else 1):
        raise OracleError("SSPI probe exit status disagrees with its diagnostic record")
    return dict(passed=passed, **payload)


def find_compiler(architecture: str) -> Path:
    name = architecture + "-w64-mingw32-clang"
    installed = shutil.which(name)
    if installed:
        return Path(installed)
    root = Path(os.environ.get("WINES_ROOT", Path.home() / "wines"))
    candidates = sorted((root / "build/toolchains").glob("llvm-mingw-*/bin/" + name), reverse=True)
    for candidate in candidates:
        if os.access(candidate, os.X_OK):
            return candidate
    raise OracleError("SSPI probe needs an LLVM-MinGW compiler; supply --compiler")


def validate_executable(path: Path, architecture: str) -> None:
    data = path.read_bytes()
    offset = int.from_bytes(data[60:64], "little") if len(data) >= 64 else -1
    expected = {"x86_64": 0x8664, "aarch64": 0xaa64}[architecture]
    if (data[:2] != b"MZ" or offset < 64 or offset + 6 > len(data)
            or data[offset:offset + 4] != b"PE\0\0"
            or struct.unpack_from("<H", data, offset + 4)[0] != expected):
        raise OracleError("SSPI diagnostic executable does not match the requested architecture")


def run(*, architecture: str = "x86_64", compiler: Path | None = None,
        launcher: Path | None = None, timeout: float = 30.0) -> dict:
    if architecture not in ARCHITECTURES or not math.isfinite(timeout) or timeout <= 0:
        raise OracleError("invalid SSPI probe architecture or timeout")
    # The Wine launcher can start prefix services with windows. Use the same
    # private display constraint as the live oracle, before building or running.
    if sys.platform.startswith("linux") and (not os.environ.get("DISPLAY")
            or os.environ.get("VIBOCEROS_ORACLE_HEADLESS") != os.environ.get("DISPLAY")):
        raise OracleError("SSPI diagnostics require dedicated Xvfb; use run_headless.sh exec")
    client = OracleClient(launcher=launcher)
    compiler = Path(compiler) if compiler is not None else find_compiler(architecture)
    source = Path(__file__).with_name("wine_sspi_probe.c")
    with tempfile.TemporaryDirectory(prefix="viboceros-wine-sspi-") as temporary:
        root = Path(temporary)
        executable = root / "probe.exe"
        try:
            build = subprocess.run([str(compiler), "-O2", "-Wall", "-Wextra", "-Werror",
                                    str(source), "-o", str(executable)],
                                   capture_output=True, text=True, timeout=timeout, check=False)
        except (OSError, subprocess.TimeoutExpired) as error:
            raise OracleError("could not compile the SSPI diagnostic") from error
        if build.returncode or not executable.is_file():
            raise OracleError("SSPI diagnostic compilation failed: " + build.stderr.strip())
        validate_executable(executable, architecture)
        completed = _run_logged([str(client.launcher), "wine", posix_to_wine_path(executable)],
                                client.repo_root, timeout, root / "wine.log")
        response = parse_response(completed.stdout, completed.returncode)
    return dict(schema_version=1, architecture=architecture, compiler=str(compiler),
                launcher=str(client.launcher),
                source_sha256={str(source.relative_to(client.repo_root)):
                               hashlib.sha256(source.read_bytes()).hexdigest()},
                **response)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--architecture", choices=ARCHITECTURES, default="x86_64")
    parser.add_argument("--compiler", type=Path)
    parser.add_argument("--launcher", type=Path)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        response = run(architecture=args.architecture, compiler=args.compiler,
                       launcher=args.launcher, timeout=args.timeout)
        serialized = json.dumps(response, indent=2, sort_keys=True) + "\n"
        if args.output is None:
            print(serialized, end="")
        else:
            args.output.write_text(serialized)
    except (OSError, OracleError) as error:
        parser.exit(2, f"SSPI diagnostic failed: {error}\n")
    return 0 if response["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
