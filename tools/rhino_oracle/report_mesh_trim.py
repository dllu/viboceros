"""Audit physical MeshTrim picks, channels, metadata, and grouped history."""
import argparse
import hashlib
import json
from pathlib import Path
from . import mesh_split_evidence as evidence
from . import report_mesh_split_channels as baseline
from .mesh_trim_cases import FAMILIES, request
from .qualification_sources import with_shared_sources

ROOT = baseline.ROOT
GAPS = ['mesh_trim_controls_plane']
SOURCES = with_shared_sources(baseline.SOURCES + [
    'crates/viboceros-command/tests/mesh_trim_native.rs',
    'crates/viboceros-command/tests/support/mesh_trim.rs',
    'tools/rhino_oracle/report_mesh_trim.py',
    'tools/rhino_oracle/test_mesh_trim.py',
    'docs/commands/mesh-trim.md',
] + ['tools/rhino_oracle/' + p for p in [
    'generate_mesh_trim.py', 'mesh_trim_cases.py', 'mesh_trim_control_cases.py',
    'mesh_trim_continuation_cases.py', 'mesh_trim_snapshot_policy_cases.py', 'mesh_trim_cutter_policy_cases.py',
    'mesh_trim_probe.py', 'mesh_trim_input.py', 'mesh_trim_picking.py',
    'component_picking.py', 'hole_picking.py',
]])


def native(families=('discovery', 'mouse', 'controls')):
    return evidence.native(families, SOURCES, case_families=FAMILIES,
                           request_factory=request, prefix='mesh_trim_',
                           allow_unknown=['mesh_trim_tolerance_menu'])


def summarize(command_log, getter_log):
    version, captured, paths = native()
    expected = {k: v for k, v in captured.items()
                if 'trim_picks' in v['input'] and not v['input']['preselect']}
    commands = baseline.measurements(command_log, 'MESH_TRIM_COMMAND',
                                    {k: v for k, v in expected.items() if v['success']})
    getters = baseline.measurements(getter_log, 'MESH_TRIM_GETTER', expected)

    def gaps(rows):
        return sorted(r['id'] for r in rows if r['success'] != expected[r['id']]['success'] or any(
            not r[s][k] for s in ['after', 'undo', 'redo'] for k in ['geometry', 'metadata', 'ngons']) or
            any(not r[s]['channels']['matched'] for s in ['after', 'undo', 'redo']))

    if any(gaps(rows) != GAPS for rows in [commands, getters]):
        raise ValueError('mesh trim compatibility gaps changed')
    return dict(scope='MeshTrim physical region picks, stored vertex channels, metadata, selection and grouped history',
                native_version=version, epsilon=baseline.channels.GEOMETRY_EPSILON,
                channel_epsilon={name: value[1] for name, value in baseline.channels.CHANNELS.items()},
                captured=len(captured),
                excluded_capture_ids=sorted(set(captured)-set(expected)),
                excluded_reason='Discovery macros test menus, cancellation and ignored typed targets; preselected macros did not deliver productive region picks and do not qualify preselected trimming',
                command_replayed=len(commands), getter_replayed=len(getters),
                command_matched=len(commands)-len(GAPS), getter_matched=len(getters)-len(GAPS),
                command_compatibility_gaps=GAPS, getter_compatibility_gaps=GAPS,
                comparison='Ordered object identity, attributes, groups, selection, surface area and samples, no output n-gons, bidirectional UV/normal/color vertex samples, Undo and Redo with original source storage',
                pending=['Temporary infinite plane capture leaves the native target unchanged while Viboceros trims it',
                         'Productive native preselection; the application has separate preselection and local Undo regressions',
                         'Repeated picks on cutters that were themselves trimmed, curved cutters, extreme scales, near contacts and comparative performance'],
                commands=commands, getters=getters,
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-trim-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('MeshTrim: {command_matched}/{command_replayed} commands, {getter_matched}/{getter_replayed} getters'.format(**record))


if __name__ == '__main__':
    main()
