"""Frozen search inputs, acknowledged command histories and representation limits."""
import gzip
import hashlib
import json
from . import arc_blend_command as recipes, arc_blend_sdk as sdk
from . import arc_blend_radius_evidence as prior
from .check_arc_blend_radius_general import ROOT
HERE=ROOT/'tools/rhino_oracle'
FAMILIES=dict(arc_blend_radius_sdk=list(sdk.RADIUS_SEARCH),
              arc_blend_radius_general=list(recipes.RADIUS_GENERAL),
              arc_blend_radius_general_holdouts=list(recipes.RADIUS_GENERAL_HOLDOUTS),
              arc_blend_radius_cocircular=list(recipes.RADIUS_COCIRCULAR))
REPRESENTATION={"arc_blend_radius_cocircular/rg_cocircular_1","arc_blend_radius_cocircular/rg_cocircular_2"}
def read(family):return prior.read(family)
def final_warning(value):
    lines=value['history'].splitlines()
    start=max(i for i,s in enumerate(lines)if s.startswith(('Enter target radius difference','Select start point location:','Select end point location:')))
    return any('Unable to achieve requested radius difference' in s for s in lines[start:])

def collect(proof):
    _,_,_,_,evidence=prior.collect(ROOT/'docs/arc-blend-radius-proof')
    evidence.add(ROOT/'docs/arc-blend-radius-validation.json')
    data=read('arc_blend_radius_targets')
    workflows={'arc_blend_radius_targets/'+r['id']:r['value']for r in data['results']}
    public={}
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(FAMILIES):raise ValueError('capture inventory changed')
    evidence.update(p for p in proof.iterdir()if p.is_file())
    captures=[]
    sha=lambda b:hashlib.sha256(b).hexdigest()
    for family,names in FAMILIES.items():
        fixture=HERE/('fixtures/'+family+'.json');path=HERE/('observations/'+family+'.json.gz');data=read(family)
        query=sdk.request(names)if family=='arc_blend_radius_sdk'else recipes.request(names)
        if data['engine']!='rhino' or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=query or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native fixed input coverage changed')
        evidence.update([fixture,path])
        for digest in data['provenance']['sources'].values():
            source=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(source.read_bytes()))!=digest:raise ValueError('frozen producer changed')
            evidence.add(source)
        receipt=next(r for r in receipts if r['family']==family)
        lines=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family] or lines!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('capture did not complete')
        captures.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for r in data['results']:
            v=r['value'];key=family+'/'+r['id']
            if family=='arc_blend_radius_sdk':
                if v['case']!=sdk.CASES[r['id']] or not v['success'] or not prior.finite(v) or not v['native_valid'] or len(v['samples'])!=33 or len(v['segments'])!=2 or any(not s['native_valid'] or len(s['samples'])!=33 or s['arc']['radius']<=0 for s in v['segments']):raise ValueError('invalid public SDK measurement')
                public[r['id']]=v
            else:workflows[key]=v
    picks=0
    for key,v in workflows.items():
        name=key.split('/',1)[1];trace=v['input_trace'][0]
        if v['prescribed_case']!=recipes.CASES[name] or not v['success'] or not v['returned_idle'] or trace['errors'] or trace['completed']!=trace['expected'] or trace['completed']!=len(v['prescribed_case']['actions']) or v['after']!=v['redo'] or 'Unknown command' in v['history']:raise ValueError('incomplete native workflow or history')
        outputs=[o for o in v['after']if 'arc_segments'in o]
        if len(outputs)!=1 or len(outputs[0]['arc_segments'])!=2 or not prior.finite(outputs) or any(not o['native_valid'] or o['selected'] for o in v['after']):raise ValueError('invalid independent arc measurements')
        for o in [outputs[0],*outputs[0]['arc_segments']]:
            if len(o['samples'])!=33 or any(len(s['point'])!=3 or len(s['tangent'])!=3 for s in o['samples']):raise ValueError('incomplete independent samples')
        if key in REPRESENTATION:
            arcs=outputs[0]['arc_segments'];normals=[]
            for arc in arcs:
                radial=[p-c for p,c in zip(arc['samples'][0]['point'],arc['center'])];t=arc['samples'][0]['tangent']
                n=[radial[1]*t[2]-radial[2]*t[1],radial[2]*t[0]-radial[0]*t[2],radial[0]*t[1]-radial[1]*t[0]]
                length=sum(x*x for x in n)**.5
                if not length:raise ValueError('invalid supporting-circle orientation')
                normals.append([x/length for x in n])
            if sum((a-b)**2 for a,b in zip(normals[0],normals[1]))>1e-20:raise ValueError('inconsistent supporting-circle orientation')
        for c in trace['clicks']:
            if not c['motion_acknowledged'] or not c['click_acknowledged'] or len(c['ray'])!=2 or not prior.finite(c['ray']):raise ValueError('invalid physical input receipt')
            picks+=1
    if (len(workflows),len(public),picks)!=(79,48,162):raise ValueError('search coverage changed')
    return workflows,public,captures,picks,evidence
