"""Queued command-line execution independent of the RunScript return path."""
from .capture_family import capture
from .generate_subd_merge_allocation import request as base


def request():
    q=base();names=['three-left-global','three-left-local','grid-L-local','disjoint-local']
    q['operations']=[o for o in q['operations']if o['case']in names]
    return q


if __name__=='__main__':
    capture('subd_merge_allocation_keyboard',request(),['generate_subd_merge_allocation_keyboard.py',
        'generate_subd_merge_allocation.py','generate_merge_faces_command.py','subd_merge_allocation_cases.py',
        'subd_merge_allocation_probe.py','subd_state.py','merge_faces_command_cases.py','merge_faces_command_input.py',
        'subd_edit_scene.py','subd_box_probe.py','number_token.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSubDMergeAllocationKeys20261029',timeout=250)
