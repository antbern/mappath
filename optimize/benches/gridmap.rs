use criterion::{black_box, criterion_group, criterion_main, Criterion};
use optimize::{util::parse_img, GridMap, MapTrait, PathFinder, PathFinderState, Point};

fn load_base_map_scaled(factor: usize) -> (GridMap<usize>, Point, Point) {
    let img = image::open("../data/maze-03_6_threshold.png").unwrap();
    let mut map = parse_img(&img).unwrap();
    let mut start = Point { row: 14, col: 0 };
    let mut goal = Point { row: 44, col: 51 };

    map.scale_up(factor);
    start.row *= factor;
    start.col *= factor;
    goal.row *= factor;
    goal.col *= factor;

    (map, start, goal)
}

fn bench_map_scaled(c: &mut Criterion, factor: usize) {
    let (map, start, goal) = load_base_map_scaled(factor);

    c.bench_function(&format!("map_scaled_{}", factor), |b| {
        b.iter(|| {
            let (res, _) = PathFinder::new(
                black_box(start),
                black_box(goal),
                black_box(map.create_storage()),
            )
            .finish(&map);
            assert!(matches!(res, PathFinderState::PathFound(_)));
        })
    });
}

pub fn map_small(c: &mut Criterion) {
    bench_map_scaled(c, 1);
}

pub fn map_medium(c: &mut Criterion) {
    bench_map_scaled(c, 2);
}

pub fn map_large(c: &mut Criterion) {
    bench_map_scaled(c, 4);
}

criterion_group!(benches, map_small, map_medium, map_large);
criterion_main!(benches);
