"""Public same-parent picks authored independently of Fillet outputs."""


def pilot(case, explicit, pick, shot):
    result = {}
    for label, points, picks in [
        ('open', [[0., 0., 0.], [4., 0., 0.], [4., 4., 0.]], [[2.5, 0., 0.], [4., 1.5, 0.]]),
        ('closed', [[0., 0., 0.], [6., 0., 0.], [6., 4., 0.], [0., 4., 0.], [0., 0., 0.]], [[4.5, 0., 0.], [6., 1.5, 0.]]),
    ]:
        result['self_' + label + '_cancel'] = case(explicit(dynamic=True, join=True) + [pick(0, p) for p in picks] + [shot('ready'), 'Cancel'], curves=[dict(type='polyline', vertices=points)], pending_redo=True)
    return result


def discovery(case, explicit, pick):
    result = {}
    sources = [
        ('open', [[0., 0., 0.], [4., 0., 0.], [4., 4., 0.]], [[2.5, 0., 0.], [4., 1.5, 0.]]),
        ('closed', [[0., 0., 0.], [6., 0., 0.], [6., 4., 0.], [0., 4., 0.], [0., 0., 0.]], [[4.5, 0., 0.], [6., 1.5, 0.]]),
    ]
    for label, points, picks in sources:
        for suffix, trim, join in [('arc', False, False), ('parts', True, False), ('joined', True, True)]:
            result['self_' + label + '_' + suffix] = case(explicit(trim=trim, join=join) + [pick(0, p) for p in picks], curves=[dict(type='polyline', vertices=points)])
    label, points, picks = sources[0]
    result['self_open_reverse'] = case(explicit(join=True) + [pick(0, p) for p in reversed(picks)], curves=[dict(type='polyline', vertices=points)])
    result['self_zero_joined'] = case(explicit(join=True, radius=0.) + [pick(0, p) for p in picks], curves=[dict(type='polyline', vertices=points)])
    points = sources[1][1]
    result['self_closed_seam'] = case(explicit(join=True) + [pick(0, [0., 1.5, 0.]), pick(0, [1.5, 0., 0.])], curves=[dict(type='polyline', vertices=points)])
    result['self_closed_numeric'] = case(explicit(dynamic=True, join=True) + [pick(0, p) for p in sources[1][2]] + [1.25, 'Enter'], curves=[dict(type='polyline', vertices=points)])
    curved = dict(type='polycurve', segments=[dict(type='line', start=[0., 0., 0.], end=[4., 0., 0.]), dict(type='arc', points=[[4., 0., 0.], [3. + 2.**-.5, 2.**-.5, 0.], [3., 1., 0.]])])
    result['self_curved_junction'] = case(explicit(join=True, radius=.2) + [pick(0, [2.5, 0., 0.]), pick(0, [3.6, .8, 0.])], curves=[curved])
    return result


def holdouts(case, explicit, pick):
    from copy import deepcopy
    original = discovery(case, explicit, pick)
    result = {}
    for base in ['self_open_parts', 'self_closed_joined', 'self_closed_parts', 'self_closed_seam']:
        recipe = deepcopy(original[base]); recipe['actions'][-2:] = list(reversed(recipe['actions'][-2:]))
        result[base + '_reversed'] = recipe
    recipe = deepcopy(original['self_closed_seam']); recipe['actions'][3] = 'Join=No'
    result['self_seam_parts'] = recipe
    points = [[-2., -1., 0.], [0., -1., 0.], [0., 0., 0.], [4., 0., 0.], [4., 4., 0.], [6., 4., 0.]]
    result['self_interior_corner'] = case(explicit(join=True) + [pick(0, [2.5, 0., 0.]), pick(0, [4., 1.5, 0.])], curves=[dict(type='polyline', vertices=points)])
    def spatial(p): return [7. + .6 * p[0], -2. + p[1], 3. + .8 * p[0]]
    recipe = deepcopy(original['self_open_joined']); recipe['curves'][0]['vertices'] = [spatial(p) for p in recipe['curves'][0]['vertices']]
    for action in recipe['actions'][-2:]: action['pick'] = spatial(action['pick'])
    result['self_spatial'] = recipe
    points = [[0., 0., 0.], [6., 0., 0.], [6., 4., 0.], [10., 4., 0.], [10., 10., 0.]]
    result['self_nonadjacent'] = case(explicit(join=True, radius=1.) + [pick(0, [4., 0., 0.]), pick(0, [10., 6., 0.])], curves=[dict(type='polyline', vertices=points)], pending_redo=True)
    arch = dict(type='nurbs', degree=2, control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [4., 0., 0.], [4., 4., 0.]]], knots=[0., 0., 0., 1., 1., 1.])
    result['self_smooth_nurbs'] = case(explicit(join=True) + [pick(0, [1.75, .25, 0.]), pick(0, [3.75, 2.25, 0.])], curves=[arch], pending_redo=True)
    recipe = deepcopy(original['self_open_joined']); points = recipe['curves'][0]['vertices']
    recipe['curves'][0] = dict(type='nurbs', degree=1, control_points=[dict(point=p, weight=1.) for p in points], knots=[0., 0., 1., 2., 2.])
    result['self_kinked_nurbs'] = recipe
    recipe = deepcopy(original['self_open_joined']); recipe['actions'][-2:] = [pick(0, [2., 0., 0.]), pick(0, [3., 0., 0.])]; recipe['pending_redo'] = True
    result['self_same_leaf'] = recipe
    return result


