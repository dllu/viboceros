"""Capture Stretch's curve and surface fitting policy through the public SDK."""
import hashlib
import json
from pathlib import Path
from .client import OracleClient
from .structural_sdk import validate_operations
HERE=Path(__file__).resolve().parent

def request():
    ops=[]
    for shape,axis,factor in [('Curve','Z',2.),('Surface','Z',2.),('Surface','Spatial',.5),('Surface','Skew',.5)]:
        for tol in [0.,1e-3,1e-5,1e-6]:ops.append(dict(op='stretch_fit_sdk',id='fit_'+str(len(ops)),shape=shape,axis=axis,factor=factor,fit_tolerance=tol))
    for tol in [0.,1e-6]:ops.append(dict(op='stretch_fit_sdk',id='fit_'+str(len(ops)),shape='Box',axis='Spatial',factor=.5,fit_tolerance=tol))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();p=HERE/'fixtures/stretch_fit.json';p.write_text(json.dumps(q,indent=2)+'\n');r=OracleClient(settings_scheme='VibocerosOracleStretchFit20261020').run_rhino(q,timeout=600.)
    assert len(r['results'])==18 and all('error'not in row for row in r['results'])
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();r['provenance']=dict(request_sha256=digest(p),sources={n:digest(HERE/n)for n in('generate_stretch_fit.py','stretch_fit_probe.py','twist_command_probe.py','rhino_worker.py','structural_sdk.py')});(HERE/'observations/stretch_fit.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n');print('Recorded Stretch SDK fits',len(r['results']),flush=True)
if __name__=='__main__':main()
