"""Freeze native parameter semantics and meshing outputs for bounded surfaces."""
from .capture_family import capture
from .nurbs_meshing_probe import SOURCES, PROFILES, validate


def request():
    operations=[]
    for source in SOURCES:
        for profile in PROFILES:
            op=dict(op='nurbs_meshing',id=source+'-'+profile,source=source,profile=profile)
            validate(op);operations.append(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    capture('nurbs_meshing',request(),['generate_nurbs_meshing.py','nurbs_meshing_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
        'VibocerosOracleNurbsMeshing20261102',timeout=1200)


if __name__=='__main__':main()
