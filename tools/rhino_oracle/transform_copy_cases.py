"""Input recipes prescribe Copy edits and target sequences before measurement."""
import json
import copy as copying


def request():
    operations = []
    def add(name, suffix, inputs, finish='Enter', grouped=False, sel_last=False):
        operations.append(dict(op='transform_copy_command', id=name.lower()+'-'+suffix,
            command=name, inputs=inputs, finish=finish, grouped=grouped, sel_last=sel_last,
            sources=[[2., 3., 4.], [5., -1., 2.]] if grouped else [[2., 3., 4.]], undo_redo=True))
    for name in ['Scale', 'Scale1D', 'Scale2D', 'Rotate', 'Rotate3D']:
        axis = ['w0,0,0', 'w0,0,1'] if name == 'Rotate3D' else ['w0,0,0']
        targets = ['w0,1,0', 'w1,1,0'] if name.startswith('Rotate') else ['w2,0,0', 'w3,0,0']
        inputs = axis+['Copy=Yes', 'w1,0,0']+targets
        add(name, 'references-enter', inputs)
        add(name, 'references-cancel', inputs, 'Cancel')
        add(name, 'references-grouped', inputs, grouped=True)
        if name != 'Scale1D':
            values = ['90', '180'] if name.startswith('Rotate') else ['2', '3']
            add(name, 'numbers-enter', axis+['Copy=Yes']+values)
            add(name, 'numbers-cancel', axis+['Copy=Yes']+values, 'Cancel')
            add(name, 'numbers-undo', axis+['Copy=Yes']+values+['Undo'])
            add(name, 'numbers-last', axis+['Copy=Yes']+values, grouped=True, sel_last=True)
            add(name, 'transient-copy-no', axis+['Copy=Yes', values[0], 'Copy=No'], 'Cancel')
            # The final in-place transform closes the native command itself.
            add(name, 'finish-in-place', axis+['Copy=Yes', values[0], 'Copy=No', values[1]], 'Automatic')
        else:
            add(name, 'directions', axis+['Copy=Yes', '2', 'w1,0,0', 'w0,1,0', 'w0,0,1'])
            add(name, 'direction-distance', axis+['Copy=Yes', '2', 'w1,0,0', '3', 'w1,0,0'], 'Cancel')
    for finish in ['Enter', 'Cancel']:
        add('Shear', 'angles-'+finish.lower(), ['w0,0,0', 'Copy=Yes', 'w1,0,0', '45', '30'], finish)
        add('Shear', 'points-'+finish.lower(), ['w0,0,0', 'Copy=Yes', 'w1,0,0', 'w1,1,0', 'w1,-1,0'], finish)
    add('Shear', 'angles-grouped', ['w0,0,0', 'Copy=Yes', 'w1,0,0', '45', '30'], grouped=True)
    for copy in ['Yes', 'No']:
        add('Mirror', 'copy-'+copy.lower(), ['Copy='+copy, 'w0,0,0', 'w0,1,0'], 'Automatic', grouped=True)
    add('Rotate3D', 'cancel-before-axis', ['Copy=Yes'], 'Cancel')
    return dict(protocol_version=1, iterations=1, operations=operations)


