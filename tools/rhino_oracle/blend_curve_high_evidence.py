"""Audit G3/G4 public captures while retaining the earlier bounded record."""
import gzip
import hashlib
import json
from . import blend_curve_command as recipes, blend_curve_evidence as prior
from .arc_blend_radius_evidence import finite, read, HERE
from .check_blend_curve_high import ROOT
BASELINE = '6917879a5ef390da32ba821412804daafa3267cd'
FAMILIES = dict(blend_curve_high_discovery=list(recipes.HIGH_DISCOVERY),
                blend_curve_high_holdouts=list(recipes.HIGH_HOLDOUTS))

def captures(proof, families=FAMILIES):
    receipts = json.loads((proof/'captures.json').read_bytes())
    paths = {p for p in proof.iterdir() if p.is_file()}
    rows, summary = {}, []
    sha = lambda b: hashlib.sha256(b).hexdigest()
    if [r['family'] for r in receipts] != list(families):
        raise ValueError('capture inventory changed')
    for receipt, (family, names) in zip(receipts, families.items()):
        fixture = HERE/('fixtures/'+family+'.json')
        path = HERE/('observations/'+family+'.json.gz')
        data = read(family)
        if (data['engine'] != 'rhino' or data['iterations'] != 1
                or [r['id'] for r in data['results']] != names
                or json.loads(fixture.read_bytes()) != recipes.request(names)
                or data['provenance']['request_sha256'] != sha(fixture.read_bytes())):
            raise ValueError('native coverage or fixed request changed')
        paths.update([fixture, path])
        for digest in data['provenance']['sources'].values():
            p = HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes())) != digest:
                raise ValueError('frozen producer changed')
            paths.add(p)
        log = gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines()
        if (receipt['exit_status'] != 0 or receipt['command'] != [
                'tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m',
                'tools.rhino_oracle.generate_'+family]
                or log != ['Capturing '+str(len(names))+' cases: '+family, 'Recorded '+family]):
            raise ValueError('incomplete higher-order capture')
        summary.append(dict(family=family, observations=len(names),
                            engine_version=data['engine_version'],
                            raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        rows.update((family+'/'+r['id'], r['value']) for r in data['results'])
    return rows, summary, paths

def orders(v):
    result = [2, 2]
    for action in v['prescribed_case']['actions']:
        for end in [0, 1]:
            prefix = 'Continuity_'+str(end+1)+'=G'
            if isinstance(action, str) and action.startswith(prefix):
                result[end] = int(action[len(prefix):])
    return result

def collect(proof):
    rows, summary, paths = captures(proof)
    *_, old_paths = prior.collect(ROOT/'docs/blend-curve-proof')
    paths.update(old_paths)
    paths.add(ROOT/'docs/blend-curve-validation.json')
    previous, _, _ = prior.captures(ROOT/'docs/blend-curve-proof')
    promoted = {k: v for k, v in previous.items() if k.split('/')[1].startswith(('g3_', 'g4_'))}
    rows = promoted | rows
    picks = stations = derivative_stations = zero_fourth_ends = 0
    for key, v in rows.items():
        name = key.split('/')[1]
        trace = v['input_trace'][0]
        if v['prescribed_case'] != recipes.CASES[name] or not v['returned_idle']:
            raise ValueError('higher-order recipe changed')
        if (not finite(v) or any(not o['native_valid'] for phase in ['before','after','undo','redo'] for o in v[phase])):
            raise ValueError('invalid native high-order geometry')
        if (trace['errors'] or trace['completed'] != trace['expected']
                or trace['expected'] != len(recipes.CASES[name]['actions'])
                or 'Unknown command' in v['history']):
            raise ValueError('incomplete higher-order workflow')
        if prior.command_result(v) != 'Nothing' or v['success'] is not False or not v['history_changed']:
            raise ValueError('native result semantics changed')
        if (prior.normalized(v['after']) != prior.normalized(v['redo'])
                or prior.normalized(v['before']) != prior.normalized(v['undo'])):
            raise ValueError('higher-order history changed')
        output = prior.output(v)
        order = orders(v)
        if output is None or len(output['samples']) != 33:
            raise ValueError('missing independent native samples')
        joined = output['native_type'] == 'PolyCurve'
        if joined:
            source = next(o for o in v['before'] if o['attributes']['name'] == 'guide0')
            if output['attributes'] != source['attributes'] or any(o['attributes']['name'] == 'guide1' for o in v['after']):
                raise ValueError('joined high-order ownership changed')
        elif output['curve']['degree'] != sum(order)+1:
            raise ValueError('minimum blend degree changed')
        if name.startswith('high_'):
            for phase in ['before', 'after']:
                for obj in v[phase]:
                    if 'curve' not in obj: continue
                    samples = obj.get('derivative_samples', [])
                    if len(samples) != 3 or any(len(s['derivatives']) != 5 or any(len(d) != 3 for d in s['derivatives']) for s in samples):
                        raise ValueError('missing native fourth-order derivatives')
                    derivative_stations += 3
            if not joined:
                for end in [0, 1]:
                    if order[end] == 4:
                        fourth = output['derivative_samples'][end*2]['derivatives'][4]
                        if max(map(abs, fourth)) > 1e-8:
                            raise ValueError('native zero-fourth default changed')
                        zero_fourth_ends += 1
        for click in trace.get('clicks', []):
            if not click['motion_acknowledged'] or not click['click_acknowledged'] or len(click['ray']) != 2:
                raise ValueError('incomplete high-order physical input')
            picks += 1
        stations += 33
    if (len(rows), len(promoted), derivative_stations) != (25, 6, 279):
        raise ValueError('higher-order qualification inventory changed')
    # A nonzero source fourth derivative distinguishes Native's G4 construction
    # policy from mathematical fourth-order continuity on arbitrary sources.
    for source in ['quartic', 'quintic', 'rational']:
        v = rows['blend_curve_high_discovery/high_'+source+'_g4']
        obj = next(o for o in v['before'] if o['attributes']['name'] == 'guide0')
        if max(map(abs, obj['derivative_samples'][-1]['derivatives'][4])) <= 1:
            raise ValueError('lost nonzero source fourth derivative evidence')
    return rows, summary, picks, stations, derivative_stations, zero_fourth_ends, paths
