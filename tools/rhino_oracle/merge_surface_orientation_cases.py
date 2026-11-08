"""Authored first-edge, chart and face-sense variants for surface merging."""
import copy
if __package__:
    from .merge_surface_cases import pair
else:
    from merge_surface_cases import pair


def edit(source, operation):
    s = copy.deepcopy(source)
    nu, nv = s['control_point_count_u'], s['control_point_count_v']
    rows = [s['control_points'][nu*j:nu*(j+1)] for j in range(nv)]
    if operation == 'u': rows = [row[::-1] for row in rows]
    elif operation == 'v': rows = rows[::-1]
    elif operation == 'uv': rows = [row[::-1] for row in rows[::-1]]
    elif operation == 'transpose':
        rows = [list(row) for row in zip(*rows)]
        for u, v in [('degree_u', 'degree_v'), ('control_point_count_u', 'control_point_count_v'),
                     ('knots_u', 'knots_v')]:
            s[u], s[v] = s[v], s[u]
    else: raise ValueError('unknown authored surface direction edit')
    s['control_points'] = [p for row in rows for p in row]
    return s


CASES = {}
for operation in ['u', 'v', 'uv', 'transpose']:
    value = pair(smooth=True)
    value['sources'][0] = edit(value['sources'][0], operation)
    CASES['merge_first_' + operation] = value
value = pair(smooth=True)
value['sources'] = [edit(s, 'transpose') for s in value['sources']]
CASES['merge_both_transpose'] = value
for smooth in [False, True]:
    value = pair(smooth=smooth)
    value['sources'].reverse()
    CASES['merge_order_' + ('smooth' if smooth else 'sharp')] = value
value = copy.deepcopy(CASES['merge_order_smooth'])
value['sources'][0].update(domain_u=[-8., -6.5], domain_v=[3., 5.])
value['sources'][1].update(domain_u=[10., 14.], domain_v=[-1., 2.])
CASES['merge_order_nonunit'] = value
for index in [0, 1]:
    value = pair(smooth=True)
    value['sources'][index] = dict(type='untrimmed_brep',
                                  surface=value['sources'][index], reversed=True)
    CASES['merge_face_flip_' + str(index)] = value


def request():
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6),
                operations=[dict(op='merge_surface_orientation_sdk', id=name, case=name) for name in CASES])
