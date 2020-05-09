use gridsim::{moore::*, Direction, Rule, SquareGrid};

use std::future::Future;

/* FIXME  <LIFE CONTAINER>
          LifeContainer implementation is bad!
          **Look at how update and tick are performed and used**
*/
pub type LifeContainer = SquareGrid<'static, LAnt>;

// TODO: for parallel ticks
// use rayon::prelude::*;

// Langton's Ant
#[derive(Clone, Debug)]
pub enum LAnt {}

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
pub struct CellState {
    ant: Option<MooreDirection>,
    color: bool,
}

impl CellState {
    pub const SIZE: usize = 20;

    pub fn at(x: f32, y: f32) -> (isize, isize) {
        (
            (x.ceil() as isize).saturating_sub(1) / Self::SIZE as isize,
            (y.ceil() as isize).saturating_sub(1) / Self::SIZE as isize,
        )
    }

    pub fn is_ant(&self) -> bool {
        self.ant.is_some()
    }

    pub fn has_color(&self) -> bool {
        self.color
    }
}

#[derive(Debug, Clone)]
pub struct State {
    life: LifeContainer,
    // is the simulation currently busy?
    is_ticking: bool,
}

impl<'a> std::default::Default for State {
    fn default() -> Self {
        State {
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
            is_ticking: false,
        }
    }
}

impl State {
    pub const SIDE: usize = 320;

    // number of cells
    pub fn cell_count(&self) -> usize {
        1
    }

    // // is there a cell at x,y
    // pub fn cell_at(&self, x: usize, y: usize) -> bool {
    //     self.life.get_cell_at(x / CellState::SIZE, y / CellState::SIZE).ant.is_some()
    // }

    pub fn cells(&self) -> &[CellState] {
        self.life.get_cells()
    }

    // TODO: we will need to be able to select a saved cell and pass it to this function for this.
    //     pub fn populate(&mut self, cell: CellState) {
    // panic!("unimplemented");
    //         // if self.is_ticking {
    //         //     // store to pending var to add on update call
    //         // } else {
    //         //     // add cell
    //         // }
    //     }

    // TODO: I don't think we want to manually kill cells... remove this
    //     pub fn unpopulate(&mut self, cell: &CellState) {
    // panic!("unimplemented");
    //         // if self.is_ticking {
    //         //     // remove cell from pending
    //         // } else {
    //         //     // remove cell
    //         // }
    //     }

    pub fn update(&mut self, life: SquareGrid<'static, LAnt>) {
        // TODO  with mut life,  add cells which are pending, remove cells pending removal

        self.life = life;
        self.is_ticking = false;
    }

    pub fn tick(
        &mut self,
        amount: usize,
    ) -> Option<impl Future<Output = Result<SquareGrid<'static, LAnt>, TickError>>> {
        if self.is_ticking {
            return None;
        }

        self.is_ticking = true;

        let mut life = self.life.clone();

        Some(async move {
            tokio::task::spawn_blocking(move || {
                for _ in 0..amount {
                    life.cycle();
                }

                life
            })
            .await
            .map_err(|_| TickError::JoinFailed)
        })
    }

    pub fn gen_xy_pos(&self, ix: usize) -> (isize, isize) {
        (
            (ix % self.life.get_width()) as isize,
            (ix / self.life.get_width()) as isize,
        )
    }
}

#[derive(Debug, Clone)]
pub enum TickError {
    JoinFailed,
}
