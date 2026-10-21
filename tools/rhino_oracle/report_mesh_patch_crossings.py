"""Report every captured crossing, including unresolved command and SDK differences."""
import argparse
import hashlib
import json
import math
import re
from fractions import Fraction
from pathlib import Path
from .report_mesh_patch import SOURCES as BASE_SOURCES

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
GROUPS = [
    ('crossings', 'CROSSING', ''),
    ('repeat_and_variants', 'VERIFY', '_verify'),
    ('untrimmed', 'UNTRIMMED', '_untrimmed'),
    ('seeded', 'SEEDED', '_seeded'),
    ('junctions', 'JUNCTION', '_junctions'),
    ('sdk_order_audit', 'AUDIT', '_audit'),
    ('point_guides', 'POINTS', '_points'),
]
SOURCES = BASE_SOURCES + [
    'tools/rhino_oracle/generate_mesh_patch_crossings.py',
    'tools/rhino_oracle/report_mesh_patch_crossings.py',
    'tools/rhino_oracle/test_mesh_patch_crossings.py',
] + [f'tools/rhino_oracle/{directory}/mesh_patch_crossings{suffix}.json'
     for _, _, suffix in GROUPS for directory in ['fixtures', 'observations']]


def finite_error(value):
    if isinstance(value, bool) or not isinstance(value, (float, int)) or not math.isfinite(value) or value < 0:
        raise ValueError('invalid mesh patch distance')
    return value


def degenerate_faces(mesh):
    """Exact zero-area checks on the captured binary64 spatial coordinates."""
    vertices = [tuple(Fraction(x) for x in p) for p in mesh['vertices']]
    invalid = []
    for index, face in enumerate(mesh['faces']):
        a, b, c = [vertices[i] for i in face]
        u = [b[i] - a[i] for i in range(3)]
        v = [c[i] - a[i] for i in range(3)]
        if all(u[(i+1)%3]*v[(i+2)%3] == u[(i+2)%3]*v[(i+1)%3] for i in range(3)):
            invalid.append(index)
    return invalid


