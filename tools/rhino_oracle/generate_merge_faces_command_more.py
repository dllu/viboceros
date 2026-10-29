"""Actual postselection and owner-order holdouts for the full face merge command."""
from .capture_family import capture
from .generate_merge_faces_command import request as base
from .merge_faces_command_cases import EXTRA_CASES


def request():
    query=base();query['operations']=[dict(op='merge_faces_command',id=name,case=name)for name in EXTRA_CASES]
    return query


if __name__=='__main__':
    capture('merge_faces_command_more',request(),['generate_merge_faces_command_more.py','generate_merge_faces_command.py',
            'merge_faces_command_cases.py','merge_faces_command_probe.py','merge_faces_command_input.py',
            'managed_component_input.py','component_picking.py','hole_picking.py','subd_edit_scene.py',
            'subd_box_probe.py','number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleMergeCommandMore20261029',timeout=750)
