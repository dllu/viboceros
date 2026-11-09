"""Own broader refinement observations without promoting rejected fitters."""
import gzip
import json
import math
import re

from . import match_surface_average_cases as sdk, match_surface_command as command
from .merge_surfaces_evidence import ROOT, HERE, sha, frozen
from .surface_average_refinement_evidence import complete, producers as sdk_producers, PROTOTYPE_SOURCES
from .fillet_getter_evidence import finite

PROOF = ROOT / 'docs/surface-average-adaptive-proof'
BASELINE = 'ea854d45ab29fffffab0366e10ee337d1c0b01b1'
FAMILIES = {
    'match_surface_average_refinement_signs': ('sdk', sdk.REFINEMENT_SIGNS, 'refinement-signs'),
    'match_surface_average_position_refinement': ('sdk', sdk.POSITION_REFINEMENT, 'position-refinement'),
    'match_surface_average_position_refine_command': ('command', command.AVERAGE_POSITION_REFINEMENT, 'position-refine-command'),
}
POSITION_SOURCES = {
    'crates/viboceros-geometry/src/surface_match/average.rs',
    'crates/viboceros-geometry/src/surface_match/refine.rs',
    'crates/viboceros-geometry/src/surface_match/average/refinement.rs',
    'crates/viboceros-command/tests/average_surfaces.rs',
    'crates/viboceros-command/tests/support/surface_average.rs',
    'crates/viboceros-command/tests/average_surface_position_refinement.rs',
}


def launch(family, stem):
    path = '/tmp/match-average-' + stem
    return ['tools/rhino_oracle/run_headless.sh', 'exec', 'bash', '-c',
            'python3 -m tools.rhino_oracle.generate_' + family + ' > ' + path + '.log 2>&1; '
            'capture_result=$?; echo "$capture_result" > ' + path + '.status; exit "$capture_result"']


def command_complete(value, recipe):
    trace = value['input_trace'][0]
    events = [e for e in value['events'] if e['name'] == 'MatchSrf']
    accepted = not any(action == 'Cancel' for action in recipe['actions'])
    if (not finite(value) or trace['completed'] != len(recipe['actions'])
            or trace['completed'] != trace['expected'] or trace['errors']
            or len(trace['prompts']) != trace['expected'] or len(trace['clicks']) != 2
            or any(not c['motion_acknowledged'] or not c['click_acknowledged'] for c in trace['clicks'])
            or not value['returned_idle'] or len(events) != 1
            or events[0]['result'] != ('Success' if accepted else 'Cancel')
            or value['success'] is not accepted or value['history_changed'] is not accepted
            or any(len(value[stage]) != 3 or any(not o['native_valid'] for o in value[stage])
                   for stage in ['before', 'after', 'undo', 'redo'])):
        raise ValueError('incomplete refined averaging command')
    # Native Undo clears the preselection; retain all other object data.
    unselected = lambda objects: [{k: v for k, v in o.items() if k != 'selected'} for o in objects]
    if unselected(value['undo']) != unselected(value['before']) or value['redo'] != value['after']:
        raise ValueError('refined averaging command histories')
    undo = value['undo_observation']
    expected_counts = [[4, 7], [4, 7]] if accepted else [[4, 4], [4, 4]]
    if ([o['surfaces'][0]['control_count'] for o in value['after'] if 'surfaces' in o] != expected_counts
            or undo['undo_succeeded'] is not accepted or undo['geometry_changed'] is not accepted
            or undo['redo_command_succeeded'] is not (True if accepted else None)):
        raise ValueError('refined averaging command outcome')
    if not all(flag in trace['prompts'][-1] for flag in ['Mode=Position', 'AverageSurfaces=Yes', 'RefineMatch=Yes']):
        raise ValueError('refined averaging command option readback')


