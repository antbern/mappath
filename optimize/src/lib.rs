use core::panic;

use std::{
    any::Any,
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use image::{DynamicImage, GenericImageView};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Cell {
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
pub trait NodeReference: Copy + Eq + 'static {}

pub trait MapTrait {
    /// The type that can be used to reference nodes in the map
    type Reference: NodeReference;

    /// Return an iterator over the neighbors of the provided node and the cost required to go there
    fn neighbors_of(&self, node: Self::Reference)
        -> impl Iterator<Item = (Self::Reference, usize)>;

    /// Create a storage for values of type T
    fn create_storage<T: Copy + Default + 'static>(
        &self,
    ) -> impl MapStorage<T, Reference = Self::Reference> + 'static;
}

pub trait MapStorage<T> {
    type Reference: NodeReference;

    fn get(&self, node: Self::Reference) -> T;
    fn get_mut(&mut self, node: Self::Reference) -> &mut T;

    fn as_any(&self) -> &dyn Any;
}

/// A MapTrait implementation that uses a rectangular grid of cells
pub struct Map {
    rows: usize,
    columns: usize,
    cells: Vec<Vec<Cell>>,
}

/// A MapStorage that uses a rectangular grid of cells (a vec in a vec)
// TODO: change from vec of vec to one single vec -> better cache friendlyness!
#[derive(Debug)]
pub struct CellStorage<T>(Vec<Vec<T>>);

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
pub struct Point {
    pub row: usize,
    pub col: usize,
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

    fn create_storage<T: Copy + Default + 'static>(
        &self,
    ) -> impl MapStorage<T, Reference = Self::Reference> + 'static {
        CellStorage(vec![vec![Default::default(); self.columns]; self.rows])
    }
}

#[derive(Eq, Debug)]
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
pub struct VisitedItem<R> {
    cost: usize,
    from: Option<R>,
}

#[derive(Clone, Copy, Debug)]
pub struct Visited<R>(Option<VisitedItem<R>>);

impl<R> Default for Visited<R> {
    fn default() -> Self {
        Visited(None)
    }
}
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

#[derive(Debug, PartialEq, Clone, Eq)]
pub struct PathResult<R> {
    path: Vec<R>,
    total_cost: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathFinderState<R> {
    Computing,
    NoPathFound,
    PathFound(PathResult<R>),
}

#[derive(Debug)]
pub struct PathFinder<
    'a,
    R: NodeReference,
    M: MapTrait<Reference = R>,
    S: MapStorage<Visited<R>, Reference = R>,
> {
    start: R,
    goal: R,
    map: &'a M,
    visited: S,
    visit_list: BinaryHeap<ToVisit<R>>,
    state: PathFinderState<R>,
}

impl<
        'a,
        R: NodeReference,
        M: MapTrait<Reference = R>,
        S: MapStorage<Visited<R>, Reference = R>,
    > PathFinder<'a, R, M, S>
{
    pub fn new(map: &'a M, start: R, goal: R, visited: S) -> Self {
        Self {
            start,
            goal,
            map,
            visited,
            visit_list: BinaryHeap::from([ToVisit {
                cost: 0,
                point: start,
                from: None,
            }]),
            state: PathFinderState::Computing,
        }
    }

    pub fn finish(mut self) -> (PathFinderState<R>, S) {
        loop {
            match self.step() {
                PathFinderState::Computing => {}
                s => return (s, self.visited),
            }
        }
    }

    pub fn step(&mut self) -> PathFinderState<R> {
        if self.state != PathFinderState::Computing {
            return self.state.clone();
        }
        if let Some(visit) = self.visit_list.pop() {
            // we have a point to process, find the valid neighbors to visit next

            if self.visited.get(visit.point).is_some() {
                return self.state.clone();
            }

            *self.visited.get_mut(visit.point) = Visited(Some(VisitedItem {
                cost: visit.cost,
                from: visit.from,
            }));

            // if this is the goal, we are done! (and should probably do some back-tracking to find the actual shortest path...)
            if visit.point == self.goal {
                println!("FOUND GOAL!: cost={}", visit.cost);

                // backtrack to find the total shortest path
                let mut path: Vec<R> = Vec::new();
                path.push(self.goal);

                let mut previous_visit = self.visited.get(self.goal);

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
                            self.visited.get(from)
                        }
                        Visited(None) => {
                            panic!("Backtracking lead to a Point that was never visited")
                        }
                    }
                }

                path.reverse();

                self.state = PathFinderState::PathFound(PathResult {
                    path: path,
                    total_cost: visit.cost,
                });

                return self.state.clone();
            }

            for (point, move_cost) in self.map.neighbors_of(visit.point) {
                if !self.visited.get(point).is_some() {
                    self.visit_list.push(ToVisit {
                        cost: visit.cost + move_cost,
                        point: point,
                        from: Some(visit.point),
                    });
                }
            }
        } else {
            self.state = PathFinderState::NoPathFound;
        }

        return self.state.clone();
    }

    pub fn get_visited(&self) -> &S {
        &self.visited
    }

    pub fn start(&self) -> R {
        self.start
    }

    pub fn goal(&self) -> R {
        self.goal
    }
}

pub fn parse_img(img: &DynamicImage) -> Result<Map, anyhow::Error> {
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

        let visited = map.create_storage();

        let finder = PathFinder::new(
            &map,
            Point { row: 1, col: 1 },
            Point { row: 1, col: 5 },
            visited,
        );

        // test the basic case
        assert!(matches!(
            finder.finish().0,
            PathFinderState::PathFound(PathResult { total_cost: 12, .. })
        ));
    }
    #[test]
    fn test_basic_no_route() {
        let map = create_basic_map();

        let visited = map.create_storage();

        let finder = PathFinder::new(
            &map,
            Point { row: 1, col: 1 },
            Point { row: 0, col: 5 },
            visited,
        );
        // no route to target
        assert!(matches!(finder.finish().0, PathFinderState::NoPathFound));
    }

    #[test]
    fn test_basic_shortcut() {
        let mut map = create_basic_map();
        // create higher cost shortcut
        map.cells[3][2] = Cell::Cost(2);
        let visited = map.create_storage();

        let finder = PathFinder::new(
            &map,
            Point { row: 1, col: 1 },
            Point { row: 1, col: 5 },
            visited,
        );

        assert!(matches!(
            finder.finish().0,
            PathFinderState::PathFound(PathResult { total_cost: 9, .. })
        ));

        let visited = map.create_storage();
        map.cells[3][2] = Cell::Cost(4);

        let finder = PathFinder::new(
            &map,
            Point { row: 1, col: 1 },
            Point { row: 1, col: 5 },
            visited,
        );

        assert!(matches!(
            finder.finish().0,
            PathFinderState::PathFound(PathResult { total_cost: 11, .. })
        ));

        let visited = map.create_storage();
        map.cells[3][2] = Cell::Cost(10);

        let finder = PathFinder::new(
            &map,
            Point { row: 1, col: 1 },
            Point { row: 1, col: 5 },
            visited,
        );

        assert!(matches!(
            finder.finish().0,
            PathFinderState::PathFound(PathResult { total_cost: 12, .. })
        ));
    }
}
