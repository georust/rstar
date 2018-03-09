pub fn create_random_points(num_points: usize, seed: [u32; 4]) -> Vec<[f32; 2]> {
    use rand::{XorShiftRng, SeedableRng, Rng};

    let mut result = Vec::with_capacity(num_points);
    let mut rng = XorShiftRng::from_seed(seed);
    for _ in 0 .. num_points {
        result.push([rng.gen(), rng.gen()]);
    }
    result
}

