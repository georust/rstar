use point::{max_inline, Point, PointExt};
use num_traits::{Bounded, One, Signed, Zero};
use envelope::Envelope;

#[derive(Clone, Debug, Copy, PartialEq)]
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
    pub fn from_point(p: P) -> Self {
        AABB { lower: p, upper: p }
    }

    pub fn lower(&self) -> P {
        self.lower
    }

    pub fn upper(&self) -> P {
        self.upper
    }

    pub fn from_points<'a, I>(i: I) -> Self
    where
        I: IntoIterator<Item = &'a P> + 'a,
        P: 'a,
    {
        i.into_iter()
            .fold(Self::new_empty(), |aabb, p| aabb.add_point(p))
    }

    fn add_point(&self, point: &P) -> Self {
        AABB {
            lower: self.lower.min_point(point),
            upper: self.upper.max_point(point),
        }
    }

    pub fn new_empty() -> Self {
        let max = P::Scalar::max_value();
        let min = P::Scalar::min_value();
        AABB {
            lower: P::from_value(max),
            upper: P::from_value(min),
        }
    }

    pub fn min_point(&self, point: &P) -> P {
        self.upper.min_point(&self.lower.max_point(point))
    }

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
        AABB::new_empty()
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
        diag.fold(one, |acc, cur| max_inline(acc, zero) * cur)
    }

    fn distance_2(&self, point: &P) -> P::Scalar {
        self.distance_2(point)
    }

    fn min_max_dist_2(&self, point: &P) -> <P as Point>::Scalar {
        let l = self.lower.sub(point);
        let u = self.upper.sub(point);
        let (mut min, mut max) = (P::new(), P::new());
        for i in 0..P::dimensions() {
            if l.nth(i).abs() < u.nth(i).abs() {
                *min.nth_mut(i) = l.nth(i);
                *max.nth_mut(i) = u.nth(i);
            } else {
                *min.nth_mut(i) = u.nth(i);
                *max.nth_mut(i) = l.nth(i);
            }
        }
        let mut result = Zero::zero();
        for i in 0..P::dimensions() {
            let mut p = min;
            // Only set one component to the maximum distance
            *p.nth_mut(i) = max.nth(i);
            let new_dist = p.length_2();
            if new_dist < result || i == 0 {
                result = new_dist
            }
        }
        result
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
        }.area()
    }

    fn margin_value(&self) -> P::Scalar {
        let diag = self.upper.sub(&self.lower);
        let zero = P::Scalar::zero();
        max_inline(diag.fold(zero, |acc, value| acc + value), zero)
    }

    fn align_envelopes<T, F>(axis: usize, envelopes: &mut [T], f: F)
    where
        F: Fn(&T) -> Self,
    {
        envelopes.sort_by(|l, r| {
            f(l).lower
                .nth(axis)
                .partial_cmp(&f(r).lower.nth(axis))
                .unwrap()
        });
    }
}
