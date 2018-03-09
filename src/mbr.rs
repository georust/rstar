use point::{Point, PointExt, max_inline};
use num_traits::{Bounded, One, Zero};

#[derive(Clone, Debug, Copy, PartialEq)]
pub struct MBR<P> where P: Point {
    lower: P,
    upper: P,
}

impl <P> MBR<P> where P: Point {
    pub fn from_point(p: P) -> Self {
        MBR {
            lower: p,
            upper: p,
        }
    }

    pub fn new_empty() -> Self {
        let max = P::Scalar::max_value();
        let min = P::Scalar::min_value();
        MBR {
            lower: P::from_value(max),
            upper: P::from_value(min),
        }
    }

    pub fn lower(&self) -> P {
        self.lower
    }

    pub fn upper(&self) -> P {
        self.upper
    }

    pub fn extend_with_mbr(&mut self, new_mbr: &Self) {
        self.lower = self.lower.min_point(&new_mbr.lower);
        self.upper = self.upper.max_point(&new_mbr.upper);
    }

    pub fn add_mbr(&self, other_mbr: &Self) -> Self {
        MBR {
            lower: self.lower.min_point(&other_mbr.lower),
            upper: self.upper.max_point(&other_mbr.upper),
        }
    }

    pub fn contains_point(&self, point: &P) -> bool {
        self.lower.all_component_wise(point, |x, y| x <= y) &&
            self.upper.all_component_wise(point, |x, y| x >= y)
    }

    pub fn contains_mbr(&self, other: &Self) -> bool {
        self.lower.all_component_wise(&other.lower, |l, r| l <= r) &&
            self.upper.all_component_wise(&other.upper, |l, r| l >= r)
    }

    pub fn intersection(&self, other: &Self) -> Self {
        MBR {
            lower: self.lower.max_point(&other.lower),
            upper: self.upper.min_point(&other.upper)
        }
    }

    pub fn intersects_mbr(&self, other: &Self) -> bool {
        self.lower.all_component_wise(&other.upper(), |l, r| l <= r) &&
            self.upper.all_component_wise(&other.lower(), |l, r| l >= r)
    }

    pub fn area(&self) -> P::Scalar {
        let zero = P::Scalar::zero();
        let one = P::Scalar::one();

        let diag = self.upper.sub(&self.lower);
        diag.fold(one, |acc, cur| max_inline(acc, zero) * cur)
    }

    pub fn diagonal_sum(&self) -> P::Scalar {
        let diag = self.upper().sub(&self.lower());
        let zero = P::Scalar::zero();
        max_inline(diag.fold(zero, |acc, value| acc + value), zero)

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
