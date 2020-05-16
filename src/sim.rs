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
use rand::{distributions::Bernoulli, Rng};
use rayon::prelude::*;
use std::iter::once;
use tokio::task::block_in_place;

type LifeContainer = SquareGrid<'static, Evonomics>;

mod brain;

const SPAWN_FOOD: u32 = 16;
const MOVE_PENALTY: u32 = 0;

const FOOD_COLOR_MULTIPLIER: f32 = 0.05;

const SOURCE_FOOD_SPAWN: u32 = 0;

// FIXME
static mut CELL_SPAWN_DISTRIBUTION: Option<Bernoulli> = None;

lazy_static::lazy_static! {
    static ref NORMAL_FOOD_DISTRIBUTION: Bernoulli = Bernoulli::new(0.01).unwrap();
    // static ref NORMAL_FOOD_DISTRIBUTION: Bernoulli = Bernoulli::new(0.0).unwrap();
    static ref SOURCE_FOOD_DISTRIBUTION: Bernoulli = Bernoulli::new(1.0).unwrap();
    static ref MUTATE_DISTRIBUTION: Bernoulli = Bernoulli::new(0.001).unwrap();
    static ref SOURCE_SPAWN_DISTRIBUTION: Bernoulli = Bernoulli::new(0.001).unwrap();
}

#[derive(Clone, Debug)]
pub struct Trade {
    pub money: i32,
    pub food: i32,
}

struct Evonomics {}

impl std::default::Default for Evonomics {
    fn default() -> Evonomics {
        Evonomics {}
    }
}

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
                    trade: None,
                },
                MooreNeighbors::new(|_| Move {
                    food: 0,
                    brain: None,
                }),
            );
        }
        // Closure for just existing (consuming food and nothing happening).
        let just_exist = |trade| {
            (
                Diff {
                    consume: 1,
                    moved: false,
                    trade,
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
                const NEIGHBOR_INPUTS: usize = 4;
                const SELF_INPUTS: usize = 1;
                const INPUTS: usize = NEIGHBOR_INPUTS * 4 + SELF_INPUTS;
                let boolnum = |n| if n { 1.0 } else { 0.0 };
                let mut inputs: ArrayVec<[f64; INPUTS]> = neighbors
                    .iter()
                    .flat_map(|n| {
                        once(boolnum(n.brain.is_some()))
                            .chain(once(boolnum(n.ty == CellType::Wall)))
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
                            trade: None,
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
                    just_exist(None)
                }
            }
            Decision::Divide(dir) => {
                if cell.food >= 2 + MOVE_PENALTY {
                    (
                        Diff {
                            consume: cell.food / 2 + 1 + MOVE_PENALTY / 2,
                            moved: false,
                            trade: None,
                        },
                        MooreNeighbors::new(|nd| {
                            if nd == dir {
                                Move {
                                    food: cell.food / 2 - MOVE_PENALTY / 2,
                                    brain: {
                                        if let Some(mut t) = cell.brain.clone() {
                                            t.generation += 1;
                                            Some(t)
                                        } else {
                                            None
                                        }
                                    },
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
                    just_exist(None)
                }
            }
            Decision::Trade(money, food) => {
                let money = std::cmp::min(money, cell.money as i32);
                // We cant trade away more than 1 less than the amount of food we have because the food goes down.
                let food = std::cmp::min(food, cell.money as i32 - 1);
                just_exist(Some(Trade { money, food }))
            }
            Decision::Nothing => just_exist(None),
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
            if brain_moves.clone().count() + cell.brain.is_some() as usize > 1 {
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
            cell.food += moves.iter().map(|m| m.food).sum::<u32>();

            // Handle mutation.
            if let Some(ref mut brain) = cell.brain {
                if rng.sample(*MUTATE_DISTRIBUTION) {
                    brain.mutate(&mut *rng);
                }
            }

            // Handle spawning.
            if cell.brain.is_none()
                && unsafe {
                    rng.sample(match CELL_SPAWN_DISTRIBUTION {
                        Some(dist) => dist,
                        None => Bernoulli::new(0.00003).unwrap(),
                    })
                }
            {
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
    pub food: u32,
    pub money: u32,
    pub ty: CellType,
    pub signal: f64,
    pub brain: Option<Brain>,
    pub trade: Option<Trade>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            food: 0,
            money: 0,
            ty: CellType::Empty,
            signal: 0.0,
            brain: None,
            trade: None,
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
    food: u32,
    brain: Option<Brain>,
}

#[derive(Clone, Debug)]
pub struct Diff {
    consume: u32,
    moved: bool,
    trade: Option<Trade>,
}

/// The entrypoint for the grid.
pub fn run_sim(
    inbound: usize,
    outbound: usize,
    width: usize,
    height: usize,
    openness: usize,
) -> (Sender<ToSim>, Receiver<FromSim>, impl Future<Output = ()>) {
    let (oncoming_tx, mut oncoming) = mpsc::channel(inbound);
    let (mut outgoing, outgoing_rx) = mpsc::channel(outbound);

    let mut sim = Sim::new(width, height, openness);
    let task = async move {
        while let Some(oncoming) = oncoming.next().await {
            match oncoming {
                ToSim::Tick(times) => {
                    sim = block_in_place(move || sim.tick(times));
                    let view = block_in_place(|| sim.view());
                    outgoing.send(FromSim::View(view)).await.unwrap();
                }
                ToSim::SetSpawnChance(new_spawn_chance) => unsafe {
                    CELL_SPAWN_DISTRIBUTION = Some(Bernoulli::new(new_spawn_chance).unwrap());
                },
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
    SetSpawnChance(f64),
}

/// Messages sent from the grid.
#[derive(Debug)]
pub enum FromSim {
    View(View),
}

/// Contains the data to display the simulation.
#[derive(Default, Debug)]
pub struct View {
    pub colors: Array2<(Color, usize)>,
    pub cells: usize,
    pub ticks: usize,
}

pub struct Sim {
    grid: LifeContainer,
    frames_elapsed: usize,
}

impl Sim {
    fn new(width: usize, height: usize, openness: usize) -> Self {
        use crate::gridgen;
        let mut grid = SquareGrid::<Evonomics>::new(width, height);
        let rng = unsafe { rng() };
        let open_scale = openness + 1;
        let (open_width, open_height) = (width / open_scale, height / open_scale);
        let os = (open_height, open_width);
        let walls = gridgen::generate_walls(rng, os);
        for (ix, cell) in grid.get_cells_mut().iter_mut().enumerate() {
            if rng.sample(*SOURCE_SPAWN_DISTRIBUTION) {
                cell.ty = CellType::Source;
            }
            let x = ix % width;
            let y = ix / width;
            let ox = x / open_scale;
            let oy = y / open_scale;
            if ox >= open_width || oy >= open_height {
                continue;
            }
            let op = (oy, ox);

            let dir = |dy, dx| walls[gridgen::dir(op, os, (dy, dx))];

            if dir(0, 0) {
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
            // Cycle the grid.
            self.grid.cycle();
            // Extract all trades.
            let trades: Vec<(usize, Trade)> = self
                .grid
                .get_cells_mut()
                .iter_mut()
                .enumerate()
                .filter_map(|(ix, cell)| cell.trade.take().map(|trade| (ix, trade)))
                .collect();
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
                    .map(|c| {
                        (
                            c.color(),
                            match &c.brain {
                                Some(brain) => brain.generation,
                                None => 0,
                            },
                        )
                    })
                    .collect::<Vec<(Color, usize)>>(),
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
