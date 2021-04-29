use crate::primitives::*;
use crate::{Point, RTreeObject};
use rand::distributions::{Distribution, Uniform};
use rand::{Rng, SeedableRng};
use rand_hc::Hc128Rng;

pub type Seed = [u8; 32];

pub const SEED_1: &Seed = b"wPYxAkIiHcEmSBAxQFoXFrpYToCe1B71";
pub const SEED_2: &Seed = b"4KbTVjPT4DXSwWAsQM5dkWWywPKZRfCX";

pub fn create_random_integers<P: Point<Scalar = i32>>(num_points: usize, seed: &Seed) -> Vec<P> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = Hc128Rng::from_seed(*seed);
    let range = Uniform::from(-100_000..100_000);

    for _ in 0..num_points {
        let p = Point::generate(|_| rng.sample(range));
        result.push(p);
    }
    result
}

pub fn create_random_points(num_points: usize, seed: &Seed) -> Vec<[f64; 2]> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = Hc128Rng::from_seed(*seed);
    for _ in 0..num_points {
        result.push(rng.gen());
    }
    result
}

pub fn create_random_lines(num_lines: usize, seed: &Seed) -> Vec<Line<[f64; 2]>> {
    let mut result = Vec::with_capacity(num_lines);
    let mut rng = Hc128Rng::from_seed(*seed);
    let factor = 10. / num_lines as f64;
    for _ in 0..num_lines {
        let point: [f64; 2] = rng.gen();
        let offset: [f64; 2] = rng.gen();
        result.push(Line::new(
            point,
            [point[0] + offset[1] * factor, point[1] + offset[1] * factor],
        ));
    }
    result
}

pub fn create_random_rectangles(num_rectangles: usize, seed: &Seed) -> Vec<Rectangle<[f64; 2]>> {
    let lines = create_random_lines(num_rectangles, seed);
    lines.iter().map(|line| line.envelope().into()).collect()
}
