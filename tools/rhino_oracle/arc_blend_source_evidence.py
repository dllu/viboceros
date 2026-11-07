"""Audit curved contacts, scalar syntax, shared point input and failed attempts."""
import gzip
import hashlib
import json
import math
import re
from . import arc_blend_command as recipes,line_construction_cases as line_recipes
from .arc_blend_radius_evidence import finite,read,HERE
from .arc_blend_preference_evidence import collect as prior_evidence
from .check_arc_blend_sources import ROOT
FAMILIES=dict(arc_blend_length_discovery=list(recipes.LENGTH_DISCOVERY),arc_blend_sources=[*recipes.CURVED_SOURCES,*recipes.LENGTH_INPUT],arc_blend_source_holdouts=list(recipes.SOURCE_INPUT_HOLDOUTS))
POINTS='arc_blend_point_quantities'
BASELINE='62225eb7bd4579d392ae9212992cd2ee77e73e2d'

def target(value):
    lines=[s for s in value['history'].splitlines()if s.startswith('Select arc blend point to adjust.')]
    match=re.search(r'RadiusDifference=([^\s)]+)',lines[-1])
    return float(match[1])if match else None

def captures(proof):
    families={**FAMILIES,POINTS:list(line_recipes.QUANTITY_INPUT)}
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family']for r in receipts]!=list(families):raise ValueError('capture inventory changed')
    evidence={p for p in proof.iterdir()if p.is_file()};rows={};summary=[];sha=lambda b:hashlib.sha256(b).hexdigest()
    for family,names in families.items():
        module=line_recipes if family==POINTS else recipes
        fixture=HERE/('fixtures/'+family+'.json');path=HERE/('observations/'+family+'.json.gz');data=read(family)
        if data['engine']!='rhino'or data['iterations']!=1 or [r['id']for r in data['results']]!=names or json.loads(fixture.read_bytes())!=module.request(names)or data['provenance']['request_sha256']!=sha(fixture.read_bytes()):raise ValueError('native source coverage changed')
        evidence.update([fixture,path])
        for digest in data['provenance']['sources'].values():
            p=HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes()))!=digest:raise ValueError('frozen producer changed')
            evidence.add(p)
        receipt=next(r for r in receipts if r['family']==family)
        log=gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]or log!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]:raise ValueError('incomplete source capture')
        summary.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for row in data['results']:rows[family+'/'+row['id']]=row['value']
    return rows,summary,evidence

def collect(proof):
    rows,summary,evidence=captures(proof)
    *_,old=prior_evidence(ROOT/'docs/arc-blend-preferences-proof');evidence.update(old);evidence.add(ROOT/'docs/arc-blend-preferences-validation.json')
    workflows={};points={};diagnostics={};picks=stations=0
    for key,v in rows.items():
        family,name=key.split('/',1);trace=v['input_trace'][0]
        recipe=line_recipes.recipe(name)if family==POINTS else recipes.CASES[name]
        if v['prescribed_case']!=recipe or not v['returned_idle']:raise ValueError('source recipe changed')
        if family=='arc_blend_length_discovery'and name!='length_control':
            expected=['incomplete owned ArcBlend input']if name!='length_unit_sum'else[]
            if trace['errors']!=expected or trace['expected']-trace['completed']!=(0 if name=='length_unit_sum'else 1)or not v['success']:raise ValueError('discovery classification changed')
            if name=='length_unit_sum'and 'Unknown command: 2mm+3mm'not in v['history']:raise ValueError('missing rejected quantity')
            diagnostics[key]=v;continue
        if name=='quantity_nested_unit':
            if trace['errors']!=['owned ArcBlend input timeout']or v['success']or 'Unknown command: 2*(.25cm)'not in v['history']:raise ValueError('nested quantity classification changed')
            diagnostics[key]=v;continue
        if trace['errors']or trace['completed']!=trace['expected']or trace['expected']!=len(recipe['actions'])or not v['success']or not v['history_changed']or 'Unknown command'in v['history']:raise ValueError('incomplete source workflow')
        if not finite(v['after'])or any(not o['native_valid']or family!=POINTS and o['selected']for o in v['after']):raise ValueError('invalid native source geometry')
        by_name=lambda objects:sorted(objects,key=lambda o:o['attributes']['name'])
        if by_name(v['after'])!=by_name(v['redo']):raise ValueError('native redo changed source geometry')
        if recipe.get('model_units')and v['model_units']!=recipe['model_units']:raise ValueError('native model units changed')
        output=[o for o in v['after']if not o['attributes']['name']]
        if len(output)!=1:raise ValueError('missing source output')
        if family==POINTS:points[name]=v;continue
        if 'arc_segments'in output[0]:
            arcs=output[0]['arc_segments']
            if len(arcs)not in (2,4)or any(len(o['samples'])!=33 for o in [output[0],*arcs]):raise ValueError('incomplete native source samples')
            stations+=33*(1+len(arcs))
        t=target(v)
        if t is not None and not math.isfinite(t):raise ValueError('invalid native target')
        for click in trace['clicks']:
            if not click['motion_acknowledged']or not click['click_acknowledged']or not finite(click['ray'])or len(click['ray'])!=2:raise ValueError('invalid source physical input')
            if 'pick_transform'in click and not finite(click['pick_transform']):raise ValueError('invalid source projection')
            picks+=1
        workflows[key]=v
    attempts=json.loads((proof/'attempts.json').read_bytes())
    if len(attempts)!=1 or attempts[0]['exit_status']!=1 or attempts[0]['family']!='arc_blend_sources':raise ValueError('failed source attempt changed')
    attempt=attempts[0];log=gzip.decompress((proof/attempt['log']).read_bytes()).decode()
    if "unexpected token '*'"not in log or 'source_cubic_default: start'not in log:raise ValueError('missing failed IronPython receipt')
    for digest in attempt['sources'].values():
        p=HERE/('observations/source_snapshots/'+digest+'.gz')
        if hashlib.sha256(gzip.decompress(p.read_bytes())).hexdigest()!=digest:raise ValueError('failed producer changed')
        evidence.add(p)
    regression=json.loads((proof/'regression.json').read_bytes());log=gzip.decompress((proof/'regression-before.log.gz').read_bytes()).decode()
    if regression!=dict(baseline_revision=BASELINE,command=['cargo','test','-p','viboceros-command','--test','arc_blend_sources','--','--nocapture'],exit_status=101)or 'native/rational parameter mismatch: 3.57200914943144e-2'not in log:raise ValueError('missing original contact regression')
    if (len(workflows),len(points),len(diagnostics),picks,stations)!=(35,2,7,98,3498):raise ValueError('source qualification coverage changed')
    return workflows,points,diagnostics,summary,picks,stations,evidence
