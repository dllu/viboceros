"""Native Cap input witnesses and output diagnostics, without filtering cases."""
import json
from pathlib import Path
HERE=Path(__file__).resolve().parent
FAMILIES={'mesh_cap_regions':14,'mesh_cap_regions_groups':6,'mesh_cap_regions_noop':2}


def record(family):
    return json.loads((HERE/'observations'/(family+'.json')).read_text())


def audit():
    non_solid=[];copies=0;cases=0
    for family,count in FAMILIES.items():
        rows=record(family)['results']
        if len(rows)!=count:raise ValueError('incomplete Native Cap family')
        for row in rows:
            cases+=1;v=row['value']
            for index,source in enumerate(v['input']['sources']):
                before=next(o['mesh']for o in v['before']if o['source']==index)
                if not before['double_precision']or not before['valid']or before['vertices']!=source['vertices']or before['faces']!=source['faces']:
                    raise ValueError('Native Cap input storage differs: '+row['id'])
            for o in v['after']:
                if'mesh'not in o:continue
                changed=o['source']is None or o['mesh']!=next(s['mesh']for s in v['before']if s['source']==o['source'])
                if changed and not o['mesh']['solid']:non_solid.append(row['id'])
                if o['source']is None:
                    copies+=1
                    if o['groups']:raise ValueError('Native Cap copy acquired source groups')
    return dict(cases=cases,input_coordinates_exact=True,native_non_solid_outputs=non_solid,copied_meshes=copies)
