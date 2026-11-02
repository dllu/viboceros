"""Inspect the public active getter and safely cancel unsupported preselection."""
def validate(op):
    if set(op)!={'op','id'} or op['op']!='surface_analysis_input':raise ValueError('invalid analysis input request')

def run(op,host,iterations,tolerance):
    validate(op)
    import clr
    clr.AddReference('WindowsBase')
    from System.Windows.Threading import DispatcherTimer
    R,S=host['Rhino'],host['System'];G=R.Geometry;doc=R.RhinoDoc.ActiveDoc
    if iterations!=1 or list(doc.Objects) or R.Commands.Command.InCommand():raise ValueError('analysis filter capture requires an empty owned document')
    mesh=G.Mesh()
    for p in [(0,0,0),(3,0,0),(3,3,0),(0,3,0)]:mesh.Vertices.Add(*p)
    mesh.Faces.AddFace(0,1,2,3)
    subd=G.SubD.CreateFromMesh(mesh)
    result=[]
    try:
        for name,source in [('mesh',mesh),('subd',subd)]:
            key=doc.Objects.Add(source)
            doc.Objects.UnselectAll();doc.Objects.Select(key)
            row=dict(source=name,filter=None,events=[])
            timer=DispatcherTimer();timer.Interval=S.TimeSpan.FromMilliseconds(500)
            def tick(sender,args):
                timer.Stop();getter=R.Input.Custom.GetObject.ActiveGetObject(doc)
                if getter is not None:row['filter']=str(getter.GeometryFilter)
                R.RhinoApp.SendKeystrokes('_Cancel',True)
            def ended(sender,event):
                row['events'].append(dict(name=str(event.CommandEnglishName),result=str(event.CommandResult)))
                if str(event.CommandEnglishName)=='CurvatureAnalysis':timer.Stop()
            timer.Tick+=tick;R.Commands.Command.EndCommand+=ended;timer.Start()
            try:R.RhinoApp.RunScript('_CurvatureAnalysis',False)
            finally:timer.Stop();R.Commands.Command.EndCommand-=ended
            obj=doc.Objects.FindId(key);row['analysis']=bool(obj.InVisualAnalysisMode())
            result.append(row)
            R.RhinoApp.RunScript('_CurvatureAnalysisOff',False);doc.Objects.Delete(key,True)
        return result,0
    finally:
        mesh.Dispose();subd.Dispose()
