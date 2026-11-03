"""Freeze public ExtractSrf behavior for editable SubD faces."""
from .capture_family import capture
from .subd_extract_cases import cases,extra_cases,validate
import argparse


def request(details=False):
    ops=[dict(op='subd_extract_command',id='-'.join((scene,'copy' if copy else 'remove','corners' if corners else 'smooth')),
              scene=scene,copy=copy,corners=corners) for scene in (extra_cases() if details else cases()) for copy in (False,True) for corners in ((False,) if scene=='default' else (False,True))]
    for op in ops:validate(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=ops)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--details',action='store_true');args=parser.parse_args()
    capture('subd_extract_details' if args.details else 'subd_extract_command',request(args.details),['generate_subd_extract.py','subd_extract_cases.py','subd_extract_probe.py',
        'subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py'],'VibocerosOracleSubDExtractDetails20261102' if args.details else 'VibocerosOracleSubDExtract20261102',timeout=900)
