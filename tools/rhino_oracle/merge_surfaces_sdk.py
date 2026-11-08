"""Owned public Brep.MergeSurfaces calls; no command or document parity claim."""
import copy
if __package__:
    from .merge_surface_cases import CASES
else:
    from merge_surface_cases import CASES


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
            or op.get('op') != 'merge_surfaces_sdk' or op.get('id') != op.get('case')
            or op.get('case') not in CASES):
        raise ValueError('invalid prescribed surface-merge SDK case')


def run(op, host, iterations, tolerance):
    validate(op)
    if iterations != 1: raise ValueError('surface-merge observations require one iteration')
    import brep_join_probe
    G = host['Rhino'].Geometry
    recipe = copy.deepcopy(CASES[op['case']])
    sources, breps, output = [], [], None
    try:
        for definition in recipe['sources']:
            source = host['_object_source'](definition, tolerance)
            sources.append(source)
            brep = source.ToBrep()
            if brep is None or not brep.IsValid:
                if brep is not None: brep.Dispose()
                raise ValueError('surface-merge input construction failed')
            breps.append(brep)
        before = [host['_nurbs_surface_definition'](s) for s in sources]
        before_breps = [brep_join_probe.geometry_record(b, tolerance, host) for b in breps]
        output = G.Brep.MergeSurfaces(breps[0], breps[1], recipe['merge_tolerance'],
            tolerance['angular'], G.Point2d.Unset, G.Point2d.Unset,
            recipe['roundness'], recipe['smooth'])
        if output is not None and not output.IsValid:
            raise ValueError('surface-merge API returned invalid geometry')
        unchanged = (before == [host['_nurbs_surface_definition'](s) for s in sources]
            and before_breps == [brep_join_probe.geometry_record(b, tolerance, host) for b in breps])
        if not unchanged: raise ValueError('surface-merge API mutated its inputs')
        return dict(prescribed_case=recipe, inputs=before_breps, source_unchanged=unchanged,
                    success=output is not None,
                    output=brep_join_probe.geometry_record(output, tolerance, host)
                    if output is not None else None), 0
    finally:
        if output is not None: output.Dispose()
        for owned in breps + sources: owned.Dispose()
