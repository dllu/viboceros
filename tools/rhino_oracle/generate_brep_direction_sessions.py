"""Capture repeated Dir options and both session endings on shared operands."""
import gzip
import argparse
import hashlib
import json
from .client import OracleClient
from .generate_brep_direction import HERE


def request():
    operations=[]
    for source in ('warped','cylinder','sphere'):
        for preselect in (False,True):
            for cancel in (False,True):
                for name,sequence in [('inspect',[]),('charts',['UReverse','SwapUV','VReverse','Flip']),('twice',['UReverse','UReverse'])]:
                    operations.append(dict(op='brep_direction_command',id='dir-session-'+source+'-'+str(int(preselect))+str(int(cancel))+'-'+name,
                        source=source,edit='u_reverse',preselect=preselect,sequence=sequence,cancel=cancel))
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--modes',action='store_true');args=parser.parse_args()
    family='brep_direction_modes_seeded' if args.modes else 'brep_direction_sessions'
    query=HERE/'fixtures'/(family+'.json');output=HERE/'observations'/(family+'.json.gz')
    if query.exists() or output.exists():raise ValueError('Dir session captures are immutable')
    q=mode_request() if args.modes else request();query.write_text(json.dumps(q,indent=2)+'\n')
    files=['generate_brep_direction_sessions.py','brep_direction_probe.py','client.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']
    files+=['fixtures/brep_direction/'+name+'.3dm' for name in ('warped','cylinder','sphere')]
    images={name:(HERE/name).read_bytes() for name in files}
    print('Capturing {} Dir sessions'.format(len(q['operations'])),flush=True)
    data=OracleClient(settings_scheme='VibocerosOracleDirSessions20261102').run_rhino(q,timeout=900)
    if images!={name:(HERE/name).read_bytes() for name in files}:raise ValueError('Dir session producer changed during capture')
    if len(data['results'])!=len(q['operations']) or any('error' in row for row in data['results']):raise ValueError('Dir session capture failed')
    sources={name:hashlib.sha256(content).hexdigest() for name,content in images.items()}
    data['provenance']=dict(request_sha256=hashlib.sha256(query.read_bytes()).hexdigest(),sources=sources)
    for name,sha in sources.items():
        path=HERE/'observations/source_snapshots'/(sha+'.gz')
        if not path.exists():path.write_bytes(gzip.compress(images[name],mtime=0))
    output.write_bytes(gzip.compress((json.dumps(data,indent=2,allow_nan=False)+'\n').encode(),mtime=0))
    print('Recorded '+str(output),flush=True)


def mode_request():
    sequences=[['Mode=FlipU'],['Mode=FlipU','FlipAll'],['Mode=FlipV','FlipAll'],['Mode=SwapUV','FlipAll']]
    sequences += [['NextMode']*count+['FlipAll'] for count in range(1,5)]
    q=request();q['operations']=[]
    for preselect in (False,True):
        for cancel in (False,True):
            for index,sequence in enumerate(sequences):
                q['operations'].append(dict(op='brep_direction_command',id='dir-modes-'+str(int(preselect))+str(int(cancel))+'-'+str(index),
                    source='warped',edit='u_reverse',preselect=preselect,sequence=['Mode=FlipNormal']+sequence,cancel=cancel,pair=True))
    return q


if __name__=='__main__':main()
