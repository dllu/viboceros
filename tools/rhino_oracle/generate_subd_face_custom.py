"""Capture material channels and custom UVWs from actual Rhino command outputs."""
from .capture_family import capture
from .generate_merge_faces_command import request as base
from .subd_face_custom_cases import CASES


def request():
    q=base();q['operations']=[dict(op='subd_face_custom',id=n,case=n)for n in CASES]
    return q


if __name__=='__main__':
    capture('subd_face_custom',request(),['generate_subd_face_custom.py','generate_merge_faces_command.py',
        'subd_face_custom_cases.py','subd_face_custom_probe.py','subd_merge_allocation_cases.py','subd_state.py',
        'merge_faces_command_cases.py','merge_faces_command_input.py','subd_edit_scene.py','subd_box_probe.py',
        'number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py',
        'fixtures/subd_face_custom_recipes.json','fixtures/subd_face_custom_assets.json',
        '../../crates/viboceros-io/examples/subd_face_custom_seeds.rs'],
        'VibocerosOracleSubDFaceCustom20261029',timeout=1000)
