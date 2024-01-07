use anyhow::anyhow;
use core::panic;
use image::GenericImageView;
use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    error::Error,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Cell {
    Invalid,
    Valid,
    Cost(usize),
}

impl Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Cell::Invalid => "X",
                Cell::Valid => " ",
                Cell::Cost(_) => "$",
            }
        )
    }
}

struct Map<T> {
    rows: usize,
    columns: usize,
    cells: Vec<Vec<T>>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Point {
    row: usize,
    col: usize,
}

impl Point {}

impl<T: Display> Display for Map<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in &self.cells {
            for cell in row {
                write!(f, "{}", cell)?;
            }
            write!(f, "\n")?;
        }

        Ok(())
    }
}

impl<T: Copy> Map<T> {
    fn get(&self, point: Point) -> T {
        self.cells[point.row][point.col]
    }
    fn get_mut(&mut self, point: Point) -> &mut T {
        &mut self.cells[point.row][point.col]
    }
    /// Returns the neighbouring points for the given point
    /// Only valid points inside the map will be returned
    fn neighbors_four(&self, point: Point) -> impl Iterator<Item = Point> {
        let mut points = Vec::with_capacity(4);

        if point.row > 0 {
            points.push(Point {
                row: point.row - 1,
                col: point.col,
            });
        }
        if point.col > 0 {
            points.push(Point {
                col: point.col - 1,
                row: point.row,
            });
        }

        if point.row < self.rows - 1 {
            points.push(Point {
                row: point.row + 1,
                col: point.col,
            });
        }
        if point.col < self.columns - 1 {
            points.push(Point {
                col: point.col + 1,
                row: point.row,
            });
        }
        points.into_iter()
    }
    /// Create a new Map with the same dimensions as another map
    fn new_as<S>(other: &Map<S>, default_value: T) -> Map<T> {
        Map {
            rows: other.rows,
            columns: other.columns,
            cells: vec![vec![default_value; other.columns]; other.rows],
        }
    }
}

fn load_image() -> Result<Map<Cell>, Box<dyn Error>> {
    let img = image::open("data/maze-03_6_threshold.png")?;

    let width = img.width() as usize;
    let height = img.height() as usize;

    let mut cells = vec![vec![Cell::Invalid; width as usize]; height as usize];

    for row in 0..height {
        for col in 0..width {
            let p = img.get_pixel(col as u32, row as u32);

            cells[row][col] = if p.0[0] < 128 {
                Cell::Invalid
            } else {
                Cell::Valid
            }
        }
    }

    Ok(Map {
        rows: height,
        columns: width,
        cells,
    })
}

#[allow(unused_must_use)]
fn main() -> Result<(), Box<dyn Error>> {
    // TODO: load image, convert to "validity mask"

    // construct hard-coded for now
    use Cell::*;
    let map: Map<Cell> = Map {
        rows: 7,
        columns: 7,
        cells: vec![
            vec![
                Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
            ],
            vec![Invalid, Valid, Invalid, Invalid, Invalid, Valid, Invalid],
            vec![Invalid, Valid, Invalid, Invalid, Invalid, Valid, Invalid],
            vec![Invalid, Valid, Cost(2), Valid, Valid, Valid, Invalid],
            vec![Invalid, Valid, Invalid, Valid, Invalid, Invalid, Invalid],
            vec![Invalid, Valid, Valid, Valid, Valid, Valid, Valid],
            vec![
                Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
            ],
        ],
    };

    let map = load_image()?;

    // implement brute force breadth-first search within the validity map
    println!("{}", map);

    let res = find_path(&map, Point { row: 14, col: 0 }, Point { row: 44, col: 51 });

    dbg!(res);
    Ok(())
}

#[derive(Eq)]
struct ToVisit {
    cost: usize,
    point: Point,
    from: Option<Point>,
}

impl Ord for ToVisit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cost.cmp(&other.cost).reverse() // reverse for BinaryHeap to be a min-heap
    }
}

