"""Freeze mesh face insets and channel behavior."""
from .capture_family import capture
from .inset_cases import mesh_cases,mesh_confirmed_cases,mesh_primed_cases,mesh_ready_cases
import argparse

def request(confirmed=False,primed=False,ready=False):
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=mesh_ready_cases()if ready else mesh_primed_cases()if primed else mesh_confirmed_cases()if confirmed else mesh_cases())

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);group=parser.add_mutually_exclusive_group();group.add_argument('--confirmed',action='store_true');group.add_argument('--primed',action='store_true');group.add_argument('--ready',action='store_true');args=parser.parse_args()
    capture('inset_mesh_ready'if args.ready else 'inset_mesh_primed'if args.primed else 'inset_mesh_confirmed'if args.confirmed else 'inset_mesh_faces',request(args.confirmed,args.primed,args.ready),['generate_inset_mesh.py','inset_mesh_probe.py','inset_probe.py','inset_cases.py','subd_offset_cases.py','subd_extract_cases.py','subd_join_probe.py','subd_join_cases.py','subd_box_probe.py','join_probe.py','merge_edges_probe.py','client.py','rhino_worker.py','structural_sdk.py'],'VibocerosOracleInsetMeshReady20261102'if args.ready else 'VibocerosOracleInsetMeshPrimed20261102'if args.primed else 'VibocerosOracleInsetMeshConfirmed20261102'if args.confirmed else 'VibocerosOracleInsetMesh20261102',timeout=1200)
