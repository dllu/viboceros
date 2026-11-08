"""Audit distinct-open zero Fillet, actual history, and retained capture failures."""
import gzip
import json
from . import fillet_command as recipes
from .fillet_getter_evidence import ROOT, HERE, normalized, result, sha
from .fillet_zero_evidence import collect_captures, difference
from .fillet_zero_history import audit

BASELINE = '07942f2acdb36de525d9eee2a35d0642b38a40aa'
FAMILIES = {'fillet_open_zero_'+s: list(getattr(recipes, 'OPEN_ZERO_'+s.upper()))
            for s in ['pilot', 'holdouts', 'overlap_final', 'bounds', 'orientations']}
EXPECTED_COUNTS = dict(observations=49, successful=45, cancelled=1, failed=3,
                       changed=42, invalid_outputs=0, images=0,
                       physical_picks=98, point_tangent_stations=6204)
EQUAL_RECORDS = {'fillet_open_zero_pilot/open_zero_meeting_parts',
                 'fillet_open_zero_pilot/open_zero_dynamic_accept',
                 'fillet_open_zero_holdouts/open_zero_meeting_nurbs'}


def sources(digests, paths):
    for digest in digests.values():
        path = HERE / ('observations/source_snapshots/'+digest+'.gz')
        if sha(gzip.decompress(path.read_bytes())) != digest: raise ValueError('frozen producer')
        paths.add(path)


def failed_capture(proof, paths):
    receipt = json.loads((proof/'failed-overlaps.json').read_bytes())
    family = receipt['family']; fixture = HERE/('fixtures/'+family+'.json')
    if family != 'fillet_open_zero_overlaps' or receipt['exit_status'] != 1 or receipt['native_observations'] != 0 or receipt['request_sha256'] != sha(fixture.read_bytes()) or receipt['command'] != ['tools/rhino_oracle/run_headless.sh','exec','python3','-m','tools.rhino_oracle.generate_'+family]: raise ValueError('failed capture receipt')
    if json.loads(fixture.read_bytes()) != recipes.request(list(recipes.OPEN_ZERO_OVERLAPS)): raise ValueError('failed capture inventory')
    text = gzip.decompress((proof/receipt['log']).read_bytes()).decode()
    if 'PICK @catenary-click:open_zero_collinear_long-7 1255 413' not in text or 'Recorded '+family in text: raise ValueError('failed capture log')
    window = json.loads((proof/'owned-window-inspection.json').read_bytes())
    pick = json.loads((proof/'selection-receipt.json').read_bytes())
    if window['pid'] != pick['pid'] or window['private_display'] != pick['display'] or '/scheme=VibocerosOracleFilletOpenZeroOverlaps20261108' not in window['cmdline'] or window['job'] not in window['cmdline'][-1].replace('\\','/') or window['first_pick_pixel'] != [1254,413] or window['second_pick_pixel'] != [1255,413] or not pick['private_owned_window'] or pick['usable_source_pick'] is not False: raise ValueError('private menu intervention')
    image = (proof/'selection-menu.png').read_bytes()
    if image[:8] != b'\x89PNG\r\n\x1a\n' or sha(image) != pick['screenshot_sha256']: raise ValueError('private menu image')
    for name, old in recipes.OPEN_ZERO_OVERLAPS.items():
        new = recipes.OPEN_ZERO_OVERLAP_FINAL['stable_'+name]
        if old['curves'] != new['curves']: raise ValueError('failed sources were not repeated')
        if name == 'open_zero_collinear_long':
            if old['actions'][:-2] != new['actions'][:-2] or [a['pick'] for a in new['actions'][-2:]] != [[80.,0.,0.],[120.,0.,0.]]: raise ValueError('corrected physical pick separation')
        elif old != new: raise ValueError('failed request was not repeated')
    paths.add(fixture); sources(receipt['sources'], paths)


def legacy_history(rows, paths):
    modern = rows['fillet_open_zero_pilot/open_zero_meeting_parts']
    corrections = []
    for family, name in [('fillet_getter_holdouts','zero_joined'), ('fillet_getter_final','zero_parts')]:
        path = HERE/('observations/'+family+'.json.gz'); data = json.loads(gzip.decompress(path.read_bytes()))
        old = next(r['value'] for r in data['results'] if r['id'] == name)
        prompt = old['input_trace'][0]['prompts'][-1]
        if old['prescribed_case']['curves'] != modern['prescribed_case']['curves'] or old['history_changed'] is not False or not all(t in prompt for t in ['Radius=0 ', 'Join=No ', 'Trim=Yes ']) or audit(modern) != dict(recorded=True,geometry_changed=False): raise ValueError('legacy history correction')
        maximum = max(difference(normalized(old[p]), normalized(modern[p])) for p in ['before','after'])
        if maximum > 1e-6: raise ValueError('legacy geometry witness')
        corrections.append(dict(id=family+'/'+name, old_inferred_record=False,
                                verified_record=True, witness='fillet_open_zero_pilot/open_zero_meeting_parts',
                                maximum_scene_difference=maximum))
        paths.add(path); sources(data['provenance']['sources'], paths)
    return corrections


def collect(proof):
    receipts = json.loads((proof/'captures.json').read_bytes())
    rows, captures, counts, paths = collect_captures(proof, receipts, FAMILIES, set(), EXPECTED_COUNTS)
    histories = {key:audit(v) for key,v in rows.items()}
    if {key for key,h in histories.items() if h['recorded'] and not h['geometry_changed']} != EQUAL_RECORDS: raise ValueError('equal-geometry history inventory')
    counts.update(geometry_changed=sum(h['geometry_changed'] for h in histories.values()),
                  unchanged_geometry_records=len(EQUAL_RECORDS), retained_failed_captures=1)
    failed_capture(proof, paths)
    corrections = legacy_history(rows, paths)
    return rows, captures, counts, paths, corrections
