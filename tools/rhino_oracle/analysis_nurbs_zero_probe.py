"""Owned public diagnostics for Rhino's zero analysis-density fallback."""
import os


def validate(op):
    if op != dict(op='analysis_nurbs_zero',id='zero'):
        raise ValueError('fixed zero-density recipe required')


def run(op,host,iterations,tolerance):
    validate(op)
    R,S=host['Rhino'],host['System']
    doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():
        raise ValueError('zero density capture requires empty owned idle document')
    path=os.path.join(os.path.dirname(__file__),'fixtures','analysis_nurbs_settings','density0.3dm')
    names=('RelativeTolerance','GridMinCount','GridAngle','RefineAngle','RefineGrid',
           'GridAspectRatio','SimplePlanes','ComputeCurvature','MinimumEdgeLength','MaximumEdgeLength')
    values=[]
    for custom in (None,.2,.9):
        loaded=R.RhinoDoc.OpenHeadless(path)
        if loaded is None:raise ValueError('owned zero density load failed')
        try:
            if custom is not None:
                mp=R.Geometry.MeshingParameters(custom)
                try:loaded.SetCustomMeshingParameters(mp)
                finally:mp.Dispose()
                enum=loaded.GetType().GetProperty('MeshingParameterStyle').PropertyType
                loaded.MeshingParameterStyle=S.Enum.Parse(enum,'Custom')
            analysis=loaded.GetAnalysisMeshingParameters()
            render=loaded.GetCurrentMeshingParameters()
            try:values.append(dict(custom=custom,style=str(loaded.MeshingParameterStyle),
                analysis={key:getattr(analysis,key) for key in names},
                render={key:getattr(render,key) for key in names}))
            finally:analysis.Dispose();render.Dispose()
        finally:loaded.Dispose()
    model=R.FileIO.File3dm.Read(path)
    try:properties=sorted(p.Name for p in model.Settings.GetType().GetProperties())
    finally:model.Dispose()
    return dict(variants=values,file_settings_properties=properties),0
