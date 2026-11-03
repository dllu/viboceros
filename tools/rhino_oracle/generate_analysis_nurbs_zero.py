"""Freeze the public effective-settings behavior for a stored zero density."""
from .capture_family import capture
from .analysis_nurbs_zero_probe import validate


def request():
    op=dict(op='analysis_nurbs_zero',id='zero');validate(op)
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=1e-8,relative=1e-12,angular=1e-8),operations=[op])


def main():
    capture('analysis_nurbs_zero',request(),['generate_analysis_nurbs_zero.py','analysis_nurbs_zero_probe.py',
        'client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py',
        'fixtures/analysis_nurbs_settings/density0.3dm'],
        'VibocerosOracleAnalysisNurbsZero20261102',timeout=900)


if __name__=='__main__':main()
