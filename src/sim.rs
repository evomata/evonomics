use arrayvec::ArrayVec;
use brain::{Brain, Decision};
use futures::{
    channel::mpsc::{self, Receiver, Sender},
    prelude::*,
    Future,
};
use gridsim::{moore::*, Neighborhood, SquareGrid};
use iced::Color;
use ndarray::Array2;
use noise::NoiseFn;
use rand::Rng;
use rayon::prelude::*;
use std::iter::once;
use tokio::task::spawn_blocking;

type LifeContainer = SquareGrid<'static, Evonomics>;

mod brain;

const CELL_SPAWN_PROBABILITY: f64 = 0.0000001;
const SPAWN_FOOD: usize = 16;
const FOOD_SPAWN_PROBABILITY: f64 = 0.05;
const MUTATE_PROBABILITY: f64 = 0.001;
const MOVE_PENALTY: usize = 2;

const LOWER_WALL_THRESH: f64 = 0.0;
const HIGHER_WALL_THRESH: f64 = 0.07;
const NOISE_FREQ: f64 = 0.02;

const FOOD_COLOR_MULTIPLIER: f32 = 0.1;

// Langton's Ant
enum Evonomics {}

impl<'a> gridsim::Sim<'a> for Evonomics {
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
            let mut brain_moves = moves.clone().iter().flat_map(|m| m.brain);
            if brain_moves.clone().count() + cell.brain.is_some() as usize >= 2 {
                // Brains that enter the same space are combined together.
                cell.brain = Some(brain::combine(
                    cell.brain.clone().into_iter().chain(brain_moves),
                ));
            } else if brain_moves.clone().count() == 1 {
                let m = brain_moves.next().unwrap();
                cell.brain = Some(m);
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

fn cap_color(n: f32) -> f32 {
    if n > 1.0 {
        1.0
    } else {
        n
    }
}

impl Cell {
    fn color(&self) -> Color {
        if self.brain.is_some() {
            Color::from_rgb(1.0, 1.0, 1.0)
        } else if self.wall {
            Color::from_rgb(1.0, 0.0, 0.0)
        } else {
            Color::from_rgb(
                0.0,
                cap_color(FOOD_COLOR_MULTIPLIER * self.food as f32),
                0.0,
            )
        }
    }
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

/// The entrypoint for the grid.
pub fn run_sim(
    inbound: usize,
    outbound: usize,
) -> (Sender<ToSim>, Receiver<FromSim>, impl Future<Output = ()>) {
    let (oncoming_tx, mut oncoming) = mpsc::channel(inbound);
    let (mut outgoing, outgoing_rx) = mpsc::channel(outbound);

    let mut sim = Sim::new();
    let task = async move {
        while let Some(oncoming) = oncoming.next().await {
            match oncoming {
                ToSim::Tick(times) => {
                    sim = spawn_blocking(move || sim.tick(times)).await.unwrap();
                    outgoing.send(FromSim::View(sim.view())).await.unwrap();
                }
            }
        }
    };

    (oncoming_tx, outgoing_rx, task)
}

/// Messages sent to the grid.
#[derive(Debug)]
pub enum ToSim {
    // Populate(evo::CellState),
    // Unpopulate(evo::CellState),
    Tick(usize),
}

/// Messages sent from the grid.
#[derive(Debug)]
pub enum FromSim {
    View(View),
}

/// Contains the data to display the simulation.
#[derive(Default, Debug)]
pub struct View {
    pub colors: Array2<Color>,
    pub cells: usize,
}

pub struct Sim {
    grid: LifeContainer,
}

impl Sim {
    fn new() -> Self {
        let mut grid = SquareGrid::<Evonomics>::new(crate::grid::SIDE, crate::grid::SIDE);
        let source = noise::Perlin::new();
        for (ix, cell) in grid.get_cells_mut().iter_mut().enumerate() {
            let x = (ix % crate::grid::SIDE) as f64;
            let y = (ix / crate::grid::SIDE) as f64;
            let n = source.get([x * NOISE_FREQ, y * NOISE_FREQ]);
            if n > LOWER_WALL_THRESH && n < HIGHER_WALL_THRESH {
                cell.wall = true;
            }
        }
        Self { grid }
    }

    pub fn tick(mut self, times: usize) -> Self {
        for _ in 0..times {
            self.grid.cycle();
        }
        self
    }

    pub fn view(&self) -> View {
        View {
            colors: Array2::from_shape_vec(
                (self.grid.get_height(), self.grid.get_width()),
                self.grid
                    .get_cells()
                    .par_iter()
                    .map(|c| c.color())
                    .collect::<Vec<Color>>(),
            )
            .unwrap(),
            cells: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TickError {
    JoinFailed,
}
