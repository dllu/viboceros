"""Fixed creator commands in both native output modes."""
from copy import deepcopy
if __package__:
    from .creator_extrusions_probe import CASES
else:
    from creator_extrusions_probe import CASES


def recipes(limits=False):
    result=deepcopy(CASES)
    for height in (-3,3):
        for both in (False,True):
            for swapped in (False,True):
                name='tube_'+str(height)+'_'+str(int(both))+str(int(swapped))
                radii='2 1' if swapped else '1 2'
                result[name]=dict(command='Tube',macro='_Tube w1,2,3 '+radii+' _BothSides=_'+('Yes' if both else 'No')+' '+str(height),
                    application='Tube 1,2,3 '+radii+' '+str(height)+' BothSides='+('Yes' if both else 'No'))
    for rail in ('line','reverse','oblique','nurbsline','polyline'):
        for cap in ('None','Flat','Round'):
            for thick in (False,True):
                suffix='1 1.5 1 1.5' if thick else '1 1'
                result['pipe_'+rail+'_'+cap+'_'+str(int(thick))]=dict(command='Pipe',rail=rail,
                    macro='_-Pipe _Output=_Surface _Thick=_No _Cap=_'+cap+' _Thick=_'+('Yes' if thick else 'No')+' '+suffix+' _Enter',
                    application='Pipe 1 Cap='+cap+(' Thick=Yes WallThickness=0.5' if thick else ' Thick=No'))
    result['surface']=dict(command='ExtrudeSrf',profile='surface',
        macro='_-ExtrudeSrf _Solid=_Yes _BothSides=_No _DeleteInput=_No 2',
        application='ExtrudeSrf 2 Solid=Yes BothSides=No DeleteInput=No')
    if limits:
        for name,values,application in [
                ('uniform_station','1 1 w0,0,2.5 1','1 Stations=0.5:1'),
                ('varying_station','1 1 w0,0,2.5 2','1 Stations=0.5:2'),
                ('tapered','1 2','1 2')]:
            result['pipe_'+name]=dict(command='Pipe',rail='line',macro='_-Pipe _Output=_Surface _Thick=_No _Cap=_Flat '+values+' _Enter',application='Pipe '+application+' Thick=No Cap=Flat')
        for name,domain in [('positive',[2.,9.]),('negative',[-5.,-2.])]:
            result['pipe_domain_'+name]=dict(command='Pipe',rail='domainline',rail_domain=domain,
                macro='_-Pipe _Output=_Surface _Thick=_No _Cap=_Flat 1 1 _Enter',application='Pipe 1 Thick=No Cap=Flat')
    return result


def cases(pilot=False):
    chosen=['creator_detail_box','creator_detail_cylinder_default','creator_detail_extrude_circle_10p','tube_3_00','pipe_line_Flat_0','pipe_line_None_1','pipe_oblique_Flat_0','surface'] if pilot else list(recipes())
    return [dict(op='use_extrusions_creators',id=mode+'_'+name,mode=mode,recipe=name)
            for name in chosen for mode in ('Extrusion','Polysurface')]


def limits_cases():
    return [dict(op='use_extrusions_creators',id=mode+'_pipe_'+name,mode=mode,recipe='pipe_'+name)
            for name in ('uniform_station','varying_station','tapered') for mode in ('Extrusion','Polysurface')]


def domain_cases():
    return [dict(op='use_extrusions_creators',id=mode+'_pipe_domain_'+name,mode=mode,recipe='pipe_domain_'+name)
            for name in ('positive','negative') for mode in ('Extrusion','Polysurface')]


def validate(op):
    if not isinstance(op,dict) or op not in cases()+limits_cases()+domain_cases():raise ValueError('fixed owned extrusion creator required')
