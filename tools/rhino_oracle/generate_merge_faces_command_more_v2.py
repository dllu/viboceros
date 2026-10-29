"""Correct archive seed names in the immutable postselection/order holdouts."""
from .capture_family import capture
from .generate_merge_faces_command_more import request


if __name__=='__main__':
    capture('merge_faces_command_more_v2',request(),['generate_merge_faces_command_more_v2.py',
            'generate_merge_faces_command_more.py','generate_merge_faces_command.py',
            'merge_faces_command_cases.py','merge_faces_command_probe.py','merge_faces_command_input.py',
            'managed_component_input.py','component_picking.py','hole_picking.py','subd_edit_scene.py',
            'subd_box_probe.py','number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleMergeCommandMoreV220261029',timeout=750)
