"""Repeat fixed captures to separate Native allocation stability from geometry."""
from .capture_family import capture
from .generate_subd_insert_order import request

if __name__=='__main__':
    capture('subd_insert_order_repeat',request(),[
        'generate_subd_insert_repeat.py','generate_subd_insert_order.py','generate_merge_faces_command.py',
        'subd_insert_order_cases.py','subd_insert_order_probe.py','subd_insert_custom_probe.py','subd_insert_custom_cases.py',
        'subd_state.py','subd_edit_scene.py','subd_box_probe.py','conic_getter_input.py','managed_mouse_callback.py',
        'snap_environment.py','number_token.py','catenary_point_picker.py','group_picking.py','join_probe.py',
        'merge_edges_probe.py','merge_faces_command_input.py','merge_faces_command_cases.py',
        'fixtures/subd_face_custom_assets.json','fixtures/subd_face_custom_holdout_assets.json',
        'client.py','rhino_worker.py','structural_sdk.py'],
        'VibocerosOracleSubDInsertRepeat20261029',timeout=1200)
