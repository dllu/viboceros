"""Freeze native surface curvature colors, analysis ownership and model lifecycle."""
import gzip
import argparse
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .curvature_analysis_probe import SOURCES,STYLES,validate
HERE=Path(__file__).resolve().parent
FAMILY='curvature_analysis_initial'

def request():
    operations=[]
    def add(identity,source,style,range,preselect,steps):
        op=dict(op='curvature_analysis',id=identity,source=source,style=style,range=range,preselect=preselect,steps=steps)
        validate(op);operations.append(op)
    for source in SOURCES:
        for preselect in (False,True):
            add('curvature-life-'+source+'-'+str(int(preselect)),source,'Gaussian',[-1,1],preselect,
                ['CurvatureAnalysis','Hide','Show','Delete','Undo','Redo','Undo','CurvatureAnalysisOff','Undo'])
    for source in ('warped','holed','sphere','cylinder','cone','torus'):
        for style in STYLES:
            range=[-0.5,0.5] if style=='Gaussian' else ([0,1] if style=='Mean' else [0,5])
            add('curvature-colors-'+source+'-'+style,source,style,range,True,['CurvatureAnalysis','CurvatureAnalysisOff'])
    for style in STYLES:
        add('curvature-reverse-'+style,'warped',style,[1,-1],True,['CurvatureAnalysis','CurvatureAnalysisOff'])
        add('curvature-equal-'+style,'box',style,[0,0],True,['CurvatureAnalysis','CurvatureAnalysisOff'])
    for source in ('warped','subd_box'):
        add('curvature-exclusive-'+source,source,'Mean',[0,1],True,
            ['CurvatureAnalysis','DraftAngleAnalysis','DraftAngleAnalysisOff','CurvatureAnalysis','CurvatureAnalysisOff','DraftAngleAnalysis','CurvatureAnalysis','CurvatureAnalysisOff','DraftAngleAnalysisOff'])
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--inputs',action='store_true');parser.add_argument('--ranges',action='store_true');args=parser.parse_args()
    family='curvature_analysis_ranges' if args.ranges else ('surface_analysis_input' if args.inputs else FAMILY)
    query=HERE/'fixtures'/(family+'.json');output=HERE/'observations'/(family+'.json.gz')
    if query.exists() or output.exists():raise ValueError('curvature captures are immutable')
    q=request()
    if args.inputs:q['operations']=[dict(op='surface_analysis_input',id='curvature-filters')]
    if args.ranges:q=ranges_request()
    query.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_curvature_analysis.py','curvature_analysis_probe.py','structural_sdk.py','rhino_worker.py','client.py','join_probe.py','merge_edges_probe.py']
    names+=['fixtures/brep_direction/'+s+'.3dm' for s in SOURCES if not s.startswith('subd_')]
    if args.inputs:names.append('analysis_input_probe.py')
    if args.ranges:names.append('curvature_analysis_controls.py')
    images={n:(HERE/n).read_bytes() for n in names}
    print('Capturing {} curvature analysis cases'.format(len(q['operations'])),flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleCurvatureQualified20261102').run_rhino(q,timeout=900)
    if images!={n:(HERE/n).read_bytes() for n in names}:raise ValueError('curvature producer changed during capture')
    if len(data['results'])!=len(q['operations']) or any('error' in r for r in data['results']):raise ValueError('curvature capture failed')
    sources={n:hashlib.sha256(b).hexdigest() for n,b in images.items()}
    data['provenance']=dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(),sources=sources)
    for n,sha in sources.items():
        p=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not p.exists():p.write_bytes(gzip.compress(images[n],mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data,indent=2,allow_nan=False)+'\n').encode(),mtime=0))
    print('Recorded '+str(output),flush=True)

def ranges_request():
    q=request();q['operations']=[dict(op='curvature_analysis',id='curvature-range-'+source+'-'+style,source=source,style=style,range=[-1,1] if style in ('Gaussian','Mean') else [0,5],preselect=True,
        steps=['CurvatureAnalysis','MaxRange','CurvatureAnalysisOff']) for source in ('box','sphere','warped') for style in STYLES]
    return q

if __name__=='__main__':main()
