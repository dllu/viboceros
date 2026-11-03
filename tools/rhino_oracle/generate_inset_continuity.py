"""Freeze edge continuity, independent-curve and curved-crease Inset controls."""
import argparse
from .capture_family import capture
from .generate_inset_surface import request
from .inset_continuity_probe import cases,limits

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--sdk',action='store_true');parser.add_argument('--limits',action='store_true')
    args=parser.parse_args();family='inset_continuity_limits' if args.limits else 'inset_continuity_sdk' if args.sdk else 'inset_continuity_command'
    query=request();query['operations']=limits() if args.limits else cases(args.sdk)
    capture(family,query,['generate_inset_continuity.py','generate_inset_surface.py','inset_continuity_probe.py',
            'inset_multiedge_probe.py','inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py',
            'inset_surface_command.py','join_probe.py','client.py','rhino_worker.py','structural_sdk.py',
            'merge_edges_probe.py'],'VibocerosOracle'+family.replace('_','')+'20261103',timeout=1800)
