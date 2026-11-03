"""Freeze native SubD component extrusion geometry and history."""
from .capture_family import capture
from .subd_extrude_cases import cases,mixed_cases
import argparse
def request(extra=False,mixed=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=mixed_cases() if mixed else cases(extra))
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group();group.add_argument('--extra',action='store_true');group.add_argument('--mixed',action='store_true');args=parser.parse_args()
    capture('subd_extrude_mixed' if args.mixed else 'subd_extrude_extra' if args.extra else 'subd_extrude_command',request(args.extra,args.mixed),['generate_subd_extrude.py','subd_extrude_probe.py','subd_extrude_cases.py',
        'subd_offset_cases.py','subd_extract_cases.py','subd_join_cases.py','subd_join_probe.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py'],'VibocerosOracleSubDExtrudeMixed20261102' if args.mixed else 'VibocerosOracleSubDExtrudeExtra20261102' if args.extra else 'VibocerosOracleSubDExtrude20261102',timeout=1200)
