use crate::test_utilities::{create_random_integers, SEED_1};
use crate::{Point, RTree, RTreeObject};
use std::collections::HashSet;
use std::fmt::Debug;
use std::hash::Hash;

#[test]
fn test_bulk_load_small() {
    let random_points = create_random_integers::<[i32; 2]>(50, SEED_1);
    create_and_check_bulk_loading_with_points(&random_points);
}

#[test]
fn test_bulk_load_large() {
    let random_points = create_random_integers::<[i32; 2]>(3000, SEED_1);
    create_and_check_bulk_loading_with_points(&random_points);
}

#[test]
fn test_bulk_load_with_different_sizes() {
    for size in (0..100).map(|i| i * 7) {
        test_bulk_load_with_size_and_dimension::<[i32; 2]>(size);
        test_bulk_load_with_size_and_dimension::<[i32; 3]>(size);
        test_bulk_load_with_size_and_dimension::<[i32; 4]>(size);
    }
}

fn test_bulk_load_with_size_and_dimension<P>(size: usize)
where
    P: Point<Scalar = i32> + RTreeObject + Send + Sync + Eq + Clone + Debug + Hash + 'static,
    P::Envelope: Send + Sync,
{
    let random_points = create_random_integers::<P>(size, SEED_1);
    create_and_check_bulk_loading_with_points(&random_points);
}

#[cfg(not(feature = "threadpool"))]
fn create_and_check_bulk_loading_with_points<P>(points: &[P])
where
    P: RTreeObject + Send + Sync + Eq + Clone + Debug + Hash + 'static,
    P::Envelope: Send + Sync,
{
    println!("Testing sequential loading ({} points)", points.len());
    create_and_check_method(points, RTree::bulk_load);
}

#[cfg(feature = "threadpool")]
fn create_and_check_bulk_loading_with_points<P>(points: &[P])
where
    P: RTreeObject + Send + Sync + Eq + Clone + Debug + Hash + 'static,
    P::Envelope: Send + Sync,
{
    println!("Testing sequential loading ({} points)", points.len());
    create_and_check_method(points, RTree::bulk_load);
    println!("Testing parallel loading ({} points)", points.len());
    create_and_check_method(points, RTree::bulk_load_parallel);
}

fn create_and_check_method<P, F>(points: &[P], f: F)
where
    P: RTreeObject + Eq + Clone + Debug + Hash,
    F: Fn(Vec<P>) -> RTree<P>,
{
    let tree = f(points.into());
    let set1: HashSet<_> = tree.iter().collect();
    let set2: HashSet<_> = points.iter().collect();
    assert_eq!(set1, set2);
    assert_eq!(tree.size(), points.len());
}
