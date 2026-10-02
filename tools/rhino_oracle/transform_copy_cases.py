"""Input recipes prescribe Copy edits and target sequences before measurement."""
import json


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


if __name__ == '__main__':
    print(json.dumps(request(), indent=2))
