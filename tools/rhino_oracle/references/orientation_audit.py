"""Source descriptions only; document policies are observations, not targets."""
import copy
import json
import sys


def request():
    cases = []
    def add(name, sources, **options):
        cases.append(dict(op="orientation_audit", id=name, sources=copy.deepcopy(sources), **options))
    for kind in ("box", "open_box", "plane", "sphere", "mesh"):
        for reversed in (False, True):
            s = dict(kind=kind, reversed=reversed)
            for mode in ("generic", "no_kink"):
                add("%s-%s-%s" % (kind, reversed, mode), [s], insertion=mode, replace_flip=True)
    for preselect in (False, True):
        for kind in ("box", "open_box", "plane", "sphere", "mesh", "line", "point"):
            add("flip-%s-%s" % (kind, preselect), [dict(kind=kind)], preselect=preselect, flip=True)
        add("flip-mixed-%s" % preselect, [dict(kind=k) for k in ("box", "open_box", "mesh", "line", "point")],
            preselect=preselect, flip=True)
    return dict(protocol_version=1, iterations=1, operations=cases)


def compound_request():
    """Independently vary shell size, sense, placement, and table order."""
    cases = []
    def box(offset=0, size=1, reverse=False):
        return dict(kind="box", offset=offset, size=size, reversed=reverse)
    layouts = [
        ("disjoint-outward", [box(-10), box(10)]),
        ("disjoint-inward", [box(-10, reverse=True), box(10, reverse=True)]),
        ("disjoint-opposed-zero", [box(-10), box(10, reverse=True)]),
        ("disjoint-opposed-negative", [box(-10), box(10, 2, True)]),
        ("disjoint-opposed-positive", [box(-10, 2), box(10, reverse=True)]),
        ("nested-cavity", [box(size=4), box(reverse=True)]),
        ("nested-reversed-cavity", [box(size=4, reverse=True), box()]),
        ("nested-both-outward", [box(size=4), box()]),
        ("nested-both-inward", [box(size=4, reverse=True), box(reverse=True)]),
        ("coincident-opposed", [box(), box(reverse=True)]),
    ]
    for name, parts in layouts:
        for reverse_order in (False, True):
            ordered = list(reversed(parts)) if reverse_order else parts
            cases.append(dict(op="orientation_audit", id=name + ("-reverse-order" if reverse_order else ""),
                sources=[dict(kind="compound", parts=copy.deepcopy(ordered))],
                replace_flip=True, flip=True, preselect=False))
    return dict(protocol_version=1, iterations=1, operations=cases)


if __name__ == "__main__":
    print(json.dumps(compound_request() if sys.argv[1:] == ["compound"] else request(), indent=2, allow_nan=False))
