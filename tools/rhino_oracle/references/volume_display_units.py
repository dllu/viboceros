"""Source-only Volume display choices; no captured values are inputs."""
import json


def box(side):
    return dict(type="mesh", vertices=[
        [0, 0, 0], [side, 0, 0], [side, side, 0], [0, side, 0],
        [0, 0, side], [side, 0, side], [side, side, side], [0, side, side]],
        faces=[[0, 3, 2, 1], [4, 5, 6, 7], [0, 1, 5, 4],
               [1, 2, 6, 5], [2, 3, 7, 6], [3, 0, 4, 7]])


def request():
    operations = [
        dict(op="volume_command", id="millimeters-to-meters", sources=[box(1000)],
             preselect=False, display_units="Meter", unit_setup=["Millimeter"], model_units=2),
        dict(op="volume_command", id="inches-to-feet", sources=[box(12)],
             preselect=False, display_units="Foot", unit_setup=["Inch"], model_units=8),
        dict(op="volume_command", id="canceled-choice-remembered-preselection", sources=[box(1000)],
             unit_setup=["Meter"], model_units=2),
    ]
    for units in ("ModelUnits", "Micron", "Millimeter", "Centimeter", "Liter", "Decimeter",
                  "Meter", "Kilometer", "Microinch", "Mil", "Inch", "Foot", "Yard", "Mile"):
        # Model coordinates are integers. Imperial choices start with inches;
        # metric choices with millimeters. Full printed residuals are retained.
        imperial = units in ("Microinch", "Mil", "Inch", "Foot", "Yard", "Mile")
        operations.append(dict(op="volume_command", id="choice-" + units.lower(),
            sources=[box(12 if imperial else 10)], preselect=False,
            display_units=units, unit_setup=["ModelUnits"], model_units=8 if imperial else 2))
    for code in (0, 2, 4, 8):
        for choice in ("ModelUnits", "Meter"):
            operations.append(dict(op="volume_command", id="model-%d-%s" % (code, choice.lower()),
                sources=[box(2)], unit_setup=[choice], model_units=code))
    operations.append(dict(op="volume_command", id="last-canceled-choice-wins", sources=[box(10)],
        unit_setup=["Meter", "Liter", "Centimeter"], model_units=2))
    reverse = box(10)
    reverse["faces"] = [face[::-1] for face in reverse["faces"]]
    operations.append(dict(op="volume_command", id="signed-liter", sources=[reverse],
        unit_setup=["Liter"], model_units=2))
    open_box = box(1000)
    pieces = [dict(type="mesh", vertices=[open_box["vertices"][i] for i in face], faces=[[0, 1, 2, 3]])
              for face in open_box["faces"]]
    for answer in ("yes", "no", "escape"):
        operations.append(dict(op="volume_command", id="open-meter-" + answer, sources=pieces,
            preselect=False, display_units="Meter", unit_setup=["ModelUnits"], model_units=2,
            open_confirmation=answer))
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == "__main__":
    print(json.dumps(request(), indent=2, allow_nan=False))