def branches(case, explicit, pick):
    from copy import deepcopy
    original = discovery(case, explicit, pick); result = {}
    for name in ['self_open_joined', 'self_closed_joined']:
        recipe = deepcopy(original[name]); recipe['curves'][0]['vertices'].reverse()
        result[name + '_backward_source'] = recipe
    recipe = deepcopy(original['self_open_joined']); recipe['actions'][-2:] = [pick(0, [3.7, 0., 0.]), pick(0, [4., .3, 0.])]
    result['self_near_corner'] = recipe
    def translated(p): return [p[0] + 1e7, p[1] - 2e7, p[2] + 3e7]
    recipe = deepcopy(original['self_open_joined']); recipe['curves'][0]['vertices'] = [translated(p) for p in recipe['curves'][0]['vertices']]
    for a in recipe['actions'][-2:]: a['pick'] = translated(a['pick'])
    result['self_far_origin'] = recipe
    points = [[0., 0., 0.], [10., 0., 0.], [10., 10., 0.], [0., 10., 0.], [0., 1., 0.], [9., 1., 0.], [9., 9., 0.], [1., 9., 0.], [1., 2., 0.]]
    result['self_folded_path'] = case(explicit(join=True) + [pick(0, [3., 0., 0.]), pick(0, [7., 10., 0.])], curves=[dict(type='polyline', vertices=points)])
    recipe = deepcopy(original['self_open_joined']); recipe['actions'][1] = 6.; recipe['pending_redo'] = True
    result['self_oversized'] = recipe
    recipe = deepcopy(original['self_closed_joined']); recipe['actions'] = explicit(dynamic=True, join=True) + recipe['actions'][-2:] + ['Trim=No', 'Enter']
    result['self_dynamic_untrimmed'] = recipe
    recipe = deepcopy(original['self_open_joined']); recipe['actions'] = explicit(dynamic=True, join=True) + recipe['actions'][-2:] + [0., 'Cancel']; recipe['pending_redo'] = True
    result['self_dynamic_zero_cancel'] = recipe
    return result


def limits(case, explicit, pick):
    result = {}
    source = dict(type='polyline', vertices=[[0., 0., 0.], [10., 0., 0.], [10., 10., 0.], [0., 10., 0.]])
    for label, radius, reverse in [('small', .5, False), ('half_gap', 5., False), ('half_gap_reverse', 5., True), ('large', 6., False)]:
        picks = [pick(0, [3., 0., 0.]), pick(0, [7., 10., 0.])]
        result['self_parallel_' + label] = case(explicit(join=True, radius=radius) + (list(reversed(picks)) if reverse else picks), curves=[source], pending_redo=True)
    source = dict(type='nurbs', degree=3, control_points=[dict(point=p, weight=1.) for p in [[0., 0., 0.], [5., 0., 0.], [5., 4., 0.], [0., 0., 0.]]], knots=[0., 0., 0., 0., 1., 1., 1., 1.])
    for label, join in [('parts', False), ('joined', True)]:
        result['self_nurbs_seam_' + label] = case(explicit(join=join, radius=.2) + [pick(0, [1.35, .108, 0.]), pick(0, [1.35, .972, 0.])], curves=[source], pending_redo=True)
    return result
