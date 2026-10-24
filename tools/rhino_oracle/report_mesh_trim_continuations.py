"""Audit cutter lifetime, descendant identity, repeated picks, and history."""
import argparse
import hashlib
import json
from pathlib import Path
from . import report_mesh_trim as trim
from .qualification_sources import with_shared_sources

ROOT = trim.ROOT
GAPS = ['mesh_trim_snapshot_policy_deleted_cutter', 'mesh_trim_snapshot_policy_replaced_cutter']
SOURCES = with_shared_sources(trim.SOURCES + [
    'tools/rhino_oracle/report_mesh_trim_continuations.py',
    'tools/rhino_oracle/test_mesh_trim_continuations.py',
    'docs/mesh-trim-continuations.md',
])


def native():
    version, rows, paths = trim.native(['continuation', 'snapshot_policy', 'cutter_policy'])
    return version, rows, paths | set(SOURCES)


def measurements(log, suffix, expected):
    rows = []
    for family in ['continuation', 'snapshot_policy', 'cutter_policy']:
        rows += trim.baseline.measurements(log, 'MESH_TRIM_'+family.upper()+'_'+suffix,
                                          {k: v for k, v in expected.items() if k.startswith('mesh_trim_'+family+'_')})
    return sorted(rows, key=lambda r: r['id'])


def summarize(command_log, getter_log):
    version, expected, paths = native()
    commands = measurements(command_log, 'COMMAND', {k: v for k, v in expected.items() if v['success']})
    getters = measurements(getter_log, 'GETTER', expected)

    def gaps(rows):
        return sorted(r['id'] for r in rows if r['success'] != expected[r['id']]['success'] or any(
            not r[s][k] for s in ['after', 'undo', 'redo'] for k in ['geometry', 'metadata', 'ngons']) or
            any(not r[s]['channels']['matched'] for s in ['after', 'undo', 'redo']))

    if any(gaps(rows) != GAPS for rows in [commands, getters]):
        raise ValueError('mesh trim continuation compatibility gaps changed')
    return dict(scope='MeshTrim original cutter geometry, descendant self-exclusion, repeated targets, cancel and grouped history',
                native_version=version, epsilon=trim.baseline.channels.GEOMETRY_EPSILON,
                command_replayed=len(commands), getter_replayed=len(getters),
                command_matched=len(commands)-len(GAPS), getter_matched=len(getters)-len(GAPS),
                command_compatibility_gaps=GAPS, getter_compatibility_gaps=GAPS,
                comparison='Ordered object identity and metadata, surface area and bidirectional samples, coherent UV/normal/color samples, no output n-gons, Undo and Redo with original source storage',
                pending=['Two perspective-view cases with curve cutters retain different surfaces; the cause is unqualified',
                         'Curved cutter meshing, extreme scales, near contacts, and comparative performance'],
                commands=commands, getters=getters,
                sha256={p: hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log', type=Path)
    parser.add_argument('getter_log', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT/'docs/mesh-trim-continuation-qualification.json')
    args = parser.parse_args()
    record = summarize(args.command_log.read_text(), args.getter_log.read_text())
    args.output.write_text(json.dumps(record, indent=2, allow_nan=False)+'\n')
    print('MeshTrim continuations: {command_matched}/{command_replayed} commands, {getter_matched}/{getter_replayed} getters'.format(**record))


if __name__ == '__main__':
    main()
