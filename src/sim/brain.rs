use arrayvec::ArrayVec;
use gridsim::moore::MooreDirection;
use itertools::Itertools;
use rand::{
    distributions::{Distribution, Standard},
    seq::SliceRandom,
    Rng,
};
use rand_distr::Exp1;
use std::sync::Arc;

const NUM_STATE: usize = 4;
const MAX_EXECUTE: usize = 128;
const INITIAL_GENOME_SCALE: f64 = 256.0;
const INITIAL_ENTRIES_SCALE: f64 = 64.0;

pub fn combine(brains: impl IntoIterator<Item = Brain>) -> Brain {
    let brains = brains.into_iter().collect_vec();
    let code = Arc::new(crossover(brains.iter().map(|b| (*b.code).clone())));
    let memory = std::iter::repeat(0.0).collect();
    Brain { memory, code }
}

#[derive(Clone, Debug)]
pub struct Brain {
    memory: ArrayVec<[f64; NUM_STATE]>,
    code: Arc<Dna>,
}

impl Brain {
    pub fn decide(&mut self, inputs: &[f64]) -> Decision {
        let mut decision = Decision::Nothing;
        let mut entries = self.code.entries.clone();
        entries.shuffle(&mut rand::thread_rng());
        for &entry in &entries {
            match self.code.execute(inputs, &self.memory, entry) {
                Action::Write(pos, v) => {
                    let writepos = pos as usize % self.memory.len();
                    self.memory[writepos] = v;
                }
                action => decision = action.into(),
            }
        }
        decision
    }

    pub fn mutate(&mut self) {
        Arc::make_mut(&mut self.code).mutate();
    }
}

impl Distribution<Brain> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Brain {
        let memory = std::iter::repeat(0.0).collect();
        let code = Arc::new(rng.gen());
        Brain { memory, code }
    }
}

fn split_points<'a, T>(points: &'a [usize], items: &'a [T]) -> impl Iterator<Item = &'a [T]> {
    // If zero is already in there or if nothing is in the points at all, we dont want to add a zero.
    (if points.first().map(|&n| n != 0).unwrap_or(false) {
        Some(0)
    } else {
        None
    })
    .into_iter()
    .chain(points.iter().copied())
    .chain(std::iter::once(items.len()))
    .tuple_windows()
    .map(move |(a, b)| &items[a..b])
}

fn crossover(dnas: impl IntoIterator<Item = Dna>) -> Dna {
    let mut thread_rng = rand::thread_rng();
    let mut dnas: Vec<Dna> = dnas.into_iter().collect();

    // First shuffle the DNA to avoid bias.
    dnas.shuffle(&mut thread_rng);

    // Now we want to turn the DNA into "genes", for which there may be an unequal number on each DNA.
    let mut genes: Vec<Vec<Vec<Codon>>> = dnas
        .into_iter()
        .map(|dna| {
            // Entries are always sorted. Extract all the sequence ranges in the DNA (genes).
            split_points(&dna.entries, &dna.sequence)
                .map(|s| s.to_vec())
                .collect_vec()
        })
        .collect_vec();

    // Now we need to figure out the longest number of genes.
    let highest_num_genes = genes
        .iter()
        .map(|g| g.len())
        .max()
        .expect("cant crossover no cells");

    // Now we need to pad each one of the genes to be of this length.
    for genes in &mut genes {
        // Figure out how many genes need to be added.
        let off_by = highest_num_genes - genes.len();
        // Distribute empty genes randomly.
        for _ in 0..off_by {
            let position = thread_rng.gen_range(0, genes.len() + 1);
            genes.insert(position, vec![]);
        }
    }

    // Now perform crossover by cycling beteween each DNA and taking a gene in order.
    let mut dna = Dna::default();
    for i in 0..highest_num_genes {
        let which = i % genes.len();
        let gene = &genes[which][i][..];
        if !gene.is_empty() {
            let position = dna.sequence.len();
            dna.sequence.extend_from_slice(gene);
            dna.entries.push(position);
        }
    }
    dna
}

#[derive(Clone, Debug, Default)]
struct Dna {
    sequence: Vec<Codon>,
    entries: Vec<usize>,
}

impl Dna {
    fn mutate(&mut self) {
        let mut rng = rand::thread_rng();
        // Handle the creation and removal of codons.
        if rng.gen_bool(0.5) {
            // Add a codon.
            let position = rng.gen_range(0, self.sequence.len() + 1);
            self.sequence.insert(position, rng.gen::<Codon>());
            // Move entries.
            for entry in &mut self.entries {
                if *entry >= position {
                    *entry += 1;
                }
            }
        } else if !self.sequence.is_empty() {
            // Remove a codon.
            let position = rng.gen_range(0, self.sequence.len());
            self.sequence.remove(position);
            // Remove any entries for that codon.
            self.entries.retain(|&e| e != position);
            // Move entries.
            for entry in &mut self.entries {
                if *entry > position {
                    *entry -= 1;
                }
            }
        }

        // Handle the creation and removal of entry points.
        if !self.sequence.is_empty() && rng.gen_bool(0.5) {
            // Add an entry.
            let position = rng.gen_range(0, self.entries.len() + 1);
            // Do not add it if it is not unique.
            if !self.entries.contains(&position) {
                self.entries
                    .insert(position, rng.gen_range(0, self.sequence.len()));
                self.entries.sort_unstable();
            }
        } else if !self.entries.is_empty() {
            // Remove an entry.
            let position = rng.gen_range(0, self.entries.len());
            self.entries.remove(position);
        }
    }

