"""Qualify native extrusion file interchange; ToSubD integration remains pending."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FAMILIES = [('tosubd_extrusions', 40), ('tosubd_pipes', 9)]
INVALID = 'tosubd_extrusion_tube_00'
SOURCES = [
    'crates/viboceros-geometry/src/lib.rs', 'crates/viboceros-geometry/src/error.rs',
    'crates/viboceros-geometry/src/brep.rs', 'crates/viboceros-geometry/src/brep/extrusion.rs',
    'crates/viboceros-geometry/src/brep/parameter_orientation.rs',
    'crates/viboceros-geometry/src/brep/surface_refine.rs',
    'crates/viboceros-geometry/src/brep/shrink.rs',
    'crates/viboceros-geometry/src/brep/join_edges/automatic/rebuild.rs',
    'crates/viboceros-geometry/src/interop/mod.rs', 'crates/viboceros-geometry/src/interop/tests.rs',
    'crates/viboceros-geometry/tests/extrusion.rs',
    'crates/viboceros-io/src/three_dm.rs', 'crates/viboceros-io/src/three_dm_curves.rs',
    'crates/viboceros-io/src/three_dm_geometry.rs', 'crates/viboceros-io/tests/extrusion.rs',
    'crates/viboceros-opennurbs/native/extrusion_codec.h',
    'crates/viboceros-opennurbs/native/extrusion_queries.h',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
    'crates/viboceros-opennurbs/native/viboceros_opennurbs.h',
    'crates/viboceros-opennurbs/src/extrusion.rs', 'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-command/src/to_nurbs.rs', 'crates/viboceros-command/tests/extrusion.rs',
    'tools/rhino_oracle/tosubd_cases.py', 'tools/rhino_oracle/tosubd_extrusion_cases.py',
    'tools/rhino_oracle/tosubd_probe.py', 'tools/rhino_oracle/generate_tosubd.py',
    'tools/rhino_oracle/structural_sdk.py', 'tools/rhino_oracle/rhino_worker.py',
    'tools/rhino_oracle/report_extrusions.py', 'tools/rhino_oracle/test_extrusions.py',
] + [f'tools/rhino_oracle/{folder}/{family}.json' for family, _ in FAMILIES for folder in ['fixtures', 'observations']]


def observations():
    values, engines = {}, set()
    for family, count in FAMILIES:
        query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        capture = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        rows = capture['results']
        if len(rows) != count or [r['id'] for r in rows] != [o['id'] for o in query['operations']]:
            raise ValueError('missing or reordered native extrusion recipes')
        engines.add(capture['engine_version'])
        for row in rows:
            key, value = row['id'], row['value']
            events = [e for e in value['events'] if e['name'] == 'ToSubD']
            if key in values or value['success'] is not True or len(events) != 1 or events[0]['result'] != 'Success' or 'Unknown command:' in value['history']:
                raise ValueError('invalid extrusion capture lifecycle')
            values[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native extrusion engines')
    invalid = values['tosubd_pipe_profile_frame_diagnostic']['before'][0]
    if invalid['extrusion']['brep_valid'] is not False or invalid['crc'] != values[INVALID]['before'][0]['crc'] or invalid['brep'] != values[INVALID]['before'][0]['brep']:
        raise ValueError('missing invalid profile-frame evidence')
    return values, engines.pop()


def summarize(text):
    values, engine = observations()
    files = {key: value for key, value in values.items() if value['source_files']}
    if len(files) != 12 or INVALID not in files:
        raise ValueError('native source file corpus changed')
    expected = {k: v for k, v in files.items() if k != INVALID}
    rows = [json.loads(line) for line in re.findall(r'^EXTRUSION_NATIVE_FILE (.+)$', text, re.M)]
    found = {r['id']: r for r in rows}
    if len(rows) != len(expected) or set(found) != set(expected):
        raise ValueError('missing, duplicate or unrelated native file comparisons')
    maximum = 0.
    for key, value in expected.items():
        row = found[key]
        for name in ['native_type_retained', 'canonical_view_unchanged', 'identity_retained', 'native_surface_stations_and_orientation', 'rational_control_nets_retained']:
            if row.get(name) is not True:
                raise ValueError('unqualified native extrusion type, geometry or identity')
        if type(row.get('faces')) is not int or row['faces'] != len(value['before'][0]['brep']) or type(row.get('solid')) is not bool or row['solid'] is not (value['before'][0]['extrusion']['cap_count'] == 2):
            raise ValueError('extrusion face or cap topology differs')
        if any(type(row.get(k)) is not int or row[k] < 1 for k in ['vertices', 'edges']):
            raise ValueError('invalid canonical topology count')
        for metric, epsilon in [('maximum_rational_control_error',1e-6), ('maximum_rational_weight_error',1e-12)]:
            value_error = row.get(metric)
            if type(value_error) not in (int,float) or not math.isfinite(value_error) or not 0 <= value_error <= epsilon:
                raise ValueError('invalid or excessive rational control-net error')
        error = row.get('maximum_surface_sample_error')
        if type(error) not in (int, float) or not math.isfinite(error) or not 0 <= error <= 1e-6:
            raise ValueError('invalid or excessive native surface sample error')
        maximum = max(maximum, error)
    rejected = [json.loads(line) for line in re.findall(r'^EXTRUSION_NATIVE_REJECTED (.+)$', text, re.M)]
    if rejected != [dict(id=INVALID, native_invalid_canonical_view=True, unsupported_object_count=1)] or type(rejected[0]['unsupported_object_count']) is not int or rejected[0]['native_invalid_canonical_view'] is not True:
        raise ValueError('invalid native source disposition differs')
    paths = SOURCES + [f'tools/rhino_oracle/fixtures/extrusions/{key}.3dm' for key in files]
    return dict(
        native_engine_version=engine, native_workflow_captures=len(values), native_file_sources=12,
        qualified_file_sources=11, rejected_invalid_file_sources=1, model_epsilon=1e-6,
        maximum_surface_sample_error=maximum,
        maximum_rational_control_error=max(r['maximum_rational_control_error'] for r in rows),
        maximum_rational_weight_error=max(r['maximum_rational_weight_error'] for r in rows), native_type_identity_and_file_round_trip=True,
        tosubd_command_qualified=False, tosubd_getter_qualified=False, full_native_parity=False,
        performance_qualified=False, comparisons=[found[k] for k in expected], rejected=rejected,
        git_sha256={
            '2ed922f736d97c642b24792415c0ecabe4ca6e05:crates/viboceros-io/src/three_dm_geometry.rs': hashlib.sha256(subprocess.check_output(['git','cat-file','blob','2ed922f736d97c642b24792415c0ecabe4ca6e05:crates/viboceros-io/src/three_dm_geometry.rs'],cwd=ROOT)).hexdigest()},
        structural_codec_refactor='Implementation and four legacy/malformed-data regression tests moved token-for-token, apart from imports and public visibility. IO reexports the shared geometry codec.',
        limits=[
            'Native files cover open/capped rectangles, negative height, circles, an arc, a kinked profile, miters, shear and proper open/capped pipes. Rational controls, weights and knots, face orientation, loop counts and 45 common-parameter surface stations per face match Rhino within 1e-6; continuous geometry error is not certified.',
            'Affine-map, archive validation, geometry-user-text removal, explicit B-rep edits and ToNURBS history/attributes have separate Rust regressions; every transform and editing command is not qualified.',
            'The first tube recipe used an inner profile in the wrong coordinate frame. A separate public Rhino diagnostic confirms its B-rep form is invalid. It is retained as an unsupported-file diagnostic; correct pipes are captured separately.',
            'Extrusions use an immutable native archive and validated canonical B-rep view. Geometric edits discard the archive; supported affine maps rebuild both views together. Native archives are bounded to 64 MiB and canonical transfers to 256 MiB.',
            'ToSubD command, option persistence, component selection, UI and creator integration remain in progress; extrusion kernel conversion is qualified separately. These source-file comparisons do not qualify those workflows.',
        ], sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in paths},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/extrusion-interop-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
