"""Audit all retained ArcBlend captures, including unsuccessful discovery inputs."""
import gzip
import hashlib
import json
import math
from . import arc_blend_sdk as sdk, arc_blend_command as command
from .check_arc_blend import ROOT

HERE = ROOT / 'tools/rhino_oracle'
FAMILIES = {
    'arc_blend_sdk': (sdk, sdk.PILOT),
    'arc_blend_unit': (sdk, list(sdk.HOLDOUTS)),
    'arc_blend_command': (command, command.PILOT),
    'arc_blend_options': (command, list(command.HOLDOUTS)),
}
MATCHED_COMMANDS = {'arc_default', 'arc_s_shape', 'arc_spatial', 'arc_alternate', 'arc_trim', 'arc_join'}
COMMAND_DIAGNOSTICS = {
    'arc_radius_difference': 'incomplete owned ArcBlend input',
    'arc_unjoined': 'Unknown command: _Join=No',
    'arc_radius_zero': 'Unable to achieve requested radius difference.',
    'arc_radius_positive': 'Unable to achieve requested radius difference.',
    'arc_radius_negative': 'Unable to achieve requested radius difference.',
    'arc_ratio_direct': 'Unknown command: 0.25',
    'arc_ratio_zero': 'Unknown command: 0.0',
}
def read(family):
    return json.loads(gzip.decompress((HERE / ('observations/' + family + '.json.gz')).read_bytes()))

def collect(proof):
    sha = lambda b: hashlib.sha256(b).hexdigest()
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES):
        raise ValueError('capture inventory changed')
    evidence = {p for p in proof.iterdir() if p.is_file()}
    matched, rejected, diagnostics, workflows = set(), set(), {}, set()
    captures = []
    for family, (module, names) in FAMILIES.items():
        fixture = HERE / ('fixtures/' + family + '.json')
        path = HERE / ('observations/' + family + '.json.gz')
        data = read(family)
        if data['engine'] != 'rhino' or data['iterations'] != 1 or [r['id'] for r in data['results']] != names or json.loads(fixture.read_bytes()) != module.request(names) or data['provenance']['request_sha256'] != sha(fixture.read_bytes()):
            raise ValueError('native coverage or fixed request changed')
        evidence.update([fixture, path])
        for digest in data['provenance']['sources'].values():
            source = HERE / ('observations/source_snapshots/' + digest + '.gz')
            if sha(gzip.decompress(source.read_bytes())) != digest:
                raise ValueError('frozen producer changed')
            evidence.add(source)
        receipt = next(r for r in receipts if r['family'] == family)
        log = gzip.decompress((proof / receipt['log']).read_bytes()).decode().splitlines()
        if receipt['exit_status'] != 0 or receipt['command'] != ['tools/rhino_oracle/run_headless.sh', 'exec', 'python3', '-m', 'tools.rhino_oracle.generate_' + family] or log != ['Capturing ' + str(len(names)) + ' cases: ' + family, 'Recorded ' + family]:
            raise ValueError('capture did not complete')
        captures.append(dict(family=family, observations=len(names), engine_version=data['engine_version'], raw_sha256=sha(gzip.decompress(path.read_bytes()))))
        for row in data['results']:
            name, value = row['id'], row['value']
            if module is sdk:
                if value['case'] != sdk.CASES[name]:
                    raise ValueError('SDK input changed')
                if not value['success']:
                    rejected.add(name)
                    continue
                if not value['native_valid'] or len(value['segments']) != 2 or len(value['samples']) != 33:
                    raise ValueError('invalid native arc blend')
                for sample in value['samples']:
                    if any(not math.isfinite(x) for key in ['point', 'tangent'] for x in sample[key]):
                        raise ValueError('nonfinite native sample')
                if name.startswith('spatial_'):
                    delta = math.dist(value['segments'][0]['samples'][-1]['tangent'], value['segments'][1]['samples'][0]['tangent'])
                    if delta <= 1e-3:
                        raise ValueError('raw-vector native diagnostic changed')
                    diagnostics[name] = dict(reason='Native non-unit direction inputs have a discontinuous join tangent', native_join_tangent_difference=delta)
                else:
                    matched.add(name)
            else:
                if value['prescribed_case'] != command.CASES[name] or not value['returned_idle']:
                    raise ValueError('native command input changed or did not return idle')
                trace = value['input_trace'][0]
                if name in COMMAND_DIAGNOSTICS:
                    reason = COMMAND_DIAGNOSTICS[name]
                    if reason not in value['history'] and reason not in trace['errors']:
                        raise ValueError('command diagnostic changed')
                    diagnostics[name] = dict(reason=reason)
                    continue
                if not value['success'] or trace['errors'] or trace['completed'] != len(command.CASES[name]['actions']) or trace['expected'] != trace['completed'] or len(trace['clicks']) != 2:
                    raise ValueError('incomplete matched native workflow')
                for click in trace['clicks']:
                    if not click['motion_acknowledged'] or not click['click_acknowledged']:
                        raise ValueError('unacknowledged native pick')
                if value['after'] != value['redo'] or any(o['selected'] or not o['native_valid'] for o in value['after']):
                    raise ValueError('native validity selection or history changed')
                workflows.add(name)
    if (len(matched), len(rejected), len(diagnostics), workflows) != (20, 17, 10, MATCHED_COMMANDS):
        raise ValueError('ArcBlend classification coverage changed')
    return matched, rejected, diagnostics, workflows, captures, evidence
