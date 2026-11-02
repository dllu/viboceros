"""Capture native direction-analysis control and model lifecycle behavior."""
import gzip
import argparse
import hashlib
import json
from .client import OracleClient
from .generate_brep_direction import HERE

def request():
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=[
        dict(op='direction_analysis',id='showdir-'+source+'-'+str(int(preselect)),source=source,preselect=preselect,
             steps=['ShowDir','ShowDirOff'])
        for source in ('warped','holed','box','sphere','line','mesh') for preselect in (False,True)])

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--controls',action='store_true');parser.add_argument('--membership',action='store_true');parser.add_argument('--foreign',action='store_true');parser.add_argument('--multi',action='store_true');args=parser.parse_args()
    family='direction_analysis_multi' if args.multi else ('direction_analysis_foreign' if args.foreign else ('direction_analysis_membership_v3' if args.membership else ('direction_analysis_controls_v2' if args.controls else 'direction_analysis_initial')))
    query=HERE/'fixtures'/(family+'.json');output=HERE/'observations'/(family+'.json.gz')
    if query.exists() or output.exists():raise ValueError('ShowDir captures are immutable')
    q=multi_request() if args.multi else (foreign_request() if args.foreign else (membership_request() if args.membership else (controls_request() if args.controls else request())));query.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_direction_analysis.py','direction_analysis_probe.py','direction_analysis_input.py','structural_sdk.py','rhino_worker.py','client.py','join_probe.py','merge_edges_probe.py']
    names += ['fixtures/brep_direction/'+source+'.3dm' for source in ('warped','holed','box','sphere')]
    images={name:(HERE/name).read_bytes() for name in names}
    print('Capturing {} direction-analysis cases'.format(len(q['operations'])),flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleShowDir20261102').run_rhino(q,timeout=900)
    if images!={name:(HERE/name).read_bytes() for name in names}:raise ValueError('ShowDir producer changed during capture')
    if len(data['results'])!=len(q['operations']) or any('error' in row for row in data['results']):raise ValueError('ShowDir capture failed')
    sources={name:hashlib.sha256(content).hexdigest() for name,content in images.items()}
    data['provenance']=dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(),sources=sources)
    for name,sha in sources.items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():path.write_bytes(gzip.compress(images[name],mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data,indent=2,allow_nan=False)+'\n').encode(),mtime=0))
    print('Recorded '+str(output),flush=True)

def controls_request():
    q=request();q['operations']=[]
    for source in ('warped','holed','box','sphere','line','mesh'):
        recipes=[('life',['ShowDir','Hide','Show','Delete','Undo','Redo','Undo','ShowDirOff']),
                 ('display',['ShowDir','Direction=No','U=No','V=No','U=Yes','V=Yes','Direction=Yes','ShowDirOff']),
                 ('flip',['ShowDir','Flip','Undo','Redo','ShowDirOff'])]
        if source not in ('line','mesh'):
            recipes += [(edit,['ShowDir',edit,'Undo','Redo','ShowDirOff']) for edit in ('UReverse','VReverse','SwapUV')]
        for name,steps in recipes:q['operations'].append(dict(op='direction_analysis',id='showdir-'+source+'-'+name,source=source,preselect=True,steps=steps))
    return q

def membership_request():
    q=request();q['operations']=[dict(op='direction_analysis',id='showdir-membership-'+source,source=source,preselect=True,companion=True,picks={'2':1,'4':1,'8':0,'10':0,'12':1,'14':0},
        steps=['ShowDir','SelectCompanion','ShowDir','SelNone','Flip','Undo','Redo','SelectPrimary','Remove','SelectPrimary','Add','SelectCompanion','Remove','SelectPrimary','Remove','ShowDirOff']) for source in ('warped','line')]
    return q

def foreign_request():
    q=request();q['operations']=[dict(op='direction_analysis',id='showdir-foreign-'+source+'-'+str(int(preselect)),source=source,preselect=True,companion=True,picks={'2':1},
        steps=['ShowDir','SelectCompanion' if preselect else 'SelNone','Flip','Undo','Redo','ShowDirOff']) for source in ('warped','line') for preselect in (False,True)]
    return q

def multi_request():
    q=request();q['operations']=[dict(op='direction_analysis',id='showdir-multi-'+edit,source='warped',preselect=True,companion=True,picks={},
        steps=['ShowDir','SelAll',edit,'Undo','Redo','ShowDirOff']) for edit in ('UReverse','VReverse','SwapUV')]
    return q

if __name__=='__main__':main()
