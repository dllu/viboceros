"""Separate actual stored mesh edits from a successful native command result."""
import json
from pathlib import Path


def classify(value, *, history_isolated=False):
    before, after = value['before'], value['after']
    geometry_changed = after is None or any(before[k] != after[k] for k in ['vertices', 'faces'])
    channels_changed = after is None or any(before[k] != after[k] for k in ['colors', 'uv', 'normals', 'ngons'])
    trace = value.get('input_trace')
    warning = value.get('native_invalid_mesh_warning')
    if 'Output mesh is invalid' in value.get('history', ''):
        warning = True
    input_verified = trace is None or (trace['completed'] == trace['expected'] and not trace['errors']
                                      and all(c['motion_acknowledged'] and c['click_acknowledged'] for c in trace.get('clicks', [])))
    usable = (geometry_changed and value['command_result'] == 'Success' and warning is False
              and after is not None and after.get('valid') is True
              and history_isolated and bool(value['undo']) and bool(value['redo']) and input_verified)
    return dict(geometry_changed=geometry_changed, channels_changed=channels_changed,
                native_invalid_mesh_warning=warning, usable_reference=usable,
                command_success=value['command_result'] == 'Success',
                source_equals_after_undo=bool(value['undo']), redo_repeats_result=bool(value['redo']),
                history_isolated=history_isolated,
                undo_verified=history_isolated and geometry_changed and bool(value['undo']) and bool(value['redo']),
                acknowledged_clicks=sum(bool(c['motion_acknowledged'] and c['click_acknowledged'])
                                        for c in trace.get('clicks', [])) if trace else 0)


def summarize(directory, families, *, history_isolated_families=()):
    root = Path(directory)
    report = {}
    for family in families:
        native = json.loads((root/'observations'/(family+'.json')).read_text())
        rows = [dict(id=row['id'], **classify(row['value'], history_isolated=family in history_isolated_families)) for row in native['results']]
        report[family] = dict(cases=len(rows), history_isolated=family in history_isolated_families,
                             geometry_changes=sum(r['geometry_changed'] for r in rows),
                             successful_noops=sum(r['command_success'] and not r['geometry_changed'] for r in rows),
                             native_warning_cases=sum(r['native_invalid_mesh_warning'] is True for r in rows),
                             native_diagnostics_missing=sum(r['native_invalid_mesh_warning'] is None for r in rows),
                             usable_references=sum(r['usable_reference'] for r in rows),
                             rows=rows)
    return report


if __name__ == '__main__':
    import sys
    print(json.dumps(summarize(Path(__file__).resolve().parent, sys.argv[1:]), indent=2))
