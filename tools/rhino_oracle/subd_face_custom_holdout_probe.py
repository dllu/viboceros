"""Independent custom-face witnesses, including public geometry transforms."""
try:
    from .subd_face_custom_holdout_cases import CASES,validate
except (ImportError,ValueError):
    from subd_face_custom_holdout_cases import CASES,validate


def run(op,host,iterations,tolerance):
    validate(op)
    from subd_edit_scene import Scene
    from subd_face_custom_probe import workflow
    if type(iterations)is not int or iterations!=1 or host['Rhino'].Commands.Command.InCommand():
        raise ValueError('owned idle custom-face holdout required')
    case=CASES[op['case']]
    class PreparedScene(Scene):
        def geometry(self,definition):
            g=Scene.geometry(self,definition)
            if 'transform'in case:
                x=host['Rhino'].Geometry.Transform.Identity
                for i in range(4):
                    for j in range(4):setattr(x,'M'+str(i)+str(j),float(case['transform'][i][j]))
                if not g.Transform(x):g.Dispose();raise ValueError('public geometry transform failed')
            return g
    return workflow(op,case,host,PreparedScene,'subd_face_custom_holdout_assets.json')
