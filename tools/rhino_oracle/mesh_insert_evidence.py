"""Separate complete point input, Native tessellation failure and API calibration."""
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
INPUT_FAILURES={'flat-edge-2','flat-same-low','warped-edge-2','warped-edge-3','warped-ngon','trapezoid','wide-warp','rotated-warp'}
TESSELLATION_FAILURES={'warped-edge-1'}
FAMILIES={'insert_point_mesh_repeats':13,'insert_point_mesh_repeats_second':13,'mesh_insert_paths':26,
          'mesh_insert_validation':18,'mesh_insert_conversion':18,'mesh_insert_degenerate':6,'mesh_insert_disconnected':4}

def record(family):return json.loads((HERE/'observations'/(family+'.json')).read_text())
def complete(value):
    traces=value['input_trace']
    return bool(traces)and all(not t['errors']and t['completed']==t['expected']and all(c['click_acknowledged']and c['motion_acknowledged']for c in t['clicks'])for t in traces)

def classify_paths():
    rows=record('mesh_insert_paths')['results'];missing=set();failed=set();eligible=[]
    for row in rows:
        name=row['id'];v=row['value']
        if not complete(v):missing.add(name)
        elif 'Tessellation failure'in v['history']:failed.add(name)
        else:eligible.append(name)
    if missing!=INPUT_FAILURES or failed!=TESSELLATION_FAILURES:raise ValueError('owned mesh input classification changed')
    return dict(input_failures=sorted(missing),native_tessellation_failures=sorted(failed),diagnostic_inputs=eligible)
