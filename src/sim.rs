use crate::rng;
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
use rand::{distributions::Bernoulli, Rng};
use rayon::prelude::*;
use std::iter::once;
use tokio::task::block_in_place;

type LifeContainer = SquareGrid<'static, Evonomics>;

mod brain;

const SPAWN_FOOD: usize = 16;
/// Food penalty for moving. Keep this as a multiple of 2.
const MOVE_PENALTY: usize = 2;

const LOWER_WALL_THRESH: f64 = -0.04;
const HIGHER_WALL_THRESH: f64 = 0.04;
const NOISE_FREQ: f64 = 0.02;

const FOOD_COLOR_MULTIPLIER: f32 = 0.05;

const SOURCE_FOOD_SPAWN: usize = 15;

lazy_static::lazy_static! {
    static ref NORMAL_FOOD_DISTRIBUTION: Bernoulli = Bernoulli::new(0.003).unwrap();
    // static ref NORMAL_FOOD_DISTRIBUTION: Bernoulli = Bernoulli::new(0.0).unwrap();
    static ref SOURCE_FOOD_DISTRIBUTION: Bernoulli = Bernoulli::new(1.0).unwrap();
    static ref MUTATE_DISTRIBUTION: Bernoulli = Bernoulli::new(0.01).unwrap();
    static ref CELL_SPAWN_DISTRIBUTION: Bernoulli = Bernoulli::new(0.000005).unwrap();
    static ref SOURCE_SPAWN_DISTRIBUTION: Bernoulli = Bernoulli::new(0.001).unwrap();
}

enum Evonomics {}

impl<'a> gridsim::Sim<'a> for Evonomics {
    type Cell = Cell;
    type Diff = Diff;
    type Move = Move;

    type Neighbors = MooreNeighbors<&'a Cell>;
    type MoveNeighbors = MooreNeighbors<Move>;

    fn step(cell: &Cell, neighbors: Self::Neighbors) -> (Diff, Self::MoveNeighbors) {
        if cell.brain.is_none() || cell.food == 0 {
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
        let decision = cell
            .brain
            .as_ref()
            .map(|brain| {
                const NEIGHBOR_INPUTS: usize = 3;
                const SELF_INPUTS: usize = 1;
                const INPUTS: usize = NEIGHBOR_INPUTS * 4 + SELF_INPUTS;
                let mut inputs: ArrayVec<[f64; INPUTS]> = neighbors
                    .iter()
                    .flat_map(|n| {
                        once(if n.brain.is_some() { 1.0 } else { 0.0 })
                            .chain(once(n.food as f64))
                            .chain(once(n.signal))
                    })
                    .chain(Some(cell.food as f64))
                    .collect();
                // This handles rotation of inputs in respect to cell.
                inputs[0..NEIGHBOR_INPUTS * 4].rotate_left(NEIGHBOR_INPUTS * brain.rotation());
                // A promise is made here not to look at the brain of any other cell elsewhere.
                let brain = unsafe { &mut *(brain as *const Brain as *mut Brain) };
                brain.decide(unsafe { rng() }, &inputs)
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
        if cell.ty != CellType::Wall {
            let rng = unsafe { rng() };
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
                    &mut *rng,
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
                if rng.sample(*MUTATE_DISTRIBUTION) {
                    brain.mutate(&mut *rng);
                }
            }

            // Handle spawning.
            if cell.brain.is_none() && rng.sample(*CELL_SPAWN_DISTRIBUTION) {
                cell.brain = Some(rng.gen());
                cell.food += SPAWN_FOOD;
            }
            if cell.ty == CellType::Source {
                if rng.sample(*SOURCE_FOOD_DISTRIBUTION) {
                    cell.food += SOURCE_FOOD_SPAWN;
                }
            } else {
                if rng.sample(*NORMAL_FOOD_DISTRIBUTION) {
                    cell.food += 1;
                }
            }

            // Handle signal.
            if let Some(ref mut brain) = cell.brain {
                cell.signal = brain.signal();
            } else {
                cell.signal = 0.0;
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CellType {
    Wall,
    Source,
    Empty,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub food: usize,
    pub ty: CellType,
    pub signal: f64,
    pub brain: Option<Brain>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            food: 0,
            ty: CellType::Empty,
            signal: 0.0,
            brain: None,
        }
    }
}

fn cap_color(n: f32) -> f32 {
    if n > 0.3 {
        0.3
    } else {
        n
    }
}

impl Cell {
    fn color(&self) -> Color {
        match self.ty {
            CellType::Wall => Color::from_rgb(0.4, 0.0, 0.0),
            CellType::Empty | CellType::Source => {
                if self.brain.is_some() {
                    self.brain.as_ref().unwrap().color()
                } else {
                    Color::from_rgb(
                        0.0,
                        cap_color(FOOD_COLOR_MULTIPLIER * self.food as f32),
                        0.0,
                    )
                }
            }
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
    width: usize,
    height: usize,
) -> (Sender<ToSim>, Receiver<FromSim>, impl Future<Output = ()>) {
    let (oncoming_tx, mut oncoming) = mpsc::channel(inbound);
    let (mut outgoing, outgoing_rx) = mpsc::channel(outbound);

    let mut sim = Sim::new(width, height);
    let task = async move {
        while let Some(oncoming) = oncoming.next().await {
            match oncoming {
                ToSim::Tick(times) => {
                    sim = block_in_place(move || sim.tick(times));
                    let view = block_in_place(|| sim.view());
                    outgoing.send(FromSim::View(view)).await.unwrap();
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
    pub ticks: usize,
}

pub struct Sim {
    grid: LifeContainer,
    frames_elapsed: usize,
}

impl Sim {
    fn new(width: usize, height: usize) -> Self {
        let mut grid = SquareGrid::<Evonomics>::new(width, height);
        let scaled = noise::ScalePoint::new(noise::OpenSimplex::new()).set_scale(1.4);
        let scale = noise::Constant::new(0.8);
        let noise_a = noise::Multiply::new(&scaled, &scale);
        let noise_b = noise::ScalePoint::new(
            noise::Worley::new()
                .enable_range(true)
                .set_displacement(0.0),
        )
        .set_scale(2.0);
        let source = noise::Min::new(&noise_a, &noise_b);
        let rng = unsafe { rng() };
        for (ix, cell) in grid.get_cells_mut().iter_mut().enumerate() {
            if rng.sample(*SOURCE_SPAWN_DISTRIBUTION) {
                cell.ty = CellType::Source;
            }
            let x = (ix % width) as f64;
            let y = (ix / height) as f64;
            let n = source.get([x * NOISE_FREQ, y * NOISE_FREQ]);
            if n > LOWER_WALL_THRESH && n < HIGHER_WALL_THRESH {
                cell.ty = CellType::Wall;
            }
        }
        Self {
            grid: grid,
            frames_elapsed: 0,
        }
    }

    pub fn tick(mut self, times: usize) -> Self {
        self.frames_elapsed = times;
        for _ in 0..times {
            self.grid.cycle();
        }
        self
    }

    pub fn view(&self) -> View {
        let temp = self.frames_elapsed;
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
            cells: self.grid.get_cells().iter().fold(0, |acc, cell| {
                acc + if cell.brain.is_some() { 1 } else { 0 }
            }),
            ticks: temp,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TickError {
    JoinFailed,
}
