"""Observe Stretch's remembered factor and options after completion or Escape."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    ops=[]
    for finish,last in [('Cancel',1.25),('Complete',.75)]:
        ops.append(dict(op='stretch_geometry_command',id='copies_'+finish,shape='Points',copy=True,rigid=True,grouped=True,factor=2.,targets=[.5,last],finish=finish))
        ops.append(dict(op='stretch_command',id='following_'+finish,start=[0.,0.,0.],end=[0.,0.,10.],points=[[2.,1.,5.]],copy=False,rigid=False,grouped=False,factor=1.9))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();p=HERE/'fixtures/stretch_preferences.json';p.write_text(json.dumps(q,indent=2)+'\n');r=OracleClient(settings_scheme='VibocerosOracleStretchPreferences20261020').run_rhino(q,timeout=300.)
    assert len(r['results'])==4 and all('error'not in row for row in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();r['provenance']=dict(request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_stretch_preferences.py','stretch_probe.py','stretch_geometry_probe.py','taper_command_probe.py','twist_command_probe.py','number_token.py','join_probe.py','merge_edges_probe.py','rhino_worker.py','structural_sdk.py')})
    (HERE/'observations/stretch_preferences.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded Stretch preference transitions',len(r['results']),flush=True)
if __name__=='__main__':main()
