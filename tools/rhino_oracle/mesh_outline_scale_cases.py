"""Separate face representation, clipping, model scale and document tolerance."""
from copy import deepcopy


def scale_cases(shapes, planes):
    result = {}
    forms = {'quad': [[0, 1, 2, 3]], 'bd': [[0, 1, 3], [1, 2, 3]],
             'ac': [[0, 1, 2], [0, 2, 3]]}

    def make(form, near, far=80., source_scale=1., view_scale=1., tolerance=None):
        mesh = deepcopy(shapes['warped'])
        mesh['faces'] = deepcopy(forms[form])
        mesh['vertices'] = [[source_scale*x for x in p] for p in mesh['vertices']]
        value = dict(mesh=mesh, method='info_plane', view='perspective', plane=deepcopy(planes['xy']),
                     frustum=[-.5*near, .5*near, -.5*near, .5*near, near, far],
                     camera=dict(location=[source_scale*x for x in [12., 16.5, 22.]],
                                 target=[source_scale*x for x in [2., 1.5, 2.]], up=[0., 0., 1.]))
        value['frustum'] = [view_scale*x for x in value['frustum']]
        if tolerance is not None:
            value['document_tolerance'] = tolerance
        return value

    for index, near in enumerate([1., .5, .2, .1, .05, .02, .01462198271019321, .01, .005, .001]):
        for form in forms:
            result['outline_scale_near_' + str(index) + '_' + form] = make(form, near)
    for index, tol in enumerate([1e-2, 1e-3, 1e-6, 1e-9]):
        for name, near in [('small', .015), ('large', 1.)]:
            for form in ['quad', 'bd']:
                result['outline_scale_tolerance_' + str(index) + '_' + name + '_' + form] = make(
                    form, near, tolerance=tol)
    for index, scale in enumerate([.001, .01, .1, 1., 10., 100.]):
        result['outline_scale_source_' + str(index)] = make('bd', 1., source_scale=scale)
        result['outline_scale_uniform_' + str(index)] = make('bd', 1., source_scale=scale, view_scale=scale)
    return result


def context_cases(shapes, planes):
    result = {}
    for index, far in enumerate([20., 25., 28., 29., 29.5, 29.539359010491335, 30., 40., 80., 120., 1000.]):
        for form, faces in [('quad', [[0, 1, 2, 3]]), ('bd', [[0, 1, 3], [1, 2, 3]])]:
            mesh = deepcopy(shapes['warped'])
            mesh['faces'] = faces
            near = .01462198271019321
            result['outline_context_far_' + str(index) + '_' + form] = dict(
                mesh=mesh, method='info_plane', view='perspective', plane=deepcopy(planes['xy']),
                frustum=[-.5*near, .5*near, -.5*near, .5*near, near, far], capture_transform=True)
    return result


def facing_cases(shapes, planes):
    result = {}
    template = context_cases(shapes, planes)['outline_context_far_5_quad']
    for far in range(40, 81):
        case = deepcopy(template)
        case['frustum'][-1] = float(far)
        result['outline_facing_far_' + str(far)] = case
    for far in [40, 80]:
        for reverse in [False, True]:
            for start in range(4):
                if not reverse and start == 0:
                    continue
                case = deepcopy(template)
                case['frustum'][-1] = float(far)
                face = [0, 1, 2, 3]
                if reverse:
                    face = list(reversed(face))
                case['mesh']['faces'] = [face[start:] + face[:start]]
                result['outline_facing_order_' + str(far) + '_' + str(int(reverse)) + str(start)] = case
    return result
