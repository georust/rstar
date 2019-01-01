mod bulk_load_common;
#[cfg(feature = "threadpool")]
mod bulk_load_parallel;
mod bulk_load_sequential;

#[cfg(test)]
mod bulk_load_tests;

#[cfg(feature = "threadpool")]
pub use self::bulk_load_parallel::bulk_load_parallel;
pub use self::bulk_load_sequential::bulk_load_sequential;
