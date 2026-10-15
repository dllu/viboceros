"""Public SDK calls, ownership and isolated worker routing; no licensed capture."""
import ast
import copy
from pathlib import Path
from types import SimpleNamespace as NS
from unittest import TestCase,mock
from .divide_creases_probe import request,validate,run


class Faces(list):
    @property
    def Count(self): return len(self)


class CreaseTests(TestCase):
    def host(self):
        surface=mock.Mock();source=mock.Mock();output=mock.Mock(IsValid=True,IsSolid=False)
        surface.ToBrep.return_value=source;source.Faces.Count=1;source.DuplicateBrep.return_value=output
        face=NS(Loops=NS(Count=1));output.Faces=Faces([face])
        output.Faces.SplitKinkyFaces=mock.Mock(side_effect=lambda *args:output.Faces.append(face))
        output.Faces.SplitFacesAtTangents=mock.Mock(side_effect=lambda:output.Faces.append(face))
        output.Edges=[NS(AdjacentFaces=lambda:[0]) for _ in range(6)]+[NS(AdjacentFaces=lambda:[0,1])]
        mass=mock.Mock(Area=4.)
        host={'_nurbs_surface_from_definition':mock.Mock(return_value=surface),
              'Rhino':NS(Geometry=NS(AreaMassProperties=NS(Compute=mock.Mock(return_value=mass)))),
              '_measure':lambda n,f:([f() for _ in range(n)][-1],0)}
        return host,surface,source,output,mass

    def test_strict_surface_and_option_validation_precedes_sdk_access(self):
        for op in request()['operations']:validate(op)
        original=request()['operations'][0]
        for updates in [dict(extra=True),dict(op='split'),dict(id=True),dict(split_at_kinks=1),dict(as_brep='Yes'),dict(surface=None)]:
            op=copy.deepcopy(original);op.update(updates)
            with self.subTest(updates=updates),self.assertRaises(ValueError):validate(op)
        for key,value in [('degree_u',True),('control_point_count_v',2.0),('knots_u',[0,0,2,1,1]),('knots_u',[0,0,1,float('nan'),2]),
                          ('control_points',[]),('control_points',[dict(point=[True,0,0],weight=1)]*6),
                          ('control_points',[dict(point=[0,0,0],weight=float('inf'))]*6),('domain_u',[0,1])]:
            op=copy.deepcopy(original);op['surface'][key]=value
            with self.subTest(key=key,value=value),self.assertRaises(ValueError):validate(op)

    def test_kink_and_tangent_sdk_calls_are_independent_and_dispose_owned_data(self):
        for k,t in [(True,False),(False,True),(True,True),(False,False)]:
            host,surface,source,output,mass=self.host()
            op=copy.deepcopy(request()['operations'][0]);op.update(split_at_kinks=k,split_at_tangents=t)
            value,_=run(op,host,1,request()['tolerance'])
            self.assertEqual(value['added_faces'],int(k)+int(t))
            self.assertEqual(value['face_count'],1+int(k)+int(t))
            self.assertEqual((value['naked_edges'],value['shared_edges']),(6,1))
            self.assertEqual(output.Faces.SplitKinkyFaces.call_count,int(k))
            if k:output.Faces.SplitKinkyFaces.assert_called_once_with(1e-8,True)
            self.assertEqual(output.Faces.SplitFacesAtTangents.call_count,int(t))
            for obj in (surface,source,output,mass):obj.Dispose.assert_called_once()
            self.assertEqual(source.Faces.Count,1)

    def test_each_iteration_has_a_separate_owned_brep_copy(self):
        host,surface,source,output,mass=self.host()
        second=mock.Mock(IsValid=True,IsSolid=False)
        second.Faces=Faces([NS(Loops=NS(Count=1))]);second.Faces.SplitKinkyFaces=mock.Mock()
        second.Edges=[]
        source.DuplicateBrep.side_effect=[output,second]
        run(request()['operations'][0],host,2,request()['tolerance'])
        output.Dispose.assert_called_once();second.Dispose.assert_called_once()
        self.assertEqual(source.DuplicateBrep.call_count,2)
        self.assertEqual(mass.Dispose.call_count,2)
        source.Dispose.assert_called_once();surface.Dispose.assert_called_once()

    def test_failure_cleanup_keeps_nulls_aliases_and_owned_objects_distinct(self):
        for stage in ('surface','brep','brep_alias','copy','copy_alias','kink','tangent','invalid','solid','area','nonfinite_area','record'):
            host,surface,source,output,mass=self.host()
            op=copy.deepcopy(request()['operations'][0]);op['split_at_tangents']=True
            if stage=='surface':host['_nurbs_surface_from_definition'].return_value=None
            elif stage=='brep':surface.ToBrep.return_value=None
            elif stage=='brep_alias':surface.ToBrep.return_value=surface
            elif stage=='copy':source.DuplicateBrep.return_value=None
            elif stage=='copy_alias':source.DuplicateBrep.return_value=source
            elif stage=='kink':output.Faces.SplitKinkyFaces.side_effect=ValueError('native failure')
            elif stage=='tangent':output.Faces.SplitFacesAtTangents.side_effect=ValueError('native failure')
            elif stage=='invalid':output.IsValid=False
            elif stage=='solid':output.IsSolid=True
            elif stage=='area':host['Rhino'].Geometry.AreaMassProperties.Compute.return_value=None
            elif stage=='nonfinite_area':mass.Area=float('nan')
            else:output.Edges=[NS(AdjacentFaces=mock.Mock(side_effect=ValueError('record failed')))]
            with self.subTest(stage=stage),self.assertRaises(ValueError):run(op,host,1,request()['tolerance'])
            if stage!='surface':surface.Dispose.assert_called_once()
            if stage not in ('surface','brep','brep_alias'):source.Dispose.assert_called_once()
            if stage not in ('surface','brep','brep_alias','copy','copy_alias'):output.Dispose.assert_called_once()
            else:output.Dispose.assert_not_called()
            if stage in ('nonfinite_area','record'):mass.Dispose.assert_called_once()

    def test_actual_worker_routes_to_the_staged_companion(self):
        tree=ast.parse(Path(__file__).with_name('rhino_worker.py').read_text())
        node=next(n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name=='_execute')
        scope={};exec(compile(ast.Module(body=[node],type_ignores=[]),'<actual worker>','exec'),scope)
        execute=mock.Mock(return_value=mock.sentinel.observation)
        op=request()['operations'][0];tolerance=request()['tolerance']
        with mock.patch.dict('sys.modules',{'divide_creases_probe':NS(run=execute)}):
            self.assertIs(scope['_execute'](op,2,tolerance),mock.sentinel.observation)
        execute.assert_called_once_with(op,scope,2,tolerance)
