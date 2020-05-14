use itertools::Itertools;
use ndarray::Array2;
use rand::{seq::SliceRandom, Rng};
use slab::Slab;
use std::collections::HashMap;
use std::iter::once;

fn wrap(pos: usize, shape: usize, delt: isize) -> usize {
    (pos + (shape as isize + delt) as usize) % shape
}

pub fn dir(pos: (usize, usize), shape: (usize, usize), delta: (isize, isize)) -> (usize, usize) {
    (wrap(pos.0, shape.0, delta.0), wrap(pos.1, shape.1, delta.1))
}

fn dirs(
    pos: (usize, usize),
    shape: (usize, usize),
) -> impl Iterator<Item = (usize, usize)> + Clone {
    let dir = |delta: (isize, isize)| dir(pos, shape, delta);
    once(dir((1, 0)))
        .chain(once(dir((0, 1))))
        .chain(once(dir((-1, 0))))
        .chain(once(dir((0, -1))))
}

pub fn generate_walls(rng: &mut impl Rng, shape: (usize, usize)) -> Array2<bool> {
    // The grid we will be modifying
    let mut grid = Array2::from_elem(shape, true);
    // A map from open tiles to the area they are connected to
    let mut open: HashMap<(usize, usize), usize> = HashMap::new();
    // An arena of areas which contain their respective open tiles
    let mut areas: Slab<Vec<(usize, usize)>> = Slab::new();
    // A vector of walls which are not connected to an open area
    let mut closed: Vec<(usize, usize)> = grid.indexed_iter().map(|(ix, _)| ix).collect();
    closed.shuffle(rng);
    for pos in closed {
        // Get the positions of its neighbors.
        let neighbors = dirs(pos, shape);
        // Get the areas this will connect with.
        let neighbor_areas: Vec<usize> = neighbors
            .clone()
            .filter_map(|n| open.get(&n).copied())
            .unique()
            .collect();
        // Get the number of open spots surrounding this wall.
        let num_open = neighbors.filter(|n| open.contains_key(&n)).count();

        // Create and finalize the area this belongs in.
        match neighbor_areas.len() {
            0 => {
                // Areas is empty, so this is a new area.
                let area = areas.insert(vec![pos]);
                // Add this new open space to the open spaces.
                open.insert(pos, area);
                // Mark the cell as open
                grid[pos] = false;
            }
            1 if num_open > 1 => {
                // If there is only one neighboring area and more than 1 spot, don't continue.
            }
            _ => {
                // Areas has at least one thing in it, so unify the areas.
                let final_area = neighbor_areas[0];
                for &neighbor in &neighbor_areas[1..] {
                    // Take the area vector out of the areas.
                    let area_vec = areas.remove(neighbor);
                    // Switch all the open spots to the final area.
                    for spot in &area_vec {
                        *open.get_mut(spot).unwrap() = final_area;
                    }
                    // Extend the final area with the spots.
                    areas[final_area].extend(area_vec.into_iter());
                }
                // Add this position to the area
                areas[final_area].push(pos);
                open.insert(pos, final_area);
                // Mark the cell as open
                grid[pos] = false;
            }
        }
    }
    grid
}
