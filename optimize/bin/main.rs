use optimize::{parse_img, CellStorage, MapStorage, MapTrait, PathFinder, Point, Visited};

#[allow(unused_must_use)]
fn main() -> Result<(), anyhow::Error> {
    let img = image::open("data/maze-03_6_threshold.png")?;
    let map = parse_img(&img)?;

    // implement brute force breadth-first search within the validity map
    println!("{}", map);

    let visited = map.create_storage();

    let (res, visited) = PathFinder::new(
        Point { row: 14, col: 0 },
        Point { row: 44, col: 51 },
        visited,
    )
    .finish(&map);

    dbg!(res);

    // a bit hacky for now to get the visited storage back into the concrete type
    // TODO: might help to have the reference type as generic argument to the map instead...
    let visited = visited
        .as_any()
        .downcast_ref::<CellStorage<Visited<Point>>>()
        .expect("Wasn't a CellStorage<Visited<Point>>!");

    println!("{}", visited);

    Ok(())
}
