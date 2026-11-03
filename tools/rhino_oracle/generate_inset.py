"""Freeze native inset topology, positions, selection and history."""
from .capture_family import capture
from .inset_cases import cases,extra_cases
import argparse

def request(extra=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=extra_cases()if extra else cases())

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--extra',action='store_true');args=parser.parse_args()
    capture('inset_extra_faces'if args.extra else 'inset_faces',request(args.extra),['generate_inset.py','inset_extra_probe.py','inset_probe.py','inset_cases.py','subd_offset_cases.py','subd_extract_cases.py','subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py'],'VibocerosOracleInsetExtra20261102'if args.extra else 'VibocerosOracleInset20261102',timeout=1200)
