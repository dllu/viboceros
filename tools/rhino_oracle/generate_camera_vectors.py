"""Freeze native raw camera vectors, navigation resets and draft View controls."""
from .camera_vectors_probe import PROJECTIONS, validate
from .capture_family import capture
from .draft_pull_probe import validate as validate_pull

FAMILY = 'camera_vectors'


def request():
    operations = []
    vectors = [('skew', [1, 2, -2], [0, 4, 1]),
               ('orthogonal', [0, 0, -3], [0, 5, 0]),
               ('small', [1e-8, 2e-8, -2e-8], [0, 4e-8, 1e-8]),
               ('large', [1e48, 2e48, -2e48], [0, 4e48, 1e48])]
    for projection in PROJECTIONS:
        for label, direction, up in vectors:
            actions = ['None']
            if label == 'skew':
                actions += ['Zoom', 'MousePan', 'MouseZoom', 'WorldTop', 'Plan']
            if label in ('skew', 'orthogonal') and projection != 'Parallel':
                actions += ['PhysicalOrbit']
            for action in actions:
                op = dict(op='camera_vectors', id=projection+'-'+label+'-'+action,
                          projection=projection, direction=direction, up=up, action=action)
                if action == 'PhysicalOrbit':
                    op['mouse_drag'] = [12, -7]
                validate(op)
                operations.append(op)
    for label, direction, _up in vectors:
        op = dict(op='draft_pull_direction', id='draft-view-'+label, mode='View',
                  plane=[0, 0, 1], view=direction, points=[[0, 0, 0], [0, 0, 1]],
                  curve='None', edge=0, reverse=False, cancel=False)
        validate_pull(op)
        operations.append(op)
    return dict(protocol_version=1, iterations=1,
                tolerance=dict(absolute=1e-8, relative=1e-12, angular=1e-8),
                operations=operations)


def main():
    capture(FAMILY, request(), [
        'generate_camera_vectors.py', 'camera_vectors_probe.py', 'structural_sdk.py',
        'rhino_worker.py', 'client.py', 'merge_edges_probe.py', 'camera_navigation.py',
        'group_picking.py', 'draft_pull_probe.py', 'draft_pull_controls.py',
        'draft_pull_input.py', 'draft_pull_host.py', 'component_picking.py', 'hole_picking.py',
    ], 'VibocerosOracleCameraVectors20261102', timeout=900)


if __name__ == '__main__':
    main()
