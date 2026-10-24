"""Numeric tolerance, preference lifetime and disconnected-result controls."""

def option_cases(scene, box):
    OPTIONS = {}
    for index, gap in enumerate([1e-6, 1e-4, 1e-2, -1e-6, -1e-4]):
        for mode in ['Auto', 'Literal']:
            value = scene(preselect=False, arguments='_Tolerance=_'+mode, document_tolerance=1e-3)
            value['sources'][1] = dict(kind='mesh', **box((4.+gap, 0., 0.)))
            OPTIONS['boolean_options_gap_{}_{}'.format(index, mode.lower())] = value
    for operation in ['intersection', 'difference']:
        for mode in ['Auto', 'Literal']:
            value = scene(operation, preselect=False, first=[0], second=[1],
                          arguments='_Tolerance=_'+mode, document_tolerance=1e-3)
            value['sources'][1] = dict(kind='mesh', **box((4.-1e-4, 0., 0.)))
            OPTIONS['boolean_options_{}_{}'.format(operation, mode.lower())] = value
    for index, tolerance in enumerate([1e-2, 1e-4]):
        for mode in ['Auto', 'Literal']:
            value = scene(preselect=False, arguments='_Tolerance=_'+mode, document_tolerance=tolerance)
            value['sources'][1] = dict(kind='mesh', **box((4.+1e-5, 0., 0.)))
            OPTIONS['boolean_options_document_{}_{}'.format(index, mode.lower())] = value
    OPTIONS['boolean_options_difference_disjoint_cutter'] = scene('difference')
    OPTIONS['boolean_options_difference_disjoint_cutter']['sources'].append(dict(kind='mesh', **box((12., 0., 0.))))
    OPTIONS['boolean_options_intersection_disconnected'] = scene('intersection', first=[0, 1], second=[2])
    OPTIONS['boolean_options_intersection_disconnected']['sources'] = [
        dict(kind='mesh', **box()), dict(kind='mesh', **box((8., 0., 0.))),
        dict(kind='mesh', **box((2., 1., .5), (10., 3., 2.)))]
    OPTIONS['boolean_options_difference_sever'] = scene('difference')
    OPTIONS['boolean_options_difference_sever']['sources'][1] = dict(kind='mesh', **box((1., -1., -1.), (2., 5., 4.)))
    return OPTIONS


def memory_cases(scene):
    MEMORY = {
        'boolean_memory_union_literal': scene(preselect=False, arguments='_Tolerance=_Literal'),
        'boolean_memory_intersection_after_union': scene('intersection', preselect=False, first=[0], second=[1]),
        'boolean_memory_difference_after_union': scene('difference', preselect=False),
        'boolean_memory_difference_cancel_keep': scene('difference', preselect=False, second_arguments='_DeleteInput=_No', cancel_second=True),
        'boolean_memory_intersection_auto': scene('intersection', preselect=False, first=[0], second=[1], arguments='_Tolerance=_Auto'),
        'boolean_memory_union_after_intersection': scene(preselect=False),
        'boolean_memory_union_cancel_literal': scene(preselect=False, arguments='_Tolerance=_Literal', cancel=True),
        'boolean_memory_difference_after_cancel': scene('difference', preselect=False),
    }
    return MEMORY


def syntax_cases(scene):
    return {
        'boolean_syntax_union_numeric_equals': scene(preselect=False, arguments='_Tolerance=0.0001'),
        'boolean_syntax_intersection_after_union': scene('intersection', preselect=False, first=[0], second=[1]),
        'boolean_syntax_difference_after_union': scene('difference', preselect=False),
        'boolean_syntax_union_numeric_split': scene(preselect=False, arguments='_Tolerance 0.001'),
        'boolean_syntax_union_auto': scene(preselect=False, arguments='_Tolerance=_Auto'),
        'boolean_syntax_difference_numeric': scene('difference', preselect=False, second_arguments='_Tolerance=0.00001'),
        'boolean_syntax_intersection_after_difference': scene('intersection', preselect=False, first=[0], second=[1]),
        'boolean_syntax_union_after_difference': scene(preselect=False),
        'boolean_syntax_union_cancel_numeric': scene(preselect=False, arguments='_Tolerance=0.01', cancel=True),
        'boolean_syntax_difference_after_cancel': scene('difference', preselect=False),
    }


def tolerance_cases(scene, box):
    rows = {
        'union_document': scene(preselect=False, arguments='_Tolerance=_Document'),
        'intersection_after_document': scene('intersection', preselect=False, first=[0], second=[1]),
        'difference_after_document': scene('difference', preselect=False),
        'union_zero': scene(preselect=False, arguments='_Tolerance=_Zero'),
        'intersection_after_zero': scene('intersection', preselect=False, first=[0], second=[1]),
        'difference_after_zero': scene('difference', preselect=False),
        'union_cancel_document': scene(preselect=False, arguments='_Tolerance=_Document', cancel=True),
        'difference_after_cancel': scene('difference', preselect=False),
        'intersection_auto': scene('intersection', preselect=False, first=[0], second=[1], arguments='_Tolerance=_Auto'),
        'union_after_auto': scene(preselect=False),
    }
    for gap_index, gap in enumerate([1e-4, -1e-4]):
        for mode in ['Auto', 'Document', 'Zero']:
            row = scene(preselect=False, arguments='_Tolerance=_'+mode, document_tolerance=1e-3)
            row['sources'][1] = dict(kind='mesh', **box((4.+gap, 0., 0.)))
            rows['gap_{}_{}'.format(gap_index, mode.lower())] = row
    for operation in ['intersection', 'difference']:
        for mode in ['Document', 'Zero']:
            row = scene(operation, preselect=False, first=[0], second=[1],
                        arguments='_Tolerance=_'+mode, document_tolerance=1e-3)
            row['sources'][1] = dict(kind='mesh', **box((4.-1e-4, 0., 0.)))
            rows[operation+'_'+mode.lower()] = row
    return {'boolean_tolerance_'+name: row for name, row in rows.items()}
