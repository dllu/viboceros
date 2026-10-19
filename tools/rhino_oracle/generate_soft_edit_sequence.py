"""Capture repeated SoftEditCrv inputs separately from immutable single edits."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


def request():
    folder=HERE/'fixtures/soft_edit_curve_sequence';folder.mkdir(exist_ok=True)
    operations=[]
    for family in ('cubic','rational','periodic','circle','ellipse','line','polyline','polycurve'):
        for mode in ('repeat','copy','cancel','distance'):
            name=family+'_'+mode
            op=dict(op='soft_edit_curve_sequence',id=name,family=family,stations=[1./3.],distance=4.,offset=[0.,0.,2.],artifact_path=str(folder/(name+'.3dm')))
            if mode=='distance':op['distance_point']=.8
            else:op['moves']=[dict(station=.2,offset=[0.,0.,1.]),dict(station=.7,offset=[-1.,0.,-1.])]
            if mode=='copy':op['copy']=True
            if mode=='cancel':op.update(cancel=True,cancel_after=True)
            operations.append(op)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=operations)
    validate_operations(q);return q


def main():
    q=request();p=HERE/'fixtures/soft_edit_curve_sequence.json';p.write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleSoftEditSequence20261019').run_rhino(q,timeout=900.)
    assert len(r['results'])==32 and all('error'not in o for o in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    r['provenance']=dict(request_sha256=digest(p),sources={n:digest(HERE/n)for n in
        ('generate_soft_edit_sequence.py','soft_edit_sequence_probe.py','soft_edit_curve_probe.py','insert_kink_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')},
        fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (HERE/'observations/soft_edit_curve_sequence.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'repeated SoftEditCrv workflows',flush=True)


if __name__=='__main__':main()
