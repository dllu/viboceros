"""Public display toggles with whole SubDs and selected control/edit grips."""
CASES={}
for kind in ['subd','subd_irregular','subd_grid','subd_strip','subd_triangles','subd_creased']:
    for flat in [False,True]:
        for mode in [None,'control','edit']:
            name='display_'+kind+('_flat_' if flat else '_smooth_')+(mode or 'whole')
            CASES[name]=dict(sources=[dict(kind=kind,mode=mode,grips=[0,2] if mode else [])],
                whole=[0],order=[0],output='PointCloud' if mode=='edit' else 'Points',layer='Input',
                getter=True,flat=flat,whole_subd=True,display_toggle=True)
