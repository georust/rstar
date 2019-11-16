use num_traits::{Bounded, Num, Signed, Zero};
use std::fmt::Debug;

/// Defines a number type that is compatible with rstar.
///
/// rstar works out of the box with the following standard library types:
///  - i32
///  - i64
///  - f32
///  - f64
///
/// This type cannot be implemented directly. Instead, it is just required to implement
/// all required traits from the `num_traits` crate.
///
/// # Example
/// ```
/// # extern crate num_traits;
/// use num_traits::{Bounded, Num, Signed};
///
/// #[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
/// struct MyFancyNumberType(f32);
///
/// impl Bounded for MyFancyNumberType {
///   // ... details hidden ...
/// # fn min_value() -> Self { MyFancyNumberType(Bounded::min_value()) }
/// #
/// # fn max_value() -> Self { MyFancyNumberType(Bounded::max_value()) }
/// }
///
/// impl Signed for MyFancyNumberType {
///   // ... details hidden ...
/// # fn abs(&self) -> Self { unimplemented!() }
/// #
/// # fn abs_sub(&self, other: &Self) -> Self { unimplemented!() }
/// #
/// # fn signum(&self) -> Self { unimplemented!() }
/// #
/// # fn is_positive(&self) -> bool { unimplemented!() }
/// #
/// # fn is_negative(&self) -> bool { unimplemented!() }
/// }
///
/// impl Num for MyFancyNumberType {
///   // ... details hidden ...
/// # type FromStrRadixErr = num_traits::ParseFloatError;
/// # fn from_str_radix(str: &str, radix: u32) -> Result<Self, Self::FromStrRadixErr> { unimplemented!() }
/// }
///
/// // There's a lot of traits that are still missing to make the above code compile,
/// // let's assume they are implemented. MyFancyNumberType type now readily implements
/// // RTreeNum and can be used with r-trees:
/// # fn main() {
/// use rstar::RTree;
/// let mut rtree = RTree::new();
/// rtree.insert([MyFancyNumberType(0.0), MyFancyNumberType(0.0)]);
/// # }
///
/// # impl num_traits::Zero for MyFancyNumberType {
/// #   fn zero() -> Self { unimplemented!() }
/// #   fn is_zero(&self) -> bool { unimplemented!() }
/// # }
/// #
/// # impl num_traits::One for MyFancyNumberType {
/// #   fn one() -> Self { unimplemented!() }
/// # }
/// #
/// # impl std::ops::Mul for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn mul(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl std::ops::Add for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn add(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl std::ops::Sub for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn sub(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl std::ops::Div for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn div(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl std::ops::Rem for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn rem(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl std::ops::Neg for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn neg(self) -> Self { unimplemented!() }
/// # }
/// #
/// ```
///
pub trait RTreeNum: Bounded + Num + Clone + Copy + Signed + PartialOrd + Debug {}

impl<S> RTreeNum for S where S: Bounded + Num + Clone + Copy + Signed + PartialOrd + Debug {}

/// Defines a point type that is compatible with rstar.
///
/// This trait should be used for interoperability with other point types, not to define custom objects
/// that can be inserted into r-trees. Use [`RTreeObject`](trait.RTreeObject.html) or
/// [`PointWithData`](primitives/struct.PointWithData.html) instead.
/// This trait defines points, not points with metadata.
///
/// `Point` is implemented out of the box for arrays like `[f32; 2]` or `[f64; 7]` (up to dimension 9).
///
/// # Implementation example
/// Supporting a custom point type might look like this:
///
/// ```
/// use rstar::Point;
///
/// #[derive(Copy, Clone, PartialEq, Debug)]
/// struct IntegerPoint
/// {
///     x: i32,
///     y: i32
/// }
///
/// impl Point for IntegerPoint
/// {
///   type Scalar = i32;
///   const DIMENSIONS: usize = 2;
///
///   fn generate(generator: impl Fn(usize) -> Self::Scalar) -> Self
///   {
///     IntegerPoint {
///       x: generator(0),
///       y: generator(1)
///     }
///   }
///
///   fn nth(&self, index: usize) -> Self::Scalar
///   {
///     match index {
///       0 => self.x,
///       1 => self.y,
///       _ => unreachable!()
///     }
///   }
///   
///   fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar
///   {
///     match index {
///       0 => &mut self.x,
///       1 => &mut self.y,
///       _ => unreachable!()
///     }
///   }
/// }
/// ```
pub trait Point: Copy + Clone + PartialEq + Debug {
    /// The number type used by this point type.
    type Scalar: RTreeNum;

    /// The number of dimensions of this point type.
    const DIMENSIONS: usize;

    /// Creates a new point value with given values for each dimension.
    ///
    /// The value that each dimension should be initialized with is given by the parameter `generator`.
    /// Calling `generator(n)` returns the value of dimension `n`, `n` will be in the range `0 .. Self::DIMENSIONS`.
    fn generate(generator: impl Fn(usize) -> Self::Scalar) -> Self;

    /// Returns a single coordinate of this point.
    ///
    /// Returns the coordinate indicated by `index`. `index` is always smaller than `Self::DIMENSIONS`.
    fn nth(&self, index: usize) -> Self::Scalar;

    /// Mutable variant of [nth](#methods.nth).
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar;
}

impl<T> PointExt for T where T: Point {}

pub trait PointExt: Point {
    fn new() -> Self {
        Self::from_value(Zero::zero())
    }

    fn component_wise(
        &self,
        other: &Self,
        f: impl Fn(Self::Scalar, Self::Scalar) -> Self::Scalar,
    ) -> Self {
        Self::generate(|i| f(self.nth(i), other.nth(i)))
    }

    fn all_component_wise(
        &self,
        other: &Self,
        f: impl Fn(Self::Scalar, Self::Scalar) -> bool,
    ) -> bool {
        // TODO: Maybe do this by proper iteration
        for i in 0..Self::DIMENSIONS {
            if !f(self.nth(i), other.nth(i)) {
                return false;
            }
        }
        true
    }

    fn dot(&self, rhs: &Self) -> Self::Scalar {
        self.component_wise(rhs, |l, r| l * r)
            .fold(Zero::zero(), |acc, val| acc + val)
    }

    fn fold<T>(&self, start_value: T, f: impl Fn(T, Self::Scalar) -> T) -> T {
        let mut accumulated = start_value;
        // TODO: Maybe do this by proper iteration
        for i in 0..Self::DIMENSIONS {
            accumulated = f(accumulated, self.nth(i));
        }
        accumulated
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

    fn map(&self, f: impl Fn(Self::Scalar) -> Self::Scalar) -> Self {
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

            fn generate(generator: impl Fn(usize) -> S) -> Self
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
