use core::panic;

use std::{
    any::Any,
    cmp::Ordering,
    collections::BinaryHeap,
    fmt::{Debug, Display},
    ops::{Add, Deref, DerefMut},
};

/// Represents an aboslute cost value
pub trait AbsoluteCost: Copy + Clone + Default + Add<Output = Self> + 'static {
    type CmpContext;
    fn context_cmp(&self, other: &Self, ctx: &Self::CmpContext) -> std::cmp::Ordering;
}

/// Represents a relative change in cost and can therefore be required to implement Equality
/// operators
// pub trait RelativeCost<A: Cost>: Copy + Clone + Add<Output = A> + 'static {}
pub trait RelativeCost: Copy + Clone + Add + PartialEq + Eq + 'static {}

impl RelativeCost for usize {}

impl AbsoluteCost for usize {
    type CmpContext = ();

    fn context_cmp(&self, other: &Self, _ctx: &Self::CmpContext) -> std::cmp::Ordering {
        self.cmp(other)
    }
}

/// Supertrait that collects all the requirements on the NodeReference values
/// Must be copy, comparable and not references (hence 'static)
pub trait NodeReference: Copy + Eq + 'static {}

// TODO: move to find.rs and rename as Map
pub trait MapTrait {
    /// The type that can be used to reference nodes in the map
    type Reference: NodeReference;

    /// The type that the map uses for storage
    type Storage<T: Default + Copy + Clone + 'static>: MapStorage<T, Reference = Self::Reference>;

    type Cost: RelativeCost;

    /// Check if the provided node reference is valid
    fn is_valid(&self, node: Self::Reference) -> bool;

    /// Return an iterator over the neighbors of the provided node and the cost required to go there
    fn neighbors_of(
        &self,
        node: Self::Reference,
    ) -> impl Iterator<Item = (Self::Reference, Self::Cost)>;

    /// Create a storage for values of type T
    fn create_storage<T: Default + Copy + Clone + 'static>(&self) -> Self::Storage<T>;
}

pub trait MapStorage<T> {
    type Reference: NodeReference;

    fn is_valid(&self, node: Self::Reference) -> bool;
    fn get(&self, node: Self::Reference) -> T;
    fn get_mut(&mut self, node: Self::Reference) -> &mut T;

    fn as_any(&self) -> &dyn Any;
}

/// A trait that is used to compare two values given a context

/// The objects that we store in the prioirty queue
#[derive(Debug)]
struct ToVisit<C: AbsoluteCost, R: Eq> {
    context: C::CmpContext,
    cost: C,
    point: R,
    from: Option<R>,
}

impl<C: AbsoluteCost, R: Eq> Ord for ToVisit<C, R> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cost.context_cmp(&other.cost, &self.context).reverse() // reverse for BinaryHeap to be a min-heap
    }
}

impl<C: AbsoluteCost, R: Eq> PartialOrd for ToVisit<C, R> {
    fn partial_cmp(&self, other: &ToVisit<C, R>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<C: AbsoluteCost, R: Eq> PartialEq for ToVisit<C, R> {
    fn eq(&self, other: &ToVisit<C, R>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<C: AbsoluteCost, R: Eq> Eq for ToVisit<C, R> {}

#[derive(Clone, Copy, Debug)]
pub struct VisitedItem<C, R> {
    pub cost: C,
    pub from: Option<R>,
}

#[derive(Clone, Copy, Debug)]
pub struct Visited<C, R>(Option<VisitedItem<C, R>>);

impl<C, R> Default for Visited<C, R> {
    fn default() -> Self {
        Visited(None)
    }
}
impl<C, R> Deref for Visited<C, R> {
    type Target = Option<VisitedItem<C, R>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<C, R> DerefMut for Visited<C, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<C: Display, R> Display for Visited<C, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(item) => write!(f, "{:03} ", item.cost),
            None => write!(f, "{:03} ", ""),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq)]
pub struct PathResult<C, R> {
    pub path: Vec<R>,
    pub start: R,
    pub goal: R,
    pub total_cost: C,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathFinderState<C, R> {
    Computing,
    NoPathFound,
    PathFound(PathResult<C, R>),
}

impl<C, R> PathFinderState<C, R> {
    fn is_done(&self) -> bool {
        !matches!(self, PathFinderState::Computing)
    }
}

#[derive(Debug)]
pub struct PathFinder<
    R: NodeReference,
    K, // the context for contextord
    C: AbsoluteCost<CmpContext = K>,
    S: MapStorage<Visited<C, R>, Reference = R>,
    M: MapTrait<Reference = R, Storage<Visited<C, R>> = S, Cost = C>,
> {
    start: R,
    goal: R,
    context: K,
    visited: S,
    visit_list: BinaryHeap<ToVisit<C, R>>,
    state: PathFinderState<C, R>,
    _map: std::marker::PhantomData<M>,
}

impl<
        R: NodeReference,
        K: Clone, // the context for contextord
        C: AbsoluteCost<CmpContext = K> + Display,
        S: MapStorage<Visited<C, R>, Reference = R>,
        M: MapTrait<Reference = R, Storage<Visited<C, R>> = S, Cost = C>,
    > PathFinder<R, K, C, S, M>
{
    pub fn new(start: R, goal: R, visited: S, context: K) -> Self {
        Self {
            start,
            goal,
            visited,
            context: context.clone(),
            visit_list: BinaryHeap::from([ToVisit {
                context,
                cost: Default::default(),
                point: start,
                from: None,
            }]),
            state: PathFinderState::Computing,
            _map: std::marker::PhantomData,
        }
    }

    pub fn finish(mut self, map: &M) -> (PathFinderState<C, R>, S) {
        loop {
            match self.step(map) {
                PathFinderState::Computing => {}
                s => return (s, self.visited),
            }
        }
    }

    pub fn step(&mut self, map: &M) -> PathFinderState<C, R> {
        if self.state.is_done() {
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
                    start: self.start,
                    goal: self.goal,
                });

                return self.state.clone();
            }

            for (point, move_cost) in map.neighbors_of(visit.point) {
                if !self.visited.get(point).is_some() {
                    self.visit_list.push(ToVisit {
                        context: self.context.clone(),
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

    pub fn state(&self) -> &PathFinderState<C, R> {
        &self.state
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
