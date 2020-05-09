use futures::{
    channel::mpsc::{self, Receiver, Sender},
    prelude::*,
    Future,
};
use gridsim::{moore::*, Direction, Rule, SquareGrid};
use iced::Color;
use ndarray::Array2;
use rayon::prelude::*;
use tokio::task::spawn_blocking;

type LifeContainer = SquareGrid<'static, LAnt>;

/// The entrypoint for the grid.
pub fn run_sim(
    inbound: usize,
    outbound: usize,
) -> (Sender<ToSim>, Receiver<FromSim>, impl Future<Output = ()>) {
    let (oncoming_tx, mut oncoming) = mpsc::channel(inbound);
    let (mut outgoing, outgoing_rx) = mpsc::channel(outbound);

    let mut sim = Sim::default();
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

// Langton's Ant
#[derive(Clone, Debug)]
enum LAnt {}

impl<'a> Rule<'a> for LAnt {
    type Cell = CellState;
    type Neighbors = MooreNeighbors<&'a CellState>;

    fn rule(cell: CellState, neighbors: Self::Neighbors) -> CellState {
        MooreDirection::directions()
            .map(MooreDirection::inv)
            .find(|&d| neighbors[d].ant == Some(d))
            .map(|d| CellState {
                ant: Some(if cell.color {
                    d.turn_clockwise()
                } else {
                    d.turn_counterclockwise()
                }),
                color: !cell.color,
            })
            .unwrap_or(CellState {
                ant: None,
                color: cell.color,
            })
    }
}

#[derive(Debug, Clone, Default)]
struct CellState {
    ant: Option<MooreDirection>,
    color: bool,
}

impl CellState {
    fn color(&self) -> Color {
        if self.ant.is_some() {
            Color::from_rgb(1.0, 0.0, 0.0)
        } else if self.color {
            Color::WHITE
        } else {
            Color::from_rgb8(0x48, 0x4C, 0x54)
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sim {
    life: LifeContainer,
}

impl std::default::Default for Sim {
    fn default() -> Self {
        Sim {
            life: SquareGrid::<LAnt>::new_coords(
                Self::SIDE,
                Self::SIDE,
                vec![(
                    (0, 0),
                    CellState {
                        ant: Some(MooreDirection::Down),
                        color: false,
                    },
                )],
            ),
        }
    }
}

impl Sim {
    pub const SIDE: usize = 320;

    pub fn tick(mut self, times: usize) -> Self {
        for _ in 0..times {
            self.life.cycle();
        }
        self
    }

    pub fn view(&self) -> View {
        View {
            colors: Array2::from_shape_vec(
                (self.life.get_height(), self.life.get_width()),
                self.life
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