impl PartialOrd for ToVisit {
    fn partial_cmp(&self, other: &ToVisit) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ToVisit {
    fn eq(&self, other: &ToVisit) -> bool {
        self.cost == other.cost
    }
}

#[derive(Clone, Copy)]
struct VisitedItem {
    cost: usize,
    from: Option<Point>,
}

#[derive(Clone, Copy)]
struct Visited(Option<VisitedItem>);

impl Deref for Visited {
    type Target = Option<VisitedItem>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Visited {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Display for Visited {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(item) => write!(f, "{:03} ", item.cost),
            None => write!(f, "{:03} ", ""),
        }
    }
}

#[derive(Debug, PartialEq)]
struct PathResult {
    path: Vec<Point>,
    total_cost: usize,
}

fn find_path(map: &Map<Cell>, start: Point, goal: Point) -> Result<PathResult, Box<dyn Error>> {
    // for keeping track of the cost up to the point and the point itself to visit
    // always prioritize visiting the lowest-cost ones, hence use a binary heap as a priority queue
    let mut visit_list: BinaryHeap<ToVisit> = BinaryHeap::new();

    // to keep track of where we have been
    let mut visited: Map<Visited> = Map::new_as(map, Visited(None));

    visit_list.push(ToVisit {
        cost: 0,
        point: start,
        from: None,
    });

    let mut result: Option<PathResult> = None;

    while let Some(visit) = visit_list.pop() {
        // we have a point to process, find the valid neighbors to visit next

        if visited.get(visit.point).is_some() {
            continue;
        }

        *visited.get_mut(visit.point) = Visited(Some(VisitedItem {
            cost: visit.cost,
            from: visit.from,
        }));

        // if this is the goal, we are done! (and should probably do some back-tracking to find the actual shortest path...)
        if visit.point == goal {
            println!("FOUND GOAL!: cost={}", visit.cost);

            // backtrack to find the total shortest path
            let mut path: Vec<Point> = Vec::new();
            path.push(goal);

            let mut previous_visit = visited.get(goal);

            loop {
                previous_visit = match previous_visit {
                    Visited(Some(VisitedItem {
                        cost: _,
                        from: None,
                    })) => {
                        // we found the starting point, we are done
                        break;
                    }
                    Visited(Some(VisitedItem {
                        cost: _,
                        from: Some(from),
                    })) => {
                        path.push(from);
                        visited.get(from)
                    }
                    Visited(None) => panic!("Backtracking lead to a Point that was never visited"),
                }
            }

            path.reverse();

            result = Some(PathResult {
                path: path,
                total_cost: visit.cost,
            });
            break;
        }

        for point in map.neighbors_four(visit.point) {
            let c = map.get(point);

            if c != Cell::Invalid && !visited.get(point).is_some() {
                let move_cost = if let Cell::Cost(c) = c { c } else { 1 };

                visit_list.push(ToVisit {
                    cost: visit.cost + move_cost,
                    point: point,
                    from: Some(visit.point),
                });
            }
        }
    }

    println!("{}", visited);
    result.ok_or(anyhow!("").into())
}

#[cfg(test)]
mod test {

    use super::*;

    fn create_basic_map() -> Map<Cell> {
        use Cell::*;
        Map {
            rows: 7,
            columns: 7,
            cells: vec![
                vec![
                    Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
                ],
                vec![Invalid, Valid, Invalid, Invalid, Invalid, Valid, Invalid],
                vec![Invalid, Valid, Invalid, Invalid, Invalid, Valid, Invalid],
                vec![Invalid, Valid, Invalid, Valid, Valid, Valid, Invalid],
                vec![Invalid, Valid, Invalid, Valid, Invalid, Invalid, Invalid],
                vec![Invalid, Valid, Valid, Valid, Valid, Valid, Valid],
                vec![
                    Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
                ],
            ],
        }
    }

    #[test]
    fn test_basic_route() {
        let map = create_basic_map();

        // test the basic case
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 }),
            Ok(PathResult { total_cost: 12, .. })
        ));
    }
    #[test]
    fn test_basic_no_route() {
        let map = create_basic_map();

        // no route to target
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 0, col: 5 }),
            Err(_)
        ));
    }

    #[test]
    fn test_basic_shortcut() {
        let mut map = create_basic_map();

        // create higher cost shortcut
        map.cells[3][2] = Cell::Cost(2);
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 }),
            Ok(PathResult { total_cost: 9, .. })
        ));

        map.cells[3][2] = Cell::Cost(4);
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 }),
            Ok(PathResult { total_cost: 11, .. })
        ));

        map.cells[3][2] = Cell::Cost(10);
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 }),
            Ok(PathResult { total_cost: 12, .. })
        ));
    }
}
