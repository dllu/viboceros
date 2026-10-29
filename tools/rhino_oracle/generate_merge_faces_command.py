"""Capture real option selection, changed/unchanged owners and document history."""
from .capture_family import capture
from .merge_faces_command_cases import CASES


def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-7,relative=1e-12,angular=1e-10),
                operations=[dict(op='merge_faces_command',id=name,case=name)for name in CASES])


if __name__=='__main__':
    capture('merge_faces_command',request(),['generate_merge_faces_command.py','merge_faces_command_cases.py',
            'merge_faces_command_probe.py','merge_faces_command_input.py','subd_edit_scene.py','subd_box_probe.py',
            'number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
            'VibocerosOracleMergeCommand20261029',timeout=750)
