use rand::distributions::{Distribution, Uniform};
use rand::{Rng, SeedableRng, XorShiftRng};

pub fn create_random_integers(num_points: usize, seed: [u8; 16]) -> Vec<[i32; 2]> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = XorShiftRng::from_seed(seed);
    let range = Uniform::from(-1000..1000);

    for _ in 0..num_points {
        let x = range.sample(&mut rng);
        let y = range.sample(&mut rng);
        result.push([x, y]);
    }
    result
}

pub fn create_random_points(num_points: usize, seed: [u8; 16]) -> Vec<[f64; 2]> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = XorShiftRng::from_seed(seed);
    for _ in 0..num_points {
        result.push(rng.gen());
    }
    result
}
