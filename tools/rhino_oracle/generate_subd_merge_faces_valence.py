"""Distinguish smooth and crease valence-two cleanup from original valence."""
from copy import deepcopy
from .capture_family import capture
from .generate_subd_merge_faces_cleanup import request as cleanup


def request():
    query = cleanup()
    grid = next(row for row in query['operations'] if row['id'] == 'grid-L-faces')
    rows = []
    for name, creases in [('smooth', []), ('dart', [[1, 4]]),
                           ('crease', [[1, 4], [3, 4]]), ('side-crease', [[4, 5], [4, 7]])]:
        row = deepcopy(grid); row.update(id='grid-L-' + name, creases=creases); rows.append(row)
    for name, x in [('collinear', 0), ('convex', -.25), ('concave', .25)]:
        row = deepcopy(grid)
        row.update(id='boundary-valence-four-' + name,
                   vertices=[[x, 0, 0], [0, -1, 0], [1, -1, 0], [1, 1, 0], [0, 1, 0]],
                   faces=[[4, 0, 3], [0, 1, 2], [0, 2, 3]],
                   components=[dict(kind='face', index=i) for i in range(3)])
        rows.append(row)
    query['operations'] = rows
    return query


if __name__ == '__main__':
    capture('subd_merge_faces_valence', request(), ['generate_subd_merge_faces_valence.py',
            'generate_subd_merge_faces_cleanup.py', 'generate_subd_merge_faces_holdouts.py',
            'generate_subd_merge_faces.py', 'subd_merge_faces_probe.py', 'subd_edit_scene.py',
            'subd_box_probe.py', 'hide_pt_input.py', 'number_token.py', 'join_probe.py',
            'merge_edges_probe.py', 'rhino_worker.py', 'client.py', 'structural_sdk.py'],
            'VibocerosOracleSubDMergeValence20261029', timeout=450)
