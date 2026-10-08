"""Owned SDK timing for the exact source surfaces exported by the Rust profiler."""
import math
import re

COUNTS = {'sphere_cap': [10, 10], 'swapped_cap': [12, 8], 'cylinder_band': [12, 8]}


def validate_request(q):
    if (not isinstance(q, dict) or set(q) != {'protocol_version', 'iterations', 'operations'}
            or type(q['protocol_version']) is not int or q['protocol_version'] != 1
            or type(q['iterations']) is not int or q['iterations'] != 1
            or not isinstance(q['operations'], list) or not 1 <= len(q['operations']) <= 3):
        raise ValueError('retrim profiling requires bounded protocol 1 recipes')
    seen = set()
    for op in q['operations']:
        if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case', 'surface', 'bounds', 'count', 'degree', 'repetitions'}
                or op['op'] != 'surface_retrim_profile' or not isinstance(op['case'], str) or op['case'] not in COUNTS
                or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,80}\Z', op['id']) is None
                or op['id'] in seen or op['count'] != COUNTS[op['case']] or op['degree'] != [3, 3]
                or type(op['repetitions']) is not int or op['repetitions'] != 3):
            raise ValueError('invalid retrim profiling recipe')
        surface = op['surface']
        if (not isinstance(surface, dict) or not isinstance(surface.get('control_points'), list)
                or not 1 <= len(surface['control_points']) <= 64):
            raise ValueError('invalid profiling surface')
        bounds = op['bounds']
        if (not isinstance(bounds, list) or len(bounds) != 2
                or any(not isinstance(a, list) or len(a) != 2
                       or any(type(t) not in (int, float) or math.isnan(t) or math.isinf(t) for t in a)
                       or a[0] >= a[1] for a in bounds)):
            raise ValueError('invalid profiling parameter bounds')
        seen.add(op['id'])


def run(op, host):
    validate_request(dict(protocol_version=1, iterations=1, operations=[op]))
    Rhino = host['Rhino']
    if Rhino.Commands.Command.InCommand():
        raise ValueError('retrim profiling requires idle execution')
    if list(Rhino.RhinoDoc.ActiveDoc.Objects):
        raise ValueError('retrim profiling requires an empty owned document')
    G = Rhino.Geometry
    doc=Rhino.RhinoDoc.ActiveDoc
    saved_tolerance=doc.ModelAbsoluteTolerance
    source = host['_nurbs_surface_from_definition'](op['surface'])
    owned = [source]
    doc.ModelAbsoluteTolerance=1e-6
    try:
        domains = [source.Domain(i) for i in (0, 1)]
        if any(a[0] < d.T0 or a[1] > d.T1 for a, d in zip(op['bounds'], domains)):
            raise ValueError('profiling bounds leave the source chart')
        base = G.Brep.CreateFromSurface(source)
        owned.append(base)
        cutters = []
        for axis in (0, 1):
            for value in op['bounds'][axis]:
                if domains[axis].T0 < value < domains[axis].T1:
                    curve = source.IsoCurve(1 - axis, value)
                    cutters.append(curve)
                    owned.append(curve)
        split = base.Faces[0].Split(cutters, 1e-6)
        if split is None:
            raise ValueError('profiling trim split failed')
        owned.append(split)
        station = [a[0] + 0.3 * (a[1] - a[0]) for a in op['bounds']]
        face = next(f for f in split.Faces if str(f.IsPointOnFace(*station)) == 'Interior')
        original = face.DuplicateFace(False)
        owned.append(original)
        before = host['_interchange_brep_record'](original)
        ticks = host['System'].Diagnostics.Stopwatch
        runs = []
        for index in range(op['repetitions']):
            host['_record_progress'](op['id'] + ' repetition ' + str(index))
            start = ticks.GetTimestamp()
            rebuilt = source.Rebuild(op['degree'][0], op['degree'][1], op['count'][0], op['count'][1])
            rebuild_ns = int((ticks.GetTimestamp() - start) * 1e9 / ticks.Frequency)
            if rebuilt is None:
                raise ValueError('profiling SDK rebuild failed')
            try:
                start = ticks.GetTimestamp()
                result = G.Brep.CreateTrimmedSurface(original.Faces[0], rebuilt, 1e-6)
                retrim_ns = int((ticks.GetTimestamp() - start) * 1e9 / ticks.Frequency)
                if result is None:
                    raise ValueError('profiling SDK retrim failed')
                try:
                    runs.append(dict(rebuild_ns=rebuild_ns, retrim_ns=retrim_ns,
                                     valid=bool(result.IsValid), brep=host['_interchange_brep_record'](result)))
                finally:
                    result.Dispose()
            finally:
                rebuilt.Dispose()
        return dict(case=op['case'], source=before, source_after=host['_interchange_brep_record'](original),
                    surface=host['_nurbs_surface_definition'](source), runs=runs), 0
    finally:
        for value in reversed(owned):
            value.Dispose()
        doc.ModelAbsoluteTolerance=saved_tolerance
