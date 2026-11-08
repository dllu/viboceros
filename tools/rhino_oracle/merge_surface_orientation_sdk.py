"""Owned public MergeSurfaces on authored charts and reversed single-face B-reps."""
import copy
if __package__:
    from .merge_surface_orientation_cases import CASES
else:
    from merge_surface_orientation_cases import CASES


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
            or op.get('op') != 'merge_surface_orientation_sdk' or op.get('id') != op.get('case')
            or op.get('case') not in CASES):
        raise ValueError('invalid prescribed surface-merge orientation case')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1: raise ValueError('one SDK observation required')
    import brep_join_probe
    G = host['Rhino'].Geometry
    recipe = copy.deepcopy(CASES[op['case']])
    sources, breps, output = [], [], None
    def record(source):
        return (brep_join_probe.geometry_record(source, tolerance, host)
                if isinstance(source, G.Brep) else host['_nurbs_surface_definition'](source))
    try:
        for definition in recipe['sources']:
            source = host['_object_source'](definition, tolerance)
            sources.append(source)
            brep = source.DuplicateBrep() if isinstance(source, G.Brep) else source.ToBrep()
            if brep is None: raise ValueError('surface orientation B-rep construction failed')
            breps.append(brep)
            if not brep.IsValid: raise ValueError('invalid surface orientation input')
        before = [record(s) for s in sources]
        before_breps = [record(b) for b in breps]
        output = G.Brep.MergeSurfaces(breps[0], breps[1], recipe['merge_tolerance'],
            tolerance['angular'], G.Point2d.Unset, G.Point2d.Unset,
            recipe['roundness'], recipe['smooth'])
        if output is not None and not output.IsValid: raise ValueError('invalid merged orientation result')
        if before != [record(s) for s in sources] or before_breps != [record(b) for b in breps]:
            raise ValueError('surface orientation API mutated an input')
        return dict(prescribed_case=recipe, source_unchanged=True, inputs=before_breps,
                    success=output is not None, output=record(output) if output is not None else None), 0
    finally:
        if output is not None: output.Dispose()
        for owned in breps + sources: owned.Dispose()