def collect(proof=PROOF):
    receipts = json.loads((proof / 'captures.json').read_bytes())
    if [r['family'] for r in receipts] != list(FAMILIES):
        raise ValueError('adaptive refinement capture inventory')
    paths = {proof / 'captures.json', HERE / 'run_headless.sh'}
    rows, commands = {}, {}
    for receipt, (family, (kind, recipes, stem)) in zip(receipts, FAMILIES.items()):
        archive = HERE / receipt['archive']
        raw = gzip.decompress(archive.read_bytes())
        data = json.loads(raw)
        fixture = HERE / ('fixtures/' + family + '.json')
        log = proof / receipt['log']
        factory = sdk if kind == 'sdk' else command
        if (receipt['kind'] != kind or receipt['archive'] != 'observations/' + family + '.json.gz'
                or receipt['log'] != 'capture-' + stem + '.log.gz'
                or receipt['command'] != launch(family, stem)
                or type(receipt['exit_status']) is not int or receipt['exit_status'] != 0
                or sha(raw) != receipt['observation_sha256']
                or sha(fixture.read_bytes()) != receipt['request_sha256']
                or data['provenance']['request_sha256'] != receipt['request_sha256']
                or json.loads(fixture.read_bytes()) != factory.request(list(recipes))
                or data['engine'] != 'rhino' or data['iterations'] != 1
                or [r['id'] for r in data['results']] != list(recipes)
                or gzip.decompress(log.read_bytes()).decode().splitlines() != [
                    'Capturing ' + str(len(recipes)) + ' cases: ' + family, 'Recorded ' + family]):
            raise ValueError('adaptive refinement capture receipt')
        required = sdk_producers(family) if kind == 'sdk' else {n + '.py' for n in [
            'generate_' + family, 'match_surface_command', 'match_surface_cases',
            'match_surface_command_probe', 'match_surface_point_picker', 'arc_point_picker',
            'catenary_point_picker', 'chamfer_point_picker', 'subcurve_direction_input',
            'group_picking', 'line_scene_probe', 'join_probe', 'merge_edges_probe',
            'conic_getter_input', 'managed_mouse_callback', 'snap_environment', 'number_token',
            'structural_sdk', 'rhino_worker', 'client', 'capture_family']}
        if set(data['provenance']['sources']) != required:
            raise ValueError('adaptive refinement producer inventory')
        frozen(data['provenance'], paths)
        paths.update([archive, fixture, log])
        destination = rows if kind == 'sdk' else commands
        for row in data['results']:
            key, value = row['id'], row['value']
            if key in destination or value['prescribed_case'] != recipes[key]:
                raise ValueError('adaptive refinement authored recipe')
            if kind == 'sdk':
                complete(value)
            else:
                command_complete(value, recipes[key])
            destination[key] = value
    if (len(rows), len(commands)) != (38, 5):
        raise ValueError('adaptive refinement scope')
    return rows, commands, paths


