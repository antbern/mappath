use crate::find::{Cost, MapStorage, MapTrait, NodeReference};
use std::any::Any;
use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Cell<C: Cost> {
    Invalid,
    Valid {
        cost: C,
    },
    OneWay {
        cost: C,
        // the direction which one can move from this cell
        direction: Direction,
        // optional target point to use as "teleport" when moving in the direction
        target: Option<Point>,
    },
}

impl<C: Cost> Default for Cell<C> {
    fn default() -> Self {
        Self::Invalid
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Direction::Up => "up",
                Direction::Down => "down",
                Direction::Left => "left",
                Direction::Right => "right",
            }
        )
    }
}

impl FromStr for Direction {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "up" => Ok(Direction::Up),
            "down" => Ok(Direction::Down),
            "left" => Ok(Direction::Left),
            "right" => Ok(Direction::Right),
            _ => Err(anyhow::anyhow!("Invalid direction: {}", s)),
        }
    }
}

impl<C: Cost + Display> Display for Cell<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Cell::Invalid => "X",
                Cell::Valid { .. } => " ",
                Cell::OneWay {
                    direction,
                    target: None,
                    ..
                } => match direction {
                    Direction::Up => "🠭",
                    Direction::Down => "🠯",
                    Direction::Left => "🠬",
                    Direction::Right => "🠮",
                },
                Cell::OneWay {
                    direction,
                    target: Some(_),
                    ..
                } => match direction {
                    Direction::Up => "↟",
                    Direction::Down => "↡",
                    Direction::Left => "↞",
                    Direction::Right => "↠",
                },
            }
        )
    }
}

/// A MapTrait implementation that uses a rectangular grid of cells
#[derive(Debug, Serialize, Deserialize)]
pub struct GridMap<C: Cost> {
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<Vec<Cell<C>>>,
}

impl<C: Cost> GridMap<C> {
    pub fn new(rows: usize, columns: usize, default_cost: C) -> Self {
        Self {
            rows,
            columns,
            cells: vec![vec![Cell::Valid { cost: default_cost }; columns]; rows],
        }
    }

    pub fn resize(&mut self, columns: usize, rows: usize) {
        // create container for holding new cells
        let mut new_cells = vec![vec![Cell::default(); columns]; rows];

        // copy old cells into new container, or fill with default if new size is larger (already
        // done above)
        for row in 0..self.rows.min(rows) {
            for col in 0..self.columns.min(columns) {
                new_cells[row][col] = self.cells[row][col];
            }
        }

        // finally replace the cells with the new container
        self.rows = rows;
        self.columns = columns;
        self.cells = new_cells;
    }
    /// Scales the map by the given factor, i.e. to make it twice as large, pass 2.
    /// Interpolates the cells by repeating the existing cells in the new grid.
    pub fn scale_up(&mut self, factor: usize) {
        let mut new_cells = vec![vec![Cell::default(); self.columns * factor]; self.rows * factor];

        for row in 0..self.rows {
            for col in 0..self.columns {
                for r in 0..factor {
                    for c in 0..factor {
                        new_cells[row * factor + r][col * factor + c] = self.cells[row][col];
                    }
                }
            }
        }

        self.rows *= factor;
        self.columns *= factor;
        self.cells = new_cells;
    }
}

/// A MapStorage that uses a rectangular grid of cells (a vec in a vec)
// TODO: change from vec of vec to one single vec -> better cache friendlyness!
#[derive(Debug)]
pub struct CellStorage<T>(Vec<Vec<T>>);

impl<T: Copy + 'static> MapStorage<T> for CellStorage<T> {
    type Reference = Point;

    fn is_valid(&self, node: Self::Reference) -> bool {
        node.row < self.0.len() && node.col < self.0[0].len()
    }

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

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub row: usize,
    pub col: usize,
}

impl NodeReference for Point {}

impl<C: Cost + Display> Display for GridMap<C> {
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

impl<C: Cost> MapTrait for GridMap<C> {
    type Reference = Point;
    type Storage<T: Default + Copy + Clone + 'static> = CellStorage<T>;
    type Cost = C;

    fn is_valid(&self, node: Self::Reference) -> bool {
        node.row < self.rows && node.col < self.columns
    }

