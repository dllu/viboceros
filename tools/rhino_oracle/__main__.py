"""Command-line entry point for the Rhino oracle Python API."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys

from .client import OracleClient, OracleError, load_request


def headless_wrapper_argv(mode: str, argv: list[str], environ: dict[str, str],
                          platform: str) -> list[str] | None:
    if (mode not in ("rhino", "compare") or not platform.startswith("linux")
            or (environ.get("DISPLAY")
                and environ.get("VIBOCEROS_ORACLE_HEADLESS") == environ.get("DISPLAY"))):
        return None
    return [str(Path(__file__).with_name("run_headless.sh")), *argv]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare Viboceros geometry operations with Rhino 8"
    )
    parser.add_argument("mode", choices=("compare", "viboceros", "rhino", "audit", "replay"))
    parser.add_argument("request", type=Path)
    parser.add_argument("--observations", type=Path, help="saved raw Rhino response for replay")
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--absolute-epsilon", type=float, default=1.0e-10)
    parser.add_argument("--relative-epsilon", type=float, default=1.0e-10)
    parser.add_argument("--launcher", type=Path)
    parser.add_argument("--repo-root", type=Path)
    parser.add_argument("--scheme", help="private Rhino settings scheme (VibocerosOracle...)")
    parser.add_argument("--output", type=Path, help="write the JSON response to this file")
    arguments = parser.parse_args()
    if (arguments.mode == "replay") != (arguments.observations is not None):
        parser.error("--observations is required for replay and is not used by other modes")

    # Live probes must not open Rhino on the caller's desktop display. The
    # wrapper starts a dedicated Xvfb server and marks its child to avoid a
    # recursive exec.
    wrapper_argv = headless_wrapper_argv(arguments.mode, sys.argv[1:], os.environ, sys.platform)
    if wrapper_argv is not None:
        os.execv(wrapper_argv[0], wrapper_argv)

    try:
        request = load_request(arguments.request)
        client = OracleClient(arguments.repo_root, arguments.launcher,
                              settings_scheme=arguments.scheme)
        if arguments.mode == "audit":
            output = client.run_viboceros_audit(request, arguments.timeout)
            passed = all(o["status"] == "success" for o in output["outcomes"])
        elif arguments.mode == "replay":
            observation = load_request(arguments.observations)
            report = client.replay(request, observation, arguments.absolute_epsilon,
                                  arguments.relative_epsilon, arguments.timeout)
            output = report.as_dict()
            passed = report.passed
        elif arguments.mode == "viboceros":
            output = client.run_viboceros(request, arguments.timeout)
            passed = True
        elif arguments.mode == "rhino":
            output = client.run_rhino(request, arguments.timeout)
            passed = True
        else:
            report = client.compare(
                request,
                absolute_epsilon=arguments.absolute_epsilon,
                relative_epsilon=arguments.relative_epsilon,
                timeout=arguments.timeout,
            )
            output = report.as_dict()
            passed = report.passed
    except (OSError, ValueError, OracleError) as error:
        parser.exit(2, f"oracle failed: {error}\n")

    serialized = json.dumps(output, indent=2, sort_keys=True, allow_nan=False)
    if arguments.output is not None:
        try:
            arguments.output.write_text(serialized + "\n", encoding="utf-8")
        except OSError as error:
            parser.exit(2, f"could not write oracle response: {error}\n")
    else:
        print(serialized)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