    fn execute(&self, inputs: &[f64], memory: &[f64], mut at: usize) -> Action {
        let mut stack = vec![];
        for _ in 0..MAX_EXECUTE {
            match self.sequence[at] {
                Codon::Add => {
                    if let Some(o) = stack.pop().and_then(|b| stack.pop().map(|a| a + b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Sub => {
                    if let Some(o) = stack.pop().and_then(|b| stack.pop().map(|a| a - b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Mul => {
                    if let Some(o) = stack.pop().and_then(|b| stack.pop().map(|a| a * b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Div => {
                    if let Some(o) = stack.pop().and_then(|b| stack.pop().map(|a| a / b)) {
                        stack.push(o);
                    } else {
                        break;
                    }
                }
                Codon::Literal(n) => stack.push(n),
                Codon::Less => {
                    if let Some(o) = stack.pop().and_then(|b| stack.pop().map(|a| a < b)) {
                        // Skip the next instruction if its false. It is only executed if true.
                        if !o {
                            at = (at + 1) % self.sequence.len();
                        }
                    } else {
                        break;
                    }
                }
                Codon::Copy(pos) => {
                    if stack.is_empty() || pos > stack.len() as u32 {
                        break;
                    }
                    let n = stack[stack.len() - 1 - pos as usize];
                    stack.push(n);
                }
                Codon::Read(pos) => stack.push(memory[pos as usize]),
                Codon::Input(pos) => stack.push(inputs[pos as usize % inputs.len()]),
                Codon::Write(pos) => {
                    if let Some(n) = stack.pop() {
                        return Action::Write(pos, n);
                    } else {
                        break;
                    }
                }
                Codon::Move(dir) => return Action::Move(dir),
                Codon::Divide(dir) => return Action::Divide(dir),
                Codon::Nothing => break,
            }
            at = (at + 1) % self.sequence.len();
        }
        Action::Nothing
    }
}

impl Distribution<Dna> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Dna {
        let sequence_len = (rng.sample::<f64, _>(Exp1) * INITIAL_GENOME_SCALE) as usize;
        let sequence = rng.sample_iter(Standard).take(sequence_len).collect();
        let entries = {
            if sequence_len == 0 {
                vec![]
            } else {
                let entries_len = (rng.sample::<f64, _>(Exp1) * INITIAL_ENTRIES_SCALE) as usize;
                let mut entries: Vec<usize> = (0..entries_len)
                    .map(|_| rng.gen_range(0, sequence_len))
                    .unique()
                    .collect();
                entries.sort_unstable();
                entries
            }
        };
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
    Less,
    Copy(u32),
    Read(u32),
    Input(u32),
    Write(u32),
    Move(MooreDirection),
    Divide(MooreDirection),
    Nothing,
}

impl Distribution<Codon> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Codon {
        match rng.gen_range(0, 13) {
            0 => Codon::Add,
            1 => Codon::Sub,
            2 => Codon::Mul,
            3 => Codon::Div,
            4 => Codon::Literal(rng.gen()),
            5 => Codon::Less,
            6 => Codon::Copy(rng.gen()),
            7 => Codon::Read(rng.gen::<u32>() % NUM_STATE as u32),
            8 => Codon::Input(rng.gen()),
            9 => Codon::Write(rng.gen::<u32>() % NUM_STATE as u32),
            10 => Codon::Move(match rng.gen_range(0, 4) {
                0 => MooreDirection::Right,
                1 => MooreDirection::Up,
                2 => MooreDirection::Left,
                3 => MooreDirection::Down,
                _ => unreachable!(),
            }),
            11 => Codon::Divide(match rng.gen_range(0, 4) {
                0 => MooreDirection::Right,
                1 => MooreDirection::Up,
                2 => MooreDirection::Left,
                3 => MooreDirection::Down,
                _ => unreachable!(),
            }),
            12 => Codon::Nothing,
            _ => unreachable!(),
        }
    }
}

pub enum Action {
    Write(u32, f64),
    Move(MooreDirection),
    Divide(MooreDirection),
    Nothing,
}

pub enum Decision {
    Move(MooreDirection),
    Divide(MooreDirection),
    Nothing,
}

impl From<Action> for Decision {
    fn from(action: Action) -> Decision {
        match action {
            Action::Move(dir) => Decision::Move(dir),
            Action::Divide(dir) => Decision::Divide(dir),
            Action::Nothing => Decision::Nothing,
            _ => panic!("you shouldn't try to turn just any action into a decision"),
        }
    }
}
