use rstar::RStarInsertionStrategy;
use rtree::InsertionStrategy;

pub trait RTreeParams {
    const MIN_SIZE: usize;
    const MAX_SIZE: usize;
    const REINSERTION_COUNT: usize;
    type DefaultInsertionStrategy: InsertionStrategy;

    fn debug_output() -> String {
        format!(
            "MinSize: {}, MaxSize: {}, ReinsertionCount: {}",
            Self::MIN_SIZE,
            Self::MAX_SIZE,
            Self::REINSERTION_COUNT
        )
    }
}

pub struct DefaultParams;

impl RTreeParams for DefaultParams {
    const MIN_SIZE: usize = 3;
    const MAX_SIZE: usize = 6;
    const REINSERTION_COUNT: usize = 1;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}
