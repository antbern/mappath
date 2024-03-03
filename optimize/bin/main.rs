use optimize::{util::parse_img, MapTrait, PathFinder, Point};

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
    println!("{}", visited);

    Ok(())
}
