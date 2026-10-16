"""Bounded requests, public SDK ownership and real worker dispatch."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase, mock
from .rail_revolve_probe import request, validate, run

class RailRevolveTests(TestCase):
    def host(self):
        sources=[mock.Mock(),mock.Mock()];copies=[mock.Mock(),mock.Mock()]
        for a,b in zip(sources,copies):a.DuplicateCurve.return_value=b
        output=mock.Mock(IsValid=True);output.ClosestPoint.return_value=True,.3,.4
        mass=mock.Mock(Area=10.);create=mock.Mock(return_value=output)
        host={'Rhino':NS(Geometry=NS(Line=mock.Mock(),NurbsSurface=NS(CreateRailRevolvedSurface=create),
                     AreaMassProperties=NS(Compute=mock.Mock(return_value=mass)))),
              '_nurbs_curve_from_definition':mock.Mock(side_effect=sources),
              '_nurbs_surface_definition':mock.Mock(return_value={'degree':[2,2]}),
              '_point':lambda q:q,'_xyz':lambda q:[1.,2.,3.],
              '_measure':lambda n,f:([f() for _ in range(n)][-1],0)}
        return host,sources,copies,output,mass,create

    def test_strict_validation_before_native_allocations(self):
        for op in request()['operations']:validate(op)
        original=request()['operations'][0]
        for mutate in [lambda o:o.update(extra=True),lambda o:o.update(scale_height=1),
                       lambda o:o.update(axis=[[0,0,0],[0,0,0]]),lambda o:o.update(queries=[]),
                       lambda o:o.update(queries=[[True,0,0]]),lambda o:o.update(queries=[[float('nan'),0,0]]),
                       lambda o:o['rail'].update(degree=True),lambda o:o['rail'].update(knots=[0,1]),
                       lambda o:o['rail'].update(domain=[1,0]),
                       lambda o:o['rail']['control_points'][0].update(weight=0),
                       lambda o:o['rail']['control_points'][0].update(weight=-1)]:
            op=copy.deepcopy(original);mutate(op)
            with self.subTest(operation=op),self.assertRaises(ValueError):validate(op)

    def test_private_inputs_and_every_iteration_output_are_disposed(self):
        host,sources,copies,output,mass,create=self.host()
        value,_=run(request()['operations'][0],host,2,request()['tolerance'])
        self.assertEqual(value['definition'],{'degree':[2,2]})
        self.assertTrue(value['created']);self.assertEqual(value['area'],10.)
        for a,b in zip(sources,copies):
            self.assertEqual(a.DuplicateCurve.call_count,2);a.Dispose.assert_called_once()
            self.assertEqual(b.Dispose.call_count,2)
        self.assertEqual(output.Dispose.call_count,2);self.assertEqual(mass.Dispose.call_count,2)
        self.assertEqual(create.call_args.args[:2],tuple(copies))
        self.assertFalse(create.call_args.args[3])

    def test_failure_cleanup_and_null_construction(self):
        for stage in ['source','copy','alias','create','closest','area','area_nan','definition','null']:
            host,sources,copies,output,mass,create=self.host()
            if stage=='source':host['_nurbs_curve_from_definition'].side_effect=[sources[0],None]
            elif stage=='copy':sources[1].DuplicateCurve.return_value=None
            elif stage=='alias':sources[1].DuplicateCurve.return_value=sources[1]
            elif stage=='create':create.side_effect=ValueError('failed')
            elif stage=='closest':output.ClosestPoint.return_value=False,0,0
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            elif stage=='area_nan':mass.Area=float('nan')
            elif stage=='definition':host['_nurbs_surface_definition'].side_effect=ValueError('failed')
            elif stage=='null':create.return_value=None
            with self.subTest(stage=stage):
                if stage=='null':
                    value,_=run(request()['operations'][0],host,1,request()['tolerance'])
                    self.assertEqual(value,dict(created=False,valid=False,area=None,closest_points=[]))
                else:
                    with self.assertRaises(ValueError):run(request()['operations'][0],host,1,request()['tolerance'])
                sources[0].Dispose.assert_called_once()
                if stage!='source':sources[1].Dispose.assert_called_once()
                if stage not in ['source','copy','alias','create','null']:output.Dispose.assert_called_once()
                if stage in ['area_nan','definition']:mass.Dispose.assert_called_once()
                if stage!='source':copies[0].Dispose.assert_called_once()
                if stage not in ['source','copy','alias']:copies[1].Dispose.assert_called_once()

    def test_actual_worker_dispatches_the_staged_probe(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_execute')
        scope={};exec(compile(ast.Module(body=[node],type_ignores=[]),'<worker>','exec'),scope)
        execute=mock.Mock(return_value=mock.sentinel.result)
        op=request()['operations'][0];tolerance=request()['tolerance']
        with mock.patch.dict('sys.modules',{'rail_revolve_probe':NS(run=execute)}):
            self.assertIs(scope['_execute'](op,1,tolerance),mock.sentinel.result)
        execute.assert_called_once_with(op,scope,1,tolerance)

class ContinuousSurfaceComparisonTests(TestCase):
    def surface(self):
        return dict(control_count=[2,2],degree=[1,1],knots_u=[0.,0.,1.,1.],knots_v=[0.,0.,1.,1.],
                    control_points=[dict(point=list(p),weight=1.) for p in [(0.,0.,0.),(1.,0.,0.),(0.,1.,0.),(1.,1.,0.)]])

    def test_rigid_displacement_and_weight_perturbation_have_outward_bounds(self):
        from .rail_revolve_comparison import continuous_bound
        a=self.surface();b=copy.deepcopy(a)
        for c in b['control_points']:c['point'][2]=.125
        self.assertEqual(continuous_bound(a,b),.125)
        b['control_points'][1]['weight']+=1e-12
        self.assertGreater(continuous_bound(a,b),.125)
        for c in b['control_points']:c['weight']*=8.
        self.assertLess(continuous_bound(a,b),.125+1e-10)

    def test_chart_transpose_and_reversal_do_not_change_the_bound(self):
        from .rail_revolve_comparison import continuous_bound
        a=self.surface();b=copy.deepcopy(a)
        b['control_points']=[a['control_points'][i] for i in [2,0,3,1]]
        b['knots_u']=[42.,42.,45.,45.];b['knots_v']=[-13.,-13.,7.,7.]
        self.assertEqual(continuous_bound(a,b),0.)
        b['knots_u']=[40.,42.,45.,46.] # superfluous outer knots
        self.assertEqual(continuous_bound(a,b),0.)
        b['control_points'][0]=dict(point=[0.,0.,0.],weight=0.)
        with self.assertRaises(ValueError):continuous_bound(a,b)
