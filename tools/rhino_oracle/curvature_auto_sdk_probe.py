"""Public Auto Range calculation on owned meshes with explicit curvature channels."""
import math
import os
import re

STYLES = ('Gaussian', 'Mean', 'MinRadius', 'MaxRadius')
RANGES = ('GaussRange', 'MeanRange', 'MinRadiusRange', 'MaxRadiusRange')


def validate(op):
    def asset(value):
        return isinstance(value, str) and re.match(r'^[a-z0-9_-]{1,80}\Z', value) is not None
    if (not isinstance(op, dict)
            or set(op) - {'companions', 'initial'} != {'op', 'id', 'asset', 'style'}
            or op['op'] != 'curvature_auto_sdk' or op['style'] not in STYLES
            or not isinstance(op['id'], str) or re.match(r'^[A-Za-z0-9_-]{1,100}\Z', op['id']) is None
            or (op['asset'] is not None and not asset(op['asset']))
            or not isinstance(op.get('companions', []), list) or len(op.get('companions', [])) > 8
            or any(not asset(v) for v in op.get('companions', []))):
        raise ValueError('invalid curvature auto-range SDK recipe')
    if 'initial' in op and (not isinstance(op['initial'], list) or len(op['initial']) != 4
            or any(not isinstance(r, list) or len(r) != 2 or r[0] == r[1]
                   or any(type(x) not in (int, float) or math.isnan(x) or math.isinf(x) for x in r)
                   for r in op['initial'])):
        raise ValueError('invalid initial auto-range settings')


def run(op, host, iterations, tolerance):
    validate(op)
    R, S = host['Rhino'], host['System']
    G = R.Geometry
    if iterations != 1:
        raise ValueError('range SDK capture requires one iteration')
    models = []
    meshes = S.Collections.Generic.List[G.Mesh]()
    count = 0
    try:
        for asset in ([op['asset']] if op['asset'] is not None else []) + op.get('companions', []):
            path = os.path.join(os.path.dirname(__file__), 'fixtures', 'curvature_auto', asset + '.3dm')
            model = R.FileIO.File3dm.Read(path)
            if model is None:
                raise ValueError('owned curvature sample mesh missing')
            models.append(model)
            if model.Objects.Count != 1:
                raise ValueError('sample file must contain exactly one mesh')
            mesh = list(model.Objects)[0].Geometry
            if not isinstance(mesh, G.Mesh) or not mesh.HasPrincipalCurvatures:
                raise ValueError('sample mesh lost its curvature channel')
            meshes.Add(mesh)
            count += mesh.Vertices.Count
        settings = R.ApplicationSettings.CurvatureAnalysisSettings
        state = settings.GetDefaultState()
        state.Style = getattr(settings.CurvatureStyle, op['style'])
        if 'initial' in op:
            for key, interval in zip(RANGES, op['initial']):
                setattr(state, key, G.Interval(*interval))
        result = settings.CalculateCurvatureAutoRange(meshes, state)
        if isinstance(result, tuple):
            success, state = result
        else:
            success = bool(result)
        return dict(success=bool(success), style=str(state.Style), count=int(count),
                    ranges=[[float(getattr(state, key).T0), float(getattr(state, key).T1)]
                            for key in RANGES]), 0
    finally:
        for model in models:
            model.Dispose()
