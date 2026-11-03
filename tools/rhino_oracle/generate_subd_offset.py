"""Freeze independent native SDK SubD offsets."""
from .capture_family import capture
from .subd_offset_cases import scenes,validate


def request():
    ops=[dict(op='subd_offset_sdk',id='-'.join((scene,str(distance),'solid' if solid else 'skin')),
              scene=scene,distance=distance,solid=solid) for scene in scenes()
         for distance in (-.75,0.,.75) for solid in (False,True)]
    for op in ops:validate(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=ops)


if __name__=='__main__':
    capture('subd_offset_sdk',request(),['generate_subd_offset.py','subd_offset_cases.py','subd_offset_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_extract_cases.py','subd_box_probe.py','merge_edges_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py'],'VibocerosOracleSubDOffsetSDK20261102',timeout=900)
