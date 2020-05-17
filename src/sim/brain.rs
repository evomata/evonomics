use arrayvec::ArrayVec;
use gridsim::{moore::MooreDirection, Direction};
use iced::Color;
use itertools::Itertools;
use nalgebra::{Complex, Unit, UnitComplex};
use rand::{
    distributions::{Bernoulli, Distribution, Standard},
    seq::SliceRandom,
    Rng,
};
use rand_distr::Exp1;
use std::sync::Arc;

const NUM_STATE: usize = 4;
const MAX_EXECUTE: usize = 128;
const INITIAL_GENOME_SCALE: f64 = 256.0;
const INITIAL_ENTRIES_SCALE: f64 = 64.0;

lazy_static::lazy_static! {
    static ref HALF_CHANCE: Bernoulli = Bernoulli::new(0.5).unwrap();
}

/// The hue in radians.
fn random_color<R: Rng + ?Sized>(rng: &mut R) -> f64 {
    rng.gen_range(0.0, 2.0 * std::f64::consts::PI)
}

fn merge_colors(rng: &mut impl Rng, colors: impl Iterator<Item = f64>) -> f64 {
    let v = colors
        .map(|v| UnitComplex::new(v).into_inner())
        .sum::<Complex<f64>>();
    let angle = Unit::new_normalize(v).angle();
    if angle.is_finite() {
        angle
    } else {
        random_color(rng)
    }
}

pub fn combine(rng: &mut impl Rng, brains: impl IntoIterator<Item = Brain>) -> Brain {
    let brains = brains.into_iter().collect_vec();
    let code = Arc::new(crossover(rng, brains.iter().map(|b| (*b.code).clone())));
    let memory = std::iter::repeat(0.0).collect();
    Brain {
        color: merge_colors(rng, brains.iter().map(|b| b.color)),
        rotation: rng.gen_range(0, 4),
        generation: brains.iter().map(|brain| brain.generation).max().unwrap(),
        memory,
        code,
    }
}

#[derive(Clone, Debug)]
pub struct Brain {
    /// The hue in radians.
    color: f64,
    /// Rotation counter-clockwise (direction of iteration in gridsim)
    rotation: usize,
    pub generation: usize,
    memory: ArrayVec<[f64; NUM_STATE]>,
    code: Arc<Dna>,
}

impl Brain {
    pub fn color(&self) -> Color {
        use palette::*;
        let hsv = Hsv::new(RgbHue::from_radians(self.color), 1.0, 1.0);
        let rgb = Srgb::<f64>::from_hsv(hsv);
        Color::from_rgb(rgb.red as f32, rgb.green as f32, rgb.blue as f32)
    }

    pub fn signal(&self) -> f64 {
        self.memory[0]
    }

    pub fn rotation(&self) -> usize {
        self.rotation
    }

    pub fn rotate(&self, mut decision: Decision) -> Decision {
        let rot = |mut dir: MooreDirection| {
            for _ in 0..self.rotation {
                dir = dir.turn_counterclockwise();
            }
            dir
        };
        match &mut decision {
            Decision::Divide(dir) => *dir = rot(*dir),
            Decision::Move(dir) => *dir = rot(*dir),
            Decision::Nothing | Decision::Trade(..) => {}
        }
        decision
    }

    pub fn decide(&mut self, rng: &mut impl Rng, inputs: &[f64]) -> Decision {
        let mut decision = Decision::Nothing;
        let mut entries = self.code.entries.clone();
        entries.shuffle(rng);
        for &entry in &entries {
            match self.code.execute(inputs, &self.memory, entry) {
                Action::Write(pos, v) => {
                    let writepos = pos as usize % self.memory.len();
                    self.memory[writepos] = v;
                }
                Action::RotateLeft => self.rotation = (self.rotation + 1) % 4,
                Action::RotateRight => self.rotation = (self.rotation + 3) % 4,
                action => decision = action.into(),
            }
        }
        self.rotate(decision)
    }

    pub fn mutate(&mut self, rng: &mut impl Rng) {
        Arc::make_mut(&mut self.code).mutate(rng);
    }
}

impl Distribution<Brain> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Brain {
        let memory = std::iter::repeat(0.0).collect();
        let rotation = rng.gen_range(0, 4);
        let code = Arc::new(rng.gen());
        let color = random_color(rng);
        Brain {
            color,
            rotation,
            generation: 0,
            memory,
            code,
        }
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

fn crossover(rng: &mut impl Rng, dnas: impl IntoIterator<Item = Dna>) -> Dna {
    let mut dnas: Vec<Dna> = dnas.into_iter().collect();

    // First shuffle the DNA to avoid bias.
    dnas.shuffle(rng);

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
            let position = rng.gen_range(0, genes.len() + 1);
            genes.insert(position, vec![]);
        }
    }

    // Now perform crossover by cycling beteween each DNA and taking a gene in order.
    let mut dna = Dna::default();
    for i in 0..highest_num_genes {
        let which = rng.gen_range(0, genes.len());
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
    fn mutate(&mut self, rng: &mut impl Rng) {
        // Handle the creation and removal of codons.
        if rng.sample(*HALF_CHANCE) {
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
        if !self.sequence.is_empty() && rng.sample(*HALF_CHANCE) {
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
                        if !o {
                            // If the condition is false, exit the gene.
                            break;
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
                Codon::Trade => {
                    let clamp = |n: f64| {
                        if n.is_finite() {
                            if n > 10_000.0 {
                                10_000.0
                            } else if n < -10_000.0 {
                                -10_000.0
                            } else {
                                n
                            }
                        } else {
                            0.0
                        }
                    };
                    match (stack.pop(), stack.pop()) {
                        (Some(a), Some(b)) => {
                            return Action::Trade(clamp(a) as i32, clamp(b) as i32)
                        }
                        _ => break,
                    }
                }
                Codon::SimpleTrade(a, b) => return Action::Trade(a, b),
                Codon::RotateLeft => return Action::RotateLeft,
                Codon::RotateRight => return Action::RotateRight,
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
    Trade,
    SimpleTrade(i32, i32),
    RotateLeft,
    RotateRight,
}

impl Distribution<Codon> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Codon {
        match rng.gen_range(0, 18) {
            0 => Codon::Add,
            1 => Codon::Sub,
            2 => Codon::Mul,
            3 => Codon::Div,
            4 => Codon::Literal(rng.gen::<f64>() * 4.0 - 2.0),
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
            12 => Codon::Trade,
            13 => Codon::RotateLeft,
            14 => Codon::RotateRight,
            _ => Codon::SimpleTrade(rng.gen_range(1, 50), rng.gen_range(-10, 10)),
        }
    }
}

pub enum Action {
    Write(u32, f64),
    Move(MooreDirection),
    Divide(MooreDirection),
    Trade(i32, i32),
    RotateLeft,
    RotateRight,
    Nothing,
}

pub enum Decision {
    Move(MooreDirection),
    Divide(MooreDirection),
    Trade(i32, i32),
    Nothing,
}

impl From<Action> for Decision {
    fn from(action: Action) -> Decision {
        match action {
            Action::Move(dir) => Decision::Move(dir),
            Action::Divide(dir) => Decision::Divide(dir),
            Action::Trade(a, b) => Decision::Trade(a, b),
            Action::Nothing => Decision::Nothing,
            _ => panic!("you shouldn't try to turn just any action into a decision"),
        }
    }
}
