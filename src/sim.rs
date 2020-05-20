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
use min_max_heap::MinMaxHeap;
use ndarray::Array2;
use rand::{distributions::Bernoulli, seq::SliceRandom, Rng};
use rayon::prelude::*;
use std::iter::once;
use tokio::task::block_in_place;

type LifeContainer = SquareGrid<'static, Evonomics>;

mod brain;

const FOOD_COLOR_MULTIPLIER: f32 = 0.05;
const MONEY_COLOR_MULTIPLIER: f32 = 0.1;

// starting food for cell
const SPAWN_FOOD: u32 = 16;
const MOVE_PENALTY: u32 = 8;

static mut CORNACOPIA_FOOD_SPAWN: u32 = 0;
static mut CELL_SPAWN_DISTRIBUTION: Option<Bernoulli> = None;
static mut MUTATE_DISTRIBUTION: Option<Bernoulli> = None;
static mut CORNACOPIA_FOOD_DISTRIBUTION: Option<Bernoulli> = None;
static mut NORMAL_FOOD_DISTRIBUTION: Option<Bernoulli> = None;

const RESERVE_MULTIPLIER: u32 = 64;

const REPO: bool = false;

#[derive(Clone, Debug)]
pub struct Trade {
    pub rate: i32,
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
                    spend: 0,
                    moved: true,
                    trade: None,
                },
                MooreNeighbors::new(|_| Move {
                    food: 0,
                    money: 0,
                    brain: None,
                }),
            );
        }
        // Closure for just existing (consuming food and nothing happening).
        let just_exist = |trade| {
            (
                Diff {
                    consume: 1,
                    spend: 0,
                    moved: false,
                    trade,
                },
                MooreNeighbors::new(|_| Move {
                    food: 0,
                    money: 0,
                    brain: None,
                }),
            )
        };
        let decision = cell
            .brain
            .as_ref()
            .map(|brain| {
                const NEIGHBOR_INPUTS: usize = 5;
                const SELF_INPUTS: usize = 2;
                const INPUTS: usize = NEIGHBOR_INPUTS * 4 + SELF_INPUTS;
                let boolnum = |n| if n { 1.0 } else { 0.0 };
                let mut inputs: ArrayVec<[f64; INPUTS]> = neighbors
                    .iter()
                    .flat_map(|n| {
                        once(boolnum(n.brain.is_some()))
                            .chain(once(boolnum(n.ty == CellType::Wall)))
                            .chain(once(n.food as f64))
                            .chain(once(n.signal))
                            .chain(once(n.money as f64))
                    })
                    .chain(once(cell.food as f64))
                    .chain(once(cell.money as f64))
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
                            spend: cell.money,
                            moved: true,
                            trade: None,
                        },
                        MooreNeighbors::new(|nd| {
                            if nd == dir {
                                Move {
                                    food: cell.food - 1 - MOVE_PENALTY,
                                    money: cell.money,
                                    brain: cell.brain.clone(),
                                }
                            } else {
                                Move {
                                    food: 0,
                                    money: 0,
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
                            spend: cell.money / 2,
                            moved: false,
                            trade: None,
                        },
                        MooreNeighbors::new(|nd| {
                            if nd == dir {
                                Move {
                                    food: cell.food / 2 - MOVE_PENALTY / 2,
                                    money: cell.money / 2,
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
                                    money: 0,
                                    brain: None,
                                }
                            }
                        }),
                    )
                } else {
                    just_exist(None)
                }
            }
            Decision::Trade(rate, food) => {
                // Only trade if we can actually make the trade.
                let cost = -rate * food;
                if food < cell.food as i32 && cost <= cell.money as i32 {
                    just_exist(Some(Trade { rate, food }))
                } else {
                    just_exist(None)
                }
            }
            Decision::Nothing => just_exist(None),
        }
    }

    fn update(cell: &mut Cell, diff: Diff, moves: Self::MoveNeighbors) {
        // Handle money movement (even if wall so that it can be reclaimed by reserve).
        cell.money += moves.clone().iter().map(|m| m.money).sum::<u32>();
        if cell.ty != CellType::Wall {
            let rng = unsafe { rng() };
            // Handle food reduction from diff.
            cell.food = cell.food.saturating_sub(diff.consume);
            // Handle money reduction from diff.
            cell.money = cell.money.saturating_sub(diff.spend);

            // Handle taking the brain.
            if diff.moved {
                cell.brain.take();
            }

            // Create trade.
            cell.trade = diff.trade;

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
            cell.food += moves.clone().iter().map(|m| m.food).sum::<u32>();

            // Handle mutation.
            if let Some(ref mut brain) = cell.brain {
                if rng.sample(unsafe {
                    match MUTATE_DISTRIBUTION {
                        Some(v) => v,
                        None => Bernoulli::new(0.0001).unwrap(),
                    }
                }) {
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
                if rng.sample(unsafe {
                    match CORNACOPIA_FOOD_DISTRIBUTION {
                        Some(val) => val,
                        None => Bernoulli::new(0.0).unwrap(),
                    }
                }) {
                    cell.food += unsafe { CORNACOPIA_FOOD_SPAWN };
                }
            } else {
                if rng.sample(unsafe {
                    match NORMAL_FOOD_DISTRIBUTION {
                        Some(val) => val,
                        None => Bernoulli::new(0.01).unwrap(),
                    }
                }) {
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

fn cap_color(n: f32, cap: f32) -> f32 {
    if n > cap {
        cap
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
                    let food_color = cap_color(FOOD_COLOR_MULTIPLIER * self.food as f32, 0.3);
                    let money_color = cap_color(MONEY_COLOR_MULTIPLIER * self.money as f32, 1.0);
                    Color::from_rgb(
                        money_color,
                        if food_color > money_color {
                            food_color
                        } else {
                            money_color
                        },
                        money_color,
                    )
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Move {
    food: u32,
    money: u32,
    brain: Option<Brain>,
}

#[derive(Clone, Debug)]
pub struct Diff {
    consume: u32,
    spend: u32,
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
    cornacopia_count_probability: f64,
) -> (Sender<ToSim>, Receiver<FromSim>, impl Future<Output = ()>) {
    let (oncoming_tx, mut oncoming) = mpsc::channel(inbound);
    let (mut outgoing, outgoing_rx) = mpsc::channel(outbound);

    let mut sim = Sim::new(width, height, openness, cornacopia_count_probability);
    let task = async move {
        while let Some(oncoming) = oncoming.next().await {
            match oncoming {
                ToSim::Tick(times) => {
                    for _ in 0..times {
                        sim = block_in_place(move || sim.tick());
                        outgoing.send(sim.market()).await.ok();
                    }
                    let view = block_in_place(|| sim.view(times));
                    outgoing.send(FromSim::View(view)).await.ok();
                }
                ToSim::SetSpawnChance(new_spawn_chance) => unsafe {
                    CELL_SPAWN_DISTRIBUTION = Some(Bernoulli::new(new_spawn_chance).unwrap());
                },
                ToSim::SetCornacopiaChance(val) => unsafe {
                    CORNACOPIA_FOOD_DISTRIBUTION = Some(Bernoulli::new(val).unwrap());
                },
                ToSim::SetCornacopiaBounty(val) => unsafe {
                    CORNACOPIA_FOOD_SPAWN = val;
                },
                ToSim::SetMutationChance(val) => unsafe {
                    MUTATE_DISTRIBUTION = Some(Bernoulli::new(val).unwrap());
                },
                ToSim::SetGeneralFoodChance(val) => unsafe {
                    NORMAL_FOOD_DISTRIBUTION = Some(Bernoulli::new(val).unwrap());
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
    SetMutationChance(f64),
    SetGeneralFoodChance(f64),
    SetCornacopiaBounty(u32),
    SetCornacopiaChance(f64),
}

/// Messages sent from the grid.
#[derive(Debug)]
pub enum FromSim {
    View(View),
    Market {
        bid: Option<i32>,
        ask: Option<i32>,
        reserve: u32,
        buy_volume: u32,
        sell_volume: u32,
    },
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
    reserve: u32,
    last_bid: Option<i32>,
    last_ask: Option<i32>,
    buy_volume: u32,
    sell_volume: u32,
}

impl Sim {
    fn new(
        width: usize,
        height: usize,
        openness: usize,
        cornacopia_count_probability: f64,
    ) -> Self {
        use crate::gridgen;
        let mut grid = SquareGrid::<Evonomics>::new(width, height);
        let rng = unsafe { rng() };
        let open_scale = openness + 1;
        let (open_width, open_height) = (width / open_scale, height / open_scale);
        let os = (open_height, open_width);
        let walls = gridgen::generate_walls(rng, os);
        let cornacopia_spawn_dist = Bernoulli::new(cornacopia_count_probability).unwrap();
        for (ix, cell) in grid.get_cells_mut().iter_mut().enumerate() {
            if rng.sample(cornacopia_spawn_dist) {
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
            reserve: width as u32 * height as u32 * RESERVE_MULTIPLIER,

            last_bid: None,
            last_ask: None,
            buy_volume: 0,
            sell_volume: 0,
        }
    }

    pub fn tick(mut self) -> Self {
        use std::cmp::Ordering;

        #[derive(PartialEq, Eq)]
        struct Order {
            index: usize,
            rate: i32,
            food: i32,
        }

        #[derive(Debug, PartialEq, Eq)]
        enum Intent {
            Bid,
            Ask,
            Nothing,
        }

        impl Order {
            fn intent(&self) -> Intent {
                if self.food < 0 {
                    Intent::Bid
                } else if self.food > 0 {
                    Intent::Ask
                } else {
                    Intent::Nothing
                }
            }
        }

        impl PartialOrd for Order {
            fn partial_cmp(&self, other: &Order) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for Order {
            fn cmp(&self, other: &Order) -> Ordering {
                self.rate.cmp(&other.rate)
            }
        }

        // Cycle the grid.
        self.grid.cycle();
        // Extract all trades.
        let mut orders: Vec<Order> = self
            .grid
            .get_cells_mut()
            .iter_mut()
            .enumerate()
            .filter_map(|(ix, cell)| cell.trade.take().map(|trade| (ix, trade)))
            .map(|(index, Trade { rate, food })| Order { index, rate, food })
            .collect();
        // Put the trades into a random order.
        orders.shuffle(unsafe { rng() });

        // Reset buy and sell volume.
        self.buy_volume = 0;
        self.sell_volume = 0;
        let mut bids: MinMaxHeap<Order> = MinMaxHeap::new();
        let mut asks: MinMaxHeap<Order> = MinMaxHeap::new();
        let fulfill = |sim: &mut Self, new: &mut Order, existing: &mut Order| {
            let rate = existing.rate;
            let num = std::cmp::min(new.food.abs(), existing.food.abs());
            {
                let new_cell = &mut sim.grid.get_cells_mut()[new.index];
                new_cell.money = (new_cell.money as i32 + rate * num * new.food.signum()) as u32;
                new_cell.food = (new_cell.food as i32 - num * new.food.signum()) as u32;
                new.food -= new.food.signum() * num;
            }
            {
                let existing_cell = &mut sim.grid.get_cells_mut()[existing.index];
                existing_cell.money =
                    (existing_cell.money as i32 + rate * num * existing.food.signum()) as u32;
                existing_cell.food =
                    (existing_cell.food as i32 - num * existing.food.signum()) as u32;
                existing.food -= existing.food.signum() * num;
            }
            sim.buy_volume += num as u32;
            sim.sell_volume += num as u32;
        };
        // Allows an ask order to be fulfilled by the reserve at a rate of one money per food.
        let fulfill_reserve = |sim: &mut Self, order: &mut Order| {
            let num = std::cmp::min(order.food, sim.reserve as i32);
            {
                let cell = &mut sim.grid.get_cells_mut()[order.index];
                cell.money = (cell.money as i32 + num * order.food.signum()) as u32;
                cell.food = (cell.food as i32 - num * order.food.signum()) as u32;
                order.food -= order.food.signum() * num;
            }
            sim.reserve -= num as u32;
            sim.sell_volume += num as u32;
        };
        // Allows a bid order to buy food from the reserve at one money per food.
        let food_reserve = |sim: &mut Self, order: &mut Order| {
            // We will take as much as there is in the order.
            let num = -order.food;
            {
                let cell = &mut sim.grid.get_cells_mut()[order.index];
                cell.money = (cell.money as i32 + num * order.food.signum()) as u32;
                cell.food = (cell.food as i32 - num * order.food.signum()) as u32;
                order.food -= order.food.signum() * num;
            }
            sim.reserve += num as u32;
            sim.buy_volume += num as u32;
        };
        for mut order in orders {
            let intent = order.intent();

            match intent {
                Intent::Bid => {
                    // Keep resolving the bid with asks until the order runs out or the asks are too high.
                    loop {
                        if let Some(mut ask) = asks.pop_min() {
                            if ask.rate > order.rate {
                                // The best asking price was higher than our bid, so just push the bid to the bids.
                                if order.food != 0 {
                                    bids.push(order);
                                }
                                break;
                            } else {
                                // Fulfill as much as possible on both ends.
                                fulfill(&mut self, &mut order, &mut ask);

                                // If the ask is not complete, we must return it to the asks.
                                if ask.food != 0 {
                                    asks.push(ask);
                                }

                                // If the order is complete, we can break from this loop.
                                if order.food == 0 {
                                    break;
                                }
                            }
                        } else {
                            if REPO {
                                // Only repo the money if there are no other ask offers out there.
                                if order.rate >= 1 {
                                    food_reserve(&mut self, &mut order);
                                }
                            }
                            // There were no asks, so push our bid.
                            if order.food != 0 {
                                bids.push(order);
                            }
                            break;
                        }
                    }
                }
                Intent::Ask => {
                    // Keep resolving the ask with bids until the order runs out or the bids are too low.
                    loop {
                        if let Some(mut bid) = bids.pop_max() {
                            if bid.rate < order.rate {
                                // The best bid price was lower than our ask, so just push the ask to the asks.
                                // Try to sell to the reserve.
                                if order.rate <= 1 {
                                    fulfill_reserve(&mut self, &mut order);
                                }
                                // There were no bids, so push our ask.
                                if order.food != 0 {
                                    asks.push(order);
                                }
                                break;
                            } else {
                                // If the reserve provides a better deal, then use the reserve.
                                if bid.rate < 1 {
                                    fulfill_reserve(&mut self, &mut order);
                                }
                                // Fulfill as much as possible on both ends.
                                fulfill(&mut self, &mut order, &mut bid);

                                // If the bid is not complete, we must return it to the bids.
                                if bid.food != 0 {
                                    bids.push(bid);
                                }

                                // If the order is complete, we can break from this loop.
                                if order.food == 0 {
                                    break;
                                }
                            }
                        } else {
                            // Try to sell to the reserve.
                            if order.rate <= 1 {
                                fulfill_reserve(&mut self, &mut order);
                            }
                            // There were no bids, so push our ask.
                            if order.food != 0 {
                                asks.push(order);
                            }
                            break;
                        }
                    }
                }
                Intent::Nothing => {}
            }
        }
        self.last_bid = bids.pop_max().map(|order| order.rate);
        self.last_ask = asks.pop_min().map(|order| order.rate);
        // Return all the money on walls to the reserve
        for cell in self.grid.get_cells_mut() {
            if cell.ty == CellType::Wall {
                self.reserve += cell.money;
                cell.money = 0;
            }
        }
        assert_eq!(
            self.grid.get_cells().iter().map(|c| c.money).sum::<u32>() + self.reserve,
            self.grid.get_width() as u32 * self.grid.get_height() as u32 * RESERVE_MULTIPLIER
        );

        self
    }

    pub fn market(&self) -> FromSim {
        FromSim::Market {
            ask: self.last_ask,
            bid: self.last_bid,
            reserve: self.reserve,
            buy_volume: self.buy_volume,
            sell_volume: self.sell_volume,
        }
    }

    pub fn view(&self, times: usize) -> View {
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
            ticks: times,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TickError {
    JoinFailed,
}
