"""Report qualified Flow maps alongside the retained unresolved spatial cases.

python3 -m tools.rhino_oracle.flow_report flow-local.json command-local.json planes-local.json --output report.json
"""
import argparse
import json
import math
from pathlib import Path

BASE = Path(__file__).parent
EPSILON = 1e-6


def point_error(actual, expected):
    if len(actual) != len(expected):
        raise ValueError('Flow point counts differ')
    errors = [math.dist(a, b) for a, b in zip(actual, expected)]
    if not errors or not all(math.isfinite(e) for e in errors):
        raise ValueError('Flow point comparison is empty or nonfinite')
    return max(errors)


def report(paths):
    rows = []
    for name, path in zip(('flow', 'flow_command', 'flow_planes'), paths):
        fixture = json.loads((BASE / 'fixtures' / (name + '.json')).read_text())
        native = json.loads((BASE / 'observations' / (name + '_rhino_20261016.json')).read_text())
        local = json.loads(path.read_text())
        if len(local['results']) != len(native['results']) or len(local['results']) != len(fixture['operations']):
            raise ValueError('Flow result counts differ')
        for op, a, n in zip(fixture['operations'], local['results'], native['results']):
            if not op['id'] == a['id'] == n['id']:
                raise ValueError('Flow result identities differ')
            a, n = a['value'], n['value']
            error = point_error(a['sdk_points'], n['sdk_points'])
            # These authored nonplanar cubics are retained failures, not removed
            # from the comparison or granted a wider tolerance.
            spatial = name == 'flow' and (op['id'].startswith('unspace-') or (op['id'].startswith('space-') and not op['id'].startswith('space-line-')))
            row = dict(fixture=name, id=op['id'], sdk_max_distance=error,
                       sdk_pass=error <= EPSILON, supported=not spatial, command=None)
            if op.get('command'):
                ac, nc = a['command'], n['command']
                if spatial:
                    row['command'] = dict(supported=False, rejected_before_edit=ac['returned'] is False and ac['undo_label'] is None,
                                          error=ac['error'])
                else:
                    meta = all(ac[k] == nc[k] for k in ('returned', 'guides_unchanged', 'guides_selected', 'group_count'))
                    aa = {o['original']: o for o in ac['outputs']}
                    nn = {o['original']: o for o in nc['outputs']}
                    meta = meta and aa.keys() == nn.keys()
                    errors = []
                    for key in aa.keys() & nn.keys():
                        meta = meta and all(aa[key][k] == nn[key][k] for k in ('selected', 'group_count'))
                        errors.append(point_error(aa[key]['points'], nn[key]['points']))
                    err = max(errors) if errors else math.inf
                    row['command'] = dict(supported=True, max_distance=err, metadata_pass=meta,
                                          passed=meta and err <= EPSILON and ac['undo_label'] == 'Flow')
            rows.append(row)
    supported = [r for r in rows if r['supported']]
    commands = [r['command'] for r in rows if r['command'] and r['command']['supported']]
    return dict(schema_version=1, epsilon=EPSILON, all_sdk_maps_pass=all(r['sdk_pass'] for r in rows),
                supported_maps_pass=all(r['sdk_pass'] for r in supported), supported_point_maps=len(supported),
                unresolved_spatial_maps=sum(not r['supported'] for r in rows),
                supported_command_count=len(commands), supported_commands_pass=all(c['passed'] for c in commands),
                native_table_enumeration_order_compared=False,
                native_frames_used_as_construction_inputs=False, cases=rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('local', nargs=3, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    value = report(args.local)
    args.output.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')
    if not value['supported_maps_pass'] or not value['supported_commands_pass']:
        raise SystemExit('A supported Flow comparison failed; see report')


if __name__ == '__main__':
    main()
