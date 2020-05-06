extern crate gridsim;
extern crate gridsim_ui;

mod brain;

use arrayvec::ArrayVec;
use brain::{Brain, Decision};
use gridsim::{moore::*, Neighborhood, Sim, SquareGrid};
use rand::Rng;

const SPAWN_PROBABILITY: f64 = 0.001;
const SPAWN_FOOD: usize = 128;

// Langton's Ant
enum Evonomics {}

impl<'a> Sim<'a> for Evonomics {
    type Cell = Cell;
    type Diff = Diff;
    type Move = Move;

    type Neighbors = MooreNeighbors<&'a Cell>;
    type MoveNeighbors = MooreNeighbors<Move>;

    fn step(cell: &Cell, neighbors: Self::Neighbors) -> (Diff, Self::MoveNeighbors) {
        if cell.food == 0 {
            (
                Diff {
                    consume: 0,
                    moved: true,
                },
                MooreNeighbors::new(|_| Move {
                    food: 0,
                    brain: None,
                }),
            )
        } else {
            let decision = cell
                .brain
                .as_ref()
                .map(|brain| {
                    let inputs: ArrayVec<[f64; 4]> = neighbors
                        .iter()
                        .map(|n| if n.brain.is_some() { 1.0 } else { 0.0 })
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
                                food: cell.food,
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
                Decision::Nothing => (
                    Diff {
                        consume: 1,
                        moved: false,
                    },
                    MooreNeighbors::new(|_| Move {
                        food: 0,
                        brain: None,
                    }),
                ),
            }
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

        // // Handle mutation.
        // if let Some(ref mut brain) = cell.brain {
        //     brain.mutate(MUTATE_LAMBDA);
        // }

        // Handle spawning.
        if cell.brain.is_none() && rand::thread_rng().gen_bool(SPAWN_PROBABILITY) {
            cell.brain = Some(rand::thread_rng().gen());
            cell.food += SPAWN_FOOD;
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
        } else {
            [0.0, 0.0, 0.0]
        }
    })
    .run(grid);
}
