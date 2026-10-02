"""Capture native hole commands using independently exported owned sources."""
import argparse
import json

from .client import OracleClient, _owned_artifact_request, load_request
from .untrim_holes_probe import validate


def capture(request, client=None, timeout=300):
    if (type(request.get("protocol_version")) is not int or request["protocol_version"] != 1
            or type(request.get("iterations", 1)) is not int or request.get("iterations", 1) != 1
            or not isinstance(request.get("operations"), list)
            or not 1 <= len(request["operations"]) <= 128):
        raise ValueError("hole command capture needs protocol 1, one iteration and 1 to 128 cases")
    client = client or OracleClient()
    with _owned_artifact_request(request) as prepared:
        exports = []
        identifiers = set()
        for index, operation in enumerate(prepared["operations"]):
            validate(operation)
            if operation["id"] in identifiers: raise ValueError("duplicate hole command capture id")
            identifiers.add(operation["id"])
            for part, source in enumerate(operation["sources"]):
                # The empty-selection geometry API exports and validates its
                # exact source before returning no edited result. No command
                # result or Rhino observation is input to this construction.
                exports.append(dict(op="brep_remove_holes", id="source-%d-%d" % (index, part),
                    source=source["brep"], loops=[]))
        export_request = dict(protocol_version=1, iterations=1, operations=exports)
        if "tolerance" in prepared: export_request["tolerance"] = prepared["tolerance"]
        client.run_viboceros(export_request, timeout)
        return client.run_rhino(prepared, timeout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request")
    parser.add_argument("--timeout", type=float, default=300)
    arguments = parser.parse_args()
    print(json.dumps(capture(load_request(arguments.request), timeout=arguments.timeout),
        indent=2, sort_keys=True, allow_nan=False))


if __name__ == "__main__": main()
