"""Freeze public native draft pull-direction controls and their input receipts."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .draft_pull_probe import validate

HERE=Path(__file__).resolve().parent
FAMILY='draft_pull_direction'


def request():
    operations=[]
    def add(mode,label,**changes):
        op=dict(op='draft_pull_direction',id='pull-'+mode+'-'+label,mode=mode,
                plane=[2,3,6],view=[0,1,0],points=[[1,2,3],[3,5,9]],
                curve='None',edge=0,reverse=False,cancel=False)
        op.update(changes);validate(op);operations.append(op)
    for mode in ('WorldZ','CPlaneZ'):
        for i,plane in enumerate(([0,0,1],[2,3,6],[1,0,0])):add(mode,str(i),plane=plane)
    for i,view in enumerate(([0,0,-1],[0,1,0],[1/3,2/3,-2/3],[0,0,1])):add('View',str(i),view=view)
    for i,points in enumerate(([[0,0,0],[0,0,1]],[[1,2,3],[3,5,9]],[[3,5,9],[1,2,3]],[[0,0,0],[.002,.003,.006]])):
        add('TwoPoints',str(i),points=points)
    add('TwoPoints','cancel',cancel=True)
    for curve in ('Line','Polyline','Arc'):
        for reverse in (False,True):add('Curve',curve+'-'+str(int(reverse)),curve=curve,reverse=reverse)
    add('Curve','Line-other',curve='Line',points=[[2,5,9],[8,4,7]])
    for edge in (0,4,7):add('Curve','edge-'+str(edge),curve='BoxEdge',edge=edge)
    add('Curve','cancel',curve='Line',cancel=True)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    query=HERE/'fixtures'/(FAMILY+'.json');output=HERE/'observations'/(FAMILY+'.json.gz')
    if query.exists() or output.exists():raise ValueError('draft pull captures are immutable')
    q=request();query.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_draft_pull.py','draft_pull_probe.py','draft_pull_controls.py','draft_pull_input.py',
           'structural_sdk.py','rhino_worker.py','client.py','merge_edges_probe.py','draft_pull_host.py','group_picking.py','component_picking.py','hole_picking.py','fixtures/brep_direction/box.3dm']
    images={n:(HERE/n).read_bytes() for n in names}
    print('Capturing {} direction cases'.format(len(q['operations'])),flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleDraftPullQualified20261102').run_rhino(q,timeout=900)
    if images!={n:(HERE/n).read_bytes() for n in names}:raise ValueError('pull-direction producer changed during capture')
    if len(data['results'])!=len(q['operations']) or any('error' in r for r in data['results']):raise ValueError('incomplete pull-direction capture')
    sources={n:hashlib.sha256(b).hexdigest() for n,b in images.items()}
    data['provenance']=dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(),sources=sources)
    for n,sha in sources.items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():path.write_bytes(gzip.compress(images[n],mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data,indent=2,allow_nan=False)+'\n').encode(),mtime=0))
    print('Recorded '+str(output),flush=True)


if __name__=='__main__':main()
