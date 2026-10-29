"""Capture allocating command/SDK edits after merged component ID gaps."""
from .capture_family import capture
from .generate_merge_faces_command import request as base
from .subd_merge_allocation_cases import CASES


def request():
    q=base();q['operations']=[dict(op='subd_merge_allocation',id=name,case=name)for name in CASES]
    return q


if __name__=='__main__':
    capture('subd_merge_allocation',request(),['generate_subd_merge_allocation.py','generate_merge_faces_command.py',
        'subd_merge_allocation_cases.py','subd_merge_allocation_probe.py','subd_state.py',
        'merge_faces_command_cases.py','merge_faces_command_input.py','subd_edit_scene.py','subd_box_probe.py',
        'number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSubDMergeAllocation20261029',timeout=900)
