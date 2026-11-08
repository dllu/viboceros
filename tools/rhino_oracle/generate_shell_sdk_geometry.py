"""Capture original box walls and circular-cap holdouts with full topology."""
from .capture_family import capture
from .shell_probe import request


def query():
    value = request()
    cylinder = dict(type='cylinder_brep', origin=[0, 0, 0], radius=5, height=8)
    for name, points, thickness in [
        ('shell_cylinder_top', [[0, 0, 8]], 1.),
        ('shell_cylinder_bottom', [[0, 0, 0]], 1.),
        ('shell_cylinder_both', [[0, 0, 0], [0, 0, 8]], 0.5),
    ]:
        value['operations'].append(dict(op='shell', id=name, source=cylinder,
                                        opening_points=points, thickness=thickness))
    for op in value['operations']:
        op['include_geometry'] = True
    return value


if __name__ == '__main__':
    capture('shell_sdk_geometry', query(), [
        'generate_shell_sdk_geometry.py', 'shell_probe.py', 'client.py',
        'rhino_worker.py', 'structural_sdk.py',
    ], 'VibocerosOracleShellSdkGeometry20261108', timeout=1200)
