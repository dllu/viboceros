"""Fixed mesh Boolean selection, attributes and history workflows."""
from copy import deepcopy
if __package__:
    from .mesh_boolean_cases import PAIRS, box
else:
    from mesh_boolean_cases import PAIRS, box


def scene(operation='union', shape='overlap', preselect=True, **extra):
    sources = [dict(kind='mesh', **deepcopy(m)) for m in PAIRS[shape]]
    result = dict(operation=operation, sources=sources, preselect=preselect)
    result.update(extra)
    return result


CASES = {}
for operation in ['union', 'intersection', 'difference']:
    for shape in ['overlap', 'disjoint', 'contained', 'identical', 'face_touch']:
        CASES[operation+'_'+shape+'_pre'] = scene(operation, shape)
    CASES[operation+'_overlap_post'] = scene(operation, preselect=False)
    CASES[operation+'_keep'] = scene(operation, preselect=False, arguments='_DeleteInput=_No')
    CASES[operation+'_reset'] = scene(operation, preselect=False, arguments='_DeleteInput=_Yes')
    CASES[operation+'_cancel'] = scene(operation, preselect=False, cancel=True)
CASES['union_three'] = scene()
CASES['union_three']['sources'].append(dict(kind='mesh', **box((4., 2., 1.))))
CASES['union_mixed_point'] = scene()
CASES['union_mixed_point']['sources'].append(dict(kind='point', point=[8., 7., 6.]))
CASES['union_brep_boxes'] = scene()
for source in CASES['union_brep_boxes']['sources']:
    source['kind'] = 'brep_box'
CASES['union_one'] = scene()
CASES['union_one']['sources'] = CASES['union_one']['sources'][:1]
CASES['union_identical_normals'] = scene(shape='identical', normals=True)
CASES['difference_two_cutters'] = scene('difference')
CASES['difference_two_cutters']['sources'].append(dict(kind='mesh', **box((-2., -1., -.5))))

# The first family records discovery, including rejected option placement and
# incomplete intersection sets. This family supplies both getters explicitly.
LIFECYCLE = {name: deepcopy(value) for name, value in CASES.items()
             if name.startswith('union_') and name not in ['union_keep', 'union_reset']}
for operation in ['intersection', 'difference']:
    for shape in ['overlap', 'disjoint', 'contained', 'identical', 'face_touch']:
        LIFECYCLE[operation+'_'+shape+'_pre'] = scene(operation, shape, first=[0], second=[1])
    LIFECYCLE[operation+'_overlap_post'] = scene(operation, preselect=False, first=[0], second=[1])
LIFECYCLE['union_reverse_post'] = scene(preselect=False, selected=[1, 0])
LIFECYCLE['union_reverse_pre'] = scene(selected=[1, 0])
LIFECYCLE['union_partial_components'] = scene()
LIFECYCLE['union_partial_components']['sources'].append(dict(kind='mesh', **box((12., 0., 0.))))
LIFECYCLE['union_disjoint_first'] = scene()
LIFECYCLE['union_disjoint_first']['sources'].insert(0, dict(kind='mesh', **box((12., 0., 0.))))
for label in ['Literal', 'Auto']:
    LIFECYCLE['union_tolerance_'+label.lower()] = scene(preselect=False, arguments='_Tolerance=_'+label)
LIFECYCLE['difference_two_cutters'] = deepcopy(CASES['difference_two_cutters'])
for label, arguments in [('keep', '_DeleteInput=_No'), ('after_keep', ''),
                         ('reset', '_DeleteInput=_Yes'), ('after_reset', '')]:
    LIFECYCLE['difference_'+label] = scene('difference', preselect=False, second_arguments=arguments)
LIFECYCLE['difference_cancel_second'] = scene('difference', preselect=False,
    second_arguments='_DeleteInput=_No', cancel_second=True)
LIFECYCLE['difference_after_cancel'] = scene('difference', preselect=False)
LIFECYCLE['difference_target_inside_cutter'] = scene('difference', 'contained', first=[1], second=[0])
LIFECYCLE['difference_through_hole'] = scene('difference')
LIFECYCLE['difference_through_hole']['sources'][1] = dict(kind='mesh', **box((1., 1., -1.), (1., 1., 4.)))
LIFECYCLE['intersection_cancel_second'] = scene('intersection', preselect=False, first=[0], second=[1], cancel_second=True)
LIFECYCLE['intersection_first_many'] = scene('intersection', preselect=False, first=[0, 1], second=[2])
LIFECYCLE['intersection_first_many']['sources'].append(dict(kind='mesh', **box((1., 0., 0.), (3., 4., 3.))))
LIFECYCLE = {'boolean_lifecycle_'+name: value for name, value in LIFECYCLE.items()}
if __package__:
    from .mesh_boolean_option_cases import option_cases, memory_cases, syntax_cases, tolerance_cases
else:
    from mesh_boolean_option_cases import option_cases, memory_cases, syntax_cases, tolerance_cases
OPTIONS = option_cases(scene, box)
MEMORY = memory_cases(scene)
SYNTAX = syntax_cases(scene)
TOLERANCE = tolerance_cases(scene, box)
FAMILIES = dict(commands=CASES, lifecycle=LIFECYCLE, options=OPTIONS, memory=MEMORY, syntax=SYNTAX, tolerance=TOLERANCE,
                policy={'boolean_policy_auto_defaults': scene(preselect=False, arguments='_Tolerance=_Auto', document_tolerance=1e-3)})
ALL_CASES = {name: value for cases in FAMILIES.values() for name, value in cases.items()}


def request(family='commands'):
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-6, relative=1e-12, angular=1e-6),
                operations=[dict(op='mesh_boolean_command', id=name, case=name) for name in FAMILIES[family]])


def validate(operation):
    if not (isinstance(operation, dict) and set(operation) == {'op', 'id', 'case'}
            and operation.get('op') == 'mesh_boolean_command'
            and isinstance(operation.get('case'), str) and operation['case'] in ALL_CASES
            and operation['id'] == operation['case']):
        raise ValueError('fixed mesh Boolean command recipe required')