    fn neighbors_of(
        &self,
        node: Self::Reference,
    ) -> impl Iterator<Item = (Self::Reference, Self::Cost)> {
        let mut points = Vec::with_capacity(4);

        let c = self.cells[node.row][node.col];

        match c {
            Cell::Valid { cost } => {
                if node.row > 0 {
                    points.push((
                        Point {
                            row: node.row - 1,
                            col: node.col,
                        },
                        cost,
                    ));
                }
                if node.col > 0 {
                    points.push((
                        Point {
                            col: node.col - 1,
                            row: node.row,
                        },
                        cost,
                    ));
                }

                if node.row < self.rows - 1 {
                    points.push((
                        Point {
                            row: node.row + 1,
                            col: node.col,
                        },
                        cost,
                    ));
                }
                if node.col < self.columns - 1 {
                    points.push((
                        Point {
                            col: node.col + 1,
                            row: node.row,
                        },
                        cost,
                    ));
                }
            }
            Cell::OneWay {
                cost,
                direction,
                target,
            } => {
                if node.row > 0 && direction != Direction::Down {
                    points.push((
                        Point {
                            row: node.row - 1,
                            col: node.col,
                        },
                        cost,
                    ));
                }
                if node.col > 0 && direction != Direction::Right {
                    points.push((
                        Point {
                            col: node.col - 1,
                            row: node.row,
                        },
                        cost,
                    ));
                }

                if node.row < self.rows - 1 && direction != Direction::Up {
                    points.push((
                        Point {
                            row: node.row + 1,
                            col: node.col,
                        },
                        cost,
                    ));
                }
                if node.col < self.columns - 1 && direction != Direction::Left {
                    points.push((
                        Point {
                            col: node.col + 1,
                            row: node.row,
                        },
                        cost,
                    ));
                }

                if let Some(target) = target {
                    points.push((target, cost));
                }
            }
            Cell::Invalid => {}
        };

        // filter to only keep valid cells
        points.retain(|(p, _)| self.cells[p.row][p.col] != Cell::Invalid);

        points.into_iter()
    }

    fn create_storage<T: Default + Copy + Clone + 'static>(&self) -> Self::Storage<T> {
        CellStorage(vec![vec![Default::default(); self.columns]; self.rows])
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::find::{PathFinder, PathFinderState, PathResult};

    fn create_basic_map() -> GridMap<usize> {
        use Cell::*;
        GridMap {
            rows: 7,
            columns: 7,
            cells: vec![
                vec![
                    Invalid, Invalid, Invalid, Invalid, Invalid, Invalid, Invalid,
                ],
                vec![
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                    Invalid,
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                ],
                vec![
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                    Invalid,
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                ],
                vec![
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                    Invalid,
                ],
                vec![
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                    Valid { cost: 1 },
                    Invalid,
                    Invalid,
                    Invalid,
                ],
                vec![
                    Invalid,
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                    Valid { cost: 1 },
                ],
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

        let finder = PathFinder::new(Point { row: 1, col: 1 }, Point { row: 1, col: 5 }, visited);

        // test the basic case
        assert!(matches!(
            finder.finish(&map).0,
            PathFinderState::PathFound(PathResult { total_cost: 12, .. })
        ));
    }
    #[test]
    fn test_basic_no_route() {
        let map = create_basic_map();

        let visited = map.create_storage();

        let finder = PathFinder::new(Point { row: 1, col: 1 }, Point { row: 0, col: 5 }, visited);
        // no route to target
        assert!(matches!(
            finder.finish(&map).0,
            PathFinderState::NoPathFound
        ));
    }

    #[test]
    fn test_basic_shortcut() {
        let mut map = create_basic_map();
        // create higher cost shortcut
        map.cells[3][2] = Cell::Valid { cost: 2 };
        let visited = map.create_storage();

        let finder = PathFinder::new(Point { row: 1, col: 1 }, Point { row: 1, col: 5 }, visited);

        assert!(matches!(
            finder.finish(&map).0,
            PathFinderState::PathFound(PathResult { total_cost: 9, .. })
        ));

        let visited = map.create_storage();
        map.cells[3][2] = Cell::Valid { cost: 4 };

        let finder = PathFinder::new(Point { row: 1, col: 1 }, Point { row: 1, col: 5 }, visited);

        assert!(matches!(
            finder.finish(&map).0,
            PathFinderState::PathFound(PathResult { total_cost: 11, .. })
        ));

        let visited = map.create_storage();
        map.cells[3][2] = Cell::Valid { cost: 10 };

        let finder = PathFinder::new(Point { row: 1, col: 1 }, Point { row: 1, col: 5 }, visited);

        assert!(matches!(
            finder.finish(&map).0,
            PathFinderState::PathFound(PathResult { total_cost: 12, .. })
        ));
    }
}
