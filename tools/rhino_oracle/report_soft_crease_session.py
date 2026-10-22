"""Qualify staged SubDCrease profiles and application getter replays."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re

from .qualification_sources import with_shared_sources
from . import report_subd_chains as chains

ROOT = chains.ROOT
SOURCES = with_shared_sources([
    'crates/viboceros-command/src/lib.rs', 'src/app.rs', 'src/viewport.rs',
    'src/viewport/soft_crease.rs', 'src/app/soft_crease.rs',
    'src/app/soft_crease/tests.rs', 'src/app/soft_crease/tests/native.rs',
    'src/app/component_selection.rs', 'src/app/command_line.rs',
    'src/app/object_selection.rs', 'src/app/block_edit.rs',
    'crates/viboceros-command/tests/soft_crease_session_native.rs',
    'crates/viboceros-command/tests/soft_crease_staging.rs',
    'crates/viboceros-command/tests/support/soft_crease.rs',
    'crates/viboceros-command/tests/support/subd.rs',
    'crates/viboceros-command/tests/support/subd_facets.rs',
    'tools/rhino_oracle/report_soft_crease_session.py',
    'tools/rhino_oracle/test_soft_crease_session.py',
])


def expected():
    result = set()
    chains.observations()  # Complete native requests, input traces and source counts.
    for family, _ in chains.FAMILIES:
        native = json.loads((chains.HERE / 'observations' / (family + '.json')).read_text())
        for row in native['results']:
            for source in row['value']['before']:
                if source.get('subd') is not None:
                    result.add((family, row['id'], source['name']))
    return result


def summarize(text):
    wanted = expected()
    seen = {'SOFT_SESSION': set(), 'SOFT_UI': set()}
    stages = {}
    maximum = 0.
    for prefix in seen:
        rows = [json.loads(s) for s in re.findall(r'^' + prefix + r' (.+)$', text, re.M)]
        for row in rows:
            key = (row['family'], row['id'], row['object'])
            if prefix == 'SOFT_UI':
                if type(row.get('preselected')) is not bool:
                    raise ValueError('missing getter selection mode')
                key += (row['preselected'],)
                if row.get('staged') is not True or row.get('selection_released') is not True:
                    raise ValueError('unqualified staged application edit')
            elif row.get('source_unchanged') is not True:
                raise ValueError('mutable staged source')
            if key[:3] not in wanted or key in seen[prefix]:
                raise ValueError('duplicate or unknown SubDCrease comparison')
            seen[prefix].add(key)
            check = row['check']
            if check.get('qualified') is not True or check.get('surface_sampled') is not True:
                raise ValueError('unqualified SubDCrease surface')
            for field in ['maximum_control_or_limit_point_error', 'maximum_sampled_surface_error']:
                value = check.get(field)
                if type(value) not in (float, int) or not math.isfinite(value) or not 0 <= value <= 1e-6:
                    raise ValueError('SubDCrease exceeds the model epsilon')
                maximum = max(maximum, value)
        stages[prefix] = sorted(rows, key=lambda r: (r['family'], r['id'], r['object'], r.get('preselected', False)))
    if seen['SOFT_SESSION'] != wanted or seen['SOFT_UI'] != {k + (mode,) for k in wanted for mode in [True, False]}:
        raise ValueError('incomplete SubDCrease profile or application comparisons')
    hashes = chains.source_hashes()
    hashes.update({p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in SOURCES})
    return dict(native_engine_version='8.32.26160.13001', native_workflows=48,
                profile_objects=len(wanted), application_workflows=96,
                model_epsilon=1e-6, maximum_measured_error=maximum,
                command_registered=True, sampled_profiles_qualified=True,
                full_native_parity=False, performance_qualified=False,
                limits=[
                    'Public native source files, accepted live sharpness recipes and recorded getter actions are replayed; no proprietary implementation is inspected.',
                    'Application replays dispatch component picks, label actions and command text. A separate egui regression checks physical projected label clicks; full native and application screen equivalence is unqualified.',
                    'The finite corpus covers source bends, discontinuities, hard runs, odd loops, per-chain options, repeated edits, invalid weights and cancellation. Arbitrary profiles and continuous surface error remain unqualified.',
                    'Lifecycle regressions check immutable previews, atomic invalid edits, stale sources, selection release before Undo, metadata retention, source ordering and Redo preservation.',
                ], stages=stages, sha256=hashes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', type=Path, nargs='+')
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/subd-crease-session-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