def prototypes(proof=PROOF):
    paths, models = set(), {}
    position_cases = {'average_h_position_refine'} | {
        key for key, recipe in sdk.REFINEMENT_PILOT.items() if recipe['continuity'] == 'Position'
    } | set(sdk.POSITION_REFINEMENT)
    for label, expected, matched_count, sources, test, prefix, filtered in [
        ('sign', set(sdk.REFINEMENT_PILOT), 27, PROTOTYPE_SOURCES,
         'average_surface_refinement', 'AVERAGE_REFINEMENT', []),
        ('position', position_cases, 30, POSITION_SOURCES,
         'average_surface_position_refinement', 'POSITION_REFINEMENT', ['position_refinement_matches']),
    ]:
        path = proof / 'prototypes' / (label + '.json')
        record = json.loads(path.read_bytes())
        log = proof / 'prototypes' / record['log']
        text = gzip.decompress(log.read_bytes()).decode()
        command_line = ['cargo', 'test', '-p', 'viboceros-command', '--test', test, *filtered, '--', '--nocapture']
        if (record['baseline_revision'] != BASELINE or record['backend'] != 'portable'
                or record['qualified'] is not False or record['log'] != label + '.log.gz'
                or record['command'] != command_line or set(record['sources']) != sources
                or type(record['exit_status']) is not int or record['exit_status'] != 101):
            raise ValueError('adaptive prototype must remain a rejected replay')
        families = ['match_surface_average_refinement_pilot'] if label == 'sign' else [
            'match_surface_average_holdouts', 'match_surface_average_refinement_pilot',
            'match_surface_average_position_refinement']
        inputs = {str((HERE / ('observations/' + family + '.json.gz')).relative_to(ROOT)) for family in families}
        if set(record['inputs']) != inputs:
            raise ValueError('adaptive prototype input inventory')
        for name, digest in record['inputs'].items():
            artifact = ROOT / name
            if sha(artifact.read_bytes()) != digest:
                raise ValueError('adaptive prototype input changed')
            paths.add(artifact)
        for name, digest in record['sources'].items():
            if not name.startswith('crates/') or not name.endswith('.rs') or re.fullmatch('[0-9a-f]{64}', digest) is None:
                raise ValueError('adaptive prototype source manifest')
            source = proof / 'prototypes/sources' / (digest + '.gz')
            if sha(gzip.decompress(source.read_bytes())) != digest:
                raise ValueError('adaptive prototype source changed')
            paths.add(source)
        admitted = [s.removeprefix(prefix + '_CASE ') for s in text.splitlines() if s.startswith(prefix + '_CASE ')]
        passed = [json.loads(s.removeprefix(prefix + ' ')) for s in text.splitlines() if s.startswith(prefix + ' ')]
        failure_prefix = 'Unmatched native cases: ' if label == 'sign' else 'Pending position cases: '
        failed = [json.loads(s.split(failure_prefix, 1)[1]) for s in text.splitlines() if failure_prefix in s]
        matched = {r['id'] for r in passed}
        if (set(admitted) != expected or len(admitted) != len(expected)
                or len(passed) != matched_count or len(matched) != matched_count or len(failed) != 1
                or matched & set(failed[0]) or matched | set(failed[0]) != expected
                or len(failed[0]) != len(expected) - matched_count
                or text.count('test result: FAILED.') != 1 or 'test result: ok.' in text):
            raise ValueError('adaptive prototype cannot hide failed cases')
        for row in passed:
            error = row['maximum_geometry_error']
            if (type(error) not in (float, int) or not math.isfinite(error) or not 0 <= error <= 1e-9
                    or type(row['outputs']) is not int or row['outputs'] != 2
                    or type(row['archives']) is not int or row['archives'] != 2):
                raise ValueError('adaptive prototype comparison gate')
        models[label] = dict(backend='portable', cases=len(expected), matched_cases=matched_count,
                             rejected_cases=sorted(failed[0]), qualified=False,
                             maximum_matched_geometry_error=max(r['maximum_geometry_error'] for r in passed))
        if label == 'position':
            grid = [json.loads(s.removeprefix('POSITION_REFINEMENT_GRID ')) for s in text.splitlines()
                    if s.startswith('POSITION_REFINEMENT_GRID ')]
            if (len(grid) != 2 * len(expected) or {(r['id'], r['side']) for r in grid} != {
                    (key, side) for key in expected for side in ['output', 'target']}
                    or any(not finite(r) or r['error'] < 0 for r in grid)):
                raise ValueError('missing paired position diagnostics')
            models[label]['rejected_grid_samples'] = [r for r in grid if r['id'] not in matched]
        paths.update([path, log])
    # This first trial stopped on the first failing case and is never admitted as a full replay.
    early = proof / 'position-early-stop.log.gz'
    if 'test result: FAILED.' not in gzip.decompress(early.read_bytes()).decode():
        raise ValueError('early position failure was erased')
    paths.add(early)
    return models, paths
