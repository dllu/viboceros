"""Qualify fixed ToSubD document/getter replays independently of kernel reports."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from .report_tosubd import ROOT, SOURCES as KERNEL_SOURCES, observations as surface_observations
from .report_tosubd_extrusions import SOURCES as EXTRUSION_SOURCES, observations as extrusion_observations
from .report_subdivide import subd_check

SOURCES = list(dict.fromkeys(KERNEL_SOURCES + EXTRUSION_SOURCES + [
    'crates/viboceros-command/src/lib.rs', 'crates/viboceros-command/src/to_subd.rs',
    'crates/viboceros-command/src/to_subd/options.rs',
    'crates/viboceros-command/src/object_selection.rs',
    'crates/viboceros-command/tests/tosubd.rs',
    'crates/viboceros-command/tests/tosubd_native.rs',
    'crates/viboceros-command/tests/support/tosubd.rs',
    'src/app.rs', 'src/app/to_subd.rs', 'src/app/to_subd/tests.rs',
    'src/app/component_selection.rs', 'src/app/object_selection.rs',
    'src/app/transform_sources.rs',
    'tools/rhino_oracle/report_tosubd_commands.py',
    'tools/rhino_oracle/test_tosubd_commands.py',
]))


def observations():
    surfaces, engine = surface_observations()
    extrusions, other = extrusion_observations()
    if engine != other or surfaces.keys() & extrusions.keys():
        raise ValueError('mixed engines or duplicate command recipes')
    result = dict(surfaces, **extrusions)
    result = {k: v for k, v in result.items()
              if 'tube_' not in k and k != 'tosubd_pipe_profile_frame_diagnostic'}
    if len(result) != 201:
        raise ValueError('ToSubD command coverage changed')
    return result, engine


def summarize(text):
    values, engine = observations()
    stages = {}
    control_error = facet_error = 0.
    for stage in ['command', 'getter']:
        wanted = {k: v for k, v in values.items() if stage == 'command' or not any(
            s['kind'] == 'mesh' and 'selected_faces' in s for s in v['input']['sources'])}
        rows = [json.loads(s) for s in re.findall('^TOSUBD_' + stage.upper() + r' (.+)$', text, re.M)]
        found = {r['id']: r for r in rows}
        if len(rows) != len(wanted) or set(found) != set(wanted):
            raise ValueError('missing, duplicate or unrelated ToSubD ' + stage)
        for key, value in wanted.items():
            row = found[key]
            targets = [o['subd'] for o in value['after_script'] if 'subd' in o]
            if (row.get('grouped_history') is not True
                    or type(row.get('mutated')) is not bool or row['mutated'] != bool(targets)
                    or row['check'].get('attributes_selection_and_identity') is not True
                    or len(row['check']['objects']) != len(targets)):
                raise ValueError('unqualified ToSubD document lifecycle')
            for actual, target in zip(row['check']['objects'], targets):
                c, f = subd_check(actual, target)
                control_error, facet_error = max(control_error, c), max(facet_error, f)
        stages[stage] = [found[k] for k in wanted]
    files = [str(p.relative_to(ROOT)) for p in sorted((ROOT / 'tools/rhino_oracle/fixtures/extrusions').glob('*.3dm'))]
    return dict(model_epsilon=1e-6, native_engine_version=engine,
                captured_workflows=206, excluded_invalid_profile_workflows=5,
                command_cases=201, getter_cases=200,
                maximum_control_or_limit_point_error=control_error,
                maximum_sampled_surface_error=facet_error,
                command_qualified=True, getter_qualified=True,
                full_native_parity=False, performance_qualified=False,
                retained_shared_evidence=dict(records=20, numeric_results_reused=True,
                    fresh_native_captures=False,
                    source_hashes='Shared dispatcher/prompt hashes refreshed after focused regressions; existing measurements retained.'),
                stages=stages, limits=[
                    'Fixed document replays cover EndCommand and post-cancel selection, identity, attributes, groups, immutable sources, grouped undo/redo and empty output. Cancellation is a no-op in command tests and is exercised through actual application prompts in getter tests.',
                    'Surface inputs use recorded underlying NURBS tables. Ordinary boxes use complete shared topology; other multi-face rejection inputs use disjoint face tables. Native trimmed-source topology is not reconstructed by these replays. Extrusions use native 3DM sources.',
                    'The viewport handler is exercised through component-click events, including selection after startup and stale-source rejection. Physical native pointer gestures and every selection modifier are not qualified.',
                    'Option values and defaults are observed natively. Preference retention on cancellation is tested locally; cross-command native preference sequences remain unqualified.',
                    'Box, Cylinder and planar ExtrudeCrv still need creator representation updates to emit typed extrusions, as the captured native creators do. Whole conversion of imported or API-created typed extrusions is supported.',
                    'Arbitrary topology, continuous surface error, full native parity and comparative speed remain unqualified.',
                ], sha256={p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest()
                           for p in dict.fromkeys(SOURCES + files)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('logs', nargs='+', type=Path)
    args = parser.parse_args()
    result = summarize('\n'.join(p.read_text() for p in args.logs))
    (ROOT / 'docs/tosubd-command-qualification.json').write_text(json.dumps(result, indent=2, allow_nan=False) + '\n')


if __name__ == '__main__':
    main()
