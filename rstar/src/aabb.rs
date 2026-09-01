use crate::point::{max_inline, Point, PointExt, RTreeNum};
use crate::{Envelope, RTreeObject};
use num_traits::{Bounded, One, Zero};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An n-dimensional axis aligned bounding box (AABB).
///
/// An object's AABB is the smallest box totally encompassing an object
/// while being aligned to the current coordinate system.
/// Although these structures are commonly called bounding _boxes_, they exist in any
/// dimension.
///
/// Note that AABBs cannot be inserted into r-trees. Use the
/// [Rectangle](crate::primitives::Rectangle) struct for this purpose.
///
/// # Type arguments
/// `P`: The struct is generic over which point type is used. Using an n-dimensional point
/// type will result in an n-dimensional bounding box.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AABB<P>
where
    P: Point,
{
    lower: P,
    upper: P,
}

impl<P> AABB<P>
where
    P: Point,
{
    /// Returns the AABB encompassing a single point.
    pub fn from_point(p: P) -> Self {
        AABB {
            lower: p.clone(),
            upper: p,
        }
    }

    /// Returns the AABB's lower corner.
    ///
    /// This is the point contained within the AABB with the smallest coordinate value in each
    /// dimension.
    pub fn lower(&self) -> P {
        self.lower.clone()
    }

    /// Returns the AABB's upper corner.
    ///
    /// This is the point contained within the AABB with the largest coordinate value in each
    /// dimension.
    pub fn upper(&self) -> P {
        self.upper.clone()
    }

    /// Creates a new AABB encompassing two points.
    pub fn from_corners(p1: P, p2: P) -> Self {
        Self {
            lower: p1.min_point(&p2),
            upper: p1.max_point(&p2),
        }
    }

    /// Returns the AABB from already known lower/upper bounds.
    pub fn from_bounds(lower: P, upper: P) -> Self {
        debug_assert_eq!(lower.min_point(&upper), lower);
        debug_assert_eq!(lower.max_point(&upper), upper);
        Self { lower, upper }
    }

    /// Creates a new AABB from a center and a distance.
    ///
    /// Creates the smallest AABB which includes all points within `distance` of `center`.
    pub fn from_center(center: P, distance: P::Scalar) -> Self {
        let distance = P::from_value(distance);

        let p1 = center.add(&distance);
        let p2 = center.sub(&distance);

        Self::from_corners(p1, p2)
    }

    /// Creates a new AABB encompassing a collection of points.
    pub fn from_points<'a, I>(i: I) -> Self
    where
        I: IntoIterator<Item = &'a P> + 'a,
        P: 'a,
    {
        i.into_iter().fold(
            Self {
                lower: P::from_value(P::Scalar::max_value()),
                upper: P::from_value(P::Scalar::min_value()),
            },
            |aabb, p| Self {
                lower: aabb.lower.min_point(p),
                upper: aabb.upper.max_point(p),
            },
        )
    }

    /// Returns the point within this AABB closest to a given point.
    ///
    /// If `point` is contained within the AABB, `point` will be returned.
    pub fn min_point(&self, point: &P) -> P {
        self.upper.min_point(&self.lower.max_point(point))
    }

    /// Returns the squared distance to the AABB's [min_point](AABB::min_point)
    pub fn distance_2(&self, point: &P) -> P::Scalar {
        if self.contains_point(point) {
            Zero::zero()
        } else {
            self.min_point(point).sub(point).length_2()
        }
    }
}

