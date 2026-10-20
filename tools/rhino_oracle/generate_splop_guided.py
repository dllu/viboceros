"""Retain native fitted, curve-guided and radius-constrained Splop references."""
import hashlib,json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
from .splop_guided_probe import FIT_POINTS,CURVES,SIZES
HERE=Path(__file__).resolve().parent

def request():
    ops=[]
    def add(mode,kind,**changes):
        op=dict(op='splop_guided',id='guided_'+str(len(ops)),mode=mode,kind=kind,plane='WorldXY');op.update(changes);ops.append(op)
    for plane in ('WorldXY','WorldZX'):
        for surface in ('Plane','Cylinder'):
            for kind in FIT_POINTS:
                for source in ('Points','CurveGrips','MeshGrips'):
                    for flags in ({},{'copy':True},{'rigid':True},{'flip':True}):add('Fit',kind,plane=plane,surface=surface,source=source,**flags)
        for kind in CURVES:
            for size in SIZES:add('Around',kind,plane=plane,size=size)
            for size in ('Radius','Point'):add('Around',kind,plane=plane,size=size,surface='Cylinder')
            add('Around',kind,plane=plane,surface='Cylinder',copy=True,rigid=True,flip=True)
        for surface in ('Plane','Cylinder'):
            for kind in ('FourthAbove','FourthBelow','Third'):
                for flags in ({},{'copy':True},{'flip':True,'rigid':True}):add('FourRadius',kind,plane=plane,surface=surface,**flags)
    for source in ('Points','CurveGrips','MeshGrips'):add('Fit','Tetra',source=source,finish='CancelReference')
    for kind in CURVES:add('Around',kind,finish='CancelReference')
    add('FourRadius','FourthAbove',finish='CancelReference')
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();path=HERE/'fixtures/splop_guided.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_splop_guided.py','splop_guided_probe.py','splop_probe.py','twist_command_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py'];digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={name:digest(HERE/name)for name in names}
    r=OracleClient(settings_scheme='VibocerosOracleSplopGuided20261020').run_rhino(q,timeout=1200.)
    assert sources=={name:digest(HERE/name)for name in names}
    assert len(r['results'])==len(q['operations'])and all('error'not in row for row in r['results'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources)
    (HERE/'observations/splop_guided.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded guided reference spheres',len(r['results']),flush=True)

if __name__=='__main__':main()
