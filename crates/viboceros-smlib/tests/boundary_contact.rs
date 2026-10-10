#![cfg(feature = "native")]
use viboceros_geometry::{Point3, Tolerance};
use viboceros_smlib::Solid;
fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}
#[test]
fn boundary_contact_separates_intersections_containment_and_disjoint_inputs() {
    let a = Solid::box_solid(p(0., 0., 0.), [10.; 3]).unwrap();
    let before = a.properties(1e-8).unwrap();
    let partial = Solid::sphere(p(5., 5., 10.), 2.).unwrap();
    assert!(a.boundary_contact(&partial, Tolerance::DEFAULT).unwrap());
    let inside = Solid::sphere(p(5., 5., 5.), 2.).unwrap();
    assert!(!a.boundary_contact(&inside, Tolerance::DEFAULT).unwrap());
    let remote = Solid::sphere(p(30., 0., 0.), 2.).unwrap();
    assert!(!a.boundary_contact(&remote, Tolerance::DEFAULT).unwrap());
    let corner = Solid::box_solid(p(10., 10., 10.), [2.; 3]).unwrap();
    assert!(!a.boundary_contact(&corner, Tolerance::DEFAULT).unwrap());
    assert_eq!(a.properties(1e-8).unwrap(), before);
}
