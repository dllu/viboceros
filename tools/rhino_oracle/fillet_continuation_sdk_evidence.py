"""Public SDK controls and the retained failed observer attempt, separate from parity."""
import gzip
import json
import math
from .fillet_getter_evidence import HERE, finite, sha
from .fillet_continuation_sdk import request


def collect(proof, command_rows):
    family = 'fillet_continuation_sdk'
    receipt = json.loads((proof/'sdk-capture.json').read_bytes())
    fixture = HERE/('fixtures/'+family+'.json')
    archive = HERE/('observations/'+family+'.json.gz')
    raw = gzip.decompress(archive.read_bytes()); data = json.loads(raw)
    query = request()
    if json.loads(fixture.read_bytes()) != query or data['engine'] != 'rhino' or data['iterations'] != 1 or [r['id'] for r in data['results']] != [o['id'] for o in query['operations']]: raise ValueError('SDK input inventory')
    command = ['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]
    if receipt['family'] != family or receipt['exit_status'] != 0 or receipt['command'] != command: raise ValueError('SDK headless receipt')
    if receipt['request_sha256'] != sha(fixture.read_bytes()) or data['provenance']['request_sha256'] != sha(fixture.read_bytes()) or receipt['observation_sha256'] != sha(raw): raise ValueError('SDK capture hashes')
    if gzip.decompress((proof/receipt['log']).read_bytes()).decode().splitlines() != ['Capturing 8 cases: '+family,'Recorded '+family]: raise ValueError('SDK capture log')
    paths = {fixture, archive}
    def frozen(sources):
        for digest in sources.values():
            p = HERE/('observations/source_snapshots/'+digest+'.gz')
            if sha(gzip.decompress(p.read_bytes())) != digest: raise ValueError('SDK frozen producer')
            paths.add(p)
    frozen(data['provenance']['sources'])
    aligned = opposed = 0; maximum = 0.
    for row, op in zip(data['results'], query['operations']):
        v = row['value']; e = v['extended']
        if not finite(v) or v['source_unchanged'] is not True or v['original'] != {'success': False} or not e['success'] or not e['arc_valid']: raise ValueError('SDK control availability')
        scale = op['curves'][1]['control_points'][-1]['point'][0]
        t = e['parameters'][1]*scale
        maximum = max(maximum, math.dist(e['contacts'][1], [t,6.+3.*t+t*t,0.]), abs(e['source_curvature']-2./(1.+(3.+2.*t)**2)**1.5))
        for contact in e['contacts']:
            maximum = max(maximum, abs(math.dist(contact, e['center'])-op['radius']))
        maximum = max(maximum, abs(math.dist(e['contacts'][0],[0.,0.,0.])-op['curves'][0]['radius']))
        dot = sum(a*b for a,b in zip(e['arc_end_tangent'],e['source_tangent']))
        if abs(abs(dot)-1.) > 1e-6 or not 0. < e['arc_angle'] <= math.pi or e['distance'] <= 0.: raise ValueError('SDK tangent witness')
        native = next(v for key,v in command_rows.items() if key.endswith('/'+row['id']))
        if (dot > 0.) is not native['history_changed']: raise ValueError('SDK admission witness')
        aligned += dot > 0.; opposed += dot < 0.
    if (aligned,opposed) != (3,5) or not 0. <= maximum <= 1e-6: raise ValueError('SDK geometry gate')
    failed = json.loads((proof/'failed-sdk.json').read_bytes())
    if failed['family'] != family or failed['command'] != command or failed['exit_status'] != 1 or failed['native_observations'] != 0 or failed['request_sha256'] != sha(fixture.read_bytes()): raise ValueError('failed SDK receipt')
    log = gzip.decompress((proof/failed['log']).read_bytes()).decode()
    if 'AttributeError' not in log or 'AngleRadians' not in log or 'Recorded '+family in log: raise ValueError('failed SDK log')
    frozen(failed['sources'])
    old = failed['sources']['fillet_continuation_sdk.py']; new = data['provenance']['sources']['fillet_continuation_sdk.py']
    source = lambda digest: gzip.decompress((HERE/('observations/source_snapshots/'+digest+'.gz')).read_bytes())
    if source(old).replace(b'arc.AngleRadians',b'arc.Angle') != source(new): raise ValueError('SDK observer correction')
    return dict(observations=8,original_contacts=0,extended_contacts=8,unchanged_inputs=8,aligned_contacts=aligned,opposed_contacts=opposed,maximum_analytic_difference=maximum,retained_failed_attempts=1,engine_version=data['engine_version']), paths
