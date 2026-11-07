"""Audit nested getters and retain the unsuccessful partition investigations."""
import gzip
import hashlib
import json
from . import arc_blend_command as recipes, arc_blend_sdk as sdk
from .arc_blend_radius_evidence import finite,read,HERE
from .check_arc_blend_getters import ROOT
from .arc_blend_general_evidence import collect as general_evidence
FAMILIES=dict(arc_blend_radius_getters=list(recipes.RADIUS_GETTERS),arc_blend_radius_getter_holdouts=list(recipes.RADIUS_GETTER_HOLDOUTS))
PARTITIONS=dict(arc_blend_roundoff=list(sdk.ROUNDOFF),arc_blend_search_rules=list(sdk.SEARCH_RULES))

def capture_set(proof,families,public=False):
    sha=lambda b:hashlib.sha256(b).hexdigest()
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(families):raise ValueError('capture inventory changed')
    evidence={p for p in proof.iterdir()if p.is_file()};rows={};captures=[]
    for family,names in families.items():
        fixture=HERE/('fixtures/'+family+'.json');path=HERE/('observations/'+family+'.json.gz');data=read(family)
        request=(sdk if public else recipes).request(names)
        if data['engine']!='rhino' or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=request or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('fixed native coverage changed')
        evidence.update([fixture,path])
        for digest in data['provenance']['sources'].values():
            p=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('frozen producer changed')
            evidence.add(p)
        receipt=next(r for r in receipts if r['family']==family);log=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family] or log!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('capture did not complete')
        captures.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for row in data['results']:rows[family+'/'+row['id']]=row['value']
    return rows,captures,evidence

def collect(proof):
    rows,captures,evidence=capture_set(proof,FAMILIES)
    research,other,paths=capture_set(ROOT/'docs/arc-blend-partition-proof',PARTITIONS,True);captures+=other;evidence.update(paths)
    workflows={};picks=0
    for key,value in rows.items():
        name=key.split('/',1)[1];trace=value['input_trace'][0]
        if value['prescribed_case']!=recipes.CASES[name] or not value['success'] or not value['returned_idle'] or trace['errors'] or trace['completed']!=trace['expected'] or trace['expected']!=len(value['prescribed_case']['actions']) or value['after']!=value['redo'] or 'Unknown command'in value['history']:raise ValueError('incomplete native getter')
        outputs=[o for o in value['after']if'arc_segments'in o]
        if len(outputs)!=1 or len(outputs[0]['arc_segments'])!=2 or not finite(outputs) or any(not o['native_valid'] or o['selected']for o in value['after']):raise ValueError('invalid native getter geometry')
        if any(len(o['samples'])!=33 for o in [outputs[0],*outputs[0]['arc_segments']]):raise ValueError('incomplete native getter samples')
        if name.startswith('circle_repeat_'):
            research[key]=value
            continue
        for click in trace['clicks']:
            if not click['motion_acknowledged'] or not click['click_acknowledged'] or not finite(click['ray'])or len(click['ray'])!=2:raise ValueError('invalid physical input receipt')
            picks+=1
        workflows[key]=value
    for key,value in research.items():
        if '/circle_repeat_'in key:continue
        name=key.split('/',1)[1]
        if value['case']!=sdk.CASES[name]or not value['success']or not value['native_valid']or not finite(value):raise ValueError('invalid partition research')
        for kind,result in value['radius_search'].items():
            if kind=='rules':
                if len(result)!=32:raise ValueError('incomplete search rules')
                results=result
            else:
                if not 1<=len(result['steps'])<=64:raise ValueError('unbounded search trial')
                results=[result]
            if any(not 1/101<=r['fraction']<=100/101 for r in results):raise ValueError('invalid search fraction')
    repeated=read('arc_blend_radius_cocircular')
    *_,old_evidence=general_evidence(ROOT/'docs/arc-blend-radius-general-proof')
    evidence.update(old_evidence)
    evidence.add(ROOT/'docs/arc-blend-radius-general-validation.json')
    path=HERE/'observations/arc_blend_radius_cocircular.json.gz';evidence.add(path)
    for suffix,index in [('positive',1),('negative',2)]:
        new=research['arc_blend_radius_getter_holdouts/circle_repeat_'+suffix]
        old=repeated['results'][index]['value']
        curve=lambda v:next(o['curve']for o in v['after']if'arc_segments'in o)
        if curve(new)!=curve(old):raise ValueError('native repeated partition changed')
    if (len(workflows),len(research),picks)!=(9,14,28):raise ValueError('getter coverage changed')
    return workflows,research,captures,picks,evidence
