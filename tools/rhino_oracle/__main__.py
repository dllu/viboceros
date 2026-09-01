"""Command-line entry point for the Rhino oracle Python API."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from .client import OracleClient, OracleError, load_request


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare Viboceros geometry operations with Rhino 8"
    )
    parser.add_argument("mode", choices=("compare", "viboceros", "rhino"))
    parser.add_argument("request", type=Path)
    parser.add_argument("--timeout", type=float, default=180.0)
    parser.add_argument("--absolute-epsilon", type=float, default=1.0e-10)
    parser.add_argument("--relative-epsilon", type=float, default=1.0e-10)
    parser.add_argument("--launcher", type=Path)
    parser.add_argument("--repo-root", type=Path)
    arguments = parser.parse_args()

    try:
        request = load_request(arguments.request)
        client = OracleClient(arguments.repo_root, arguments.launcher)
        if arguments.mode == "viboceros":
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

    print(json.dumps(output, indent=2, sort_keys=True, allow_nan=False))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
