"""Check nondegenerate document extents in the public pseudo-infinite getter."""
from .capture_family import capture
from .surface_interpolate_plane_probe import bounded_request
if __name__ == '__main__':
    capture('surface_interpolate_plane_bounded', bounded_request(),
            ['generate_surface_interpolate_plane_bounded.py','surface_interpolate_plane_probe.py','surface_interpolate_plane_cases.py','surface_interpolate_plane_input.py',
             'conic_getter_input.py','managed_mouse_callback.py','snap_environment.py','number_token.py',
             'catenary_point_picker.py','group_picking.py','join_probe.py','merge_edges_probe.py',
             'structural_sdk.py','rhino_worker.py','client.py'],
            'VibocerosOracleSurfaceInterpolatePlaneBounded20261106',timeout=1500)
