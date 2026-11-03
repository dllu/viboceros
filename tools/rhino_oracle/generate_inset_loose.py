"""Freeze Loose inset basis controls, commands and preference history."""
import argparse
from .capture_family import capture
from .generate_inset_surface import request
from .inset_loose_controls import cases as controls
from .inset_loose_command import cases as commands, PREFERENCES

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--command',action='store_true');parser.add_argument('--preferences',action='store_true')
    args=parser.parse_args();query=request()
    family='inset_loose_preferences' if args.preferences else 'inset_loose_command' if args.command else 'inset_loose_controls'
    query['operations']=[PREFERENCES] if args.preferences else commands() if args.command else controls()
    capture(family,query,['generate_inset_loose.py','generate_inset_surface.py','inset_loose_controls.py',
            'inset_loose_command.py','inset_planar_curves.py','inset_surface_probe.py','inset_surface_command.py',
            'join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracle'+family.replace('_','')+'20261103',timeout=1200)
