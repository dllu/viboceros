"""Fixed mesh/SubD component extraction and ordering controls."""

CASES = {}
for kind in ['mesh_strip', 'mesh_unwelded', 'subd']:
    for label, picks in [
        ('face', [('face', 0)]), ('faces', [('face', 0), ('face', 1)]),
        ('faces_reverse', [('face', 1), ('face', 0)]),
        ('edge', [('edge', 0)]), ('edges', [('edge', 2), ('edge', 0)]),
        ('vertex', [('vertex', 0)]), ('vertices', [('vertex', 2), ('vertex', 0)]),
        ('mixed', [('face', 0), ('edge', 0), ('vertex', 0)]),
    ]:
        for flat in ([False, True] if kind == 'subd' else [False]):
            name = 'component_'+kind+'_'+label+('_flat' if flat else '')
            CASES[name] = dict(sources=[dict(kind=kind, mode=None, grips=[])],
                whole=[], order=[0], output='Points', layer='Input', flat=flat,
                components=[[0, k, i] for k,i in picks], component_capture=True)
for kind in ['mesh_strip', 'subd']:
    base = CASES['component_'+kind+'_faces']
    CASES['component_'+kind+'_cloud'] = dict(base, output='PointCloud', layer='Current', output_first=True)

CURVES = {'component_subd_curve_profile': dict(CASES['component_subd_edge'], curve_profile=True)}

EDGE_HOLDOUTS = {}
for kind in ['subd_irregular','subd_grid','subd_strip','subd_triangles','subd_creased']:
    EDGE_HOLDOUTS['component_edges_'+kind] = dict(CASES['component_subd_edge'],
        sources=[dict(kind=kind,mode=None,grips=[])], curve_profile=True, all_edge_profiles=True)

SELECTION = {'checked_'+name:dict(case, component_records=True) for name,case in CASES.items()}

MIXED = {}
for output in ['Points','PointCloud']:
    for reverse in [False,True]:
        sources = [dict(kind='mesh_strip',mode=None,grips=[]),dict(kind='rational',mode='control',grips=[2,0])]
        if reverse:
            sources.reverse()
        sources.append(dict(kind='curve',mode=None,grips=[]))
        MIXED['component_mixed_'+output+('_reverse' if reverse else '')] = dict(
            sources=sources,whole=[2],order=[0,1,2],output=output,layer='Input',flat=False,
            components=[[1 if reverse else 0,'face',0]],component_capture=True,component_records=True,
            output_first=(output=='PointCloud'))
    MIXED['component_two_owners_'+output] = dict(sources=[dict(kind='mesh_strip',mode=None,grips=[]),dict(kind='subd',mode=None,grips=[])],
        whole=[],order=[0,1],output=output,layer='Input',flat=False,components=[[1,'face',0],[0,'face',0]],
        component_capture=True,component_records=True,output_first=(output=='PointCloud'))
    MIXED['component_with_grips_'+output] = dict(sources=[dict(kind='subd',mode='control',grips=[2,0])],
        whole=[],order=[0],output=output,layer='Input',flat=False,components=[[0,'face',0]],
        component_capture=True,component_records=True,output_first=(output=='PointCloud'))

# Preselection executes after the first option. Output-first captures use the
# command's fresh Current layer default; Input-layer clouds need a getter.
MIXED_VERIFIED = {'checked_'+name:dict(case,layer='Current' if case['output']=='PointCloud' else 'Input') for name,case in MIXED.items()}

PICKED_CONTROLS = {'picked_'+name:dict(MIXED_VERIFIED[name],component_getter=True) for name in ['checked_component_two_owners_Points','checked_component_with_grips_PointCloud']}

PICKED = {}
for cases in [SELECTION,EDGE_HOLDOUTS,MIXED_VERIFIED]:
    for name,case in cases.items():
        PICKED['postselected_'+name] = dict(case,component_getter=True,curve_profile=False)
