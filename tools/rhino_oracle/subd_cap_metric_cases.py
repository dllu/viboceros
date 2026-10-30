"""Affine planar cages that distinguish world-distance and dropped-axis metrics."""
try:
    from .subd_cap_annuli_cases import SHAPES, cage
except (ImportError, ValueError):
    from subd_cap_annuli_cases import SHAPES, cage

CASES = {}
for shape in ['offset', 'skew', 'pentagon']:
    for frame in ['tilted', 'oblique']:
        for crease in [True, False]:
            source = cage(SHAPES[shape])
            if frame == 'tilted':
                source['vertices'] = [[32.+x+2.*y+z, -16.+2.*x-y-z, 8.+3.*y+z] for x, y, z in source['vertices']]
            else:
                source['vertices'] = [[32.+3.*x+y-z, -16.+x+2.*y+z, 8.-x+y+z] for x, y, z in source['vertices']]
            name = shape+'-'+frame+'-'+('crease' if crease else 'smooth')
            CASES[name] = dict(command='Cap', sources=[source], whole=True, preselected=True,
                command_arguments='_DeleteInput=_Yes _Crease=_'+('Yes' if crease else 'No')+' _Triangles=_Yes')


def validate(op):
    if not (isinstance(op, dict) and set(op) == {'op', 'id', 'case'}
            and op.get('op') == 'subd_cap_metric' and isinstance(op.get('case'), str)
            and op['case'] in CASES and op['id'] == op['case']):
        raise ValueError('fixed owned metric SubD cap recipe required')
