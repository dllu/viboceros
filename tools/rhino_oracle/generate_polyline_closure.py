"""Retain real SDK/command closure behavior and one source-only 3dm matrix."""
from pathlib import Path
import hashlib
import json
import math
import subprocess
import time
from .client import OracleClient
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]

def cases():
    out=[];zero=2.**-32
    profiles=[('exact',[0.,0.,0.]),('near',[1e-10,0.,0.]),('threshold',[zero,0.,0.]),
              ('below',[math.nextafter(zero,0.),0.,0.]),('above',[math.nextafter(zero,math.inf),0.,0.]),
              ('diagonal',[2e-10,2e-10,2e-10]),('open',[1e-7,0.,0.]),('negative',[-1e-10,0.,0.])]
    definitions=[(name,[[0.,0.,0.],[4.,0.,0.],[4.,3.,0.],end])for name,end in profiles]
    definitions.extend([('two',[[0.,0.,0.],[4.,0.,0.]]),('two_segments',[[0.,0.,0.],[4.,0.,0.],[0.,0.,0.]]),
        ('tiny_exact',[[0.,0.,0.],[1e-11,0.,0.],[1e-11,1e-11,0.],[0.,0.,0.]]),
        ('tiny_near',[[0.,0.,0.],[1e-11,0.,0.],[1e-11,1e-11,0.],[1e-12,0.,0.]]),
        ('tiny_distinct',[[0.,0.,0.],[1e-9,0.,0.],[1e-9,1e-9,0.],[1e-11,0.,0.]]),
        ('collinear',[[0.,0.,0.],[4.,0.,0.],[2.,0.,0.],[1e-10,0.,0.]]),
        ('relative_closed',[[1e12,1e12,1e12],[1e12+4.,1e12,1e12],[1e12+4.,1e12+3.,1e12],[1e12+.4,1e12,1e12]]),
        ('relative_open',[[1e12,1e12,1e12],[1e12+4.,1e12,1e12],[1e12+4.,1e12+3.,1e12],[1e12+.6,1e12,1e12]]),
        # Coincidence is not transitive. An interior vertex must differ from both ends.
        ('no_witness',[[0.,0.,0.],[-zero*.75,0.,0.],[zero*.5,0.,0.],[zero,0.,0.]]),
        ('witness_last',[[0.,0.,0.],[zero*.1,0.,0.],[4.,3.,0.],[zero,0.,0.]])])
    for form in('polyline','nurbs','weighted_nurbs','unit_rational_nurbs','polycurve','nested'):
        for name,p in definitions:
            out.append(dict(id=form+'_'+name,form=form,points=p,
                extract=name in('exact','near','open','tiny_exact','tiny_near','two_segments','relative_closed','no_witness'),
                move_grip=name in('near','relative_closed')))
    for form in('polyline','nurbs','polycurve'):
        for name,d in [('shifted',[-3.,17.]),('tiny_domain',[0.,1e-170]),('large_domain',[0.,1e170])]:
            out.append(dict(id=form+'_'+name,form=form,points=definitions[1][1],domain=d))
    for degree in(2,3):
        for name,p in definitions:
            if len(p)==4:
                out.append(dict(id='degree%d_'%degree+name,form='nurbs',degree=degree,points=p,extract=name in('exact','near','relative_closed'),move_grip=name=='relative_closed'))
    for form in ('polyline','nurbs','weighted_nurbs','unit_rational_nurbs','polycurve','nested'):
        p=next(p for name,p in definitions if name=='relative_closed')
        out.append(dict(id=form+'_grip1',form=form,points=p,move_grip=True,grip_index=1))
        out.append(dict(id=form+'_zero',form=form,points=p,move_grip=True,move_vector=[0.,0.,0.]))
    for degree in (2,3):
        out.append(dict(id='degree%d_grip1'%degree,form='nurbs',degree=degree,points=p,move_grip=True,grip_index=1))
    return out

def request():
    q=dict(protocol_version=1,iterations=1,operations=[dict(op='polyline_closure',id='closure_matrix',cases=cases(),artifact_path=str(ROOT/'tools/rhino_oracle/fixtures/polyline_closure.3dm'))])
    validate_operations(q);return q

def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'polyline_closure.request.json').write_text(json.dumps(q,indent=2)+'\n')
    start=time.monotonic();r=OracleClient(settings_scheme='VibocerosOraclePolylineClosure20261019').run_rhino(q,timeout=360.)
    if len(r['results'])!=1 or len(r['results'][0]['value']['rows'])!=len(cases()):raise ValueError('incomplete closure capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    names=('polyline_closure_probe.py','generate_polyline_closure.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-start,
        sources={n:digest(Path(__file__).with_name(n))for n in names},fixture=digest(Path(q['operations'][0]['artifact_path'])))
    (folder/'polyline_closure.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(cases()),'closure sources with commands and grips',flush=True)
if __name__=='__main__':main()
