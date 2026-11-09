"""Physical FitSrf pilot: SDK nulls, source redundancy and output policies."""
from .capture_family import capture
from .generate_surface_fit_adaptive import bezier,dense
from .generate_surface_knot_reduction import surface


def request():
    cases=[('plane_copy',surface('plane'),.01,False,'input',False),
           ('plane_replace_current',surface('plane'),.01,True,'current',True),
           ('polynomial_null',surface('polynomial'),.01,False,'input',False),
           ('polynomial_dense',dense(surface('polynomial')),.01,False,'input',False),
           ('rational_dense',dense(bezier(True)),.01,True,'input',True),
           ('redundant_polynomial',dense(bezier()),.001,True,'input',False),
           ('zero_tolerance',dense(bezier()),0.,False,'current',False),
           ('degree_two',dense(surface('polynomial')),.01,False,'input',False)]
    return dict(protocol_version=1,iterations=1,tolerance=dict(absolute=.001,relative=1e-9,angular=1e-6),
        operations=[dict(op='surface_fit_command',id='fit-command-'+name,surface=source,
            degree_u=2 if name=='degree_two'else 3,degree_v=2 if name=='degree_two'else 3,
            fit_tolerance=tolerance,delete_input=delete,retrim=False,output_layer=layer,preselect=pre)
            for name,source,tolerance,delete,layer,pre in cases])


if __name__=='__main__':
    capture('surface_fit_commands',request(),['generate_surface_fit_commands.py','surface_fit_command_probe.py',
        'generate_surface_fit_adaptive.py','generate_surface_knot_reduction.py','generate_surface_knot_reduction_holdouts.py',
        'structural_sdk.py','rhino_worker.py','merge_edges_probe.py','client.py'],
        'VibocerosOracleSurfaceFitCommand20261108',timeout=1200)
