"""Capture actual ReducePolyline commands from public, independently saved sources."""
from pathlib import Path
import hashlib
import json
import math
import subprocess
import time
from .client import OracleClient
from .structural_sdk import validate_operations

ROOT = Path(__file__).resolve().parents[2]


def request():
    folder = ROOT/'tools/rhino_oracle/fixtures/reduce_polyline'; folder.mkdir(exist_ok=True)
    def op(name, points, value=.15, form='polyline', **kw):
        return dict(op='reduce_polyline_command', id=name, points=points, form=form,
                    reduction_tolerance=value, artifact_path=str(folder/(name+'.3dm')), **kw)
    p = lambda pairs: [[float(x),float(y),0.] for x,y in pairs]
    wave = p([(0,0),(1,.1),(2,0),(3,.1),(4,0),(5,2),(6,2.01),(7,2)])
    rect = p([(0,0),(1,0),(4,0),(4,3),(0,3),(0,1),(0,0)])
    cases = [op('wave',wave),op('wave_default',wave,entry='default'),op('line',p([(0,0),(1,0),(4,0)])),
             op('two',p([(0,0),(3,0)])),op('backtrack',p([(0,0),(3,0),(1,0),(5,0)])),
             op('closed',rect),op('closed_large',rect,10.),
             op('triangle_large',p([(0,0),(4,0),(0,4),(0,0)]),10.),
             op('short_start',p([(0,0),(.01,.01),(1,2),(3,0)])),
             op('short_end',p([(0,0),(1,2),(3,0),(3.01,.01)])),
             op('short_middle',p([(0,0),(2,2),(2.01,2),(4,0)])),
             op('short_hairpin',p([(0,0),(2,0),(2,.05),(0,.05)])),
             op('narrow_closed',p([(0,0),(2,0),(2,.05),(0,.05),(0,0)])),
             op('self_cross',p([(0,0),(3,3),(0,3),(3,0),(0,0)])),
             op('spatial',[[i,.1*(i%2),float(i%3)]for i in range(12)]),
             op('all_short',p([(0,0),(.01,.02),(.03,0),(.04,.01)])),
             op('long_small_angle',p([(0,0),(1e6,.001),(2e6,0)]),.0001)]
    for value,label in [(.099999999999,'below'),(.1,'equal'),(.100000000001,'above')]:
        cases.append(op('threshold_'+label,p([(0,0),(1,.1),(2,0)]),value))
    for form in ('nurbs','weighted_nurbs','polycurve','nested','elevated_nurbs'):
        cases.extend([op(form,wave,form=form),op(form+'_closed',rect,form=form),
                      op(form+'_domain',wave,form=form,domain=[-3.,17.]),
                      op(form+'_unchanged',wave,1e-8,form=form,domain=[-3.,17.])])
    for name,points in [('wave',wave),('closed',rect),('two',p([(0,0),(3,0)]))]:
        cases.extend([op(name+'_copy',points,copy=True),op(name+'_post',points,preselect=False),
                      op(name+'_post_copy',points,copy=True,preselect=False)])
    cases.extend([op('overshoot',p([(0,0),(3,0),(1,0)])),
                  op('cubic_line',p([(0,0),(1,0),(2,0),(3,0)]),form='cubic'),
                  op('domain',wave,domain=[-3.,17.]),
                  op('ineligible_line',p([(0,0),(3,0)]),form='line',mixed=True),
                  op('ineligible_cubic',p([(0,0),(1,2),(2,-1),(3,0)]),form='cubic',mixed=True)])
    for count,value in [(25,.02),(64,.1),(65,.8)]:
        points=[[float(i)/4.,math.sin(i*.3),.05*math.cos(i*.7)] for i in range(count)]
        cases.append(op('sampled_'+str(count),points,value))
    cases.append(op('flat_1024',[[float(i),.00001*(i%2),0.] for i in range(1024)],.01,benchmark=True))
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases)
    validate_operations(q); return q


def main():
    q=request(); folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'reduce_polyline.request.json').write_text(json.dumps(q,indent=2)+'\n')
    start=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleReducePolyline20261018').run_rhino(q,timeout=300.)
    if [v['id']for v in r['results']]!=[v['id']for v in q['operations']]:raise ValueError('incomplete ReducePolyline capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources=('reduce_polyline_probe.py','generate_reduce_polyline.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-start,
        sources={f:digest(Path(__file__).with_name(f))for f in sources},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'reduce_polyline.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'actual ReducePolyline commands',flush=True)


if __name__=='__main__':main()
