use point::Point;

pub trait Envelope: Clone + Copy + PartialEq + ::std::fmt::Debug {
    type Point: Point;

    fn new_empty() -> Self;

    fn contains_point(&self, point: &Self::Point) -> bool;
    fn contains_envelope(&self, aabb: &Self) -> bool;

    fn merge(&mut self, other: &Self);
    fn merged(&self, other: &Self) -> Self;
    fn intersects(&self, other: &Self) -> bool;
    fn intersection_area(&self, other: &Self) -> <Self::Point as Point>::Scalar;

    fn area(&self) -> <Self::Point as Point>::Scalar;

    fn distance_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;
    fn min_max_dist_2(&self, point: &Self::Point) -> <Self::Point as Point>::Scalar;
    fn center(&self) -> Self::Point;
    fn margin_value(&self) -> <Self::Point as Point>::Scalar;

    fn align_envelopes<T, F>(axis: usize, envelopes: &mut [T], f: F)
    where
        F: Fn(&T) -> Self;
}
