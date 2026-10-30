"""Independently authored holed cages for SubD Cap triangulation holdouts."""
from copy import deepcopy

OUTER = [[0., 0.], [4., 0.], [4., 3.], [0., 3.]]
INNER = [[1., 1.], [3., 1.], [3., 2.], [1., 2.]]
SHAPES = {
    'centered': [OUTER, INNER],
    'offset': [OUTER, [[.5, .5], [2.5, .75], [2.25, 2.1], [.75, 2.5]]],
    'skew': [[[0., 0.], [6., 0.], [5., 4.], [-1., 3.]], INNER],
    'triangle': [[[0., 0.], [8., 0.], [4., 8.]], [[3., 2.], [5., 2.], [4., 4.]]],
    'pentagon': [[[0., 0.], [8., 0.], [10., 4.], [5., 8.], [-2., 4.]],
                 [[2., 2.], [5., 1.5], [7., 3.], [4.5, 5.], [1., 3.5]]],
    'two-holes': [[[0., 0.], [12., 0.], [12., 8.], [0., 8.]],
                  [[1., 1.], [4., 1.], [4., 4.], [1., 4.]],
                  [[7., 3.], [10., 3.], [10., 6.], [7., 6.]]],
}


def cage(rings):
    vertices, faces = [], []
    for index, ring in enumerate(rings):
        n, base = len(ring), len(vertices)
        for z in [0., 2.]:
            vertices.extend([[x, y, z] for x, y in ring])
        for side in range(n):
            nxt = (side + 1) % n
            face = [base+side, base+nxt, base+nxt+n, base+side+n]
            if index:
                face.reverse()
            faces.append(face)
    return dict(kind='subd', vertices=vertices, faces=faces)


CASES = {}


def add(name, rings, crease, reverse=False, tilted=False):
    source = cage(rings)
    if reverse:
        for face in source['faces']:
            face.reverse()
    if tilted:
        source['vertices'] = [[16.+x-y+z, -32.+x+y-z, 8.+2.*y+z] for x, y, z in source['vertices']]
    CASES[name] = dict(command='Cap', sources=[source], whole=True, preselected=True,
        command_arguments='_DeleteInput=_Yes _Crease=_'+('Yes' if crease else 'No')+' _Triangles=_Yes')


for name, rings in SHAPES.items():
    for crease in [True, False]:
        add(name+'-'+('crease' if crease else 'smooth'), rings, crease)
for crease in [True, False]:
    suffix = '-'+('crease' if crease else 'smooth')
    add('reversed'+suffix, SHAPES['centered'], crease, reverse=True)
    add('tilted'+suffix, SHAPES['centered'], crease, tilted=True)
    rings = deepcopy(SHAPES['centered']); rings[0] = rings[0][1:]+rings[0][:1]
    add('outer-cycle'+suffix, rings, crease)
    rings = deepcopy(SHAPES['centered']); rings[1] = rings[1][1:]+rings[1][:1]
    add('inner-cycle'+suffix, rings, crease)


def validate(op):
    if not (isinstance(op, dict) and set(op) == {'op', 'id', 'case'}
            and op.get('op') == 'subd_cap_annuli' and isinstance(op.get('case'), str)
            and op['case'] in CASES and op['id'] == op['case']):
        raise ValueError('fixed owned annular SubD cap recipe required')
