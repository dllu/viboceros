"""Freeze joined curved planar Inset outputs, selection and actual history."""
from .capture_family import capture
from .generate_inset_surface import request
from .inset_planar_curves_command import cases

if __name__ == '__main__':
    query = request(); query['operations'] = cases()
    capture('inset_planar_curves_command',query,
            ['generate_inset_planar_curves_command.py','generate_inset_surface.py',
             'inset_planar_curves_command.py','inset_planar_curves.py','inset_surface_probe.py',
             'inset_surface_command.py','join_probe.py','client.py','rhino_worker.py','structural_sdk.py','merge_edges_probe.py'],
            'VibocerosOracleInsetPlanarCurvesCommand20261103',timeout=1200)
