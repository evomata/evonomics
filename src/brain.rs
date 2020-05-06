use gridsim::moore::MooreDirection;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use rand_distr::Exp1;
use std::sync::Arc;

const NUM_STATE: usize = 4;
const INPUTS: usize = 4;

pub struct Brain {
    state: [f64; NUM_STATE],
    code: Arc<Dna>,
}

impl Brain {
    fn think(&self, inputs: [f64; INPUTS]) -> Action {
        unimplemented!()
    }
}

struct Dna {
    sequence: Vec<Codon>,
    entries: Vec<usize>,
}

impl Distribution<Dna> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Dna {
        let sequence_len = rng.sample::<f64, _>(Exp1) as usize;
        let sequence = rng.sample_iter(Standard).take(sequence_len).collect();
        let entries_len = rng.sample::<f64, _>(Exp1) as usize;
        let entries = (0..entries_len)
            .map(|_| rng.gen_range(0, sequence_len))
            .collect();
        Dna { sequence, entries }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Codon {
    Add,
    Sub,
    Mul,
    Div,
    Less(i16, i16),
    Jump(i16),
    Literal(f64),
    Copy(u8),
    Read(u8),
    Input(u8),
    Write(u8),
    Move(MooreDirection),
    Nothing,
}

impl Distribution<Codon> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Codon {
        use Codon::*;
        match rng.gen_range(0, 12) {
            0 => Add,
            1 => Sub,
            2 => Mul,
            3 => Div,
            4 => Less(rng.gen(), rng.gen()),
            5 => Jump(rng.gen()),
            6 => Literal(rng.gen()),
            7 => Copy(rng.gen()),
            8 => Read(rng.gen()),
            9 => Input(rng.gen()),
            10 => Write(rng.gen()),
            11 => Move(match rng.gen_range(0, 4) {
                0 => MooreDirection::Right,
                1 => MooreDirection::Up,
                2 => MooreDirection::Left,
                3 => MooreDirection::Down,
                _ => unreachable!(),
            }),
            12 => Nothing,
            _ => unreachable!(),
        }
    }
}

pub enum Action {
    Write(u8, f64),
    Move(MooreDirection),
    Nothing,
}

pub enum Decision {
    Move(MooreDirection),
    Nothing,
}

impl From<Action> for Decision {
    fn from(action: Action) -> Decision {
        use Decision::*;
        match action {
            Action::Move(dir) => Move(dir),
            _ => Nothing,
        }
    }
}
