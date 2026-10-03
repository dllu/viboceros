"""Owned Move/Copy input recipes, prescribed independently of native results."""
import copy

SOURCES = [[2, 3, 4], [-1, 2, 3], [5, -2, 1], [1, 1, 1]]


def request():
    operations = []
    def add(command, label, inputs, selection='pre', grouped=False, finish=None, selected=None, history=True, plane=None):
        row = dict(op='transform_copy_command', id=command.lower()+'-'+label, command=command,
                   sources=copy.deepcopy(SOURCES), grouped=grouped, inputs=inputs,
                   finish=finish or ('Enter' if command == 'Copy' else 'Automatic'),
                   undo_redo=history, sel_last=history)
        if selection == 'pre': row['selected'] = [2, 0] if selected is None else selected
        else: row['source_selection'] = [2, 0, 'Enter'] if selection == 'post' else selection
        if plane is not None: row['cplane'] = plane
        operations.append(row)
    for command in ['Move', 'Copy']:
        for selection in ['pre', 'post']:
            for grouped in [False, True]:
                for name, inputs in [('pair', ['w1,2,3', 'w6,-2,8']),
                                     ('identity', ['w1,2,3', 'w1,2,3']),
                                     ('center', ['Enter', 'w6,-2,8'])]:
                    add(command, selection+'-'+str(grouped)+'-'+name, inputs, selection, grouped)
            for phase, inputs in [('base', ['Vertical']), ('target', ['w1,2,3'])]:
                add(command, selection+'-cancel-'+phase, inputs, selection, finish='Cancel', history=False)
        for phase, steps in [('empty-enter', ['Enter']), ('empty-cancel', ['Cancel']),
                             ('picked-cancel', [2, 'Cancel']), ('none', [2, 'SelNone', 'Enter'])]:
            add(command, phase, [], steps, finish='Automatic', history=False)
        add(command, 'all', ['w1,2,3', 'w6,-2,8'], ['SelAll', 'Enter'], grouped=True)
        add(command, 'reselect', ['w1,2,3', 'w6,-2,8'], [0, 'SelNone', 2, 'Enter'], grouped=True)
        for selected in [[2], [0, 1, 2, 3]]:
            add(command, 'group-size-'+str(len(selected)), ['w1,2,3', 'w6,-2,8'], grouped=True, selected=selected)
    for selection in ['pre', 'post']:
        for finish in ['Enter', 'Cancel']:
            add('Copy', selection+'-repeat-'+finish, ['w1,2,3', 'w6,-2,8', 'w9,7,-1', 'Undo', 'w0,3,4'], selection, True, finish)
        add('Copy', selection+'-inplace', ['InPlace'], selection, True, 'Automatic')
        add('Copy', selection+'-inplace-single', ['InPlace'], [2, 'Enter'] if selection == 'post' else 'pre', True, 'Automatic', selected=[2])
        add('Copy', selection+'-empty-first-target', ['w1,2,3', 'Enter'], selection, history=False)
        for option in ['FromLastPoint', 'UseLastDistance', 'UseLastDirection']:
            for target, label in [('w9,7,-1', 'perpendicular'), ('w-7,8,-4', 'negative'), ('w8,6,11', 'spatial')]:
                add('Copy', selection+'-'+option+'-'+label,
                    ['w1,2,3', 'w6,-2,8', option+'=Yes', target, option+'=No', 'w0,3,4'], selection, True)
        for combination in [('UseLastDistance=Yes', 'UseLastDirection=Yes'),
                            ('FromLastPoint=Yes', 'UseLastDistance=Yes'),
                            ('FromLastPoint=Yes', 'UseLastDirection=Yes'),
                            ('FromLastPoint=Yes', 'UseLastDistance=Yes', 'UseLastDirection=Yes')]:
            add('Copy', selection+'-combined-'+str(len(operations)),
                ['w1,2,3', 'w6,-2,8']+list(combination)+['w8,6,11', 'w-7,8,-4'], selection, True)
        add('Copy', selection+'-early-options-rejected',
            ['w1,2,3', 'FromLastPoint=Yes', 'UseLastDistance=Yes', 'UseLastDirection=Yes', 'w6,-2,8', 'w9,7,-1'], selection)
    planes = [('world', None), ('tilted', dict(origin=[5, -6, 2], x_axis=[.8, .6, 0], y_axis=[-.36, .48, .8]))]
    for command in ['Move', 'Copy']:
        for plane_name, plane in planes:
            for selection in ['pre', 'post']:
                for phase, inputs in [('base', ['Vertical', 'w1,2,3', 'w6,-2,8']),
                                      ('center', ['Vertical', 'Enter', 'w6,-2,8']),
                                      ('scalar', ['Vertical', 'w1,2,3', '-3.5'])]:
                    add(command, selection+'-vertical-'+plane_name+'-'+phase, inputs, selection, True, plane=plane)
    return dict(protocol_version=1, iterations=1, operations=operations)


