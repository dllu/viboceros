"""Fixed public Plane recipes, independent of local construction results."""
from copy import deepcopy

PILOT = {
    'corners': dict(actions=[[1, 2, 3], [9, 7, 11]]),
    'reversed': dict(actions=[[9, 7, 3], [1, 2, 11]]),
    'three_point': dict(actions=['3Point', [1, 2, 3], [4, 6, 8], [7, -1, 3]]),
    'center': dict(actions=['Center', [1, 2, 3], [5, 7, 9]]),
    'vertical': dict(actions=['Vertical', [1, 2, 3], [5, 8, 9], [1, 2, 10]]),
    'deformable': dict(actions=['Deformable', 'UDegree=3', 'VDegree=2', 'UPointCount=6', 'VPointCount=5', [1, 2, 3], [9, 7, 11]]),
    'plain_option': dict(actions=['Deformable=No', [1, 2, 3], [9, 7, 11]]),
    'cancel': dict(actions=['3Point', [1, 2, 3], 'Cancel']),
}

CASES = dict(PILOT)
CASES.update({
    'negative_x': dict(actions=[[1, 2, 3], [-7, 7, -9]]),
    'negative_y': dict(actions=[[1, 2, 3], [9, -3, -9]]),
    'dimensions': dict(actions=[[1, 2, 3], 8, 5]),
    'square_enter': dict(actions=[[1, 2, 3], 8, 'Enter']),
    'center_dimensions': dict(actions=['Center', [1, 2, 3], 8, 5]),
    'center_three': dict(actions=['Center', [1, 2, 3], '3Point', [4, 6, 8], [7, -1, 3]]),
    'edge_midpoint': dict(actions=['3Point', 'EdgeMidpoint', [1, 2, 3], [4, 6, 8], [7, -1, 3]]),
    'three_dimensions': dict(actions=['3Point', [1, 2, 3], 8, [4, 6, 8], 5, [7, -1, 3]]),
    'vertical_negative': dict(actions=['Vertical', [1, 2, 3], [5, 8, 3], [1, 2, -4]]),
    'vertical_dimensions': dict(actions=['Vertical', [1, 2, 3], 8, [5, 8, 3], 5, [1, 2, 10]]),
    'around_line': dict(actions=['AroundCurve', [0, 0, 5], 8, 5], guide=[[0, 0, 0], [0, 0, 10]]),
    'around_oblique': dict(actions=['AroundCurve', [2, 3, 4], 8, 5], guide=[[1, 2, 3], [4, 6, 8]]),
    'cplane': dict(actions=[[1, 2, 3], [9, 7, 11]], construction_plane=[[2, 3, 4], [0, 1, 0], [0, 0, 1]]),
    'cplane_oblique': dict(actions=['Center', [1, 2, 3], [9, 7, 11]], construction_plane=[[2, 3, 4], [1, 2, 3], [-2, 4, 1]]),
    'cancel_start': dict(actions=['Cancel']),
    'cancel_width': dict(actions=['3Point', [1, 2, 3], [4, 6, 8], 'Cancel']),
    'zero_edge': dict(actions=['3Point', [1, 2, 3], [1, 2, 3], 'Cancel']),
    'zero_width': dict(actions=[[1, 2, 3], [1, 8, 3], 'Cancel']),
})
for name, degree, count in [
        ('default', [3, 3], [10, 10]), ('linear', [1, 1], [4, 5]),
        ('mixed', [2, 4], [3, 7]), ('high', [11, 9], [12, 16])]:
    CASES['deform_' + name] = dict(actions=['Deformable'] + [
        key + '=' + str(value) for key, value in zip(
            ['UDegree', 'VDegree', 'UPointCount', 'VPointCount'], degree + count)
    ] + [[1, 2, 3], [9, 7, 11]])
WORKFLOWS = [name for name in CASES if name != 'plain_option']
AUDIT = []
for name, edits in [
        ('degree_one', ['UDegree=1']), ('degree_twelve', ['UDegree=12']),
        ('low_count', ['UPointCount=3']),
        ('count_then_degree', ['UPointCount=4', 'UDegree=5']),
        ('degree_then_count', ['UDegree=5', 'UPointCount=4']),
        ('integer_getters', ['UDegree', 2, 'VDegree', 4, 'UPointCount', 7, 'VPointCount', 6]),
        ('integer_defaults', ['UDegree', 'Enter', 'VPointCount', 'Enter']),
        ('cancel_integer', ['UDegree', 'Cancel']),
        ('center', ['Center']), ('three_point', ['3Point'])]:
    key = 'audit_' + name
    AUDIT.append(key)
    basis = ['Deformable', 'UDegree=3', 'VDegree=3', 'UPointCount=10', 'VPointCount=10']
    points = [[1, 2, 3], [9, 7, 11]] if name != 'three_point' else [[1, 2, 3], [4, 6, 8], [7, -1, 3]]
    CASES[key] = dict(actions=basis + edits + ([] if name == 'cancel_integer' else points))

if __package__:
    from .plane_surface_holdouts import HOLDOUT_CASES
    from .plane_surface_guide_cases import GUIDE_CASES
else:
    from plane_surface_holdouts import HOLDOUT_CASES
    from plane_surface_guide_cases import GUIDE_CASES
CASES.update(HOLDOUT_CASES)
HOLDOUTS = list(HOLDOUT_CASES)
CASES.update(GUIDE_CASES)
VERTICAL_AUDIT = []
for suffix, basis in [('plain', []), ('deform_minimum', ['Deformable','UDegree=2','VDegree=4','UPointCount=3','VPointCount=5']),
                      ('deform_cubic', ['Deformable','UDegree=3','VDegree=3','UPointCount=4','VPointCount=4']),
                      ('deform_default', ['Deformable','UDegree=3','VDegree=3','UPointCount=10','VPointCount=10']),
                      ('repeat', ['Deformable','UDegree=2','VDegree=4','UPointCount=5','VPointCount=7'])]:
    key = 'vertical_audit_' + suffix
    VERTICAL_AUDIT.append(key)
    original = deepcopy(HOLDOUT_CASES['holdout_11'])
    original['actions'] = basis + original['actions'][5:]
    CASES[key] = original
for suffix, divisor in [('translated',1.),('small',100000.)]:
    key = 'vertical_audit_' + suffix
    VERTICAL_AUDIT.append(key)
    original = deepcopy(CASES['vertical_audit_plain'])
    offset = original['construction_plane'][0]
    original['actions'] = [a if isinstance(a,str) else [(n-offset[i])/divisor for i,n in enumerate(a)] for a in original['actions']]
    original['construction_plane'][0] = [0.,0.,0.]
    CASES[key] = original


def request(names=None):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6),
                operations=[dict(op='plane_surface_command', id=name, case=name)
                            for name in (WORKFLOWS if names is None else names)])


def qualified_request():
    return request(WORKFLOWS + AUDIT + HOLDOUTS)


def validate(op):
    if (not isinstance(op, dict) or set(op) != {'op', 'id', 'case'}
            or op.get('op') != 'plane_surface_command' or not isinstance(op.get('case'), str)
            or op.get('case') not in CASES
            or op.get('id') != op['case']):
        raise ValueError('invalid fixed Plane recipe')


def recipe(name):
    return deepcopy(CASES[name])
