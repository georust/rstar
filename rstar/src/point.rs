use num_traits::{Bounded, Num, Signed, Zero};
use generic_array::{ArrayLength, GenericArray};
use ::std::fmt::Debug;

pub trait RTreeNum: Bounded + Num + Clone + Copy + Signed + PartialOrd + Debug { }

impl <S> RTreeNum for S where S: Bounded + Num + Clone + Copy + Signed + PartialOrd + Debug { }

pub trait Point: Copy + Clone + PartialEq + Debug {
    type Scalar: RTreeNum;

    fn generate<F>(f: F) -> Self where F: Fn(usize) -> Self::Scalar;
    fn dimensions() -> usize;
    fn nth(&self, index: usize) -> Self::Scalar;
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar;
}

impl <S, N> Point for GenericArray<S, N>
    where S: RTreeNum,
          N: ArrayLength<S>,
          N::ArrayType: Copy {

    type Scalar = S;

    fn nth(&self, index: usize) -> Self::Scalar {
        self[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self[index]
    }

    fn generate<F>(f: F) -> Self where F: Fn(usize) -> Self::Scalar {
        GenericArray::generate(f)
    }

    fn dimensions() -> usize {
        N::to_usize()
    }
}

impl <T> PointExt for T where T: Point { }

pub trait PointExt: Point {

    fn new() -> Self {
        Self::from_value(Zero::zero())
    }

    fn component_wise<F>(&self, other: &Self, f: F) -> Self 
        where F: Fn(Self::Scalar, Self::Scalar) -> Self::Scalar
    {
        Self::generate(|i| f(self.nth(i), other.nth(i)))
    }

    fn all_component_wise<F>(&self, other: &Self, f: F) -> bool
        where F: Fn(Self::Scalar, Self::Scalar) -> bool
    {
        // TODO: Maybe do this by proper iteration
        for i in 0 .. Self::dimensions() {
            if !f(self.nth(i), other.nth(i)) {
                return false;
            }
        }
        true
    }

    fn fold<T, F: Fn(T, Self::Scalar) -> T>(&self, mut acc: T, f: F) -> T {
        // TODO: Maybe do this by proper iteration
        for i in 0 .. Self::dimensions() {
            acc = f(acc, self.nth(i));
        }
        acc
    }


    fn from_value(value: Self::Scalar) -> Self {
        Self::generate(|_| value)
    }


    fn min_point(&self, other: &Self) -> Self {
        self.component_wise(other, |l, r| min_inline(l, r))
    }

    fn max_point(&self, other: &Self) -> Self {
        self.component_wise(other, |l, r| max_inline(l, r))
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

    fn distance_2(&self, other: &Self) -> Self::Scalar {
        self.sub(other).length_2()
    }
}

#[inline]
pub fn min_inline<S>(a: S, b: S) -> S where S: RTreeNum {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
pub fn max_inline<S>(a: S, b: S) -> S where S: RTreeNum {
    if a > b {
        a
    } else {
        b
    }
}

impl <S> Point for [S; 2]
    where S: RTreeNum {
    type Scalar = S;

    fn generate<F>(generator: F) -> Self
        where F: Fn(usize) -> Self::Scalar
    {
        [generator(0), generator(1)]
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        self[index]
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self[index]
    }

    fn dimensions() -> usize {
        2
    }
}
