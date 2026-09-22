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
                    r")\) (liters|cubic [a-z]+)(?: for [1-9][0-9]* solids)?$", re.MULTILINE)
UNIT_LABELS = dict(zip(
    ("Micron", "Millimeter", "Centimeter", "Liter", "Decimeter", "Meter", "Kilometer",
     "Microinch", "Mil", "Inch", "Foot", "Yard", "Mile"),
    ("cubic microns", "cubic millimeters", "cubic centimeters", "liters", "cubic decimeters",
     "cubic meters", "cubic kilometers", "cubic microinches", "cubic mils", "cubic inches",
     "cubic feet", "cubic yards", "cubic miles")))
MODEL_UNITS = {0: ("None", "cubic units"), 2: ("Millimeters", "cubic millimeters"),
               4: ("Meters", "cubic meters"), 8: ("Inches", "cubic inches")}


def history_volume(value, operation=None):
    operation = operation or {}
    display = value.get("display")
    model_name, model_label = MODEL_UNITS[operation.get("model_units", 2)]
    choice = operation.get("display_units", operation.get("unit_setup", ["ModelUnits"])[-1])
    expected_label = model_label if choice == "ModelUnits" else UNIT_LABELS[choice]
    if display != dict(precision=7, units=model_name):
        raise OracleProtocolError("unsupported scalar volume display settings")
    matches = VOLUME.findall(value["history"].replace("\r\n", "\n"))
    if not value["succeeded"]:
        if matches: raise OracleProtocolError("failed Volume command reported a volume")
        return None
    if len(matches) != 1: raise OracleProtocolError("missing or ambiguous Volume result")
    volume, error = map(float, matches[0][:2])
    if matches[0][2] != expected_label:
        raise OracleProtocolError("Volume output has unexpected display units")
    if not math.isfinite(volume) or not math.isfinite(error) or error < 0:
        raise OracleProtocolError("invalid Volume result")
    # The printed uncertainty is retained in history, NOT used to widen epsilon.
    return volume


def prepare(request, observed):
    validation_request = copy.deepcopy(request)
    validation_observed = copy.deepcopy(observed)
    scalar = []
    setups = []
    for operation in validation_request["operations"]:
        is_scalar = operation.get("op") == "volume_command"
        validate(operation, "volume", not is_scalar)
        scalar.append(is_scalar)
        setups.append(operation.get("unit_setup"))
        for key in ("display_units", "unit_setup", "model_units"):
            operation.pop(key, None)
        operation["op"] = "volume_centroid_command"
    if len(validation_observed["results"]) != len(scalar):
        raise OracleProtocolError("volume observation count differs")
    volumes = []
    for op, is_scalar, row in zip(request["operations"], scalar, validation_observed["results"]):
        if is_scalar:
            volumes.append(history_volume(row["value"], op))
            row["value"].pop("display")
            if "unit_setup" in op:
                steps = row["value"].pop("unit_setup", None)
                if not isinstance(steps, list) or len(steps) != len(op["unit_setup"]):
                    raise OracleProtocolError("incomplete volume unit setup evidence")
                for requested, step in zip(op["unit_setup"], steps):
                    if (not isinstance(step, dict) or set(step) != {"units", "succeeded", "history"}
                            or step["units"] != requested or step["succeeded"] is not False
                            or not isinstance(step["history"], str)
                            or "Units=" + requested not in step["history"]
                            or VOLUME.search(step["history"])):
                        raise OracleProtocolError("invalid canceled volume unit setup")
            if row["value"]["points"]: raise OracleProtocolError("scalar Volume created geometry")
        else: volumes.append(None)
    _, evidence = prepare_centroids(validation_request, validation_observed, measure="volume")
    for setup, is_scalar, volume, row in zip(setups, scalar, volumes, evidence["results"]):
        if is_scalar: row["value"]["volume"] = volume
        if setup is not None: row["value"]["unit_setup"] = [False] * len(setup)
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
