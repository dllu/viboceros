"""Capture physical averaging picks, both identities and complete history."""
from .capture_family import capture
from .match_surface_command import AVERAGE_WORKFLOWS,request


if __name__=='__main__':
    capture('match_surface_average_command',request(list(AVERAGE_WORKFLOWS)),[
        'generate_match_surface_average_command.py','match_surface_command.py','match_surface_cases.py',
        'match_surface_command_probe.py','match_surface_point_picker.py','arc_point_picker.py','catenary_point_picker.py',
        'chamfer_point_picker.py','subcurve_direction_input.py','group_picking.py',
        'line_scene_probe.py','join_probe.py','merge_edges_probe.py','conic_getter_input.py',
        'managed_mouse_callback.py','snap_environment.py','number_token.py','structural_sdk.py','rhino_worker.py','client.py',
    ],'VibocerosOracleMatchSurfaceAverageCommand20261108',timeout=1200)
