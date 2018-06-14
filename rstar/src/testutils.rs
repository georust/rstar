use rand::{Rng, SeedableRng, XorShiftRng};

pub fn create_random_points(num_points: usize, seed: [u8; 16]) -> Vec<[f64; 2]> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = XorShiftRng::from_seed(seed);
    for _ in 0..num_points {
        result.push(rng.gen());
    }
    result
}
