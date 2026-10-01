"""Source recipes for the public hole-removal API, independent of observations."""
import copy
import json

from .untrim_cases import request as untrim_request


def request():
    cases = untrim_request()["operations"]
    def source(name):
        return copy.deepcopy(next(op["sources"][0] for op in cases
            if op["id"] == name + "-keep-0-pre-0"))
    recipes = []
    for name in ("outer", "hole", "two-holes", "reversed-two-holes", "shifted-domains",
                 "annulus", "paraboloid-annulus", "paraboloid-oblique", "full-border"):
        choices = [("all", None), ("first", [[0, 1]])]
        if name == "outer": choices = [("all", None), ("empty", []), ("outer", [[0, 0]])]
        if name == "paraboloid-oblique": choices = [("all", None)]
        for label, loops in choices:
            recipes.append((name + "-" + label, dict(source=source(name)), loops))
    for label, loops in (("last", [[0, 2]]), ("duplicate", [[0, 1], [0, 1]]), ("empty", [])):
        recipes.append(("two-holes-" + label, dict(source=source("two-holes")), loops))
    for label, keep in (("solid", None), ("open", [0, 1, 2]), ("caps", [2, 3])):
        recipe = dict(source=dict(type="solid_tube", radii=[2., 5.], height=8., keep_faces=keep))
        for suffix, loops in (("all", None), ("first", [[0, 1]] if label == "caps" else [[2, 1]])):
            recipes.append(("tube-" + label + "-" + suffix, recipe, loops))
    recipes.append(("tube-wall-outer", dict(source=dict(type="solid_tube", radii=[2., 5.], height=8.)), [[1, 0]]))
    return dict(protocol_version=1, iterations=1, operations=[
        dict(op="brep_remove_holes", id=name, source=copy.deepcopy(recipe), loops=loops)
        for name, recipe, loops in recipes])


if __name__ == "__main__": print(json.dumps(request(), indent=2))
