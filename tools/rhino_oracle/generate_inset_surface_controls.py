"""Freeze affine surface inset controls without a document command."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_surface_controls import cases

if __name__ == '__main__':
    query = request(); query['operations'] = cases()
    capture('inset_surface_controls',query,
            ['generate_inset_surface_controls.py','generate_inset_surface.py','inset_surface_controls.py',
             'inset_surface_probe.py','capture_family.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetSurfaceControls20261102',timeout=1200)
