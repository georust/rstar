use num_traits::{Bounded, Num, Signed, Zero};
use std::fmt::Debug;

pub trait RTreeNum: Bounded + Num + Clone + Copy + Signed + PartialOrd + Debug {}

impl<S> RTreeNum for S where S: Bounded + Num + Clone + Copy + Signed + PartialOrd + Debug {}

pub trait Point: Copy + Clone + PartialEq + Debug {
    type Scalar: RTreeNum;

    const DIMENSIONS: usize;

    fn generate<F>(f: F) -> Self
    where
        F: Fn(usize) -> Self::Scalar;

    fn nth(&self, index: usize) -> Self::Scalar;
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar;
}

impl<T> PointExt for T where T: Point {}

pub trait PointExt: Point {
    fn new() -> Self {
        Self::from_value(Zero::zero())
    }

    fn component_wise<F>(&self, other: &Self, f: F) -> Self
    where
        F: Fn(Self::Scalar, Self::Scalar) -> Self::Scalar,
    {
        Self::generate(|i| f(self.nth(i), other.nth(i)))
    }

    fn all_component_wise<F>(&self, other: &Self, f: F) -> bool
    where
        F: Fn(Self::Scalar, Self::Scalar) -> bool,
    {
        // TODO: Maybe do this by proper iteration
        for i in 0..Self::DIMENSIONS {
            if !f(self.nth(i), other.nth(i)) {
                return false;
            }
        }
        true
    }

    fn dot(&self, rhs: &Self) -> Self::Scalar {
        self.component_wise(rhs, |l, r| l * r).fold(Zero::zero(), |acc, val| acc + val)
    }

    fn fold<T, F: Fn(T, Self::Scalar) -> T>(&self, mut acc: T, f: F) -> T {
        // TODO: Maybe do this by proper iteration
        for i in 0..Self::DIMENSIONS {
            acc = f(acc, self.nth(i));
        }
        acc
    }

    fn from_value(value: Self::Scalar) -> Self {
        Self::generate(|_| value)
    }

    fn min_point(&self, other: &Self) -> Self {
        self.component_wise(other, min_inline)
    }

    fn max_point(&self, other: &Self) -> Self {
        self.component_wise(other, max_inline)
    }

    fn length_2(&self) -> Self::Scalar {
        self.fold(Zero::zero(), |acc, cur| cur * cur + acc)
    }

    fn sub(&self, other: &Self) -> Self {
        self.component_wise(other, |l, r| l - r)
    }

    fn add(&self, other: &Self) -> Self {
        self.component_wise(other, |l, r| l + r)
    }

    fn mul(&self, scalar: Self::Scalar) -> Self {
        self.map(|coordinate| coordinate * scalar)
    }

    fn map<F>(&self, f: F) -> Self
    where
        F: Fn(Self::Scalar) -> Self::Scalar,
    {
        Self::generate(|i| f(self.nth(i)))
    }

    fn distance_2(&self, other: &Self) -> Self::Scalar {
        self.sub(other).length_2()
    }
}

#[inline]
pub fn min_inline<S>(a: S, b: S) -> S
where
    S: RTreeNum,
{
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub fn max_inline<S>(a: S, b: S) -> S
where
    S: RTreeNum,
{
    if a > b {
        a
    } else {
        b
    }
}

macro_rules! count_exprs {
    () => (0);
    ($head:expr) => (1);
    ($head:expr, $($tail:expr),*) => (1 + count_exprs!($($tail),*));
}

macro_rules! implement_point_for_array {
    ($($index:expr),*) => {
        impl<S> Point for [S; count_exprs!($($index),*)]
        where
            S: RTreeNum,
        {
            type Scalar = S;

            const DIMENSIONS: usize = count_exprs!($($index),*);

            fn generate<F>(generator: F) -> Self
            where
                F: Fn(usize) -> Self::Scalar,
            {
                [$(generator($index)),*]
            }

            fn nth(&self, index: usize) -> Self::Scalar {
                self[index]
            }

            fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
                &mut self[index]
            }
        }
    };
}

implement_point_for_array!(0, 1);
implement_point_for_array!(0, 1, 2);
implement_point_for_array!(0, 1, 2, 3);
implement_point_for_array!(0, 1, 2, 3, 4);
implement_point_for_array!(0, 1, 2, 3, 4, 5);
implement_point_for_array!(0, 1, 2, 3, 4, 5, 6);
implement_point_for_array!(0, 1, 2, 3, 4, 5, 6, 7);
implement_point_for_array!(0, 1, 2, 3, 4, 5, 6, 7, 8);