def summarize(log):
    groups = {}
    unique = {}
    initial = {}
    invalid_native = []
    capture_count = 0
    for name, prefix, suffix in GROUPS:
        request = json.loads((HERE / f'fixtures/mesh_patch_crossings{suffix}.json').read_text())
        native = json.loads((HERE / f'observations/mesh_patch_crossings{suffix}.json').read_text())
        ids = [op['id'] for op in request['operations']]
        if [row['id'] for row in native['results']] != ids:
            raise ValueError('native capture is incomplete')
        reference = {}
        for row in native['results']:
            value = row['value']
            mesh = next(o['mesh'] for o in value['after_script'] if o['role'] == 'output')
            reference[row['id']] = (degenerate_faces(mesh), degenerate_faces(value['sdk']))
            if reference[row['id']][0] and row['id'] not in invalid_native:
                invalid_native.append(row['id'])
        stages = {}
        for stage in ['geometry', 'command', 'getter']:
            marker = f'MESH_PATCH_{prefix}_{stage.upper()}'
            rows = [json.loads(s) for s in re.findall(marker + r' (\{[^\n]+\})', log)]
            if [row['id'] for row in rows] != ids:
                raise ValueError('incomplete or duplicate ' + marker + ' replay')
            for row in rows:
                invalid, sdk_invalid = reference[row['id']]
                native_error = row.get('native_mesh_error')
                if bool(native_error) != bool(invalid) or native_error is not None and not isinstance(native_error, str):
                    raise ValueError('native degenerate facets must remain explicit')
                error = row['maximum_bidirectional_sample_error']
                if invalid:
                    if error is not None:
                        raise ValueError('invalid native facets cannot receive a measured distance')
                else:
                    finite_error(error)
                if stage == 'geometry':
                    sdk_error = row.get('sdk_mesh_error')
                    if bool(sdk_error) != bool(sdk_invalid):
                        raise ValueError('invalid SDK facets must remain explicit')
                    if invalid or sdk_invalid:
                        if row['sdk_command_difference'] is not None:
                            raise ValueError('invalid native facets cannot receive an SDK comparison')
                    else:
                        finite_error(row['sdk_command_difference'])
                    counts = [row[k] for k in ['actual_faces', 'native_faces', 'actual_vertices', 'native_vertices']]
                    if any(type(n) is not int or n <= 0 for n in counts):
                        raise ValueError('invalid topology counts')
                    counts_match = counts[0] == counts[1] and counts[2] == counts[3]
                else:
                    counts_match = row['counts_match']
                    if type(counts_match) is not bool or row['source_metadata_preserved'] is not True or row['grouped_history'] is not True:
                        raise ValueError('source attributes and history must be checked')
                if row['qualified'] is not (not invalid and error < 1e-6 and counts_match):
                    raise ValueError('mesh patch qualification hides a geometry or topology gap')
            stages[stage] = dict(recipes=len(rows), qualified_recipes=sum(row['qualified'] for row in rows),
                                 unqualified_recipes=[row['id'] for row in rows if not row['qualified']], cases=rows)
        for index, id_ in enumerate(ids):
            flags = [stages[stage]['cases'][index]['qualified'] for stage in stages]
            if flags != [flags[0]] * 3:
                raise ValueError('geometry, command and getter disagree')
            if id_ in unique and unique[id_] != flags[0]:
                raise ValueError('repeat qualification changed')
            unique[id_] = flags[0]
            if suffix in ['', '_verify', '_untrimmed', '_seeded']:
                initial[id_] = flags[0]
        capture_count += len(ids)
        groups[name] = dict(native_engine_version=native['engine_version'], **stages)
    errors = [row['maximum_bidirectional_sample_error'] for group in groups.values()
              for stage in ['geometry', 'command', 'getter'] for row in group[stage]['cases'] if row['qualified']]
    return dict(
        model_epsilon=1e-6, full_native_parity=False, performance_qualified=False,
        native_target='public MeshPatch command', captures=capture_count,
        unique_scenes=len(unique), qualified_unique_scenes=sum(unique.values()),
        unqualified_scenes=[id_ for id_, qualified in unique.items() if not qualified],
        original_scenes=len(initial), original_qualified_scenes=sum(initial.values()),
        original_unqualified_scenes=[id_ for id_, qualified in initial.items() if not qualified],
        native_invalid_meshes=invalid_native,
        maximum_qualified_sample_difference=max(errors), groups=groups,
        guarantees=[
            'Every capture is replayed in geometry, scripted command and interactive getter tests; unresolved results remain explicit.',
            'Source storage, names, layers, selection and grouped Undo/Redo match the captured command even for geometry gaps.',
            'Earlier recovered guide constraints take priority without generated intersection vertices. Point seeds and recovered parts of crossing holes participate in trimming.',
            'With automatic trimming, guides passing through explicit point seeds recover after unsplit guides, using exact spatial collinearity to preserve intended seed stations after chart rounding.',
            'Forward overlaps beginning inside an earlier guide trim the recovered adjacent face. Source directions and earlier guide ownership remain explicit.',
            'Constraint walks, recovered edges and weighted exact spatial checks have a cumulative 4000000 work limit; input and perimeter records remain capped at 100000.',
            'The SDK ownership and execution-order audit retains input polylines and confirms unchanged command meshes in 11 before/after call pairs.',
        ],
        limits=[
            'Native SDK and command meshes can differ for identical inputs. Command qualification does not imply SDK parity; every SDK/command sample difference is recorded.',
            'Shared endpoints, overlapping or self-crossing guides, some direction changes, multiple crossings and points on guides retain measured compatibility gaps.',
            'Two tilted native command meshes contain exactly collinear facets. Their invalid artifacts and unavailable comparison distances remain explicit; local patches omit degenerate facets.',
            'The 1e-6 gate uses bidirectional barycentric triangle samples and vertex/face counts; it does not certify universal topology or native face ordering.',
            'General curved guides, folded or periodic charts, starting B-rep trims and native performance need further qualification.',
        ],
        sha256={path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in SOURCES},
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', type=Path, nargs='+')
    args = parser.parse_args()
    report = summarize('\n'.join(path.read_text() for path in args.logs))
    (ROOT / 'docs/mesh-patch-crossings-qualification.json').write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
