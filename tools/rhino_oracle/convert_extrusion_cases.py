"""Fixed native extrusion sources and public conversion input forms."""
ASSETS = (
    'tosubd_extrusion_rectangle_00','tosubd_extrusion_rectangle_open_00',
    'tosubd_extrusion_rectangle_negative_00','tosubd_extrusion_circle_00',
    'tosubd_extrusion_circle_open_00','tosubd_extrusion_open_kink_00',
    'tosubd_extrusion_arc_00','tosubd_extrusion_rectangle_miter_00',
    'tosubd_extrusion_circle_shear_00','tosubd_pipe_000','tosubd_pipe_100')


def pilot_cases():
    return [dict(op='convert_extrusion',id='pilot_'+str(i),asset=ASSETS[0],delete=delete,preselect=pre,grouped=False)
            for i,(delete,pre) in enumerate([(None,True),(True,True),(False,True),(True,False),(False,False)])]


def cases():
    rows=[dict(op='convert_extrusion',id=asset+'_'+str(int(delete))+str(int(pre)),asset=asset,
               delete=delete,preselect=pre,grouped=False)
          for asset in ASSETS for delete in (False,True) for pre in (False,True)]
    rows += [dict(op='convert_extrusion',id='grouped_'+str(int(delete))+str(int(pre)),asset=ASSETS[0],
                  delete=delete,preselect=pre,grouped=True)
             for delete in (False,True) for pre in (False,True)]
    rows += [dict(op='convert_extrusion',id='default_'+str(i),asset=ASSETS[0],delete=delete,preselect=pre,grouped=False)
             for i,(delete,pre) in enumerate([(False,True),(None,True),(False,False),(None,False)])]
    rows += [dict(op='convert_extrusion',id='pair_'+str(int(delete))+str(int(pre))+str(int(reverse)),asset=ASSETS[0],
                  delete=delete,preselect=pre,grouped=False,pair=True,reverse=reverse)
             for delete in (False,True) for pre in (False,True) for reverse in (False,True)]
    return rows


def validate(op):
    if (not isinstance(op,dict) or op not in pilot_cases()+cases()
            or type(op.get('preselect')) is not bool or type(op.get('grouped')) is not bool
            or any(name in op and type(op[name]) is not bool for name in ('pair','reverse'))
            or (op.get('delete') is not None and type(op['delete']) is not bool)):
        raise ValueError('fixed owned extrusion conversion recipe required')
