"""Replay actual Volume history values, preserving display/integration residuals."""
import argparse
import copy
import json
import math
import re
from .area_centroid_probe import validate
from .area_centroid_replay import prepare as prepare_centroids
from .client import OracleClient, OracleError, OracleProtocolError, compare_responses, load_request

NUMBER = r"[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?"
VOLUME = re.compile(r"^(?:Cumulative )?Volume = ("+NUMBER+r") \(\+/- ("+NUMBER+
                    r")\) cubic millimeters(?: for [1-9][0-9]* solids)?$", re.MULTILINE)


def history_volume(value):
    display = value.get("display")
    if display != dict(precision=7, units="Millimeters"):
        raise OracleProtocolError("unsupported scalar volume display settings")
    matches = VOLUME.findall(value["history"].replace("\r\n", "\n"))
    if not value["succeeded"]:
        if matches: raise OracleProtocolError("failed Volume command reported a volume")
        return None
    if len(matches) != 1: raise OracleProtocolError("missing or ambiguous Volume result")
    volume, error = map(float, matches[0])
    if not math.isfinite(volume) or not math.isfinite(error) or error < 0:
        raise OracleProtocolError("invalid Volume result")
    # The printed uncertainty is retained in history, NOT used to widen epsilon.
    return volume


def prepare(request, observed):
    validation_request = copy.deepcopy(request)
    validation_observed = copy.deepcopy(observed)
    scalar = []
    for operation in validation_request["operations"]:
        is_scalar = operation.get("op") == "volume_command"
        validate(operation, "volume", not is_scalar)
        scalar.append(is_scalar)
        operation["op"] = "volume_centroid_command"
    if len(validation_observed["results"]) != len(scalar):
        raise OracleProtocolError("volume observation count differs")
    volumes = []
    for is_scalar, row in zip(scalar, validation_observed["results"]):
        if is_scalar:
            volumes.append(history_volume(row["value"]))
            row["value"].pop("display")
            if row["value"]["points"]: raise OracleProtocolError("scalar Volume created geometry")
        else: volumes.append(None)
    _, evidence = prepare_centroids(validation_request, validation_observed, measure="volume")
    for is_scalar, volume, row in zip(scalar, volumes, evidence["results"]):
        if is_scalar: row["value"]["volume"] = volume
    return copy.deepcopy(request), evidence


def replay(request, observed, client=None):
    native, evidence = prepare(request, observed)
    actual = (client or OracleClient()).run_viboceros(native)
    for row, expected in zip(actual["results"], evidence["results"]):
        row["value"] = {k: row["value"][k] for k in expected["value"]}
    return compare_responses(actual, evidence, absolute_epsilon=1e-9, relative_epsilon=0.)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("request"); parser.add_argument("observations")
    args = parser.parse_args()
    try:
        report = replay(load_request(args.request), load_request(args.observations))
        result = report.as_dict()
        result["scope"] = "actual command values/points, selection and warning; fixed 1e-9 absolute tolerance including printed-value residuals; API diagnostics not command targets"
        print(json.dumps(result, indent=2, allow_nan=False))
        return 0 if report.passed else 1
    except (ValueError, KeyError, TypeError, OSError, OracleError) as error:
        parser.exit(2, "volume replay failed: %s\n" % error)


if __name__ == "__main__": raise SystemExit(main())
