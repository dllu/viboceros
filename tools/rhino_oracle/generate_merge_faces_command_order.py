"""Repeat public MergeFaces with identical object UUIDs and selection order."""
from .capture_family import capture
from .merge_faces_command_cases import ORDER_CASES
from .generate_merge_faces_command import request as base


def request():
    query=base()
    query['operations']=[dict(op='merge_faces_command',id=name,case=name)
                         for name in sorted(ORDER_CASES)]
    return query


if __name__=='__main__':
    capture('merge_faces_command_order',request(),[
        'generate_merge_faces_command_order.py','generate_merge_faces_command.py','merge_faces_command_cases.py',
        'merge_faces_command_probe.py','merge_faces_command_input.py',
        'managed_component_input.py','component_picking.py','hole_picking.py',
        'subd_edit_scene.py','subd_box_probe.py','number_token.py','join_probe.py',
        'merge_edges_probe.py','rhino_worker.py','client.py','structural_sdk.py'],
        'VibocerosOracleMergeCommandOrder20261029',timeout=1000)
