use crate::point::{max_inline, Point, PointExt};
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
/// [Rectangle](primitives/struct.Rectangle.html) struct for this purpose.
///
/// # Type arguments
/// `P`: The struct is generic over which point type is used. Using an n-dimensional point
/// type will result in an n-dimensional bounding box.
#[derive(Clone, Debug, Copy, PartialEq, Eq, Ord, PartialOrd)]
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
        AABB { lower: p, upper: p }
    }

    /// Returns the AABB's lower corner.
    ///
    /// This is the point contained within the AABB with the smallest coordinate value in each
    /// dimension.
    pub fn lower(&self) -> P {
        self.lower
    }

    /// Returns the AABB's upper corner.
    ///
    /// This is the point contained within the AABB with the largest coordinate value in each
    /// dimension.
    pub fn upper(&self) -> P {
        self.upper
    }

    /// Creates a new AABB encompassing two points.
    pub fn from_corners(p1: P, p2: P) -> Self {
        AABB {
            lower: p1.min_point(&p2),
            upper: p1.max_point(&p2),
        }
    }

    /// Creates a new AABB encompassing a collection of points.
    pub fn from_points<'a, I>(i: I) -> Self
    where
        I: IntoIterator<Item = &'a P> + 'a,
        P: 'a,
    {
        i.into_iter()
            .fold(Self::new_empty(), |aabb, p| aabb.add_point(p))
    }

    /// Returns the AABB that contains `self` and another point.
    fn add_point(&self, point: &P) -> Self {
        AABB {
            lower: self.lower.min_point(point),
            upper: self.upper.max_point(point),
        }
    }

    /// Returns the point within this AABB closest to a given point.
    ///
    /// If `point` is contained within the AABB, `point` will be returned.
    pub fn min_point(&self, point: &P) -> P {
        self.upper.min_point(&self.lower.max_point(point))
    }

    /// Returns the squared distance to the AABB's [min_point](#method.min_point).
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
        new_empty()
    }

    fn contains_point(&self, point: &P) -> bool {
        self.lower.all_component_wise(point, |x, y| x <= y)
            && self.upper.all_component_wise(point, |x, y| x >= y)
    }

    fn contains_envelope(&self, other: &Self) -> bool {
        self.lower.all_component_wise(&other.lower, |l, r| l <= r)
            && self.upper.all_component_wise(&other.upper, |l, r| l >= r)
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
        self.lower.all_component_wise(&other.upper, |l, r| l <= r)
            && self.upper.all_component_wise(&other.lower, |l, r| l >= r)
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
        let mut max_diff = (Zero::zero(), Zero::zero(), 0); // diff, min, index
        let mut result = P::new();

        for i in 0..P::DIMENSIONS {
            let mut min = l.nth(i);
            let mut max = u.nth(i);
            max = max * max;
            min = min * min;
            if max < min {
                std::mem::swap(&mut min, &mut max);
            }

            let diff = max - min;
            *result.nth_mut(i) = max;

            if diff >= max_diff.0 {
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
        envelopes.sort_by(|l, r| {
            l.envelope()
                .lower
                .nth(axis)
                .partial_cmp(&r.envelope().lower.nth(axis))
                .unwrap()
        });
    }

    fn partition_envelopes<T: RTreeObject<Envelope = Self>>(
        axis: usize,
        envelopes: &mut [T],
        selection_size: usize,
    ) {
        ::pdqselect::select_by(envelopes, selection_size, |l, r| {
            l.envelope()
                .lower
                .nth(axis)
                .partial_cmp(&r.envelope().lower.nth(axis))
                .unwrap()
        });
    }
}

fn new_empty<P: Point>() -> AABB<P> {
    let max = P::Scalar::max_value();
    let min = P::Scalar::min_value();
    AABB {
        lower: P::from_value(max),
        upper: P::from_value(min),
    }
}

#[cfg(test)]
mod test {
    use crate::envelope::Envelope;
    use crate::object::PointDistance;
    use super::AABB;

    /// Test that min_max_dist_2 is identical to distance_2 for the equivalent
    /// min max corner of the AABB. This is necessary to prevent optimizations
    /// from inadvertently changing floating point order of operations.
    #[test]
    fn test_min_max_dist_2_issue_40_regression() {
        let a = [
            0.7018702292340033,
            0.2121617955083932,
            0.8120562975177115,
        ];
        let b = [
            0.7297749764202988,
            0.23020869735094462,
            0.8194675310336391,
        ];
        let aabb = AABB::from_corners(a, b);
        let p = [
            0.6950876013070484,
            0.220750082121574,
            0.8186032137709887,
        ];
        let corner = [a[0], b[1], a[2]];
        assert_eq!(aabb.min_max_dist_2(&p), corner.distance_2(&p));
    }
}
