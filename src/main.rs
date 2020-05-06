extern crate gridsim;
extern crate gridsim_ui;

mod brain;

use arrayvec::ArrayVec;
use brain::{Brain, Decision};
use gridsim::{moore::*, Neighborhood, Sim, SquareGrid};
use rand::Rng;
use std::iter::once;

const CELL_SPAWN_PROBABILITY: f64 = 0.00001;
const SPAWN_FOOD: usize = 16;
const FOOD_SPAWN_PROBABILITY: f64 = 0.001;
const MUTATE_PROBABILITY: f64 = 0.1;

// Langton's Ant
enum Evonomics {}

impl<'a> Sim<'a> for Evonomics {
    type Cell = Cell;
    type Diff = Diff;
    type Move = Move;

    type Neighbors = MooreNeighbors<&'a Cell>;
    type MoveNeighbors = MooreNeighbors<Move>;

    fn step(cell: &Cell, neighbors: Self::Neighbors) -> (Diff, Self::MoveNeighbors) {
        // Closure for just existing (consuming food and nothing happening).
        let just_exist = || {
            (
                Diff {
                    consume: 1,
                    moved: false,
                },
                MooreNeighbors::new(|_| Move {
                    food: 0,
                    brain: None,
                }),
            )
        };
        if cell.food == 0 || cell.brain.is_none() {
            return (
                Diff {
                    consume: 0,
                    moved: true,
                },
                MooreNeighbors::new(|_| Move {
                    food: 0,
                    brain: None,
                }),
            );
        }
        let decision = cell
            .brain
            .as_ref()
            .map(|brain| {
                let inputs: ArrayVec<[f64; 5]> = neighbors
                    .iter()
                    .flat_map(|n| {
                        once(if n.brain.is_some() { 1.0 } else { 0.0 }).chain(once(n.food as f64))
                    })
                    .chain(Some(cell.food as f64))
                    .collect();
                // A promise is made here not to look at the brain of any other cell elsewhere.
                let brain = unsafe { &mut *(brain as *const Brain as *mut Brain) };
                brain.decide(&inputs)
            })
            .unwrap_or(Decision::Nothing);

        match decision {
            Decision::Move(dir) => (
                Diff {
                    consume: cell.food,
                    moved: true,
                },
                MooreNeighbors::new(|nd| {
                    if nd == dir {
                        Move {
                            food: cell.food - 1,
                            brain: cell.brain.clone(),
                        }
                    } else {
                        Move {
                            food: 0,
                            brain: None,
                        }
                    }
                }),
            ),
            Decision::Divide(dir) => {
                if cell.food >= 2 {
                    (
                        Diff {
                            consume: cell.food / 2 + 1,
                            moved: false,
                        },
                        MooreNeighbors::new(|nd| {
                            if nd == dir {
                                Move {
                                    food: cell.food / 2,
                                    brain: cell.brain.clone(),
                                }
                            } else {
                                Move {
                                    food: 0,
                                    brain: None,
                                }
                            }
                        }),
                    )
                } else {
                    just_exist()
                }
            }
            Decision::Nothing => just_exist(),
        }
    }

    fn update(cell: &mut Cell, diff: Diff, moves: Self::MoveNeighbors) {
        // Handle food reduction from diff.
        cell.food = cell.food.saturating_sub(diff.consume);

        // Handle taking the brain.
        if diff.moved {
            cell.brain.take();
        }

        // Handle brain movement.
        let mut brain_moves = moves.clone().iter().filter(|m| m.brain.is_some());
        if brain_moves.clone().count() == 1 {
            let m = brain_moves.next().unwrap();
            cell.brain = m.brain;
        }

        // Handle food movement.
        cell.food += moves.iter().map(|m| m.food).sum::<usize>();

        // Handle mutation.
        if let Some(ref mut brain) = cell.brain {
            if rand::thread_rng().gen_bool(MUTATE_PROBABILITY) {
                brain.mutate();
            }
        }

        // Handle spawning.
        if cell.brain.is_none() && rand::thread_rng().gen_bool(CELL_SPAWN_PROBABILITY) {
            cell.brain = Some(rand::thread_rng().gen());
            cell.food += SPAWN_FOOD;
        }
        if rand::thread_rng().gen_bool(FOOD_SPAWN_PROBABILITY) {
            cell.food += 1;
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub food: usize,
    pub brain: Option<Brain>,
}

#[derive(Clone, Debug)]
pub struct Move {
    food: usize,
    brain: Option<Brain>,
}

#[derive(Clone, Debug)]
pub struct Diff {
    consume: usize,
    moved: bool,
}

fn main() {
    let grid = SquareGrid::<Evonomics>::new(256, 256);
    gridsim_ui::Loop::new(|c: &Cell| {
        if c.brain.is_some() {
            [1.0, 1.0, 1.0]
        } else if c.food != 0 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 0.0]
        }
    })
    .run(grid);
}
