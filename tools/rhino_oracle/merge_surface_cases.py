"""Independently authored pairs for public surface-merge observations."""
import copy


def surface(second, cubic=True, rational=False, gap=0.):
    xs = [0., .3, .7, 1.] if cubic else [0., 1.]
    zs = [-.2, -.1, 0., 0.] if cubic else [0., 0.]
    if second:
        xs = [1., 1.3, 1.7, 2.] if cubic else [1., 2.]
        zs = [0., .2, .4, .2] if cubic else [0., 0.]
    degree = 3 if cubic else 1
    controls = []
    for j in range(4):
        for i, (x, z) in enumerate(zip(xs, zs)):
            controls.append(dict(point=[x + (gap if second else 0.), j / 3., z + .1 * j / 3.],
                                 weight=1. + .1 * ((i + j) % 3) if rational else 1.))
    return dict(type='surface', degree_u=degree, degree_v=3,
                control_point_count_u=len(xs), control_point_count_v=4,
                control_points=controls, knots_u=[0.] * (degree + 1) + [1.] * (degree + 1),
                knots_v=[0.] * 4 + [1.] * 4)


def pair(cubic=True, rational=False, gap=0., smooth=False, roundness=1., tolerance=1e-4):
    return dict(sources=[surface(False, cubic, rational), surface(True, cubic, rational, gap)],
                smooth=smooth, roundness=roundness, merge_tolerance=tolerance)


PILOT = {
    'merge_plane_sharp': pair(cubic=False),
    'merge_plane_smooth': pair(cubic=False, smooth=True),
    'merge_cubic_sharp': pair(),
    'merge_cubic_smooth_zero': pair(smooth=True, roundness=0.),
    'merge_cubic_smooth_half': pair(smooth=True, roundness=.5),
    'merge_cubic_smooth_one': pair(smooth=True),
    'merge_cubic_sharp_zero': pair(roundness=0.),
    'merge_rational_sharp': pair(rational=True),
    'merge_rational_smooth': pair(rational=True, smooth=True),
    'merge_small_gap': pair(gap=2e-5),
    'merge_large_gap': pair(gap=.01),
}
CASES = copy.deepcopy(PILOT)


def bezier(degree, second, degree_v=3):
    controls = []
    for j in range(degree_v + 1):
        for i in range(degree + 1):
            t, y = i / float(degree), j / float(degree_v)
            z = .6 * t - .4 * t * t if second else -.2 * (1. - t) ** 2
            controls.append(dict(point=[float(second) + t, y, z + .1 * y], weight=1.))
    return dict(type='surface', degree_u=degree, degree_v=degree_v,
                control_point_count_u=degree + 1, control_point_count_v=degree_v + 1,
                control_points=controls, knots_u=[0.] * (degree + 1) + [1.] * (degree + 1),
                knots_v=[0.] * (degree_v + 1) + [1.] * (degree_v + 1))


def degree_pair(first, second=None, roundness=1.):
    value = pair(smooth=True, roundness=roundness)
    value['sources'] = [bezier(first, False), bezier(second or first, True)]
    return value


HOLDOUTS = {
    'merge_quadratic_smooth': degree_pair(2),
    'merge_quartic_smooth': degree_pair(4),
    'merge_quartic_quarter': degree_pair(4, roundness=.25),
    'merge_degrees_two_four': degree_pair(2, 4),
    'merge_gap_smooth': pair(gap=2e-5, smooth=True),
}
for suffix, smooth in [('sharp', False), ('smooth', True)]:
    value = pair(smooth=smooth)
    value['sources'][0].update(domain_u=[2., 3.], domain_v=[-3., 5.])
    value['sources'][1].update(domain_u=[7., 9.], domain_v=[10., 20.])
    HOLDOUTS['merge_nonunit_' + suffix] = value
value = pair(smooth=True)
value['sources'][1] = bezier(3, True, degree_v=2)
HOLDOUTS['merge_along_degrees'] = value
for multiple in [True, False]:
    value = pair(smooth=True)
    source = value['sources'][0]
    n = 7 if multiple else 5
    source['control_point_count_u'] = n
    source['knots_u'] = [0.] * 4 + [.5] * (3 if multiple else 1) + [1.] * 4
    source['control_points'] = [dict(point=[i / float(n - 1), j / 3.,
        -.2 * (1. - i / float(n - 1)) ** 2 + .1 * j / 3.], weight=1.)
        for j in range(4) for i in range(n)]
    HOLDOUTS['merge_internal_' + ('full' if multiple else 'simple')] = value
for axis in ['u', 'v', 'transpose']:
    value = pair(smooth=True)
    s = value['sources'][1]
    controls = [s['control_points'][4*j:4*j+4] for j in range(4)]
    if axis == 'u': controls = [row[::-1] for row in controls]
    elif axis == 'v': controls = controls[::-1]
    else: controls = [list(row) for row in zip(*controls)]
    s['control_points'] = [p for row in controls for p in row]
    HOLDOUTS['merge_second_' + axis] = value
value = pair()
for j in [1, 2]: value['sources'][1]['control_points'][4*j]['point'][0] += .01
HOLDOUTS['merge_bowed_boundary'] = value
CASES.update(copy.deepcopy(HOLDOUTS))

FINAL = {}
for smooth in [False, True]:
    for gauge in [10., .001]:
        value = pair(rational=True, smooth=smooth)
        for point in value['sources'][1]['control_points']: point['weight'] *= gauge
        FINAL['merge_gauge_' + ('smooth' if smooth else 'sharp') + ('_large' if gauge > 1. else '_small')] = value
value = copy.deepcopy(HOLDOUTS['merge_along_degrees'])
value['sources'][0].update(domain_u=[2., 3.], domain_v=[-3., 5.])
value['sources'][1].update(domain_u=[7., 8.], domain_v=[10., 20.])
FINAL['merge_degrees_nonunit'] = value
CASES.update(copy.deepcopy(FINAL))


def request(names):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-9, angular=1e-6),
                operations=[dict(op='merge_surfaces_sdk', id=name, case=name) for name in names])
