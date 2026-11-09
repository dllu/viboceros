"""Sharp closed-loop seam and adjacent-turn holdouts after the first loop audit."""
from .capture_family import capture
from .generate_ribbon import curve


def request():
    polygons = [
        ('elbow',[[0.,0.,0.],[6.,0.,0.],[6.,6.,0.],[3.,6.,0.],[3.,3.,0.],[0.,3.,0.]], [1,4,5]),
        ('arrow',[[0.,0.,0.],[8.,0.,0.],[8.,6.,0.],[4.,3.,0.],[0.,6.,0.]], [0,3,4]),
    ]
    operations = []
    for name,points,seams in polygons:
        for seam in seams:
            shifted = points[seam:]+points[:seam]+[points[seam]]
            operations.append(dict(op='ribbon_command',id=name+'-seam-'+str(seam),curve=curve(shifted),
                distance=.2,side=[2.,-2.,0.],both_sides=True,corner='Sharp',loose=False,
                in_cplane=False,output_layer='Current'))
    return dict(protocol_version=1,iterations=1,
        tolerance=dict(absolute=1e-6,relative=1e-9,angular=1e-6),operations=operations)


if __name__ == '__main__':
    capture('ribbon_sharp_seams',request(),['generate_ribbon_sharp_seams.py','generate_ribbon.py',
        'ribbon_probe.py','join_probe.py','merge_edges_probe.py','structural_sdk.py','rhino_worker.py','client.py'],
        'VibocerosOracleRibbonSharpSeams20261109',timeout=1200)
