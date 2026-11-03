"""Freeze smooth edge partitions through public InsetFaces and actual commands."""
import argparse
import json
from pathlib import Path
from .capture_family import capture
from .client import OracleClient
from .generate_inset_surface import request
from .inset_multiedge_probe import cases

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk',action='store_true');parser.add_argument('--preflight',action='store_true')
    args=parser.parse_args();family='inset_multiedge_sdk' if args.sdk else 'inset_multiedge_command'
    query=request();query['operations']=cases(args.sdk)
    scheme='VibocerosOracle'+family.replace('_','')+'20261103'
    if args.preflight:
        query['operations']=[op for op in cases(True) if op['distance']==0.3 and op['crease_corners']]
        data=OracleClient(settings_scheme=scheme+'Preflight').run_rhino(query,timeout=600)
        Path('/tmp/inset-multiedge-preflight.json').write_text(json.dumps(data))
        for row in data['results']:
            if 'error' in row or not row['value']['output']['valid']:raise ValueError(str(row.get('error',row['id'])))
        print('Validated all six source partitions',flush=True)
    else:
        capture(family,query,['generate_inset_multiedge.py','generate_inset_surface.py','inset_multiedge_probe.py',
                'inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
                'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],scheme,timeout=2400)
