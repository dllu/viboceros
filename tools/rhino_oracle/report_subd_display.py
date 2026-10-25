"""Compare nested display toggles and extraction with immutable Rhino scenes."""
import argparse
import hashlib
import json
from pathlib import Path
from .extract_points_cases import FAMILIES as ALL_FAMILIES
from .qualification_sources import with_shared_sources
from .report_extract_points import ROOT,native,canonical,compare
from .report_extract_point_ranges import measured

FAMILIES={'display':ALL_FAMILIES['display']}
SOURCES=with_shared_sources([
    'crates/viboceros-command/src/lib.rs','crates/viboceros-document/src/lib.rs','src/app.rs',
    'crates/viboceros-command/src/interface.rs','crates/viboceros-command/src/interface/tests.rs',
    'crates/viboceros-command/tests/subd_display_native.rs',
    'crates/viboceros-geometry/src/subd/control_net.rs','crates/viboceros-opennurbs/native/subd_control_mesh.h',
    'src/viewport/display_cache.rs','src/viewport/display_cache/subd_tests.rs',
    'src/viewport/polygon_edges.rs','src/viewport/polygon_components.rs','src/viewport/scene.rs',
    'src/viewport/grip_preview.rs','src/viewport/morph_preview.rs','src/app/command_line.rs',
    'tools/rhino_oracle/report_subd_display.py','tools/rhino_oracle/test_subd_display.py',
    'docs/commands/subd-display.md',
])

def native_display():
    version,rows,paths=native(FAMILIES,qualified=FAMILIES)
    for row in rows.values():
        flat=row['input']['flat'];toggle=row['display_toggle']
        if (type(toggle['initial']) is not bool or toggle['initial']==flat or
                type(toggle['result']) is not bool or toggle['result']!=flat or
                toggle['before']!=toggle['after'] or toggle['after']!=row['before'] or
                row['display_modes']!=[flat]*3):
            raise ValueError('Native display toggle changed selection, geometry or history appearance')
    return version,rows,paths

def summarize(command_log,getter_log):
    version,expected,paths=native_display()
    commands=measured(command_log,'SUBD_DISPLAY_COMMAND',set(expected),lambda r:r['id'])
    getters=measured(getter_log,'SUBD_DISPLAY_GETTER',set(expected),lambda r:r['id'])
    for row in commands+getters:
        wanted=expected[row['id']]
        if (row['success'] is not True or row['toggle_preserved'] is not True or row['source_storage_unchanged'] is not True or
                row['modes']!=wanted['display_modes']):raise ValueError('Display status or history mode differs')
        row['maximum_deviation']=max(compare(row[s],canonical(wanted[s])) for s in ['after','undo','redo'])
    return dict(scope='Global smooth/flat SubD toggles, unchanged whole/grip selections, nested extraction and history appearance',
        native_version=version,epsilon=1e-7,captured=len(expected),command_replayed=len(commands),getter_replayed=len(getters),
        maximum_deviation=max(r['maximum_deviation'] for r in commands+getters),commands=commands,getters=getters,
        sha256={p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in sorted(paths|set(SOURCES))})

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command_log',type=Path);parser.add_argument('getter_log',type=Path)
    parser.add_argument('--output',type=Path,default=ROOT/'docs/subd-display-qualification.json')
    args=parser.parse_args();r=summarize(args.command_log.read_text(),args.getter_log.read_text())
    args.output.write_text(json.dumps(r,indent=2,allow_nan=False)+'\n')
    print('SubD display: {} commands, {} getters; maximum deviation {}'.format(r['command_replayed'],r['getter_replayed'],r['maximum_deviation']))
if __name__=='__main__':main()
