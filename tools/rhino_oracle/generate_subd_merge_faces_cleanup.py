"""Topology holdouts for original valence, winding and overlapping request priority."""
from copy import deepcopy
from .capture_family import capture
from .generate_subd_merge_faces_holdouts import request as holdouts
from .generate_subd_merge_faces import request as base


def request():
    scenes = {r['id']: r for r in base()['operations'] + holdouts()['operations']}
    rows = []
    face = lambda *indices: [dict(kind='face', index=i) for i in indices]
    edge = lambda a, b: [dict(kind='edge', vertices=[a, b])]
    vertex = lambda i: [dict(kind='vertex', index=i)]
    def add(name, source, components=None, **options):
        row = deepcopy(scenes[source]); row.update(id=name, **options)
        if components is not None: row['components'] = components
        rows.append(row)
    add('grid-L-faces', 'quad-grid-faces', face(0, 1, 2))
    add('grid-L-edges', 'quad-grid-faces', edge(1, 4) + edge(3, 4))
    add('grid-L-other-corner', 'quad-grid-faces', face(1, 2, 3))
    add('grid-L-reversed', 'quad-grid-faces', face(0, 1, 2),
        faces=[list(reversed(f)) for f in scenes['quad-grid-faces']['faces']])
    add('tri-reordered-faces', 'tri-faces', faces=list(reversed(scenes['tri-faces']['faces'])))
    add('tri-rotated-start', 'tri-faces', faces=[f[1:] + f[:1] for f in scenes['tri-faces']['faces']])
    add('tri-corner-edge', 'tri-faces', edge(0, 2), corners=[0, 2])
    add('tri-corner-vertex', 'tri-faces', vertex(0), corners=[0, 2])
    add('strip-corner-edge', 'strip-faces', edge(1, 2), corners=[1, 2])
    add('strip-corner-vertex', 'strip-faces', vertex(1), corners=[1, 2])
    for name, picks in [('ignored-edge-with-vertex', face(0) + edge(1, 2) + vertex(4)),
                        ('overlapping-vertices', vertex(1) + vertex(4)),
                        ('face-opposite-vertex', face(0) + vertex(5)),
                        ('edge-opposite-vertex', edge(1, 2) + vertex(7))]:
        add(name, 'three-faces', picks)
    query = base(); query['operations'] = rows
    return query


if __name__ == '__main__':
    capture('subd_merge_faces_cleanup', request(), ['generate_subd_merge_faces_cleanup.py',
            'generate_subd_merge_faces_holdouts.py', 'generate_subd_merge_faces.py',
            'subd_merge_faces_probe.py', 'subd_edit_scene.py', 'subd_box_probe.py', 'hide_pt_input.py',
            'number_token.py', 'join_probe.py', 'merge_edges_probe.py', 'rhino_worker.py',
            'client.py', 'structural_sdk.py'], 'VibocerosOracleSubDMergeCleanup20261029', timeout=450)
