"""Retain true Enter and escaped copy batches for objects and grips."""
import json,hashlib
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    ops=[]
    def add(kind,source,**kw):
        row=dict(op='splop_finish',id='finish_'+str(len(ops)),kind=kind,source=source,surface='Plane',postselect=False,edit=False,parent=False,finish='Enter');row.update(kw);ops.append(row)
    for surface in('Plane','Cylinder'):
        for finish in('Enter','Cancel'):
            for source in('Cloud','Curve'):
                for post in(False,True):add('Whole',source,surface=surface,postselect=post,finish=finish)
            for source,edit in[('rational',False),('rational',True),('surface',False),('mesh',False)]:
                for parent in(False,True):add('Grips',source,surface=surface,edit=edit,parent=parent,finish=finish)
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();path=HERE/'fixtures/splop_finish.json';path.write_text(json.dumps(q,indent=2)+'\n')
    names=['generate_splop_finish.py','splop_finish_probe.py','splop_reference_probe.py','splop_probe.py','splop_grips_probe.py','grip_transform_probe.py','twist_command_probe.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py'];digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();sources={n:digest(HERE/n)for n in names}
    r=OracleClient(settings_scheme='VibocerosOracleSplopFinish20261020').run_rhino(q,timeout=600.)
    assert sources=={n:digest(HERE/n)for n in names}
    assert len(r['results'])==len(q['operations'])and all('error'not in row for row in r['results'])
    r['provenance']=dict(request_sha256=digest(path),sources=sources);(HERE/'observations/splop_finish.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded true Splop finish cases',len(r['results']),flush=True)
if __name__=='__main__':main()
