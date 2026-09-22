"""Source-only box assemblies across signed-volume underflow and overflow."""
import json
import math


def request(exponent=0):
    operations = []
    scale = math.ldexp(1.0, exponent)
    for mask in (0, 1, 42, 63):
        for reverse_order in (False, True):
            for command in ("Join", "JoinCopy"):
                for preselect in (False, True):
                    # The second face is opposite the first. Command-first
                    # selection skips it, exercising partial open assemblies.
                    indices = list(range(6))
                    if reverse_order:
                        indices.reverse()
                    sources = [dict(brep=dict(
                        source=dict(type="box", min=[0, 0, 0],
                                    max=[scale, 2*scale, 4*scale], keep_faces=[i]),
                        reversed=bool(mask & (1 << i)))) for i in indices]
                    operations.append(dict(
                        op="join_command",
                        id="scale-%d-mask-%d-order-%s-%s-pre-%s" % (
                            exponent, mask, reverse_order, command, preselect),
                        sources=sources, command=command, preselect=preselect,
                        definition_only=True,
                        absolute_tolerance=math.ldexp(scale, -30)))
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=math.ldexp(1.0, exponent-30),
                               relative=1e-12, angular=1e-10),
                operations=operations)


def repeat_request():
    result = request(360)
    result["operations"] = [result["operations"][i] for i in (1, 3, 9, 11, 17, 19, 29, 31)]
    scale = math.ldexp(1.0, 360)
    for reversed in (False, True):
        result["operations"].append(dict(
            op="brep_solid_orientation", id="uninserted-large-reversed-%s" % reversed,
            sources=[dict(source=dict(type="box", min=[0,0,0],
                                     max=[scale,2*scale,4*scale]), reversed=reversed)]))
    return result


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exponent", type=int, nargs="?", default=0)
    print(json.dumps(request(parser.parse_args().exponent), indent=2, allow_nan=False))
