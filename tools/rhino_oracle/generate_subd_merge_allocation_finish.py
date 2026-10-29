"""Separate EndCommand state from macro cleanup after allocating SubD edits."""
from .capture_family import capture
from .generate_subd_merge_allocation import request


if __name__=='__main__':
    capture('subd_merge_allocation_finish',request(),['generate_subd_merge_allocation_finish.py',
        'generate_subd_merge_allocation.py','generate_merge_faces_command.py','subd_merge_allocation_cases.py',
        'subd_merge_allocation_probe.py','subd_state.py','merge_faces_command_cases.py','merge_faces_command_input.py',
        'subd_edit_scene.py','subd_box_probe.py','number_token.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSubDMergeAllocationFinish20261029',timeout=900)
