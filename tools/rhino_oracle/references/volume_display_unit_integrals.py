"""Exact dimensional witnesses on source meshes, with no native/oracle outputs."""
from fractions import Fraction as F
import json
from .volume_display_units import request
from .volume_primitives_integrals import collection


def exact_volume(operation):
    source_units = {0: None, 2: F(1, 1000), 4: F(1), 8: F(127, 5000)}
    meters = {"Micron": F(1, 10**6), "Millimeter": F(1, 1000), "Centimeter": F(1, 100),
              "Liter": F(1, 10), "Decimeter": F(1, 10), "Meter": F(1), "Kilometer": F(1000),
              "Microinch": F(127, 5*10**9), "Mil": F(127, 5*10**6), "Inch": F(127, 5000),
              "Foot": F(381, 1250), "Yard": F(1143, 1250), "Mile": F(201168, 125)}
    choice = operation.get("display_units", operation.get("unit_setup", ["ModelUnits"])[-1])
    source = source_units[operation.get("model_units", 2)]
    scale = F(1) if source is None or choice == "ModelUnits" else source / meters[choice]
    return F(collection(operation["sources"], "cone")["volume"]) * scale**3


def reference():
    return {op["id"]: dict(volume=str(exact_volume(op)), rounded_volume=float(exact_volume(op)))
            for op in request()["operations"]}


if __name__ == "__main__":
    print(json.dumps(reference(), indent=2, allow_nan=False))
