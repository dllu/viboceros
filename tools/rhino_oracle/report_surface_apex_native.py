"""Rebuild licensed surface-to-apex geometry and history comparisons."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
from .qualification_sources import with_shared_sources
from .surface_apex_extrusion_probe import request as sdk_request
from .surface_apex_command_probe import request as command_request

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
SOURCES = with_shared_sources([
    'crates/viboceros-command/src/extrude_surface_apex.rs',
    'crates/viboceros-command/src/extrude_surface.rs',
    'crates/viboceros-command/src/extrude_surface_apex/tests.rs',
    'crates/viboceros-oracle/src/surface_apex_extrusion.rs',
    'crates/viboceros-oracle/src/lib.rs',
    'crates/viboceros-oracle/tests/surface_apex_native.rs',
    'crates/viboceros-oracle/tests/support/surface_apex.rs',
    'crates/viboceros-smlib/src/native/surface_extrusion.rs',
    'crates/viboceros-smlib/native/bridge.cpp',
    'crates/viboceros-smlib/native/bridge.h',
    'crates/viboceros-geometry/src/brep/components.rs',
    'crates/viboceros-geometry/src/brep/components/tests.rs',
    'crates/viboceros-geometry/src/brep/join_edges/automatic/components.rs',
    'crates/viboceros-geometry/src/brep.rs',
    'src/app.rs', 'src/app/extrude_surface.rs',
    'src/app/extrude_surface_apex.rs', 'src/app/tests/extrude_surface_apex_native.rs',
    'third_party/usd-brep/source/SMLib/src/SmBSplineSurfaceNLib.cpp',
    'tools/rhino_oracle/surface_apex_extrusion_probe.py',
    'tools/rhino_oracle/surface_apex_command_probe.py',
    'tools/rhino_oracle/generate_surface_apex.py',
    'tools/rhino_oracle/generate_surface_apex_command.py',
    'tools/rhino_oracle/report_surface_apex_native.py',
    'tools/rhino_oracle/test_surface_apex_native.py',
    'docs/commands/extrude-srf-to-point.md',
])


def compare(actual, expected):
    if isinstance(expected, bool) or expected is None:
        if actual is not expected:
            raise ValueError('surface apex state differs')
        return 0.
    if isinstance(expected, (int, float)):
        if type(actual) not in (int, float) or not math.isfinite(actual):
            raise ValueError('surface apex metric is not finite')
        error = abs(actual-expected)
        if error > 1e-6:
            raise ValueError('surface apex metric differs: {}'.format(error))
        # Counts are discrete even though volumes and bounds are tolerant.
        if type(expected) is int and (type(actual) is not int or actual != expected):
            raise ValueError('surface apex count differs')
        return error
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            raise ValueError('surface apex product count differs')
        return max([compare(a, b) for a, b in zip(actual, expected)]+[0.])
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or set(actual) != set(expected):
            raise ValueError('surface apex fields differ')
        return max([compare(actual[k], expected[k]) for k in expected]+[0.])
    raise ValueError('unexpected surface apex metric')


def native():
    paths = set()
    captures = []
    for fixture, observation, recipe in [
        ('surface_apex_extrusion', 'surface_apex_sdk_rhino_20261024', sdk_request),
        ('surface_apex_command', 'surface_apex_command_tolerance', command_request),
    ]:
        query = HERE/('fixtures/'+fixture+'.json')
        saved = HERE/('observations/'+observation+'.json')
        q, data = json.loads(query.read_text()), json.loads(saved.read_text())
        if q != recipe() or hashlib.sha256(query.read_bytes()).hexdigest() != data['provenance']['request_sha256']:
            raise ValueError('native surface apex request changed')
        paths.update(str(p.relative_to(ROOT)) for p in [query, saved])
        for source, sha in data['provenance']['sources'].items():
            archive = HERE/'observations/source_snapshots'/(sha+'.gz')
            if hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest() != sha:
                raise ValueError('native surface apex producer changed: '+source)
            paths.add(str(archive.relative_to(ROOT)))
        if [o['id'] for o in q['operations']] != [r['id'] for r in data['results']]:
            raise ValueError('native surface apex coverage differs')
        if fixture == 'surface_apex_command':
            for op, row in zip(q['operations'], data['results']):
                v = row['value']
                compare(v['document_tolerance'], q['tolerance'])
                if (not v['after']['breps'] or v['after']['source_present'] is op['delete_input']
                        or v['after']['source_unchanged'] is not True):
                    raise ValueError('native surface apex command failed')
                compare(v['undo'], dict(source_present=True, source_unchanged=True, breps=[]))
                compare(v['redo'], v['after'])
                if not any(e['name'] == 'ExtrudeSrfToPoint' and e['result'] == 'Success' for e in v['events']):
                    raise ValueError('native surface apex command did not complete')
        captures.append(data)
    return captures, paths


def measured(log, prefix, expected, command=False):
    rows = [json.loads(line.split(prefix+' ', 1)[1]) for line in log.splitlines() if prefix+' ' in line]
    ids = [r['id'] for r in rows]
    if len(ids) != len(set(ids)) or set(ids) != set(expected):
        raise ValueError('surface apex measurement coverage differs')
    for row in rows:
        for phase in ['after', 'undo', 'redo'] if command else [None]:
            measured = row[phase] if phase else row
            target = expected[row['id']][phase] if phase else expected[row['id']]
            error = compare(measured['value'], target)
            if measured['maximum_error'] != error:
                raise ValueError('surface apex reported error differs')
    return sorted(rows, key=lambda r: r['id'])


def summarize(oracle_log, app_log):
    captures, paths = native()
    sdk = measured(oracle_log, 'SURFACE_APEX_SDK', {r['id']: r['value'] for r in captures[0]['results']})
    expected = {r['id']: r['value'] for r in captures[1]['results']}
    command = measured(oracle_log, 'SURFACE_APEX_COMMAND', expected, True)
    app = measured(app_log, 'SURFACE_APEX_APP', expected, True)
    errors = [r['maximum_error'] for r in sdk]+[r[p]['maximum_error'] for rows in [command, app] for r in rows for p in ['after', 'undo', 'redo']]
    return dict(scope='Explicit apex geometry, product counts, source retention and Undo/Redo; selection, attributes and getter equivalence are not measured',
        native_versions=[c['engine_version'] for c in captures], sdk_captured=len(sdk),
        command_document_tolerance=command_request()['tolerance'],
        command_captured=len(command), app_replayed=len(app), mismatches=0,
        absolute_epsilon=1e-6, maximum_error=max(errors),
        full_rhino_parity=False, continuous_error_certificate=False, performance_qualified=False,
        sdk=sdk, command=command, app=app,
        sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths|set(SOURCES))})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('oracle_log', type=Path)
    parser.add_argument('app_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/surface-apex-native-qualification.json')
    args = parser.parse_args()
    result = summarize(args.oracle_log.read_text(), args.app_log.read_text())
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False)+'\n')
    print('Surface apex: {} SDK, {} commands, {} GUI; maximum error {}'.format(
        result['sdk_captured'], result['command_captured'], result['app_replayed'], result['maximum_error']))


if __name__ == '__main__':
    main()
