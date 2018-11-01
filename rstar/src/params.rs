use rstar::RStarInsertionStrategy;
use rtree::InsertionStrategy;

pub trait RTreeParams {
    const MIN_SIZE: usize;
    const MAX_SIZE: usize;

    type DefaultInsertionStrategy: InsertionStrategy;

    fn debug_output() -> String {
        format!("MinSize: {}, MaxSize: {}", Self::MIN_SIZE, Self::MAX_SIZE,)
    }
}

pub struct DefaultParams;

impl RTreeParams for DefaultParams {
    const MIN_SIZE: usize = 3;
    const MAX_SIZE: usize = 6;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}
