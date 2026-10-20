"""Capture native Splop control/edit grip owners, mixed sources and history."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


def request():
    ops=[]
    def add(**changes):
        op=dict(op='splop_grips',id='splop_grips_'+str(len(ops)),source='rational',selected=[0,1],copy=False,rigid=False,flip=False,surface='Plane')
        op.update(changes);ops.append(op)
    for target in ('Plane','Cylinder'):
        for kind in ('curve','rational','closed','periodic','line','polyline','circle','arc','surface','surface_closed','mesh'):
            for flags in ({},{'copy':True},{'rigid':True},{'flip':True}):
                add(source=kind,surface=target,**flags)
        for kind in ('curve','rational','closed','periodic'):
            for flags in ({},{'copy':True},{'rigid':True},{'copy':True,'rigid':True,'flip':True}):
                add(source=kind,surface=target,edit=True,**flags)
        for mixed in ('parent','point'):
            for copy in (False,True):
                for rigid in (False,True):add(surface=target,copy=copy,rigid=rigid,**{mixed:True})
        for copy in (False,True):add(surface=target,copy=copy,rigid=True,grouped=True,point=True)
    for kind,edit in [('rational',False),('surface',False),('mesh',False),('rational',True)]:
        for finish in ('Complete','Cancel'):
            add(source=kind,edit=edit,surface='Cylinder',copy=True,targets=[[1.,90.],[3.,-45.]],finish=finish)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops)
    validate_operations(q);return q


def main():
    q=request();p=HERE/'fixtures/splop_grips.json';p.write_text(json.dumps(q,indent=2)+'\n')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=['generate_splop_grips.py','splop_grips_probe.py','grip_transform_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py']
    sources={n:digest(HERE/n)for n in names}
    r=OracleClient(settings_scheme='VibocerosOracleSplopGrips20261020').run_rhino(q,timeout=900.)
    assert sources=={n:digest(HERE/n)for n in names}
    assert len(r['results'])==len(q['operations'])and all('error'not in row for row in r['results'])
    r['provenance']=dict(request_sha256=digest(p),sources=sources)
    (HERE/'observations/splop_grips.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded Splop grip workflows',len(r['results']),flush=True)


if __name__=='__main__':main()
