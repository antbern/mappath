use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use optimize::{
    find::{MapTrait, PathFinder, PathFinderState},
    grid::{GridMap, Point},
    util::parse_img,
};

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

pub fn map_scaled_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("map_scaled_factor");
    for factor in [1, 2, 3, 4, 5, 6, 7, 8].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(factor), factor, |b, &factor| {
            let (map, start, goal) = load_base_map_scaled(factor);
            b.iter_batched(
                || map.create_storage(),
                |storage| {
                    let (res, _) =
                        PathFinder::new(black_box(start), black_box(goal), black_box(storage), ())
                            .finish(&map);
                    assert!(matches!(res, PathFinderState::PathFound(_)));
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, map_scaled_factor);
criterion_main!(benches);
