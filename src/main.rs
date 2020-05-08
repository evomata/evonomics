extern crate gridsim;
extern crate gridsim_ui;

mod brain;

use arrayvec::ArrayVec;
use brain::{Brain, Decision};
use gridsim::{moore::*, Neighborhood, Sim, SquareGrid};
use noise::NoiseFn;
use rand::Rng;
use std::iter::once;

const CELL_SPAWN_PROBABILITY: f64 = 0.0001;
const SPAWN_FOOD: usize = 16;
const FOOD_SPAWN_PROBABILITY: f64 = 0.05;
const MUTATE_PROBABILITY: f64 = 0.0001;
const MOVE_PENALTY: usize = 2;

const LOWER_WALL_THRESH: f64 = 0.0;
const HIGHER_WALL_THRESH: f64 = 0.07;
const NOISE_FREQ: f64 = 0.02;

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
            Decision::Move(dir) => {
                if cell.food > MOVE_PENALTY {
                    (
                        Diff {
                            consume: cell.food,
                            moved: true,
                        },
                        MooreNeighbors::new(|nd| {
                            if nd == dir {
                                Move {
                                    food: cell.food - 1 - MOVE_PENALTY,
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
            Decision::Divide(dir) => {
                if cell.food >= 2 + MOVE_PENALTY {
                    (
                        Diff {
                            consume: cell.food / 2 + 1 + MOVE_PENALTY / 2,
                            moved: false,
                        },
                        MooreNeighbors::new(|nd| {
                            if nd == dir {
                                Move {
                                    food: cell.food / 2 - MOVE_PENALTY / 2,
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
        if !cell.wall {
            // Handle food reduction from diff.
            cell.food = cell.food.saturating_sub(diff.consume);

            // Handle taking the brain.
            if diff.moved {
                cell.brain.take();
            }

            // Handle brain movement.
            let mut brain_moves = moves.clone().iter().filter(|m| m.brain.is_some());
            if brain_moves.clone().count() >= 1 && cell.brain.is_some() {
                cell.brain = None;
            } else if brain_moves.clone().count() == 1 {
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
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub food: usize,
    pub wall: bool,
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
    let mut grid = SquareGrid::<Evonomics>::new(1024, 768);
    let source = noise::Perlin::new();
    for (ix, cell) in grid.get_cells_mut().iter_mut().enumerate() {
        let x = (ix % 1024) as f64;
        let y = (ix / 1024) as f64;
        let n = source.get([x * NOISE_FREQ, y * NOISE_FREQ]);
        if n > LOWER_WALL_THRESH && n < HIGHER_WALL_THRESH {
            cell.wall = true;
        }
    }
    gridsim_ui::Loop::new(|c: &Cell| {
        if c.wall {
            [1.0, 0.0, 0.0]
        } else if c.brain.is_some() {
            [1.0, 1.0, 1.0]
        } else if c.food != 0 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 0.0]
        }
    })
    .run(grid);
}
