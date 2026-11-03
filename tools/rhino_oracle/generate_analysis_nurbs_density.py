"""Freeze native analysis settings and a native rewrite for each density."""
from .capture_family import capture
from .analysis_nurbs_density_probe import ARCHIVES,validate


def request():
    operations=[dict(op='analysis_nurbs_density',id=name,archive=name) for name in ARCHIVES]
    for op in operations:validate(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=operations)


def main():
    capture('analysis_nurbs_density',request(),['generate_analysis_nurbs_density.py','analysis_nurbs_density_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py',
        '../../crates/viboceros-io/examples/analysis_nurbs_fixtures.rs',
        '../../crates/viboceros-io/src/three_dm.rs','../../crates/viboceros-io/src/three_dm/analysis_mesh.rs',
        '../../crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
        '../../crates/viboceros-opennurbs/native/viboceros_opennurbs.h',
        *['fixtures/analysis_nurbs_settings/'+name+'.3dm' for name in ARCHIVES]],
        'VibocerosOracleAnalysisNurbsDensity20261102',timeout=900)


if __name__=='__main__':main()
