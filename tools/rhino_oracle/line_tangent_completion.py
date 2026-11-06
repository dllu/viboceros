"""Repeat an arc boundary observation and complete Planar/number recipes."""
if __package__:from .line_tangent_cases import case,CIRCLE,ARC,SPATIAL
else:from line_tangent_cases import case,CIRCLE,ARC,SPATIAL
COMPLETION={
 'arc_middle_repeat_a':case(ARC,[3.5355339059327378,3.5355339059327378,0],[-8,6,0]),
 'arc_middle_repeat_b':case(ARC,[3.5355339059327378,3.5355339059327378,0],[-8,6,0]),
 'arc_middle_other_side':case(ARC,[3.5355339059327378,3.5355339059327378,0],[-6,8,0]),
 'planar_circle_complete':dict(case(CIRCLE,[4.99,.2,0],[8,6,2]),planar=True),
 'planar_circle_elevated':dict(case(dict(CIRCLE,center=[0,0,3]),[4.99,.2,3],[8,6,9]),planar=True),
 'planar_spatial_exact':dict(case(SPATIAL,[4.5,3,2],[8.32,4.08,3.16]),planar=True),
 'oncurve_both_negative':dict(curves=[CIRCLE],actions=['Tangent','PointOnCurve',dict(curve=0,pick=[4.99,.2,0]),'BothSides',[3,4,0],-5]),
 'oncurve_both_positive':dict(curves=[CIRCLE],actions=['Tangent','PointOnCurve',dict(curve=0,pick=[4.99,.2,0]),'BothSides',[3,4,0],5]),
 'oncurve_negative':dict(curves=[CIRCLE],actions=['Tangent','PointOnCurve',dict(curve=0,pick=[4.99,.2,0]),[3,4,0],-5]),
 'fixed_negative':case(CIRCLE,[4.99,.2,0],'FromFirstPoint',-5),
 'point_end_enter':case(CIRCLE,[4.99,.2,0],'Point','Enter',[8,6,0]),
}
