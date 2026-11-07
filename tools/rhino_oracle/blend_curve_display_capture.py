"""Passive public viewport pixels and graph settings on the owned Xvfb display."""
import base64
import os
import re

class Observer:
    def __init__(self,host,view,root,E,case_id):
        self.host,self.view,self.root,self.E=host,view,root,E
        if re.match(r'\A[a-zA-Z0-9_-]+\Z',case_id)is None:raise ValueError('invalid owned snapshot case')
        self.case_id=case_id
        self.shots=[]

    def expression(self,index,label):
        import clr
        from System.Linq.Expressions import ParameterExpression
        S,E=self.host['System'],self.E
        if re.match(r'\A[a-z0-9_-]+\Z',label) is None:
            raise ValueError('invalid owned viewport snapshot label')
        path=os.path.join(self.root,'blend-display-'+self.case_id+'-'+str(index)+'-'+label+'.png')
        if os.path.exists(path):raise ValueError('viewport snapshot already exists')
        settings=S.Array[S.Int32]([-1,-1,-1])
        R=self.host['Rhino']
        mode=R.Display.VisualAnalysisMode.Find(R.Display.VisualAnalysisMode.RhinoCurvatureGraphAnalysisModeId)
        if mode is None:raise ValueError('public curvature graph analysis mode unavailable')
        objects=list(R.RhinoDoc.ActiveDoc.Objects)
        analysis=S.Array[S.Boolean]([False]*len(objects))
        def array(values):return S.Array[E](values)
        def method(t,name,types):return clr.GetClrType(t).GetMethod(name,S.Array[S.Type]([clr.GetClrType(v)for v in types]))
        def call(target,m,values):return E.Call(target,m,array(values))
        def integer(value):return E.Constant(S.Int32(value))
        if label.startswith('desktop_'):
            rect=S.Windows.Forms.SystemInformation.VirtualScreen
            size,origin=rect.Size,rect.Location
        else:
            size=self.view.ActiveViewport.Size
            origin=self.view.ClientToScreen(S.Drawing.Point(0,0))
        bitmap=E.Variable(clr.GetClrType(S.Drawing.Bitmap),'bitmap')
        graphics=E.Variable(clr.GetClrType(S.Drawing.Graphics),'graphics')
        image_type=S.Drawing.Imaging.ImageFormat
        capture=call(graphics,method(S.Drawing.Graphics,'CopyFromScreen',[S.Int32,S.Int32,S.Int32,S.Int32,S.Drawing.Size,S.Drawing.CopyPixelOperation]),[
            integer(origin.X),integer(origin.Y),integer(0),integer(0),E.Constant(size),E.Constant(S.Drawing.CopyPixelOperation.SourceCopy)])
        save=call(bitmap,method(S.Drawing.Image,'Save',[S.String,image_type]),[E.Constant(path),E.Property(None,clr.GetClrType(image_type).GetProperty('Png'))])
        body=[E.Assign(bitmap,E.New(clr.GetClrType(S.Drawing.Bitmap).GetConstructor(S.Array[S.Type]([clr.GetClrType(S.Int32),clr.GetClrType(S.Int32)])),array([integer(size.Width),integer(size.Height)]))),
              E.TryFinally(E.Block(S.Array[ParameterExpression]([graphics]),array([
                  E.Assign(graphics,call(None,method(S.Drawing.Graphics,'FromImage',[S.Drawing.Image]),[bitmap])),
                  E.TryFinally(E.Block(array([capture,save])),call(graphics,method(S.Drawing.Graphics,'Dispose',[]),[]))])),
                  call(bitmap,method(S.Drawing.Image,'Dispose',[]),[]))]
        for i,name in enumerate(['HairScale','HairDensity','SampleDensity']):
            prop=clr.GetClrType(self.host['Rhino'].ApplicationSettings.CurvatureGraphSettings).GetProperty(name)
            body.append(E.Assign(E.ArrayAccess(E.Constant(settings),array([integer(i)])),E.Convert(E.Property(None,prop),clr.GetClrType(S.Int32))))
        for i,obj in enumerate(objects):
            body.append(E.Assign(E.ArrayAccess(E.Constant(analysis),array([integer(i)])),call(E.Constant(obj),method(R.DocObjects.RhinoObject,'InVisualAnalysisMode',[R.Display.VisualAnalysisMode]),[E.Constant(mode,clr.GetClrType(R.Display.VisualAnalysisMode))])))
        body.append(E.Empty())
        self.shots.append((index,label,path,size,settings,[(o.Attributes.Name or '')for o in objects],analysis))
        return E.Block(S.Array[ParameterExpression]([bitmap]),array(body))

    def records(self):
        rows=[]
        for index,label,path,size,settings,names,analysis in self.shots:
            row=dict(action=index,label=label,width=int(size.Width),height=int(size.Height),captured=os.path.isfile(path),
                     settings=dict(scale=int(settings[0]),density=int(settings[1]),sample_density=int(settings[2])),
                     object_graphs=[dict(name=n,enabled=bool(a))for n,a in zip(names,analysis)])
            if row['captured']:
                with open(path,'rb')as stream:row['png_base64']=base64.b64encode(stream.read()).decode('ascii')
            rows.append(row)
        return rows

def completed(host,case_id,desktop=False):
    import clr
    from System.Linq.Expressions import Expression as E,ParameterExpression
    R,S=host['Rhino'],host['System']
    R.RhinoDoc.ActiveDoc.Views.Redraw();R.RhinoApp.Wait()
    observer=Observer(host,R.RhinoDoc.ActiveDoc.Views.ActiveView,os.path.dirname(host['__file__']),E,case_id)
    for i,label in enumerate(['completed']+(['desktop_completed']if desktop else[])):
        expression=observer.expression(1000+i,label)
        E.Lambda(clr.GetClrType(S.Action),expression,S.Array[ParameterExpression]([])).Compile().Invoke()
    return observer.records()

def setup(host,guides,indices,reset=False):
    R=host['Rhino'];doc=R.RhinoDoc.ActiveDoc
    mode=R.Display.VisualAnalysisMode.Find(R.Display.VisualAnalysisMode.RhinoCurvatureGraphAnalysisModeId)
    if reset:R.RhinoApp.RunScript('_CurvatureGraphOff',False)
    for index in indices:
        if type(index)is not int or not 0<=index<len(guides):raise ValueError('invalid graph setup source')
        if not doc.Objects.FindId(guides[index]).EnableVisualAnalysisMode(mode,True):raise ValueError('unsupported graph setup source')
    doc.Views.Redraw();R.RhinoApp.Wait()