impl<P> Envelope for AABB<P>
where
    P: Point,
{
    type Point = P;

    fn new_empty() -> Self {
        let max = P::Scalar::max_value();
        let min = P::Scalar::min_value();
        Self {
            lower: P::from_value(max),
            upper: P::from_value(min),
        }
    }

    fn is_empty(&self) -> bool {
        self.lower.nth(0).ord() > self.upper.nth(0).ord()
    }

    fn contains_point(&self, point: &P) -> bool {
        self.lower
            .all_component_wise(point, |x, y| x.ord() <= y.ord())
            && self
                .upper
                .all_component_wise(point, |x, y| x.ord() >= y.ord())
    }

    fn contains_envelope(&self, other: &Self) -> bool {
        self.lower
            .all_component_wise(&other.lower, |l, r| l.ord() <= r.ord())
            && self
                .upper
                .all_component_wise(&other.upper, |l, r| l.ord() >= r.ord())
    }

    fn merge(&mut self, other: &Self) {
        self.lower = self.lower.min_point(&other.lower);
        self.upper = self.upper.max_point(&other.upper);
    }

    fn merged(&self, other: &Self) -> Self {
        AABB {
            lower: self.lower.min_point(&other.lower),
            upper: self.upper.max_point(&other.upper),
        }
    }

    fn intersects(&self, other: &Self) -> bool {
        self.lower
            .all_component_wise(&other.upper, |l, r| l.ord() <= r.ord())
            && self
                .upper
                .all_component_wise(&other.lower, |l, r| l.ord() >= r.ord())
    }

    fn area(&self) -> P::Scalar {
        let zero = P::Scalar::zero();
        let one = P::Scalar::one();
        let diag = self.upper.sub(&self.lower);
        diag.fold(one, |acc, cur| max_inline(cur, zero) * acc)
    }

    fn distance_2(&self, point: &P) -> P::Scalar {
        self.distance_2(point)
    }

    fn min_max_dist_2(&self, point: &P) -> <P as Point>::Scalar {
        let l = self.lower.sub(point);
        let u = self.upper.sub(point);
        // diff, min, index
        let mut max_diff: (P::Scalar, P::Scalar, usize) = (Zero::zero(), Zero::zero(), 0);
        let mut result = P::new();

        for i in 0..P::DIMENSIONS {
            let mut min = l.nth(i);
            let mut max = u.nth(i);
            max = max * max;
            min = min * min;
            if max.ord() < min.ord() {
                core::mem::swap(&mut min, &mut max);
            }

            let diff = max - min;
            *result.nth_mut(i) = max;

            if diff.ord() >= max_diff.0.ord() {
                max_diff = (diff, min, i);
            }
        }

        *result.nth_mut(max_diff.2) = max_diff.1;
        result.fold(Zero::zero(), |acc, curr| acc + curr)
    }

    fn center(&self) -> Self::Point {
        let one = <Self::Point as Point>::Scalar::one();
        let two = one + one;
        self.lower.component_wise(&self.upper, |x, y| (x + y) / two)
    }

    fn intersection_area(&self, other: &Self) -> <Self::Point as Point>::Scalar {
        AABB {
            lower: self.lower.max_point(&other.lower),
            upper: self.upper.min_point(&other.upper),
        }
        .area()
    }

    fn perimeter_value(&self) -> P::Scalar {
        let diag = self.upper.sub(&self.lower);
        let zero = P::Scalar::zero();
        max_inline(diag.fold(zero, |acc, value| acc + value), zero)
    }

    fn sort_envelopes<T: RTreeObject<Envelope = Self>>(axis: usize, envelopes: &mut [T]) {
        envelopes.sort_unstable_by(|l, r| {
            l.envelope()
                .lower
                .nth(axis)
                .ord()
                .cmp(&r.envelope().lower.nth(axis).ord())
        });
    }

    fn partition_envelopes<T: RTreeObject<Envelope = Self>>(
        axis: usize,
        envelopes: &mut [T],
        selection_size: usize,
    ) {
        envelopes.select_nth_unstable_by(selection_size, |l, r| {
            l.envelope()
                .lower
                .nth(axis)
                .ord()
                .cmp(&r.envelope().lower.nth(axis).ord())
        });
    }
}

#[cfg(test)]
mod test {
    use super::AABB;
    use crate::envelope::Envelope;
    use crate::object::PointDistance;
    use crate::RTree;

    #[test]
    fn empty_rect() {
        let empty = AABB::<[f32; 2]>::new_empty();

        let other = AABB::from_corners([1.0, 1.0], [1.0, 1.0]);
        let subject = empty.merged(&other);
        assert_eq!(other, subject);

        let other = AABB::from_corners([0.0, 0.0], [0.0, 0.0]);
        let subject = empty.merged(&other);
        assert_eq!(other, subject);

        let other = AABB::from_corners([0.5, 0.5], [0.5, 0.5]);
        let subject = empty.merged(&other);
        assert_eq!(other, subject);

        let other = AABB::from_corners([-0.5, -0.5], [-0.5, -0.5]);
        let subject = empty.merged(&other);
        assert_eq!(other, subject);
    }

