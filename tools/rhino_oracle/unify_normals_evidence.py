"""Reconstruct recorded winding edits without duplicating the traversal kernel."""
from copy import deepcopy

RAW_FIELDS = ['vertices', 'faces', 'normals', 'uv', 'colors', 'ngons']


def winding(value, indices):
    if (not isinstance(indices, list) or any(type(i) is not int for i in indices)
            or indices != sorted(set(indices))
            or any(i < 0 or i >= len(value['faces']) for i in indices)):
        raise ValueError('invalid recorded winding indices')
    result = {key: deepcopy(value[key]) for key in RAW_FIELDS}
    affected = set()
    for i in indices:
        face = result['faces'][i]
        affected.update(face)
        result['faces'][i] = face[:1] + list(reversed(face[1:]))
    if result['normals']:
        for i in affected:
            result['normals'][i] = [-x for x in result['normals'][i]]
    return result


def raw(value):
    return {key: value[key] for key in RAW_FIELDS}


def check_sdk_row(row, value):
    actual = winding(value['before'], row.get('flipped_faces'))
    repeated = winding(actual, row.get('repeat_flipped_faces'))
    expected, expected_second = raw(value['after']), raw(value['after_second'])
    wanted = dict(vertices=len(actual['vertices']), faces=len(actual['faces']),
                  predicted=len(row['flipped_faces']), changed=len(row['flipped_faces']),
                  second=len(row['repeat_flipped_faces']))
    for key, target in wanted.items():
        if type(row.get(key)) is not int or row[key] != target:
            raise ValueError('incorrect recorded winding counts')
    flags = dict(geometry=actual['faces'] == expected['faces'],
                 channels=all(actual[k] == expected[k] for k in ['uv', 'colors']),
                 normals=actual['normals'] == expected['normals'],
                 ngons=actual['ngons'] == expected['ngons'], count_only=True, source_unchanged=True,
                 counts=row['predicted'] == value['predicted'] and row['changed'] == value['changed'],
                 repeat=repeated == expected_second and row['second'] == value['second'])
    flags['qualified'] = all(flags.values())
    flags['idempotent'] = repeated == actual and row['second'] == 0
    if any(row.get(key) is not target for key, target in flags.items()):
        raise ValueError('winding flags do not match recorded face edits')
