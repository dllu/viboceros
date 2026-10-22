"""Bind finite temporary-range and component comparisons to native captures."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
from .report_control_polygon import ROOT, HERE, SOURCES

FAMILIES = ['control_polygon_components', 'control_polygon_picks']


def summarize(command_log, ui_log):
    native = {r['id']:r['value'] for family in FAMILIES for r in
              json.loads((HERE/'observations'/(family+'.json')).read_text())['results']}
    ranges = {key for key, value in native.items() if value['subcurve_parameters'] is not None}
    components = set(native)-ranges-{'segment_polycurve'}
    comparisons = {}
    for kind, marker, log, wanted in [
        ('command','CONTROL_POLYGON_RANGE_NATIVE',command_log,ranges),
        ('ui','CONTROL_POLYGON_RANGE_UI',ui_log,{(key,inline) for key in ranges for inline in [False,True]}),
        ('components','CONTROL_POLYGON_COMPONENT_UI',ui_log,components),
    ]:
        rows = [json.loads(s) for s in re.findall(r'^'+marker+r' (.+)$',log,re.M)]
        seen = set()
        for row in rows:
            fields = {'id','maximum_error','metadata_exact','selection_exact','history_exact'}
            if kind == 'ui': fields.add('inline')
            if kind == 'components': fields.update(['success','component_selection_exact'])
            if not isinstance(row,dict) or set(row) != fields: raise ValueError('unexpected range comparison fields')
            key = (row['id'],row['inline']) if kind == 'ui' else row['id']
            if key not in wanted or key in seen: raise ValueError('unexpected or duplicate range comparison')
            if kind == 'ui' and type(row['inline']) is not bool: raise ValueError('invalid getter mode')
            if kind == 'components' and (type(row['success']) is not bool or row['success'] != native[row['id']]['success']):
                raise ValueError('native component outcome differs')
            if any(row[k] is not True for k in fields if k.endswith('_exact')): raise ValueError('inexact range state')
            error = row['maximum_error']
            if type(error) not in (int,float) or not math.isfinite(error) or not 0 <= error <= 1e-6:
                raise ValueError('range control vertices differ')
            seen.add(key)
        if seen != wanted: raise ValueError('missing range comparisons')
        comparisons[kind] = sorted(rows,key=lambda r:(r['id'],r.get('inline',False)))
    paths = list(SOURCES) + [
        'crates/viboceros-command/tests/extract_control_polygon_ranges.rs',
        'tools/rhino_oracle/report_control_polygon_ranges.py',
        'crates/viboceros-document/src/object_admission.rs',
        'crates/viboceros-geometry/src/curve_trim.rs',
    ]
    for family in FAMILIES:
        paths += ['tools/rhino_oracle/'+folder+'/'+family+'.json' for folder in ['fixtures','observations']]
        paths += [str(p.relative_to(ROOT)) for p in sorted((HERE/'fixtures'/family).rglob('*.3dm'))]
        captured = json.loads((HERE/'observations'/(family+'.json')).read_text())
        paths += ['tools/rhino_oracle/observations/source_snapshots/'+d+'.gz' for d in captured['provenance']['sources'].values()]
    return dict(native_engine_version='8.32.26160.13001', temporary_ranges=len(ranges),
                interactive_range_paths=2*len(ranges), component_workflows=len(components), epsilon=1e-6,
                maximum_error=max(r['maximum_error'] for rows in comparisons.values() for r in rows),
                full_native_parity=False, performance_qualified=False,
                limits=['Five Current-layer temporary ranges; Input-layer ranges, repeated/mixed range inputs and other curves remain unqualified.',
                        'Native ignores preselected face and polycurve-segment components. UI compares seven face-only preselection cases, one mixed curve/face case and two rejected box face clicks. Segment components lack a UI representation.',
                        'Native temporary-range cancellation timed out; retained separately as a diagnostic. Local cancellation and stale-source rejection are regression-tested.',
                        'Texture display and comparative performance remain unqualified.'],
                comparisons=comparisons,
                sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in dict.fromkeys(paths)})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path)
    parser.add_argument('ui_log',type=Path)
    args = parser.parse_args()
    (ROOT/'docs/control-polygon-ranges-qualification.json').write_text(json.dumps(
        summarize(args.command_log.read_text(),args.ui_log.read_text()),indent=2,allow_nan=False)+'\n')


if __name__ == '__main__': main()
