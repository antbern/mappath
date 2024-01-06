use std::{collections::VecDeque, error::Error, fmt::Display};
use anyhow::anyhow;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Cell {
    Invalid,
    Valid,
}

impl Display for Cell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Cell::Invalid => "X",
                Cell::Valid => " ",
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
            cells: vec![vec![default_value; other.rows]; other.columns],
        }
    }
}

fn main() {
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
            vec![Invalid, Valid, Invalid, Valid, Valid, Valid, Invalid],
            vec![Invalid, Valid, Invalid, Valid, Invalid, Invalid, Invalid],
            vec![Invalid, Valid, Valid, Valid, Valid, Valid, Valid],
            vec![
                Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
            ],
        ],
    };

    // implement brute force breadth-first search within the validity map
    println!("{}", map);

    let res = find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 });

    dbg!(res);
}

fn find_path(map: &Map<Cell>, start: Point, goal: Point) -> Result<usize, Box<dyn Error>> {
    // for keeping track of the cost up to the point and the point itself to visit
    let mut visit_list: VecDeque<(usize, Point)> = VecDeque::new();

    // to keep track of where we have been
    let mut visited = Map::new_as(map, false);

    visit_list.push_back((0, start));

    let mut path_cost: Option<usize> = None;

    while let Some((cost, p)) = visit_list.pop_front() {
        // we have a point to process, find the valid neighbors to visit next

        if visited.get(p) {
            continue;
        }

        *visited.get_mut(p) = true;

        // if this is the goal, we are done! (and should probably do some back-tracking)
        if p == goal {
            println!("FOUND GOAL!: cost={}", cost);
            path_cost = Some(cost);
            break;
        }

        for point in map.neighbors_four(p) {
            let c = map.get(point);

            if c == Cell::Valid && !visited.get(point) {
                visit_list.push_back((cost + 1, point));
            }
        }
    }
    //else {
    //    println!("Exited without finding the goal!");
    //}

    println!("{}", visited);
    path_cost.ok_or(anyhow!("").into())
}
