"""Capture sphere-sized Splop workflows through the public command interface."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent


def request():
    ops=[]
    def add(**changes):
        op=dict(op='splop_command',id='splop_'+str(len(ops)),shape='Cloud',surface='Plane',copy=False,rigid=False,flip=False,grouped=False,angle=0.,radius=4.)
        op.update(changes);ops.append(op)
    for surface in ('Plane','Cylinder','Sphere'):
        for shape in ('Cloud','Points','Line','Curve','Surface','Box','Mesh'):
            for changes in ({},{'copy':True},{'rigid':True},{'flip':True}):
                add(surface=surface,shape=shape,**changes)
        for changes in [dict(grouped=True,rigid=True),dict(grouped=True,rigid=True,copy=True),dict(grouped=True,rigid=True,flip=True)]:
            add(surface=surface,shape='Points',**changes)
        for angle in (90.,-135.,45.):add(surface=surface,angle=angle,flip=angle==45.)
        for change in [dict(reference_point=True),dict(reverse=True),dict(locked_radius=3.,angle=45.),dict(plane='WorldZX'),dict(plane='WorldYZ'),dict(postselect=True),dict(postselect=True,copy=True),dict(postselect=True,rigid=True,copy=True)]:
            add(surface=surface,**change)
    for shape,rigid,grouped in [('Cloud',False,False),('Curve',False,False),('Mesh',True,False),('Points',True,True)]:
        for finish in ('Complete','Cancel'):
            add(surface='Cylinder',shape=shape,rigid=rigid,grouped=grouped,copy=True,targets=[[1.,90.],[3.,-45.]],finish=finish)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops)
    validate_operations(q)
    return q


def main():
    q=request();p=HERE/'fixtures/splop.json';p.write_text(json.dumps(q,indent=2)+'\n')
    sources=['generate_splop.py','splop_probe.py','twist_command_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py']
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    hashes={n:digest(HERE/n) for n in sources}
    r=OracleClient(settings_scheme='VibocerosOracleSplop20261020').run_rhino(q,timeout=900.)
    assert hashes=={n:digest(HERE/n) for n in sources}
    assert len(r['results'])==len(q['operations']) and all('error' not in row for row in r['results'])
    r['provenance']=dict(request_sha256=digest(p),sources=hashes)
    (HERE/'observations/splop.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded Splop workflows',len(r['results']),flush=True)


if __name__=='__main__':main()
