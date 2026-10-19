"""Capture public MarkFoci commands and independently saved source geometry."""
from pathlib import Path
import hashlib
import json
import subprocess
import time
from .client import OracleClient
from .structural_sdk import validate_operations
from .references.conic_centers import cases, elevated, split_quadratic

ROOT=Path(__file__).resolve().parents[2]

def request():
    folder=ROOT/'tools/rhino_oracle/fixtures/mark_foci';folder.mkdir(exist_ok=True)
    def op(name,family='ellipse',**kw):
        return dict(op='mark_foci_command',id=name.replace('.', 'p'),family=family,artifact_path=str(folder/(name.replace('.', 'p')+'.3dm')),**kw)
    out=[]
    for family in ('ellipse','ellipse_arc','hyperbola','parabola','circle','circle_arc'):
        out.extend([op(family,family),op(family+'_axis',family,draw_axis=True),
                    op(family+'_preselected',family,preselect=True,draw_axis=True),
                    op(family+'_tilted',family,draw_axis=True,tilted=True),
                    op(family+'_elevated',family,draw_axis=True,elevate=5),
                    op(family+'_polycurve',family,draw_axis=True,polycurve=True)])
    for item in cases():
        if item['id'] in ('ellipse','quarter','elevated-12','refined-0.5','projective-weights','stationary-quartic','sheared',
                          'eccentric-0.1','altered-span','nonplanar','near-conic','negative-gauge'):
            out.append(op('definition_'+item['id'],'definition',curve=item['curve'],draw_axis=True))
    for family in ('hyperbola','parabola'):
        base=next(i['curve']for i in cases()if i['id']==family)
        for label,curve in [('refined',split_quadratic(base,.3,True)),('elevated12',elevated(base,12))]:
            out.append(op(family+'_'+label,'definition',curve=curve,draw_axis=True))
    for family in ('line','cubic'):
        out.extend([op(family,family),op(family+'_mixed',family,mixed=True,draw_axis=True)])
    out.extend([op('ellipse_reverse',reverse=True,draw_axis=True),op('hyperbola_domain','hyperbola',domain=[-3.,17.],draw_axis=True)])
    q=dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=out)
    validate_operations(q);return q

def main():
    q=request();folder=ROOT/'tools/rhino_oracle/observations'
    (folder/'mark_foci.request.json').write_text(json.dumps(q,indent=2)+'\n')
    start=time.monotonic()
    r=OracleClient(settings_scheme='VibocerosOracleMarkFoci20261019').run_rhino(q,timeout=300.)
    if [v['id']for v in r['results']]!=[v['id']for v in q['operations']]:raise ValueError('incomplete MarkFoci capture')
    digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    sources=('mark_foci_probe.py','generate_mark_foci.py','rhino_worker.py','structural_sdk.py','join_probe.py','merge_edges_probe.py','references/conic_centers.py')
    r['provenance']=dict(base_commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),seconds=time.monotonic()-start,
        sources={f:digest(Path(__file__).with_name(f.split('/')[0]).joinpath(*f.split('/')[1:]))for f in sources},fixtures={Path(o['artifact_path']).name:digest(Path(o['artifact_path']))for o in q['operations']})
    (folder/'mark_foci.json').write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('Recorded',len(r['results']),'actual MarkFoci commands',flush=True)

if __name__=='__main__':main()
