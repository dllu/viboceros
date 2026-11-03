"""Freeze planar Inset curves, separated surfaces and option history."""
import argparse
from .capture_family import capture
from .generate_inset_surface import request
from .inset_outputs_probe import cases, limits, PREFERENCES

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--preferences',action='store_true')
    modes.add_argument('--limits',action='store_true')
    args = parser.parse_args()
    family = 'inset_output_preferences' if args.preferences else 'inset_output_limits' if args.limits else 'inset_output_modes'
    query = request(); query['operations'] = [PREFERENCES] if args.preferences else limits() if args.limits else cases()
    capture(family,query,['generate_inset_outputs.py','generate_inset_surface.py',
            'inset_outputs_probe.py','inset_planar_curves.py','inset_surface_probe.py',
            'inset_surface_command.py','join_probe.py','client.py','rhino_worker.py',
            'structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracle'+family.replace('_','')+'20261103',timeout=1200)
