"""Independent closed solids and decimal positions under acknowledged shaded picks."""
from .capture_family import capture


def request():
    cases = [
        ('closed-cube', [[0,0,0],[2,0,0],[2,2,0],[0,2,0],[0,0,2],[2,0,2],[2,2,2],[0,2,2]],
         [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]], 1),
        ('closed-octahedron', [[0,0,2],[2,0,0],[0,2,0],[-2,0,0],[0,-2,0],[0,0,-2]],
         [[0,1,2],[0,2,3],[0,3,4],[0,4,1],[5,2,1],[5,3,2],[5,4,3],[5,1,4]], 2),
        ('closed-tetrahedron', [[0,0,0],[2,0,0],[0,2,0],[0,0,3]],
         [[0,2,1],[0,1,3],[0,3,2],[1,2,3]], 3),
        ('decimal-fan', [[0.1,0.2,0.3],[2.3,0.2,0.3],[0.1,1.7,0.3],[-1.2,0.2,0.3],[0.1,-2.4,0.3],[2.3,1.7,0.3]],
         [[0,1,2],[0,2,3],[0,3,4],[0,4,1],[1,5,2]], 1),
    ]
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='mesh_face_collapse_mode',id=name+'-'+channels,
                                 vertices=vertices,faces=faces,face=face,channels=channels,display_mode='Shaded')
                            for name,vertices,faces,face in cases for channels in ['none','all']])


if __name__ == '__main__':
    capture('mesh_face_collapse_holdouts', request(), [
        'generate_mesh_face_collapse_holdouts.py','mesh_face_collapse_probe.py',
        'conic_getter_input.py','managed_mouse_callback.py','snap_environment.py',
        'number_token.py','catenary_point_picker.py','group_picking.py',
        'join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleFaceCollapseHoldouts20261029', timeout=240)
