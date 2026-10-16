"""Input bounds, SDK ownership and actual worker routing for two-rail sweeps."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .two_rail_sweep_probe import request, validate, run, ordered_sections


class SweepTests(TestCase):
    def test_sdk_preparation_orders_sections_and_reverses_only_private_inputs(self):
        class Point:
            def __init__(self,x,z):self.x=x;self.z=z
            def DistanceTo(self,p):return ((self.x-p.x)**2+(self.z-p.z)**2)**.5
        class Segment:
            def __init__(self,a,b):self.PointAtStart=a;self.PointAtEnd=b;self.reversals=0
            def PointAt(self,t):
                a,b=self.PointAtStart,self.PointAtEnd
                return Point(a.x+t*(b.x-a.x),a.z+t*(b.z-a.z))
            def ClosestPoint(self,p):
                a,b=self.PointAtStart,self.PointAtEnd
                return True, max(0.,min(1.,((p.x-a.x)*(b.x-a.x)+(p.z-a.z)*(b.z-a.z))/((b.x-a.x)**2+(b.z-a.z)**2)))
            def Reverse(self):
                self.PointAtStart,self.PointAtEnd=self.PointAtEnd,self.PointAtStart;self.reversals+=1;return True
        rails=[Segment(Point(0,0),Point(0,5)),Segment(Point(2,5),Point(2,0))]
        sections=[Segment(Point(2,5),Point(0,5)),Segment(Point(0,0),Point(2,0))]
        ordered=ordered_sections(rails,sections,1e-9)
        self.assertEqual(ordered,[sections[1],sections[0]])
        self.assertEqual([s.reversals for s in sections],[1,0])
        self.assertEqual([r.reversals for r in rails],[0,1])

    def host(self):
        source = mock.Mock(); duplicate = mock.Mock()
        source.DuplicateCurve.return_value = duplicate
        endpoint = mock.Mock(); endpoint.DistanceTo.return_value = 0.
        duplicate.PointAtStart = endpoint; duplicate.PointAtEnd = endpoint
        duplicate.ClosestPoint.return_value = True, 0.
        duplicate.PointAt.return_value = endpoint
        face = mock.Mock(); face.ClosestPoint.return_value = True, .3, .4
        p = mock.Mock(); p.DistanceTo.return_value = 0.; face.PointAt.return_value = p
        faces = [face]
        class Faces(list):
            @property
            def Count(self): return len(self)
        output = mock.Mock(IsValid=True, IsSolid=False); output.Faces = Faces(faces)
        mass = mock.Mock(Area=10.)
        create = mock.Mock(return_value=[output])
        host = {'Rhino': NS(Geometry=NS(Brep=NS(CreateFromSweep=create),
                    AreaMassProperties=NS(Compute=mock.Mock(return_value=mass)))),
                '_nurbs_curve_from_definition': mock.Mock(return_value=source),
                '_point': lambda q: q, '_xyz': lambda q: [1., 2., 3.],
                '_measure': lambda n, f: ([f() for _ in range(n)][-1], 0)}
        return host, source, duplicate, output, mass, create

    def test_strict_validation_rejects_nonfinite_oversized_and_incompatible_bases(self):
        for op in request()['operations']: validate(op)
        original = request()['operations'][0]
        for mutate in [lambda o:o.update(extra=True), lambda o:o.update(sections=[]),
                       lambda o:o.update(queries=[[True,0,0]]), lambda o:o.update(queries=[[float('nan'),0,0]]),
                       lambda o:o['rails'][0].update(degree=True), lambda o:o['rails'][0].update(knots=[0,1]),
                       lambda o:o['rails'][0].update(domain=[1,0]),
                       lambda o:o['rails'][0]['control_points'][0].update(weight=0),
                       lambda o:o['rails'][0]['control_points'][0].update(weight=2)]:
            op=copy.deepcopy(original);mutate(op)
            with self.subTest(operation=op), self.assertRaises(ValueError): validate(op)

    def test_sdk_uses_private_curve_copies_and_disposes_every_owned_output(self):
        host,source,duplicate,output,mass,create=self.host()
        op=request()['operations'][0]; value,_=run(op,host,2,request()['tolerance'])
        self.assertEqual(value['area'],10.);self.assertEqual(value['face_count'],1)
        count=len(op['rails'])+len(op['sections'])
        self.assertEqual(source.DuplicateCurve.call_count,2*count)
        self.assertEqual(source.Dispose.call_count,count)
        self.assertEqual(duplicate.Dispose.call_count,2*count)
        self.assertEqual(output.Dispose.call_count,2);self.assertEqual(mass.Dispose.call_count,2)
        args=create.call_args.args
        self.assertIs(args[0],duplicate);self.assertIs(args[1],duplicate)
        self.assertEqual(args[3:],(False,request()['tolerance']['absolute']))

    def test_sdk_failure_cleanup_covers_partial_inputs_sweep_queries_and_mass(self):
        for stage in ['source','copy','alias','sweep','empty','invalid','closest','area','area_nan']:
            host,source,duplicate,output,mass,create=self.host()
            if stage=='source': host['_nurbs_curve_from_definition'].side_effect=[source,None]
            elif stage=='copy': source.DuplicateCurve.side_effect=[duplicate,None]
            elif stage=='alias': source.DuplicateCurve.return_value=source
            elif stage=='sweep':create.side_effect=ValueError('failed')
            elif stage=='empty':create.return_value=[]
            elif stage=='invalid':output.IsValid=False
            elif stage=='closest':output.Faces[0].ClosestPoint.return_value=False,0,0
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            elif stage=='area_nan':mass.Area=float('nan')
            with self.subTest(stage=stage),self.assertRaises(ValueError):
                run(request()['operations'][0],host,1,request()['tolerance'])
            self.assertEqual(source.Dispose.call_count,1 if stage=='source' else 4)
            if stage not in ['source','copy','alias','sweep','empty']:output.Dispose.assert_called_once()
            if stage=='area_nan':mass.Dispose.assert_called_once()

    def test_actual_worker_dispatches_the_staged_probe(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_execute')
        scope={};exec(compile(ast.Module(body=[node],type_ignores=[]),'<worker>','exec'),scope)
        execute=mock.Mock(return_value=mock.sentinel.result)
        op=request()['operations'][0];tolerance=request()['tolerance']
        with mock.patch.dict('sys.modules',{'two_rail_sweep_probe':NS(run=execute)}):
            self.assertIs(scope['_execute'](op,1,tolerance),mock.sentinel.result)
        execute.assert_called_once_with(op,scope,1,tolerance)
