"""Closed-source Fillet recipes, authored before observing command results."""
from math import sqrt


def pilot(case, explicit, pick):
    circle = dict(type='circle', center=[0., 0., 0.], radius=5.,
                  x_axis=[1., 0., 0.], normal=[0., 0., 1.])
    line = dict(type='line', start=[0., -8., 0.], end=[0., 8., 0.])
    other = dict(circle, center=[6., 0., 0.])
    result = {}
    for label, curves, picks in [
        ('circle_line', [circle, line], [pick(0, [2.5, sqrt(18.75), 0.]), pick(1, [0., 6.8, 0.])]),
        ('two_circles', [circle, other], [pick(0, [1.5, sqrt(22.75), 0.]), pick(1, [4.5, sqrt(22.75), 0.])]),
    ]:
        for suffix, trim, join in [('arc', False, False), ('parts', True, False), ('joined', True, True)]:
            result[label + '_' + suffix] = case(explicit(radius=.5, trim=trim, join=join) + picks, curves=curves)
    return result


def holdouts(case, explicit, pick):
    from copy import deepcopy
    original = pilot(case, explicit, pick)
    result = {}
    for base in ['circle_line_joined', 'two_circles_joined']:
        recipe = deepcopy(original[base])
        recipe['actions'][-2:] = list(reversed(recipe['actions'][-2:]))
        result[base + '_reverse'] = recipe
    recipe = deepcopy(original['circle_line_parts'])
    recipe['actions'][-2:] = [pick(0, [2.5, -sqrt(18.75), 0.]), pick(1, [0., -6.8, 0.])]
    result['circle_line_lower'] = recipe
    recipe = deepcopy(original['circle_line_parts'])
    recipe['actions'][-1] = pick(1, [0., 2., 0.])
    result['circle_line_inner'] = recipe
    recipe = deepcopy(original['circle_line_parts'])
    points = [[5, 0, 0], [5, 5, 0], [0, 5, 0], [-5, 5, 0], [-5, 0, 0], [-5, -5, 0], [0, -5, 0], [5, -5, 0], [5, 0, 0]]
    recipe['curves'][0] = dict(type='nurbs', degree=2, control_points=[dict(point=p, weight=1. if i % 2 == 0 else sqrt(.5)) for i, p in enumerate(points)], knots=[0., 0., 0., 1., 1., 2., 2., 3., 3., 4., 4., 4.])
    result['nurbs_circle_line'] = recipe
    recipe = deepcopy(original['circle_line_parts'])
    recipe['curves'][0] = dict(type='ellipse', center=[0., 0., 0.], x_axis=[1., 0., 0.], y_axis=[0., 1., 0.], radius_x=5., radius_y=3.)
    recipe['actions'][-2] = pick(0, [3., 2.4, 0.])
    result['ellipse_line'] = recipe
    recipe = deepcopy(original['circle_line_joined'])
    def spatial(p): return [20. + .6 * p[0], -8. + p[1], 3. + .8 * p[0]]
    recipe['curves'][0].update(center=spatial([0., 0., 0.]), x_axis=[.6, 0., .8], normal=[-.8, 0., .6])
    recipe['curves'][1] = dict(type='line', start=spatial([0., -8., 0.]), end=spatial([0., 8., 0.]))
    recipe['actions'][-2:] = [pick(0, spatial([2.5, sqrt(18.75), 0.])), pick(1, spatial([0., 6.8, 0.]))]
    recipe['view'] = 'Perspective'
    result['circle_line_spatial'] = recipe
    for suffix, tail in [('enter', ['Enter']), ('radius', [1.25, 'Enter']), ('cancel', [1.25, 'Cancel'])]:
        recipe = deepcopy(original['circle_line_joined'])
        recipe['actions'] = explicit(dynamic=True, join=True) + recipe['actions'][-2:] + tail
        if suffix == 'cancel': recipe['pending_redo'] = True
        result['closed_dynamic_' + suffix] = recipe
    return result