    /// Test that min_max_dist_2 is identical to distance_2 for the equivalent
    /// min max corner of the AABB. This is necessary to prevent optimizations
    /// from inadvertently changing floating point order of operations.
    #[test]
    fn test_min_max_dist_2_issue_40_regression() {
        let a = [0.7018702292340033, 0.2121617955083932, 0.8120562975177115];
        let b = [0.7297749764202988, 0.23020869735094462, 0.8194675310336391];
        let aabb = AABB::from_corners(a, b);
        let p = [0.6950876013070484, 0.220750082121574, 0.8186032137709887];
        let corner = [a[0], b[1], a[2]];
        assert_eq!(aabb.min_max_dist_2(&p), corner.distance_2(&p));
    }

    #[test]
    fn test_from_points_issue_170_regression() {
        let aabb = AABB::from_points(&[(3., 3., 3.), (4., 4., 4.)]);
        assert_eq!(aabb, AABB::from_corners((3., 3., 3.), (4., 4., 4.)));
    }

    #[test]
    fn test_is_empty() {
        let empty = AABB::<[f32; 2]>::new_empty();
        assert!(empty.is_empty());

        let not_empty = AABB::from_corners([1.0, 1.0], [1.0, 1.0]);
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn rtree_operations_with_nan_do_not_panic() {
        let mut points: Vec<_> = (0..64).map(|value| [value as f64, value as f64]).collect();
        points[16][0] = f64::NAN;

        let tree = RTree::bulk_load(points.clone());
        assert_eq!(tree.size(), points.len());
        assert_eq!(
            tree.nearest_neighbor_iter([f64::NAN, 0.0]).count(),
            points.len()
        );

        let mut tree = RTree::new();
        for point in &points {
            tree.insert(*point);
        }

        assert_eq!(tree.size(), points.len());
        assert_eq!(
            tree.nearest_neighbor_iter([f64::NAN, 0.0]).count(),
            points.len()
        );
    }

    #[test]
    fn tree_with_nan_is_still_queryable() {
        let before_the_nan = [10.0, 0.0];
        let after_the_nan = [20.0, 0.0];

        let mut tree = RTree::new();
        tree.insert(before_the_nan);
        tree.insert([f64::NAN, 0.0]);
        tree.insert(after_the_nan);

        assert_eq!(tree.size(), 3);
        assert_eq!(tree.iter().count(), 3);

        assert_eq!(tree.locate_at_point(before_the_nan), Some(&before_the_nan));
        assert_eq!(tree.locate_at_point(after_the_nan), Some(&after_the_nan));

        assert!(tree.contains(&before_the_nan));
        assert!(tree.contains(&after_the_nan));

        assert_eq!(tree.remove(&before_the_nan), Some(before_the_nan));
        assert_eq!(tree.remove(&after_the_nan), Some(after_the_nan));
    }

    /// `-0.0` and `0.0` are the same point as far as `==` is concerned, so a query for
    /// one must find data inserted under the other.
    ///
    /// This is why the scalar ordering is `OrderedFloat` rather than `f64::total_cmp`:
    /// which would rank `-0.0` strictly below `0.0`.
    #[test]
    fn signed_zeroes_are_interchangeable_in_queries() {
        let envelope = AABB::from_corners([0.0f64, 0.0], [1.0, 1.0]);
        assert!(envelope.contains_point(&[-0.0f64, -0.0]));

        let mut tree = RTree::new();
        tree.insert([0.0f64, 0.0]);
        assert_eq!(tree.locate_at_point([-0.0f64, -0.0]), Some(&[0.0, 0.0]));

        let mut tree = RTree::new();
        tree.insert([-0.0f64, -0.0]);
        assert_eq!(tree.locate_at_point([0.0f64, 0.0]), Some(&[-0.0, -0.0]));
    }
}
