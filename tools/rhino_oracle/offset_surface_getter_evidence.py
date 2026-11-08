"""Audit all OffsetSrf discoveries, qualified inputs and real history receipts."""
import gzip
import json
from collections import Counter
from . import offset_surface_command as recipes
from .fillet_getter_evidence import ROOT, HERE, finite, normalized, sha
from .fillet_zero_history import audit

BASELINE = 'b32dbd8ab56b650cba858079bcbf82a7b40a8593'
FAMILIES = {'offset_srf_getter_'+name:list(getattr(recipes,name.upper()))
            for name in ['pilot','stages','qualified','holdouts','final','completion','batch','menus']}
DIAGNOSTICS = {
    'offset_default_accept':'Completed Corner=Round pilot; round-corner admission is outside this increment.',
    'offset_number_cancel':'Discovery macro supplied DeleteInput while Solid=No; native rejected that option.',
    'offset_menu_accept':'Discovery macro entered a nested Distance command and timed out.',
    'offset_plain_accept':'Discovery macro accepted the distance subprompt and then timed out.',
    'offset_postselect_accept':'Discovery macro accepted the distance subprompt and then timed out.',
    'offset_q_through':'ThroughPoint committed before the redundant final Enter could be sent.',
    'offset_h_distance_points':'The driver did not yet recognize the second-distance-point prompt.',
    'offset_b_negative_distance_default':'A negative distance default cannot be accepted with Enter; the discovery timed out.',
    'offset_f_through_cancel':'One Escape left ThroughPoint; a second was needed to cancel the command.',
}
EXPECTED = dict(observations=56,successful=41,cancelled=15,qualified=47,
                qualified_successful=39,qualified_cancelled=8,diagnostic=9,
                history_records=41,qualified_history_records=39,physical_picks=1)

def result(value):
    events=[e for e in value['events'] if e['name']=='OffsetSrf']
    if len(events)!=1 or events[0]['result'] not in ('Success','Cancel'): raise ValueError('native OffsetSrf result')
    return events[0]['result']

def frozen(sources,paths):
    for digest in sources.values():
        p=HERE/('observations/source_snapshots/'+digest+'.gz')
        if sha(gzip.decompress(p.read_bytes()))!=digest: raise ValueError('frozen producer')
        paths.add(p)

def collect(proof):
    receipts=json.loads((proof/'captures.json').read_bytes())
    if [r['family'] for r in receipts]!=list(FAMILIES): raise ValueError('capture inventory')
    rows={}; captures=[];counts=Counter();paths={proof/'captures.json',HERE/'run_headless.sh'}
    for receipt,(family,names) in zip(receipts,FAMILIES.items()):
        fixture=HERE/('fixtures/'+family+'.json');archive=HERE/('observations/'+family+'.json.gz')
        raw=gzip.decompress(archive.read_bytes());data=json.loads(raw)
        if data['engine']!='rhino' or data['iterations']!=1 or [r['id'] for r in data['results']]!=names or json.loads(fixture.read_bytes())!=recipes.request(names): raise ValueError('native input inventory')
        if data['provenance']['request_sha256']!=sha(fixture.read_bytes()) or receipt['request_sha256']!=sha(fixture.read_bytes()) or receipt['observation_sha256']!=sha(raw): raise ValueError('capture hashes')
        if receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]: raise ValueError('headless capture receipt')
        log=proof/receipt['log'];lines=gzip.decompress(log.read_bytes()).decode().splitlines()
        if lines!=['Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]: raise ValueError('incomplete capture')
        paths.update([fixture,archive,log]);frozen(data['provenance']['sources'],paths)
        captures.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(raw)))
        for row in data['results']:
            key=row['id'];v=row['value'];trace=v['input_trace'][0];status=result(v)
            if key in rows or v['prescribed_case']!=recipes.CASES[key] or not finite(v) or not v['returned_idle'] or v['success'] is not (status=='Success'): raise ValueError('native recipe or result')
            rows[key]=v;counts['observations']+=1;counts['successful' if status=='Success' else 'cancelled']+=1
            history=audit(v,'OffsetSrf');counts['history_records']+=history['recorded']
            if normalized(v['before'])!=normalized(v['undo']) or normalized(v['after'])!=normalized(v['redo']): raise ValueError('native history geometry')
            for phase in ['before','after','undo','redo']:
                if not all(o['native_valid'] for o in v[phase]): raise ValueError('unexpected invalid native output')
            cancels=[i for i,a in enumerate(v['prescribed_case']['actions'][:trace['completed']]) if a=='Cancel']
            if v.get('external_keys')!=[dict(action=i,key='Escape',private_owned_window=True) for i in cancels]: raise ValueError('owned cancellation receipt')
            for click in trace.get('clicks',[]):
                if not click['motion_acknowledged'] or not click['click_acknowledged']: raise ValueError('physical click receipt')
                counts['physical_picks']+=1
            incomplete=bool(trace['errors']) or trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions'])
            unknown='Unknown command:' in v['history']
            if key in DIAGNOSTICS:
                if key!='offset_default_accept' and not (incomplete or unknown): raise ValueError('diagnostic classification changed')
                counts['diagnostic']+=1
            else:
                if incomplete or unknown: raise ValueError('unqualified native input')
                counts['qualified']+=1;counts['qualified_successful' if status=='Success' else 'qualified_cancelled']+=1
                counts['qualified_history_records']+=history['recorded']
    if dict(counts)!=EXPECTED: raise ValueError('native coverage counts: '+str(dict(counts)))
    failed=HERE/'observations/offset_srf_getter_failed_import.json';record=json.loads(failed.read_bytes())
    fixture=HERE/record['request'];log=HERE/record['log'];paths.update([failed,fixture,log])
    if sha(fixture.read_bytes())!=record['request_sha256'] or record['qualified_workflows']!=0 or record['response_retained'] is not False or b'only one ** allowed' not in gzip.decompress(log.read_bytes()): raise ValueError('failed import receipt')
    frozen(record['sources'],paths)
    return rows,captures,dict(counts),paths
