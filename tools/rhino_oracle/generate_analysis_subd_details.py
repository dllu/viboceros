"""Freeze native limit points and native reads of Viboceros-written SubD settings."""
from .capture_family import capture, HERE
from .analysis_subd_details_probe import ARCHIVES, validate


def request():
    op = dict(op='analysis_subd_details',id='details'); validate(op)
    return dict(protocol_version=1,iterations=1,
                tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=[op])


def main():
    capture('analysis_subd_details',request(),
            ['generate_analysis_subd_details.py','analysis_subd_details_probe.py','client.py',
             'rhino_worker.py','structural_sdk.py','merge_edges_probe.py',
             '../../crates/viboceros-io/examples/analysis_subd_fixtures.rs',
             '../../crates/viboceros-io/src/three_dm.rs',
             '../../crates/viboceros-opennurbs/native/viboceros_opennurbs.cpp',
             '../../crates/viboceros-opennurbs/native/viboceros_opennurbs.h',
             *['fixtures/analysis_subd_settings/'+name+'.3dm' for name in ARCHIVES]],
            'VibocerosOracleAnalysisSubDDetails20261102',timeout=900)


if __name__=='__main__': main()
