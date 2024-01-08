use anyhow::anyhow;
use core::panic;
use image::GenericImageView;
use std::{
    any::Any,
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

/// Supertrait that collects all the requirements on the NodeReference values
/// Must be copy, comparable and not references (hence 'static)
trait NodeReference: Copy + Eq + 'static {}

trait MapTrait {
    /// The type that can be used to reference nodes in the map
    type Reference: NodeReference;

    /// Return an iterator over the neighbors of the provided node and the cost required to go there
    fn neighbors_of(&self, node: Self::Reference)
        -> impl Iterator<Item = (Self::Reference, usize)>;

    /// Create a storage for values of type T
    fn create_storage<T: Copy + 'static>(
        &self,
        default_value: T,
    ) -> impl MapStorage<T, Reference = Self::Reference>;
}

trait MapStorage<T> {
    type Reference: NodeReference;

    fn get(&self, node: Self::Reference) -> T;
    fn get_mut(&mut self, node: Self::Reference) -> &mut T;

    fn as_any(&self) -> &dyn Any;
}

/// A MapTrait implementation that uses a rectangular grid of cells
struct Map {
    rows: usize,
    columns: usize,
    cells: Vec<Vec<Cell>>,
}

/// A MapStorage that uses a rectangular grid of cells (a vec in a vec)
// TODO: change from vec of vec to one single vec -> better cache friendlyness!
#[derive(Debug)]
struct CellStorage<T>(Vec<Vec<T>>);

impl<T: Copy + 'static> MapStorage<T> for CellStorage<T> {
    type Reference = Point;

    fn get(&self, node: Self::Reference) -> T {
        self.0[node.row][node.col]
    }

    fn get_mut(&mut self, node: Self::Reference) -> &mut T {
        &mut self.0[node.row][node.col]
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<T: Display> Display for CellStorage<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for row in &self.0 {
            for cell in row {
                write!(f, "{}", cell)?;
            }
            write!(f, "\n")?;
        }

        Ok(())
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct Point {
    row: usize,
    col: usize,
}

impl NodeReference for Point {}

impl Display for Map {
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

impl MapTrait for Map {
    type Reference = Point;

    fn neighbors_of(
        &self,
        node: Self::Reference,
    ) -> impl Iterator<Item = (Self::Reference, usize)> {
        let mut points = Vec::with_capacity(4);

        let c = self.cells[node.row][node.col];

        if c == Cell::Invalid {
            return points.into_iter();
        }

        let move_cost = if let Cell::Cost(cost) = c { cost } else { 1 };

        if node.row > 0 {
            points.push((
                Point {
                    row: node.row - 1,
                    col: node.col,
                },
                move_cost,
            ));
        }
        if node.col > 0 {
            points.push((
                Point {
                    col: node.col - 1,
                    row: node.row,
                },
                move_cost,
            ));
        }

        if node.row < self.rows - 1 {
            points.push((
                Point {
                    row: node.row + 1,
                    col: node.col,
                },
                move_cost,
            ));
        }
        if node.col < self.columns - 1 {
            points.push((
                Point {
                    col: node.col + 1,
                    row: node.row,
                },
                move_cost,
            ));
        }

        // filter to only keep valid cells
        points.retain(|(p, _)| self.cells[p.row][p.col] != Cell::Invalid);

        points.into_iter()
    }

    fn create_storage<T: Copy + 'static>(
        &self,
        default_value: T,
    ) -> impl MapStorage<T, Reference = Self::Reference> {
        CellStorage(vec![vec![default_value; self.columns]; self.rows])
    }
}

fn load_image() -> Result<Map, Box<dyn Error>> {
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
    let map = load_image()?;

    // implement brute force breadth-first search within the validity map
    println!("{}", map);

    let (res, visited) = find_path(&map, Point { row: 14, col: 0 }, Point { row: 44, col: 51 })?;

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

#[derive(Eq)]
struct ToVisit<R: Eq> {
    cost: usize,
    point: R,
    from: Option<R>,
}

impl<R: Eq> Ord for ToVisit<R> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cost.cmp(&other.cost).reverse() // reverse for BinaryHeap to be a min-heap
    }
}

impl<R: Eq> PartialOrd for ToVisit<R> {
    fn partial_cmp(&self, other: &ToVisit<R>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<R: Eq> PartialEq for ToVisit<R> {
    fn eq(&self, other: &ToVisit<R>) -> bool {
        self.cost == other.cost
    }
}

#[derive(Clone, Copy, Debug)]
struct VisitedItem<R> {
    cost: usize,
    from: Option<R>,
}

#[derive(Clone, Copy, Debug)]
struct Visited<R>(Option<VisitedItem<R>>);

impl<R> Deref for Visited<R> {
    type Target = Option<VisitedItem<R>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<R> DerefMut for Visited<R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<R> Display for Visited<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(item) => write!(f, "{:03} ", item.cost),
            None => write!(f, "{:03} ", ""),
        }
    }
}

#[derive(Debug, PartialEq)]
struct PathResult<R> {
    path: Vec<R>,
    total_cost: usize,
}

fn find_path<'a, R: NodeReference, M: MapTrait<Reference = R>>(
    map: &'a M,
    start: R,
    goal: R,
) -> Result<
    (
        PathResult<R>,
        impl MapStorage<Visited<R>, Reference = R> + 'a,
    ),
    Box<dyn Error>,
> {
    // for keeping track of the cost up to the point and the point itself to visit
    // always prioritize visiting the lowest-cost ones, hence use a binary heap as a priority queue
    let mut visit_list: BinaryHeap<ToVisit<R>> = BinaryHeap::new();

    // to keep track of where we have been
    let mut visited = map.create_storage(Visited(None));

    visit_list.push(ToVisit {
        cost: 0,
        point: start,
        from: None,
    });

    let mut result: Option<PathResult<R>> = None;

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
            let mut path: Vec<R> = Vec::new();
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

        for (point, move_cost) in map.neighbors_of(visit.point) {
            if !visited.get(point).is_some() {
                visit_list.push(ToVisit {
                    cost: visit.cost + move_cost,
                    point: point,
                    from: Some(visit.point),
                });
            }
        }
    }

    // println!("{}", visited);
    result.ok_or(anyhow!("").into()).map(|r| (r, visited))
}

#[cfg(test)]
mod test {

    use super::*;

    fn create_basic_map() -> Map {
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
            Ok((PathResult { total_cost: 12, .. }, _))
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
            Ok((PathResult { total_cost: 9, .. }, _))
        ));

        map.cells[3][2] = Cell::Cost(4);
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 }),
            Ok((PathResult { total_cost: 11, .. }, _))
        ));

        map.cells[3][2] = Cell::Cost(10);
        assert!(matches!(
            find_path(&map, Point { row: 1, col: 1 }, Point { row: 1, col: 5 }),
            Ok((PathResult { total_cost: 12, .. }, _))
        ));
    }
}
