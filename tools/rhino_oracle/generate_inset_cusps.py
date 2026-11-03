"""Freeze curvature-limit Insets through public SDK and actual commands."""
import argparse
from .capture_family import capture
from .generate_inset_surface import request
from .inset_cusp_probe import cases

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk',action='store_true');args=parser.parse_args()
    family='inset_cusp_sdk' if args.sdk else 'inset_cusp_command'
    query=request();query['operations']=cases(args.sdk)
    capture(family,query,['generate_inset_cusps.py','generate_inset_surface.py','inset_cusp_probe.py',
            'inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
            'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracle'+family.replace('_','')+'20261103',timeout=1200)
