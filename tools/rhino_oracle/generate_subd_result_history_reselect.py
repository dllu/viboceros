"""Reselect an original result face after clearing it and replaying history."""
from .capture_family import capture
from .generate_subd_result_history import request


if __name__=='__main__':
    capture('subd_result_history_reselect',request(),['generate_subd_result_history_reselect.py',
        'generate_subd_result_history.py','generate_merge_faces_command.py','subd_result_history_probe.py',
        'subd_state.py','subd_merge_allocation_cases.py','merge_faces_command_cases.py','merge_faces_command_input.py',
        'subd_edit_scene.py','subd_box_probe.py','number_token.py','join_probe.py','merge_edges_probe.py',
        'rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleSubDResultReselect20261029',timeout=600)
