"""Capture microscopic knot-span creators in a private Xvfb native session."""
from .capture_family import capture
from .short_span_extrusions_cases import cases

if __name__=='__main__':
    capture('short_span_extrusions',dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-12,angular=1e-6),operations=cases()),
        ['generate_short_span_extrusions.py','short_span_extrusions_cases.py','short_span_extrusions_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleShortSpanExtrusions20261106',timeout=1200)
