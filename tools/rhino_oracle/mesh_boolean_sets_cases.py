"""Independent multi-set and Split command ownership controls."""
from copy import deepcopy


def set_cases(scene, box):
    rows = {}
    def sources(*meshes):
        return [dict(kind='mesh', **deepcopy(mesh)) for mesh in meshes]
    layouts = {
        'disconnected': [box(), box((8., 0., 0.)), box((2., 1., .5), (10., 3., 2.))],
        'untouched_peer': [box(), box((20., 0., 0.)), box((2., 1., .5))],
        'all_disjoint': [box(), box((8., 0., 0.)), box((20., 1., .5))],
        'overlapping_targets': [box(), box((2., 1., .5)), box((1., 0., 0.), (3., 4., 3.))],
        'nested_targets': [box(), box((1., 1., .5), (1., 1., 1.)), box((2., 0., 0.))],
        'hole': [box(), box((8., 0., 0.)), box((1., 1., -1.), (10., 1., 4.))],
    }
    for operation in ['intersection', 'difference']:
        for name, meshes in layouts.items():
            for preselect in [True, False]:
                rows['boolean_sets_{}_{}_{}'.format(operation, name, 'pre' if preselect else 'post')] = scene(
                    operation, preselect=preselect, sources=sources(*meshes), first=[0, 1], second=[2])
        for preselect in [True, False]:
            rows['boolean_sets_{}_reverse_{}'.format(operation, 'pre' if preselect else 'post')] = scene(
                operation, preselect=preselect, sources=sources(*layouts['disconnected']), first=[1, 0], second=[2])
        rows['boolean_sets_'+operation+'_two_cutters'] = scene(operation, preselect=False,
            sources=sources(box(), box((-2., -1., -.5)), box((2., 1., .5))), first=[0], second=[1, 2])
    rows['boolean_sets_difference_keep'] = deepcopy(rows['boolean_sets_difference_disconnected_post'])
    rows['boolean_sets_difference_keep']['second_arguments'] = '_DeleteInput=_No'
    rows['boolean_sets_difference_after_keep'] = deepcopy(rows['boolean_sets_difference_untouched_peer_post'])
    return rows


def split_cases(scene, box):
    rows = {}
    for shape in ['overlap', 'disjoint', 'contained', 'identical', 'face_touch']:
        for preselect in [True, False]:
            rows['boolean_split_{}_{}'.format(shape, 'pre' if preselect else 'post')] = scene(
                'split', shape, preselect=preselect, first=[0], second=[1])
    for name, arguments in [('keep', '_DeleteInput=_No'), ('after_keep', ''),
                            ('reset', '_DeleteInput=_Yes'), ('after_reset', '')]:
        rows['boolean_split_'+name] = scene('split', preselect=False, first=[0], second=[1], second_arguments=arguments)
    rows['boolean_split_cancel_first'] = scene('split', preselect=False, first=[0], second=[1], cancel=True)
    rows['boolean_split_cancel_second'] = scene('split', preselect=False, first=[0], second=[1], cancel_second=True)
    for mode in ['Document', 'Zero', 'Auto']:
        rows['boolean_split_tolerance_'+mode.lower()] = scene('split', preselect=False, first=[0], second=[1], arguments='_Tolerance=_'+mode)
    rows['boolean_split_sever'] = scene('split', first=[0], second=[1])
    rows['boolean_split_sever']['sources'][1] = dict(kind='mesh', **box((1., -1., -1.), (2., 5., 4.)))
    rows['boolean_split_brep_boxes'] = scene('split', first=[0], second=[1])
    for source in rows['boolean_split_brep_boxes']['sources']:
        source['kind'] = 'brep_box'
    rows['boolean_split_many_targets'] = scene('split', preselect=False, first=[0, 1], second=[2])
    rows['boolean_split_many_targets']['sources'] = [dict(kind='mesh', **m) for m in [
        box(), box((8., 0., 0.)), box((2., 1., .5), (10., 3., 2.))]]
    rows['boolean_split_two_cutters'] = scene('split', first=[0], second=[1, 2])
    rows['boolean_split_two_cutters']['sources'] = [dict(kind='mesh', **m) for m in [
        box(), box((.5, -1., -1.), (1., 5., 4.)), box((2.5, -1., -1.), (1., 5., 4.))]]
    return rows


def split_controls(scene, box):
    rows = {
        'target_inside': scene('split', 'contained', first=[1], second=[0]),
        'cancel_document': scene('split', preselect=False, first=[0], second=[1], second_arguments='_Tolerance=_Document', cancel_second=True),
        'union_after_split': scene(preselect=False),
        'difference_keep': scene('difference', preselect=False, second_arguments='_DeleteInput=_No'),
        'split_after_difference': scene('split', preselect=False, first=[0], second=[1]),
        'difference_after_split': scene('difference', preselect=False),
    }
    for preselect in [True, False]:
        row = scene('split', preselect=preselect, first=[1, 0], second=[2])
        row['sources'] = [dict(kind='mesh', **m) for m in [
            box(), box((8., 0., 0.)), box((2., 1., .5), (10., 3., 2.))]]
        rows['reverse_'+('pre' if preselect else 'post')] = row
    rows['untouched_peer'] = deepcopy(rows['reverse_post'])
    rows['untouched_peer']['first'] = [0, 1]
    rows['untouched_peer']['sources'][1] = dict(kind='mesh', **box((20., 0., 0.)))
    rows['two_cutters_repeat'] = deepcopy(split_cases(scene, box)['boolean_split_two_cutters'])
    rows['two_cutters_repeat']['normals'] = True
    rows['disjoint_cutter'] = scene('split', preselect=False, first=[0], second=[1, 2])
    rows['disjoint_cutter']['sources'].insert(1, dict(kind='mesh', **box((20., 0., 0.))))
    return {'boolean_split_controls_'+name: row for name, row in rows.items()}


def split_stages(scene, box):
    rows = {}
    rows['no_op_last'] = scene('split', preselect=False, first=[0], second=[1, 2])
    rows['no_op_last']['sources'].append(dict(kind='mesh', **box((20., 0., 0.))))
    rows['three_cutters'] = scene('split', preselect=False, first=[0], second=[1, 2, 3])
    rows['three_cutters']['sources'] = [dict(kind='mesh', **m) for m in [box()] + [
        box((x, -1., -1.), (.5, 5., 4.)) for x in [.25, 1.25, 2.25]]]
    rows['contained_second'] = scene('split', preselect=False, first=[0], second=[1, 2])
    rows['contained_second']['sources'].append(dict(kind='mesh', **box((-1., -1., -1.), (8., 8., 8.))))
    return {'boolean_split_stages_'+name: value for name, value in rows.items()}
