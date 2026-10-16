"""Network request limits, standalone routing and SDK ownership failures."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase,mock
from .network_surface_probe import request,validate,run

class NetworkSurfaceTests(TestCase):
    def host(self):
        count=len(request()['operations'][0]['curves'])
        sources=[mock.Mock()for _ in range(count)];copies=[mock.Mock()for _ in range(count)]
        for a,b in zip(sources,copies):a.DuplicateCurve.return_value=b
        surface=mock.Mock(IsValid=True);surface.ClosestPoint.return_value=True,.3,.4
        surface.Domain.return_value=NS(T0=0.);surface.PointAt.return_value=NS(X=1.,Y=2.,Z=3.)
        surface.Duplicate.return_value.Transform.return_value=True
        mass=mock.Mock(Area=12.);create=mock.Mock(return_value=(surface,0))
        host={'Rhino':NS(Geometry=NS(NurbsSurface=NS(CreateNetworkSurface=create),AreaMassProperties=NS(Compute=mock.Mock(return_value=mass)),Transform=NS(Translation=mock.Mock()))),
              '_nurbs_curve_from_definition':mock.Mock(side_effect=sources),'_nurbs_surface_definition':mock.Mock(return_value={'degree':[3,3]}),
              '_point':lambda q:q,'_xyz':lambda q:[1.,2.,3.],'_measure':lambda n,f:([f() for _ in range(n)][-1],0)}
        return host,sources,copies,surface,mass,create

    def test_bounded_validation_rejects_bad_layout_nonfinite_values_and_oversized_requests(self):
        for op in request()['operations']:validate(op)
        original=request()['operations'][0]
        for mutate in [lambda o:o.update(extra=True),lambda o:o.update(curves=[]),lambda o:o.update(curves=o['curves']*17),
                       lambda o:o.update(queries=[]),lambda o:o.update(queries=[[float('inf'),0,0]]),lambda o:o.update(queries=[[True,0,0]]),
                       lambda o:o.update(edge_tolerance=0),lambda o:o.update(interior_tolerance=float('nan')),
                       lambda o:o['curves'][0].update(degree=True),lambda o:o['curves'][0].update(knots=[0,1]),
                       lambda o:o['curves'][0].update(domain=[1,0]),lambda o:o['curves'][0]['control_points'][0].update(weight=-1)]:
            op=copy.deepcopy(original);mutate(op)
            with self.subTest(operation=op),self.assertRaises(ValueError):validate(op)
        op=copy.deepcopy(original);curve=dict(degree=1,control_points=[dict(point=[i,0,0],weight=1.)for i in range(128)],knots=[0.,0.]+[i/127. for i in range(1,127)]+[1.,1.]);op['curves']=[curve]*17
        with self.assertRaisesRegex(ValueError,'control limit'):validate(op)

    def test_sdk_auto_sorts_only_private_copies_and_disposes_every_iteration(self):
        host,sources,copies,surface,mass,create=self.host();op=request()['operations'][0]
        value,_=run(op,host,2,request()['tolerance'])
        self.assertTrue(value['created']);self.assertEqual(value['area'],12.)
        self.assertNotIn('source_samples',value)
        self.assertEqual(create.call_args.args,(copies,1,op['edge_tolerance'],op['interior_tolerance'],request()['tolerance']['angular']))
        for a,b in zip(sources,copies):a.Dispose.assert_called_once();self.assertEqual(a.DuplicateCurve.call_count,2);self.assertEqual(b.Dispose.call_count,2)
        self.assertEqual(surface.Dispose.call_count,2);self.assertEqual(mass.Dispose.call_count,2)
        self.assertEqual(surface.Duplicate.return_value.Dispose.call_count,2)
        host['Rhino'].Geometry.Transform.Translation.assert_called_with(-1.,-2.,-3.)

    def test_source_sample_fractions_reject_invalid_or_unbounded_layouts(self):
        op=request()['operations'][0]
        for samples in [None,True,[True],[-.1],[1.1],[float('nan')],[.5,.5],[1.,0.],[i/65. for i in range(66)]]:
            changed=copy.deepcopy(op);changed['source_sample_fractions']=samples
            with self.subTest(samples=samples),self.assertRaisesRegex(ValueError,'source sample fractions'):
                validate(changed)
        for samples in [[],[.5],[0.,.25,.5,1.]]:
            changed=copy.deepcopy(op);changed['source_sample_fractions']=samples;validate(changed)

    def source_host(self):
        host,sources,copies,surface,mass,create=self.host()
        for source in sources:
            source.Domain=NS(ParameterAt=lambda s:-3.+10.*s)
            source.PointAt.return_value=NS(IsValid=True)
        surface.Domain.return_value=NS(T0=0.,NormalizedParameterAt=lambda t:t)
        surface.PointAt.return_value=mock.Mock(IsValid=True)
        surface.PointAt.return_value.X=1.;surface.PointAt.return_value.Y=2.;surface.PointAt.return_value.Z=3.
        surface.PointAt.return_value.DistanceTo.return_value=2e-5
        host['_checked_closest_parameter']=lambda domain,t:t
        return host,sources,copies,surface,mass,create

    def test_source_distances_use_original_domains_and_keep_failures_out_of_output(self):
        op=copy.deepcopy(request()['operations'][0]);op['source_sample_fractions']=[0.,.25,1.]
        host,sources,copies,surface,mass,create=self.source_host()
        before=copy.deepcopy(op);value,_=run(op,host,1,request()['tolerance'])
        rows=value['source_samples'];self.assertEqual(len(rows),len(sources)*3)
        self.assertEqual([r['source_parameter']for r in rows[:3]],[-3.,-.5,7.])
        self.assertEqual([r['source_curve_index']for r in rows],[i for i in range(len(sources))for _ in range(3)])
        self.assertTrue(all(r['distance']==2e-5 and r['normalized_parameters']==[.3,.4]for r in rows))
        self.assertEqual(op,before)
        for source in sources:
            source.Dispose.assert_called_once()
            self.assertEqual(source.PointAt.call_args_list,[mock.call(-3.),mock.call(-.5),mock.call(7.)])
        surface.Dispose.assert_called_once()
        for failure in ['search','distance','source','closest']:
            host,sources,copies,surface,mass,create=self.source_host()
            if failure=='search':surface.ClosestPoint.side_effect=[(True,.3,.4)]*len(op['queries'])+[(False,0,0)]
            elif failure=='distance':surface.PointAt.return_value.DistanceTo.return_value=float('nan')
            elif failure=='source':sources[0].PointAt.return_value.IsValid=False
            else:surface.PointAt.return_value.IsValid=False
            with self.subTest(failure=failure),self.assertRaises(ValueError):run(op,host,1,request()['tolerance'])
            surface.Dispose.assert_called_once()
            for source in sources:source.Dispose.assert_called_once()

    def test_failure_cleanup_includes_partial_input_copies_queries_and_definitions(self):
        for stage in ['source','copy','alias','create','null','error','closest','area_copy','area_alias','area_transform','area','area_nan','definition']:
            host,sources,copies,surface,mass,create=self.host()
            if stage=='source':host['_nurbs_curve_from_definition'].side_effect=[sources[0],None]
            elif stage=='copy':sources[1].DuplicateCurve.return_value=None
            elif stage=='alias':sources[1].DuplicateCurve.return_value=sources[1]
            elif stage=='create':create.side_effect=ValueError('failed')
            elif stage=='null':create.return_value=None,3
            elif stage=='error':create.return_value=surface,3
            elif stage=='closest':surface.ClosestPoint.return_value=False,0,0
            elif stage=='area_copy':surface.Duplicate.return_value=None
            elif stage=='area_alias':surface.Duplicate.return_value=surface
            elif stage=='area_transform':surface.Duplicate.return_value.Transform.return_value=False
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            elif stage=='area_nan':mass.Area=float('nan')
            elif stage=='definition':host['_nurbs_surface_definition'].side_effect=ValueError('failed')
            with self.subTest(stage=stage):
                if stage=='null':
                    value,_=run(request()['operations'][0],host,1,request()['tolerance']);self.assertFalse(value['created']);self.assertEqual(value['error'],3)
                else:
                    with self.assertRaises(ValueError):run(request()['operations'][0],host,1,request()['tolerance'])
                for i,source in enumerate(sources):self.assertEqual(source.Dispose.call_count,1 if stage!='source' or i==0 else 0)
                if stage not in ['source','copy','alias','create','null']:surface.Dispose.assert_called_once()
                if stage in ['area_nan','definition']:mass.Dispose.assert_called_once()
                if stage in ['area_transform','area','area_nan','definition']:surface.Duplicate.return_value.Dispose.assert_called_once()
                if stage!='source':copies[0].Dispose.assert_called_once()
                if stage not in ['source','copy','alias']:
                    for c in copies:c.Dispose.assert_called_once()

    def test_actual_worker_dispatches_the_staged_sdk_probe(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text());node=next(n for n in tree.body if isinstance(n,ast.FunctionDef)and n.name=='_execute')
        scope={};exec(compile(ast.Module(body=[node],type_ignores=[]),'<worker>','exec'),scope)
        execute=mock.Mock(return_value=mock.sentinel.result);op=request()['operations'][0];tolerance=request()['tolerance']
        with mock.patch.dict('sys.modules',{'network_surface_probe':NS(run=execute)}):self.assertIs(scope['_execute'](op,1,tolerance),mock.sentinel.result)
        execute.assert_called_once_with(op,scope,1,tolerance)
