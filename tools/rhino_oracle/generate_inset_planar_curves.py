"""Freeze Rhino's curved planar boundary insets on a private display."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_planar_curves import cases

if __name__ == '__main__':
    query = request(); query['operations'] = cases()
    capture('inset_planar_curves',query,
            ['generate_inset_planar_curves.py','generate_inset_surface.py','inset_planar_curves.py',
             'inset_surface_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetPlanarCurves20261103',timeout=1200)