def edges_request():
    """Degenerate directions, vertical repetition, and numeric line distances."""
    operations = []
    def add(label, inputs, selection, command='Copy'):
        row = dict(op='transform_copy_command', id=command.lower()+'-'+selection+'-'+label,
                   command=command, sources=copy.deepcopy(SOURCES), grouped=True,
                   inputs=inputs, finish='Enter' if command == 'Copy' else 'Cancel',
                   undo_redo=True, sel_last=True)
        if selection == 'pre': row['selected'] = [2, 0]
        else: row['source_selection'] = [2, 0, 'Enter']
        operations.append(row)
    for selection in ['pre', 'post']:
        for option in ['UseLastDistance', 'UseLastDirection']:
            add(option+'-zero-first', ['w1,2,3', 'w1,2,3', option+'=Yes', 'w6,-2,8'], selection)
            add(option+'-zero-next', ['w1,2,3', 'w6,-2,8', option+'=Yes', 'w1,2,3'], selection)
        for options, suffix in [(['UseLastDirection=Yes'], 'direction'),
                                (['UseLastDirection=Yes', 'UseLastDistance=Yes'], 'both'),
                                (['FromLastPoint=Yes', 'UseLastDirection=Yes'], 'from-last')]:
            for scalar in ['3', '-3', '0']:
                add('scalar-'+suffix+'-'+scalar, ['w1,2,3', 'w6,-2,8']+options+[scalar], selection)
        add('vertical-repeat', ['Vertical', 'w1,2,3', 'w6,-2,8', 'w9,7,-1'], selection)
        add('vertical-free-repeat', ['Vertical', 'w1,2,3', 'w6,-2,8', 'UseLastDirection=No', 'w9,7,-1'], selection)
        add('vertical-scalar-repeat', ['Vertical', 'w1,2,3', '-3.5', '2'], selection)
        add('vertical-no', ['Vertical=Yes', 'Vertical=No', 'w1,2,3', 'w6,-2,8'], selection)
        add('empty-target', ['w1,2,3', 'Enter'], selection, 'Move')
    return dict(protocol_version=1, iterations=1, operations=operations)


def mouse_request():
    operations = []
    for command in ('Move', 'Copy'):
        for view in ('Front', 'Right', 'Perspective'):
            for tilted in (False, True):
                row = dict(op='transform_copy_command', id=command.lower()+'-mouse-'+view.lower()+('-tilted' if tilted else '-world'),
                    command=command, sources=[[2,3,4]], grouped=False, inputs=['Vertical','w1,2,3','Mouse'],
                    finish='Automatic' if command == 'Move' else 'Enter', undo_redo=True, sel_last=True,
                    mouse_target=dict(view=view, bounds=[[-10,-10,-10],[10,10,10]], aim=[4,5,7], offset=[7,-11]))
                if tilted:
                    row['cplane'] = dict(origin=[5,-6,2], x_axis=[.8,.6,0], y_axis=[-.36,.48,.8])
                operations.append(row)
    for view in ('Front','Right','Perspective'):
        for from_last in (False, True):
            operations.append(dict(op='transform_copy_command', id='copy-mouse-direction-'+view.lower()+('-last' if from_last else '-base'),
                command='Copy', sources=[[2,3,4]], grouped=False,
                inputs=['w1,2,3','w4,-2,6','UseLastDirection=Yes']+(['FromLastPoint=Yes'] if from_last else [])+['Mouse'],
                finish='Enter', undo_redo=True, sel_last=True,
                mouse_target=dict(view=view, bounds=[[-10,-10,-10],[10,10,10]], aim=[5,4,8], offset=[13,-9])))
    return dict(protocol_version=1, iterations=1, operations=operations)
