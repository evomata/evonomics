use gridsim::moore::MooreDirection;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use rand_distr::Exp1;
use std::sync::Arc;

const NUM_STATE: usize = 4;
const MAX_EXECUTE: usize = 32;

pub struct Brain {
    memory: [f64; NUM_STATE],
    code: Arc<Dna>,
}

impl Brain {
    pub fn think(&mut self, inputs: &[f64]) -> Decision {
        let mut decision = Decision::Nothing;
        for &entry in &self.code.entries {
            match self.code.execute(inputs, &self.memory, entry) {
                Action::Write(pos, v) => self.memory[pos as usize % self.memory.len()] = v,
                action => decision = action.into(),
            }
        }
        decision
    }
}

impl Distribution<Brain> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Brain {
        let memory = [0.0; NUM_STATE];
        let code = Arc::new(rng.gen());
        Brain { memory, code }
    }
}

struct Dna {
    sequence: Vec<Codon>,
    entries: Vec<usize>,
}

impl Dna {
    fn execute(&self, inputs: &[f64], memory: &[f64], mut at: usize) -> Action {
        let mut stack = vec![];
        for _ in 0..MAX_EXECUTE {
            match self.sequence[at] {
                Codon::Add => {
                    if let Some(o) = stack.pop().and_then(|a| stack.pop().map(|b| a + b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Sub => {
                    if let Some(o) = stack.pop().and_then(|a| stack.pop().map(|b| a - b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Mul => {
                    if let Some(o) = stack.pop().and_then(|a| stack.pop().map(|b| a * b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Div => {
                    if let Some(o) = stack.pop().and_then(|a| stack.pop().map(|b| a / b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Literal(n) => stack.push(n),
                Codon::Less(t, f) => {
                    if let Some(o) = stack.pop().and_then(|a| stack.pop().map(|b| a < b)) {
                        let diff = if o { t } else { f };
                        at = ((at as i32 + diff) % self.sequence.len() as i32).abs() as usize;
                        // Do not follow through to the at incrementer.
                        continue;
                    } else {
                        break;
                    }
                }
                Codon::Jump(diff) => {
                    at = ((at as i32 + diff) % self.sequence.len() as i32).abs() as usize;
                    // Do not follow through to the at incrementer.
                    continue;
                }
                Codon::Copy(pos) => {
                    if stack.is_empty() {
                        break;
                    }
                    // Force pos to be within bounds.
                    let pos = pos as usize % stack.len();
                    let n = stack[stack.len() - 1 - pos];
                    stack.push(n);
                }
                Codon::Read(pos) => stack.push(memory[pos as usize % memory.len()]),
                Codon::Input(pos) => stack.push(inputs[pos as usize % inputs.len()]),
                Codon::Write(pos) => {
                    if let Some(n) = stack.pop() {
                        return Action::Write(pos, n);
                    } else {
                        break;
                    }
                }
                Codon::Move(dir) => return Action::Move(dir),
                Codon::Nothing => break,
            }
            at = (at + 1) % self.sequence.len();
        }
        Action::Nothing
    }
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
    Literal(f64),
    Less(i32, i32),
    Jump(i32),
    Copy(u32),
    Read(u32),
    Input(u32),
    Write(u32),
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
            4 => Literal(rng.gen()),
            5 => Less(rng.gen(), rng.gen()),
            6 => Jump(rng.gen()),
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
    Write(u32, f64),
    Move(MooreDirection),
    Nothing,
}

pub enum Decision {
    Move(MooreDirection),
    Nothing,
}

impl From<Action> for Decision {
    fn from(action: Action) -> Decision {
        match action {
            Action::Move(dir) => Decision::Move(dir),
            Action::Nothing => Decision::Nothing,
            _ => panic!("you shouldn't try to turn just any action into a decision"),
        }
    }
}
