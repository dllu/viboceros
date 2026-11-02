"""Capture immutable native draft-angle membership and vertex-color evidence."""
import gzip
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .draft_angle_probe import SOURCES, validate

HERE=Path(__file__).resolve().parent
FAMILY='draft_angle_analysis_initial'

def request():
    operations=[]
    def add(identity,source,preselect,angles,up,steps,wires=False):
        op=dict(op='draft_angle_analysis',id=identity,source=source,preselect=preselect,range=angles,up=up,steps=steps,wires=wires)
        validate(op);operations.append(op)
    life=['DraftAngleAnalysis','Hide','Show','Delete','Undo','Redo','Undo','DraftAngleAnalysisOff','Undo']
    for source in SOURCES:
        for preselect in (False,True):
            add('draft-life-'+source+'-'+str(int(preselect)),source,preselect,[0,5],[0,0,1],life,preselect)
    ranges=[([-5,5],[0,0,1]),([5,-5],[0,0,1]),([5,5],[0,0,1]),([0,0],[0,0,1]),
            ([-90,90],[0,0,1]),([0,1e-7],[0,0,1]),([-5,5],[0,0,2]),
            ([-5,5],[1,0,0]),([-5,5],[0,0,-1]),([-35,45],[1,2,3])]
    for i,(angles,up) in enumerate(ranges):
        add('draft-colors-'+str(i),'panels',True,angles,up,['DraftAngleAnalysis','DraftAngleAnalysisOff'])
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)

def main():
    query=HERE/'fixtures'/(FAMILY+'.json');output=HERE/'observations'/(FAMILY+'.json.gz')
    if query.exists() or output.exists():raise ValueError('draft-angle captures are immutable')
    q=request();query.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_draft_angle.py','draft_angle_probe.py','structural_sdk.py','rhino_worker.py','client.py','join_probe.py','merge_edges_probe.py']
    names+=['fixtures/brep_direction/'+source+'.3dm' for source in SOURCES if source not in ('mesh','panels')]
    images={name:(HERE/name).read_bytes() for name in names}
    print('Capturing {} draft-angle cases'.format(len(q['operations'])),flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleDraftQualified20261102').run_rhino(q,timeout=900)
    if images!={name:(HERE/name).read_bytes() for name in names}:raise ValueError('draft-angle producer changed during capture')
    if len(data['results'])!=len(q['operations']) or any('error' in row for row in data['results']):raise ValueError('draft-angle capture failed')
    sources={name:hashlib.sha256(content).hexdigest() for name,content in images.items()}
    data['provenance']=dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(),sources=sources)
    for name,sha in sources.items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():path.write_bytes(gzip.compress(images[name],mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data,indent=2,allow_nan=False)+'\n').encode(),mtime=0))
    print('Recorded '+str(output),flush=True)

if __name__=='__main__':main()
