"""Capture both output policies in a private native Xvfb session."""
import argparse
from .capture_family import capture
from .use_extrusions_creators_cases import cases,limits_cases,domain_cases

if __name__=='__main__':
    parser=argparse.ArgumentParser();group=parser.add_mutually_exclusive_group();group.add_argument('--pilot',action='store_true');group.add_argument('--limits',action='store_true');group.add_argument('--domains',action='store_true');args=parser.parse_args()
    capture('use_extrusions_creators'+('_pilot' if args.pilot else '_limits' if args.limits else '_domains' if args.domains else ''),
        dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=domain_cases() if args.domains else limits_cases() if args.limits else cases(args.pilot)),
        ['generate_use_extrusions_creators.py','use_extrusions_creators_cases.py','use_extrusions_creators_probe.py','creator_extrusions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleExtrusionCreators20261106',timeout=1800)
