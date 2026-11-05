"""Fixed SubD diagnostic-edge display recipes and shared soft-crease inputs."""
ASSETS = ('variable_both_ends', 'hard_soft_chain')


def cases():
    return [dict(op='subd_edge_analysis', id=scene + '_' + str(int(pre)) + '_' + str(int(flat)),
                 scene=scene, preselect=pre, flat=flat, command='ShowEdges',
                 actions=['Show=Naked', 'Show=All', 'Show=NonManifold'])
            for scene in ('open_cube', 'cube', 'creased_cube', 'asset_hard_soft_chain')
            for pre in (False, True) for flat in (False, True)]


def validate(op):
    if (not isinstance(op, dict) or op not in cases() + controls() + panels() + panels_mouse() + capabilities()
            or any(type(op.get(k)) is not bool for k in ('preselect', 'flat'))
            or ('shared_input' in op and type(op['shared_input']) is not bool)
            or ('native_click' in op and type(op['native_click']) is not bool)):
        raise ValueError('fixed owned SubD edge-analysis case required')


def controls():
    rows = [dict(op='subd_edge_analysis', id=scene+'_'+str(int(pre))+'_'+str(int(flat))+'_'+str(i),
                 scene=scene, preselect=pre, flat=flat, command='ZoomNaked', shared_input=True,
                 actions=actions, finish=finish)
            for scene in ('bent_grid','open_cube','pentagon','fan7')
            for pre in (False,True) for flat in (False,True)
            for i,(actions,finish) in enumerate([(['All','Mark'],'Enter'),(['Mark','Next','Mark'],'Cancel')])]
    return rows


def panels():
    rows = [dict(op='subd_edge_analysis',id='panel_'+scene+'_'+str(int(flat)),
                  scene=scene,preselect=True,flat=flat,command='ShowEdges',shared_input=True,
                  actions=['Show=Naked','Show=All'],zoom=['All','Mark'])
             for scene in ('asset_variable_both_ends','asset_hard_soft_chain')for flat in (False,True)]
    return rows


def panels_mouse():
    return [dict(op,id=op['id']+'_mouse',native_click=True)for op in panels()]


def capabilities():
    return [dict(op='subd_edge_analysis',id='cap_'+scene+'_'+str(int(flat)),
                 scene=scene,preselect=True,flat=flat,command='ShowEdges',
                 actions=['Show=Naked','Show=All','Show=NonManifold'])
            for scene in ('open_cube','asset_variable_both_ends','asset_hard_soft_chain')
            for flat in (False,True)]
