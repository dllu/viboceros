"""Account for every Shell capture, actual history receipt and failed attempt."""
import gzip
import json
from . import shell_command as recipes
from .check_shell_native import ROOT, PROOF
from .fillet_getter_evidence import finite, normalized, sha
from .fillet_zero_history import audit
from .offset_surface_getter_evidence import frozen

HERE = ROOT/'tools/rhino_oracle'
BASELINE = '17ce1e16f7de5c477fd3dbdc53e335df6905b6f3'
COMMANDS = {'shell_getter_'+name:list(getattr(recipes,name.upper()))
            for name in ['pilot','qualified','holdouts']}
FAMILIES = [*COMMANDS, 'shell_sdk_signed', 'shell_sdk_geometry']
FAILURES = ['sdk-import-failure','sdk-finite-failure','sdk-negative-distance-failure',
            'getter-dispatch-failure','getter-number-dispatch-failure']
DIAGNOSTIC = 'shell_getter_inline'


def result(v):
    events = [e for e in v['events'] if e['name']=='Shell']
    if len(events)!=1 or events[0]['result'] not in ['Success','Cancel']:
        raise ValueError('native Shell result')
    return events[0]['result']


def collect(proof=PROOF):
    receipts = json.loads((proof/'captures.json').read_bytes())
    if [r['family'] for r in receipts] != FAMILIES: raise ValueError('capture inventory')
    paths = {proof/'captures.json', HERE/'run_headless.sh'}
    commands = {}; sdk = {}; signed = {}; captures = []; picks = 0
    for receipt in receipts:
        family = receipt['family']; fixture = HERE/('fixtures/'+family+'.json')
        archive = HERE/('observations/'+family+'.json.gz')
        raw = gzip.decompress(archive.read_bytes()); data = json.loads(raw)
        query = json.loads(fixture.read_bytes()); names = [op['id'] for op in query['operations']]
        if (data['engine']!='rhino' or data['iterations']!=1 or not finite(data)
                or [r['id'] for r in data['results']]!=names): raise ValueError('native coverage')
        if (data['provenance']['request_sha256']!=sha(fixture.read_bytes())
                or receipt['request_sha256']!=sha(fixture.read_bytes())
                or receipt['observation_sha256']!=sha(raw)): raise ValueError('capture hashes')
        if (receipt['exit_status']!=0 or receipt['command']!=['tools/rhino_oracle/run_headless.sh',
                'exec','python3','-m','tools.rhino_oracle.generate_'+family]): raise ValueError('headless receipt')
        log = proof/receipt['log']
        if gzip.decompress(log.read_bytes()).decode().splitlines()!=[
                'Capturing '+str(len(names))+' cases: '+family,'Recorded '+family]: raise ValueError('capture log')
        paths.update([fixture,archive,log]); frozen(data['provenance']['sources'],paths)
        captures.append(dict(family=family,observations=len(names),engine_version=data['engine_version'],raw_sha256=sha(raw)))
        if family in COMMANDS:
            if names!=COMMANDS[family] or query!=recipes.request(names): raise ValueError('authored recipes')
            for row in data['results']:
                key=row['id']; v=row['value']; trace=v['input_trace'][0]; status=result(v)
                if key in commands or v['prescribed_case']!=recipes.CASES[key] or not v['returned_idle']:
                    raise ValueError('command recipe')
                audit(v,'Shell')
                if (v['success'] is not (status=='Success') or v['history_changed'] is not (status=='Success')
                        or normalized(v['before'])!=normalized(v['undo'])
                        or normalized(v['after'])!=normalized(v['redo'])): raise ValueError('native history')
                for phase in ['before','after','undo','redo']:
                    if not all(o['native_valid'] and o['selected']==0 for o in v[phase]): raise ValueError('native validity or selection')
                incomplete = trace['completed']!=trace['expected'] or trace['expected']!=len(v['prescribed_case']['actions']) or bool(trace['errors'])
                unknown = 'Unknown command:' in v['history']
                if (key==DIAGNOSTIC) is not bool(incomplete or unknown): raise ValueError('diagnostic classification')
                cancels = [i for i,a in enumerate(v['prescribed_case']['actions'][:trace['completed']]) if a=='Cancel']
                if v['external_keys']!=[dict(action=i,key='Escape',private_owned_window=True) for i in cancels]: raise ValueError('private Escape receipt')
                for click in trace.get('clicks',[]):
                    if not click['motion_acknowledged'] or not click['click_acknowledged']: raise ValueError('physical click receipt')
                    picks+=1
                commands[key]=v
        elif family=='shell_sdk_geometry':
            for row in data['results']:
                if not row['value']['breps'] or any(not b['is_solid'] for b in row['value']['breps']): raise ValueError('SDK wall solid')
                sdk[row['id']]=row['value']
        else:
            for row in data['results']:
                v=row['value']
                if (v['source_orientation']!='Outward' or [a['distance'] for a in v['attempts']]!=[-1.,1.]
                        or any(b['is_solid'] for b in v['attempts'][0]['breps'])
                        or not all(b['is_solid'] and b['native_valid'] for b in v['attempts'][1]['breps'])): raise ValueError('SDK sign discovery')
                signed[row['id']]=v
    for name in FAILURES:
        p=proof/(name+'.json'); value=json.loads(p.read_bytes()); log=proof/(name+'.log.gz')
        fixture=ROOT/value['request']; paths.update([p,log,fixture]); frozen(value['sources'],paths)
        if (value['captured_native_results']!=0 or sha(fixture.read_bytes())!=value['request_sha256']
                or sha(gzip.decompress(log.read_bytes()))!=value['log_sha256']): raise ValueError('failed attempt inventory')
    counts=dict(observations=40,command_observations=len(commands),qualified_commands=len(commands)-1,
                successful_commands=sum(result(v)=='Success' for v in commands.values()),
                cancelled_commands=sum(result(v)=='Cancel' for v in commands.values()),
                diagnostics=1,sdk_geometry_cases=len(sdk),sdk_sign_cases=len(signed),physical_picks=picks,
                retained_failed_attempts=len(FAILURES))
    if counts!=dict(observations=40,command_observations=25,qualified_commands=24,
            successful_commands=17,cancelled_commands=8,diagnostics=1,sdk_geometry_cases=9,
            sdk_sign_cases=6,physical_picks=22,retained_failed_attempts=5): raise ValueError('evidence counts: '+str(counts))
    return commands,sdk,signed,captures,counts,paths
