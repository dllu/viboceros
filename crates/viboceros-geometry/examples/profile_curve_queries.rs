//! Single-threaded allocation diagnostic, not a timing or allocation test gate.
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use viboceros_geometry::{NurbsCurve, Point3, Tolerance, WeightedPoint3};

struct CountingAllocator;
static CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

// SAFETY: Every allocation operation delegates to System with unchanged
// pointer/layout arguments. Counters never allocate or change ownership.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(size as u64, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn measure<T>(name: &str, mut query: impl FnMut() -> T) {
    const ITERATIONS: u64 = 500;
    drop(black_box(query()));
    CALLS.store(0, Ordering::Relaxed);
    BYTES.store(0, Ordering::Relaxed);
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        drop(black_box(query()));
    }
    let elapsed = start.elapsed().as_nanos() as f64 / ITERATIONS as f64;
    let calls = CALLS.load(Ordering::Relaxed) as f64 / ITERATIONS as f64;
    let bytes = BYTES.load(Ordering::Relaxed) as f64 / ITERATIONS as f64;
    println!("{name}: {calls:.0} allocations, {bytes:.0} requested bytes, {elapsed:.1} ns/query");
}

fn p(x: f64, y: f64, z: f64) -> Point3 {
    Point3::try_new(x, y, z).unwrap()
}

fn main() {
    let circle = NurbsCurve::try_new_rational(
        2,
        [(1., 0., 1.), (1., 1., 0.5_f64.sqrt()), (0., 1., 1.)]
            .map(|(x, y, w)| WeightedPoint3::try_new(p(x, y, 0.), w).unwrap())
            .to_vec(),
        vec![0., 0., 0., 1., 1., 1.],
    )
    .unwrap();
    let multispan = NurbsCurve::try_new_rational(
        3,
        [
            (-5., 0., 1.),
            (-4., 7., 0.8),
            (-1., -6., 1.7),
            (1., 6., 0.6),
            (4., -7., 1.4),
            (6., 1., 1.),
            (9., 3., 0.9),
        ]
        .map(|(x, y, w)| WeightedPoint3::try_new(p(x, y, 0.), w).unwrap())
        .to_vec(),
        vec![-3., -3., -3., -3., -2.4, 0.75, 2.8, 4., 4., 4., 4.],
    )
    .unwrap();
    let line = NurbsCurve::try_new(
        1,
        vec![p(-2., 3., 1.), p(4., 3., -2.)],
        vec![-5., -5., 7., 7.],
    )
    .unwrap();
    for (name, curve, target) in [
        ("circle", circle, p(2_f64.sqrt(), 2_f64.sqrt(), 0.)),
        ("multispan", multispan, p(4.7, -1.1, 0.3)),
        ("line", line, p(-9., 1., 5.)),
    ] {
        println!("{name}");
        let domain = curve.domain();
        let parameter = *domain.start() * 0.57 + *domain.end() * 0.43;
        measure("point", || curve.evaluate(black_box(parameter)).unwrap());
        measure("second jet", || {
            curve
                .evaluate_with_second_derivative(black_box(parameter))
                .unwrap()
        });
        measure("closest", || {
            curve
                .closest_parameter(black_box(target), Tolerance::DEFAULT)
                .unwrap()
        });
    }
}
