"""Qualify endpoint weight geometry and native archive readback separately."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

from .qualification_sources import with_shared_sources
from .report_subd_box import HERE, ROOT, error

FAMILIES = [('soft_crease_discovery', 32), ('soft_crease', 72),
            ('soft_crease_numeric_labels', 29)]
DIAGNOSTICS = [('soft_crease_labels_discovery', 3), ('soft_crease_extra', 21),
               ('soft_crease_precision', 21)]
SOURCES = with_shared_sources([
    'crates/viboceros-opennurbs/build.rs', 'crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h', 'crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'crates/viboceros-opennurbs/src/lib.rs', 'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-opennurbs/native/subd_crease.h',
    'crates/viboceros-opennurbs/native/subd_edges.h',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/tests/subd_soft_crease.rs',
    'crates/viboceros-command/tests/soft_crease_native.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'tools/rhino_oracle/report_subd_soft_crease.py',
    'tools/rhino_oracle/test_soft_crease.py',
    'tools/rhino_oracle/soft_crease_probe.py', 'tools/rhino_oracle/soft_crease_cases.py',
    'tools/rhino_oracle/subd_edit_scene.py', 'tools/rhino_oracle/generate_soft_crease.py',
    'tools/rhino_oracle/conic_getter_input.py', 'tools/rhino_oracle/client.py',
    'tools/rhino_oracle/structural_sdk.py', 'tools/rhino_oracle/rhino_worker.py',
])


def observations():
    expected = {}
    for family, count in FAMILIES:
        query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = native['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in query['operations']]:
            raise ValueError('missing or reordered soft crease observations')
        if native['engine_version'] != '8.32.26160.13001':
            raise ValueError('unexpected native engine')
        for row in rows:
            value = row['value']
            events = [e for e in value['events'] if e['name'] == 'SubDCrease']
            if len(events) != 1 or events[0]['objects'] != value['after']:
                raise ValueError('missing command completion event')
            if value['success'] is not (events[0]['result'] == 'Success'):
                raise ValueError('native status differs')
            if family == 'soft_crease_numeric_labels':
                if row['id'].startswith('boundary_'):
                    if float(value['macro'].split()[1]) != value['input']['actions'][0]:
                        raise ValueError('numeric command token lost precision')
                else:
                    trace = value['input_trace'][0]
                    if trace['errors'] or trace['completed'] != trace['expected']:
                        raise ValueError('incomplete native mouse workflow')
                    if not all(c['motion_acknowledged'] and c['click_acknowledged'] for c in trace['clicks']):
                        raise ValueError('unacknowledged native mouse input')
            for obj in value['before']:
                if obj['source'] is not None:
                    key = (family, row['id'], obj['source'])
                    if key in expected:
                        raise ValueError('duplicate source')
                    expected[key] = (value, obj)
    return expected


def source_hashes():
    paths = list(SOURCES)
    for family, _ in FAMILIES + DIAGNOSTICS:
        paths.extend('tools/rhino_oracle/' + d + '/' + family + '.json'
                     for d in ['fixtures', 'observations'])
        paths.extend(str(p.relative_to(ROOT)) for p in sorted((HERE / 'fixtures' / family).rglob('*.3dm')))
        native = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        paths.extend('tools/rhino_oracle/observations/source_snapshots/' + digest + '.gz'
                     for digest in native['provenance']['sources'].values())
    return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)}


def geometry_check(check, native):
    if check.get('qualified') is not True or check.get('surface_sampled') is not True:
        raise ValueError('unqualified soft crease geometry')
    for key, field in [('vertices', 'points'), ('edges', 'edges'), ('faces', 'faces')]:
        if type(check.get(key)) is not int or check[key] != len(native[field]):
            raise ValueError('native topology differs')
    return max(error(check.get('maximum_control_or_limit_point_error')),
               error(check.get('maximum_sampled_surface_error')))


def summarize(log):
    expected = observations()
    rows = [json.loads(s) for s in re.findall(r'^SOFT_KERNEL (.+)$', log, re.M)]
    seen, maximum = set(), 0.
    for row in rows:
        key = (row['family'], row['id'], row['source'])
        if key not in expected or key in seen or type(row['source']) is not int:
            raise ValueError('missing, duplicate or unknown source comparison')
        seen.add(key)
        value, before = expected[key]
        after = next(o for o in value['after'] if o['source'] == row['source'])
        if row.get('sharpness') is not True or row.get('source_unchanged') is not True:
            raise ValueError('unqualified sharpness or immutable source')
        if type(row.get('native_archive_normalizes_sharpness')) is not bool:
            raise ValueError('missing archive characterization')
        if type(row.get('changed')) is not bool or row['changed'] is not (before['crc'] != after['crc']):
            raise ValueError('native change status differs')
        maximum = max(maximum, geometry_check(row['check'], after['subd']))
    if seen != set(expected):
        raise ValueError('incomplete kernel comparison')
    io = [json.loads(s) for s in re.findall(r'^SOFT_IO (.+)$', log, re.M)]
    io_expected = {k[1]: v[0] for k, v in expected.items()
                   if k[0] == 'soft_crease_numeric_labels' and k[1].startswith('boundary_')}
    io_seen, maximum_io, native_change = set(), 0., 0.
    for row in io:
        key = row['id']
        if key not in io_expected or key in io_seen:
            raise ValueError('duplicate or unknown archive comparison')
        io_seen.add(key)
        value = io_expected[key]
        reread = value['roundtrip'][0]['subd']
        maximum_io = max(maximum_io, geometry_check(row['check'], reread))
        before = next(o['subd'] for o in value['after'] if o['source'] == 0)
        a, b = before['surface_mesh']['vertices'], reread['surface_mesh']['vertices']
        if len(a) != len(b):
            raise ValueError('native roundtrip changes sample count')
        native_change = max(native_change, max(math.dist(p, q) for p, q in zip(a, b)))
    if io_seen != set(io_expected):
        raise ValueError('incomplete archive comparison')
    return dict(model_epsilon=1e-6, native_engine_version='8.32.26160.13001',
                compared_workflows=sum(n for _, n in FAMILIES), component_sources=len(rows),
                captured_workflows=sum(n for _, n in FAMILIES + DIAGNOSTICS),
                diagnostic_families=[f for f, _ in DIAGNOSTICS],
                maximum_geometry_error=maximum, maximum_archive_import_error=maximum_io,
                maximum_native_roundtrip_surface_change=native_change,
                command_registered=False, full_native_parity=False, performance_qualified=False,
                scope='Immutable geometry API: per-edge endpoint percentages, hard/soft conversion, stable internal archives and external 3DM readback. Command admission and the application getter are subsequent work.',
                limits=[
                    'Rhino 8.32 changes some near-integer float sharpness values during 3DM readback. Internal Viboceros snapshots preserve their accepted values; external import follows the observed native readback.',
                    'The initial mouse families did not complete their input plans and remain diagnostics. The earlier numeric formatters rounded neighboring doubles; only the numeric-label family qualifies those input values.',
                    'label_after_constant missed a label after its preview moved. Its subsequent numeric weight was global; it does not qualify a vertex-weight edit.',
                    'Object attributes, document history, general chain construction and frontend label placement are not qualified by these low-level component comparisons.',
                    'Control stations and density-two surface facets are sampled evidence, not a continuous Hausdorff certificate.',
                ], stages=dict(kernel=sorted(rows, key=lambda r:(r['family'], r['id'], r['source'])),
                               archive=sorted(io, key=lambda r:r['id'])), sha256=source_hashes())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', type=Path, nargs='+')
    args = parser.parse_args()
    report = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-soft-crease-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
