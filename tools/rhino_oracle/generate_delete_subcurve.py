"""Native DeleteSubCrv command captures, including exact input artifacts."""
from pathlib import Path
import hashlib,json,subprocess,time
from .client import OracleClient
from .delete_subcurve_probe import FAMILIES
from .structural_sdk import validate_operations
ROOT=Path(__file__).resolve().parents[2]
def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/delete_subcurve';folder.mkdir(exist_ok=True)
    def op(name,family='line',start=.2,end=.7,**extra):return dict(op='delete_subcurve_command',id=name,family=family,start=start,end=end,artifact_path=str(folder/(name+'.3dm')),**extra)
    ops=[]
    for family in FAMILIES:
        ops.extend([op(family,family),op(family+'_back',family,start=.7,end=.2)])
    for family in('line','arc','nurbs','polyline','polycurve'):
        ops.extend([op(family+'_start',family,start=0.,end=.3),op(family+'_end',family,start=.7,end=1.),op(family+'_all',family,start=0.,end=1.)])
    for family in('circle','nurbs_circle','closed_polyline','ellipse'):
        ops.extend([op(family+'_seam_start',family,start=0.,end=.25),op(family+'_seam_end',family,start=.75,end=1.),op(family+'_full_length',family,start=.2,end=.8,length_fraction=1.)])
    ops.extend([op('numeric',length=3.,end=.8),op('numeric_backward',start=.7,end=.2,length=3.),op('negative',start=.7,end=.2,length=-3.),op('clamp',start=.9,end=1.,length=3.),op('zero',length=0.),op('same_point',start=.5,end=.5),op('circle_same','circle',start=.4,end=.4),op('numeric_circle','circle',start=.8,end=.1,length=3.),op('numeric_closed','closed_polyline',start=.8,end=.1,length=3.)])
    for family in('line','circle','nurbs','closed_polyline','polycurve'):
        ops.append(op(family+'_domain',family,domain=[-3.,7.]))
    ops.extend([op('line_post',preselect=False),op('closed_post','closed_polyline',preselect=False),op('reversed_circle','circle',reverse=True)])
    ops.extend([op('opposite',start=.2,end=0.,length=3.),op('negative_opposite',start=.7,end=1.,length=-3.),op('no_confirm',length=3.,confirmation=False),op('circle_no_confirm','circle',length=3.,confirmation=False),op('closed_no_confirm','closed_polyline',length=3.,confirmation=False),op('rational_full_no_confirm','nurbs_circle',length_fraction=1.,confirmation=False),op('numeric_circle_backward','circle',start=.2,end=.8,length=3.),op('numeric_closed_backward','closed_polyline',start=.2,end=.8,length=3.)])
    for name,family,a,b,length,extra in [('early_repeat','circle',.2,.8,3.,{}),('early_toward','circle',.2,.4,3.,{}),('early_large','circle',.2,.8,10.,{}),('near_middle','circle',.4,.8,3.,{}),('late','circle',.6,.8,3.,{}),('early_domain','circle',.2,.8,3.,dict(domain=[-7.,13.])),('poly_early_toward','closed_polyline',.2,.4,3.,{}),('poly_early_large','closed_polyline',.2,.8,10.,{})]:
        ops.append(op(name,family,start=a,end=b,length=length,**extra))
    for family in('nurbs_circle','ellipse'):
        ops.extend([op(family+'_numeric_early',family,start=.2,end=.8,length=3.),op(family+'_numeric_late',family,start=.8,end=.1,length=3.)])
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=ops);validate_operations(q);return q

def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations';start=time.monotonic()
    (folder/'delete_subcurve.request.json').write_text(json.dumps(q,indent=2)+'\n')
    r=OracleClient(settings_scheme='VibocerosOracleDeleteSubCrv20261018FinalV4').run_rhino(q,timeout=360.)
    if len(r['results'])!=len(q['operations']):raise ValueError('incomplete native DeleteSubCrv capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    files=['delete_subcurve_probe.py','generate_delete_subcurve.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py']
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-start,sources={f:digest(Path(__file__).with_name(f))for f in files},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'delete_subcurve.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'actual DeleteSubCrv commands',flush=True)
if __name__=='__main__':main()