def script_request():
    """Single native edits corresponding to complete registry invocations."""
    operations = []
    for name in ['Scale', 'Scale1D', 'Scale2D', 'Rotate', 'Rotate3D', 'Mirror', 'Shear']:
        axis = ['w0,0,0', 'w0,0,1'] if name == 'Rotate3D' else ['w0,0,0']
        tail = ['2', 'w1,0,0'] if name == 'Scale1D' else ['w0,1,0'] if name == 'Mirror' else ['w1,0,0', '45'] if name == 'Shear' else ['90'] if name.startswith('Rotate') else ['2']
        references = ['w0,1,0'] if name == 'Mirror' else ['w1,0,0', 'w0,1,0'] if name.startswith('Rotate') else ['w1,0,0', 'w1,1,0'] if name == 'Shear' else ['w1,0,0', 'w2,0,0']
        for suffix, copy, selected, grouped, points in [
                ('grouped-copy', True, [0, 1], True, tail),
                ('peer-in-place', False, [0, 1], False, tail),
                ('peer-copy', True, [0, 1], False, tail),
                ('partial-group-copy', True, [0], True, tail),
                ('references-in-place', False, [0, 1], False, references),
                ('reversed-copy', True, [1, 0], False, tail),
                ('identity-in-place', False, [0, 1], False,
                 ['1', 'w1,0,0'] if name == 'Scale1D' else ['w0,1,0'] if name == 'Mirror' else ['w1,0,0', '0'] if name == 'Shear' else ['0'] if name.startswith('Rotate') else ['1'])]:
            inputs = axis+['Copy=Yes' if copy else 'Copy=No']+points
            operations.append(dict(op='transform_copy_command', id=name.lower()+'-'+suffix,
                command=name, inputs=inputs, selected=selected, grouped=grouped,
                sources=[[0., 3., 4.], [0., -1., 2.], [8., 7., -3.]] if name == 'Mirror' and suffix == 'identity-in-place' else [[2., 3., 4.], [5., -1., 2.], [8., 7., -3.]],
                finish='Enter' if copy and name != 'Mirror' else 'Automatic', undo_redo=True, sel_last=True))
    for operation in list(operations):
        if operation['id'].endswith('identity-in-place'):
            operation = copying.deepcopy(operation)
            operation['id'] = operation['id'].replace('identity-in-place', 'identity-copy')
            operation['inputs'] = [token.replace('Copy=No', 'Copy=Yes') for token in operation['inputs']]
            operation['finish'] = 'Automatic' if operation['command'] == 'Mirror' else 'Enter'
            operations.append(operation)
    return dict(protocol_version=1, iterations=1, operations=operations)


def center_request():
    operations = []
    for label, plane in [('world', None),
                         ('rotated', dict(origin=[10., -7., 4.], x_axis=[.6, .8, 0.], y_axis=[-.8, .6, 0.])),
                         ('tilted', dict(origin=[10., -7., 4.], x_axis=[.6, .8, 0.], y_axis=[-.64, .48, .6]))]:
        for name in ['Scale', 'Scale1D', 'Scale2D']:
            for copy in [False, True]:
                inputs = ['Copy=Yes' if copy else 'Copy=No', 'Enter', '2']
                if name == 'Scale1D':
                    inputs.append('w6,3,0.5')
                operation = dict(op='transform_copy_command', id=name.lower()+'-'+label+'-center-copy-'+str(copy).lower(),
                    command=name, inputs=inputs, grouped=False,
                    sources=[[2., 3., 4.], [5., -1., 2.], [8., 7., -3.]],
                    finish='Enter' if copy else 'Automatic', undo_redo=True, sel_last=True)
                if plane is not None:
                    operation['cplane'] = plane
                operations.append(operation)
    return dict(protocol_version=1, iterations=1, operations=operations)


def identity_request():
    """Measure object renewal at exact and nearby identity input values."""
    operations = []
    recipes = []
    for name in ['Scale', 'Scale1D', 'Scale2D']:
        for factor in ['1.00000000000001', '1.000000001']:
            tail = [factor, 'w1,0,0'] if name == 'Scale1D' else [factor]
            recipes.append((name, 'factor-'+factor, ['w0,0,0'], tail, None))
    tilted = dict(origin=[10., -7., 4.], x_axis=[.6, .8, 0.], y_axis=[-.64, .48, .6])
    recipes.append(('Scale2D', 'tilted-unit-factor', ['w0,0,0'], ['1'], tilted))
    for name in ['Rotate', 'Rotate3D']:
        axis = ['w0,0,0', 'w0,0,1'] if name == 'Rotate3D' else ['w0,0,0']
        for angle in ['360', '-360', '720', '1e-12', '8e-7', '9e-7',
                      '360.0000008', '360.0000009', '90.0000008', '90.0000009',
                      '1.2e-6', '90.0000012']:
            recipes.append((name, 'angle-'+angle, axis, [angle], None))
    recipes.append(('Shear', 'angle-1e-12', ['w0,0,0'], ['w1,0,0', '1e-12'], None))
    for name, suffix, axis, tail, plane in recipes:
        for copy in [False, True]:
            operation = dict(op='transform_copy_command', id=name.lower()+'-'+suffix+'-copy-'+str(copy).lower(),
                command=name, inputs=axis+['Copy=Yes' if copy else 'Copy=No']+tail,
                selected=[0, 1], grouped=False,
                sources=[[2., 3., 4.], [5., -1., 2.], [8., 7., -3.]],
                finish='Enter' if copy else 'Automatic', undo_redo=True, sel_last=True)
            if plane is not None:
                operation['cplane'] = plane
            operations.append(operation)
    return dict(protocol_version=1, iterations=1, operations=operations)


if __name__ == '__main__':
    print(json.dumps(request(), indent=2))
