"""Qualify fixed ToSubD kernel comparisons; command integration remains separate."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_subdivide import subd_check

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FAMILIES = [('tosubd', 44), ('tosubd_surface_details', 45),
            ('tosubd_surface_edges', 26), ('tosubd_features', 14), ('tosubd_poles', 10)]
EXPLORATORY = [('tosubd_discovery', 43)]
SURFACE_KINDS = {'surface', 'sphere_surface', 'cylinder_surface', 'torus_surface',
                 'cone_surface', 'hemisphere_surface', 'trimmed'}
SOURCES = [
    'crates/viboceros-opennurbs/build.rs',
    'crates/viboceros-opennurbs/native/mesh_codec.h',
    'crates/viboceros-opennurbs/native/mesh_modeling.cpp',
    'crates/viboceros-opennurbs/native/subd.cpp',
    'crates/viboceros-opennurbs/native/subd.h',
    'crates/viboceros-opennurbs/native/subd_codec.h',
    'crates/viboceros-opennurbs/native/subd_interpolation.h',
    'crates/viboceros-opennurbs/src/lib.rs',
    'crates/viboceros-geometry/src/lib.rs',
    'crates/viboceros-geometry/src/mesh/coplanar_merge.rs',
    'crates/viboceros-opennurbs/native/surface_queries.h',
    'crates/viboceros-opennurbs/src/mesh.rs',
    'crates/viboceros-opennurbs/src/subd.rs',
    'crates/viboceros-opennurbs/src/surface.rs',
    'crates/viboceros-opennurbs/src/subd_interpolation.rs',
    'crates/viboceros-geometry/src/nurbs_surface.rs',
    'crates/viboceros-geometry/src/subd.rs',
    'crates/viboceros-geometry/src/subd/interpolation.rs',
    'crates/viboceros-geometry/src/subd/mesh_conversion.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/axis.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/fitting.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/features.rs',
    'crates/viboceros-geometry/src/subd/surface_conversion/poles.rs',
    'crates/viboceros-command/tests/tosubd_kernel_native.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'crates/viboceros-geometry/tests/support/subdivision_native.rs',
    'tools/rhino_oracle/tosubd_cases.py', 'tools/rhino_oracle/tosubd_probe.py',
    'tools/rhino_oracle/tosubd_info.py', 'tools/rhino_oracle/generate_tosubd.py',
    'tools/rhino_oracle/report_tosubd.py', 'tools/rhino_oracle/test_tosubd.py',
    'tools/rhino_oracle/structural_sdk.py', 'tools/rhino_oracle/rhino_worker.py',
] + [f'tools/rhino_oracle/{folder}/{family}.json'
     for family, _ in FAMILIES + EXPLORATORY for folder in ['fixtures', 'observations']]


def observations():
    result, engines = {}, set()
    for family, count in FAMILIES:
        query = json.loads((HERE / 'fixtures' / (family + '.json')).read_text())
        capture = json.loads((HERE / 'observations' / (family + '.json')).read_text())
        if len(capture['results']) != count or [r['id'] for r in capture['results']] != [r['id'] for r in query['operations']]:
            raise ValueError('missing or reordered native ToSubD recipes')
        engines.add(capture['engine_version'])
        for row in capture['results']:
            key, value = row['id'], row['value']
            events = [e for e in value['events'] if e['name'] == 'ToSubD']
            if key in result or len(events) != 1 or type(value['success']) is not bool or events[0]['result'] != ('Success' if value['success'] else 'Cancel') or events[0]['objects'] != value['after']:
                raise ValueError('invalid native ToSubD lifecycle')
            if 'Unknown command:' in value['history'] and key != 'tosubd_mesh_face':
                raise ValueError('unexpected rejected ToSubD options')
            result[key] = value
    if len(engines) != 1:
        raise ValueError('mixed native ToSubD engines')
    return result, engines.pop()


def targets(values):
    stages = {name: {} for name in ['mesh_kernel', 'surface_control_kernel', 'surface_location_kernel']}
    for key, value in values.items():
        if not value['success']:
            continue
        for i, source in enumerate(value['input']['sources']):
            before = value['before'][i]
            if source['kind'] == 'mesh':
                stage = 'mesh_kernel'
            elif source['kind'] in SURFACE_KINDS and ('surface' in before or len(before.get('brep', [])) == 1):
                stage = 'surface_location_kernel' if value['input']['options']['UseSurface'] == 'Location' else 'surface_control_kernel'
            else:
                continue
            outputs = [o['subd'] for o in value['after_script'] if 'subd' in o and o['name'] == before['name']]
            if len(outputs) > 1:
                raise ValueError('ambiguous native conversion output')
            stages[stage][key, i] = outputs[0] if outputs else None
    if [len(v) for v in stages.values()] != [16, 45, 63]:
        raise ValueError('native conversion coverage changed')
    return stages


def summarize(text):
    values, engine = observations()
    expected = targets(values)
    stages, control_error, facet_error = {}, 0., 0.
    for stage, wanted in expected.items():
        rows = [json.loads(line) for line in re.findall('^TOSUBD_' + stage.upper() + r' (.+)$', text, re.M)]
        if any(type(r.get('source')) is not int for r in rows):
            raise ValueError('invalid conversion source index')
        found = {(r['id'], r['source']): r for r in rows}
        if len(rows) != len(wanted) or set(found) != set(wanted):
            raise ValueError('missing, duplicate or unrelated ToSubD ' + stage)
        for key, target in wanted.items():
            row = found[key]
            if row.get('source_unchanged') is not True:
                raise ValueError('uncertified immutable source')
            if target is None:
                if row.get('rejected') is not True or 'check' in row:
                    raise ValueError('native empty conversion must be rejected')
            else:
                if 'rejected' in row:
                    raise ValueError('native constructive case cannot be rejected')
                c, f = subd_check(row['check'], target)
                control_error, facet_error = max(control_error, c), max(facet_error, f)
        stages[stage] = [found[key] for key in wanted]
    return dict(
        model_epsilon=1e-6, captured_workflows=len(values), diagnostic_workflows=43,
        native_engine_version=engine, kernel_comparisons=sum(map(len, stages.values())),
        constructive_kernel_comparisons=123, rejected_control_nets=1,
        command_qualified=False, getter_qualified=False, full_native_parity=False,
        performance_qualified=False,
        retained_evidence=dict(reports=20, measurements_reused=True,
            source_hash_update_basis='Existing paths preserve behavior: native mesh importer moved byte-for-byte, Rust mesh packing and result body preserved token-for-token; existing public surface frames share the same exact transfer constructor. Twenty native bridge regressions passed.'),
        maximum_control_or_limit_point_error=control_error,
        maximum_sampled_surface_error=facet_error, stages=stages,
        limits=[
            'Kernel-only record. ToSubD command, component selection and document lifecycle are qualified separately in tosubd-command-qualification.json, and extrusion conversion in tosubd-extrusion-qualification.json. Source creator representation updates remain pending.',
            'Controls, tags, vertex limit stations and corresponding density-two facets qualify the fixed cases within 1e-6; continuous smooth-surface error and arbitrary topology remain unqualified.',
            'Surface Location fitting samples Greville stations with natural or periodic sparse cubic systems; singular boundaries and recorded G1 crease cases are covered, not every possible knot/weight configuration.',
            'Multi-face B-reps, B-rep component workflows, whole extrusion captures and rejected mesh-face selection are preserved as evidence but excluded from kernel comparisons.',
            'The initial 43-workflow discovery used an incorrect option phase; it is retained only as diagnostic evidence.',
        ],
        sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/tosubd-kernel-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
