use core::fmt::Debug;
use core::num::Wrapping;
use num_traits::{Bounded, Num, Signed, Zero};
use ordered_float::OrderedFloat;

/// Defines a number type that is compatible with rstar.
///
/// rstar works out of the box with the following standard library types:
///  - i8, i16, i32, i64, i128, isize
///  - [Wrapping](core::num::Wrapping) versions of the above
///  - f32, f64
///
/// r-trees require a *total* order on their scalars.
/// [`RTreeNum::OrdType`] supplies that ordering. Integral types, which are already Ord,
/// are their own OrdType. `f32` and `f64` use [`ordered_float::OrderedFloat`].
///
/// # Example
/// ```
/// # extern crate num_traits;
/// use num_traits::{Bounded, Num, Signed};
/// use rstar::RTreeNum;
///
/// #[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
/// struct MyFancyNumberType(f32);
///
/// impl num_traits::Bounded for MyFancyNumberType {
///   // ... details hidden ...
/// # fn min_value() -> Self { Self(Bounded::min_value()) }
/// #
/// # fn max_value() -> Self { Self(Bounded::max_value()) }
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
/// impl RTreeNum for MyFancyNumberType {
///   // Any `Ord` type will do; `OrderedFloat` is from the `ordered-float` crate.
///   type OrdType = ordered_float::OrderedFloat<f32>;
///   fn ord(self) -> Self::OrdType { ordered_float::OrderedFloat(self.0) }
/// }
///
/// // Lots of traits are still missing to make the above code compile, but
/// // let's assume they're implemented. `MyFancyNumberType` type now readily implements
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
/// # impl core::ops::Mul for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn mul(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl core::ops::Add for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn add(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl core::ops::Sub for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn sub(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl core::ops::Div for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn div(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl core::ops::Rem for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn rem(self, rhs: Self) -> Self { unimplemented!() }
/// # }
/// #
/// # impl core::ops::Neg for MyFancyNumberType {
/// #   type Output = Self;
/// #   fn neg(self) -> Self { unimplemented!() }
/// # }
/// #
/// ```
///
pub trait RTreeNum: Bounded + Num + Clone + Copy + Signed + Debug {
    /// A type that orders values of `Self` *totally*, i.e. any two values can be
    /// compared and the comparison is reflexive, antisymmetric and transitive.
    ///
    /// For integral types this is simply `Self`. Floating point types use
    /// [`OrderedFloat`] to account for NaN.
    type OrdType: Ord;

    /// Converts `self` into a value that can be ordered totally.
    ///
    /// ```
    /// use rstar::RTreeNum;
    ///
    /// assert!(!(f64::NAN <= f64::NAN));           // PartialOrd: refuses to order the pair
    /// assert!(f64::NAN.ord() <= f64::NAN.ord());  // Ord: always commits to an answer
    /// ```
    fn ord(self) -> Self::OrdType;
}

impl RTreeNum for f32 {
    type OrdType = OrderedFloat<Self>;
    #[inline]
    fn ord(self) -> Self::OrdType {
        OrderedFloat(self)
    }
}

impl RTreeNum for f64 {
    type OrdType = OrderedFloat<Self>;
    #[inline]
    fn ord(self) -> Self::OrdType {
        OrderedFloat(self)
    }
}

macro_rules! impl_rtree_num_for_ord {
    ($($type:ty),+ $(,)?) => {
        $(
            impl RTreeNum for $type {
                type OrdType = $type;
                #[inline]
                fn ord(self) -> Self::OrdType {
                    self
                }
            }
        )+
    };
}

// Keep this list in sync with `tests::test_types`, which asserts these types implement `RTreeNum`.
impl_rtree_num_for_ord!(i8, i16, i32, i64, i128, isize);

impl<T> RTreeNum for Wrapping<T>
where
    T: RTreeNum,
    Wrapping<T>: Bounded + Num + Clone + Copy + Signed + Debug,
{
    type OrdType = T::OrdType;
    #[inline]
    fn ord(self) -> Self::OrdType {
        self.0.ord()
    }
}

#[cfg(test)]
mod rtree_num_tests {
    use super::RTreeNum;
    use core::cmp::Ordering;

    /// Away from `-0.0` and `-NaN`, [`RTreeNum::ord`] agrees with the platform's
    /// intrinsic total order. Those two values are the only deliberate divergences, so
    /// callers of this helper must leave them out of `values` on purpose, not by
    /// accident. They are covered by the two tests below.
    fn assert_matches_intrinsic_total_cmp<S: RTreeNum + core::fmt::Debug>(
        values: &[S],
        intrinsic_total_cmp: impl Fn(&S, &S) -> Ordering,
    ) {
        for left in values {
            for right in values {
                assert_eq!(
                    left.ord().cmp(&right.ord()),
                    intrinsic_total_cmp(left, right),
                    "unexpected ordering for {left:?} and {right:?}",
                );
            }
        }
    }

    #[test]
    fn ord_matches_f32_intrinsic_total_cmp() {
        assert_matches_intrinsic_total_cmp(
            &[f32::NEG_INFINITY, -1.0, 0.0, 1.0, f32::INFINITY, f32::NAN],
            f32::total_cmp,
        );
    }

    #[test]
    fn ord_matches_f64_intrinsic_total_cmp() {
        assert_matches_intrinsic_total_cmp(
            &[f64::NEG_INFINITY, -1.0, 0.0, 1.0, f64::INFINITY, f64::NAN],
            f64::total_cmp,
        );
    }

    /// Every `NaN` ranks above every other value and equal to every other `NaN`,
    /// whatever its sign bit. This is what keeps a `NaN` coordinate *conservative*: it
    /// always loses a `min` and always wins a `max`, so it can only widen an envelope's
    /// upper bound, never shrink a bound inwards and strand the finite children already
    /// merged in.
    ///
    /// [`f64::total_cmp`] does not have this property -- it sorts `-NaN` below
    /// `-inf` -- which would let an arbitrary `NaN`'s sign bit decide which end of the
    /// envelope it corrupted.
    #[test]
    fn every_nan_ranks_highest_regardless_of_sign() {
        for nan in [f64::NAN, -f64::NAN] {
            for other in [f64::NEG_INFINITY, -1.0, 0.0, 1.0, f64::INFINITY] {
                assert_eq!(nan.ord().cmp(&other.ord()), Ordering::Greater);
                assert_eq!(other.ord().cmp(&nan.ord()), Ordering::Less);
            }
            // Reflexive, unlike `<=`, which is what makes this usable as a comparator.
            assert_eq!(nan.ord().cmp(&nan.ord()), Ordering::Equal);
            assert_eq!(nan.ord().cmp(&(-nan).ord()), Ordering::Equal);
        }
    }

    /// `-0.0` and `0.0` must stay *equal*, as they are under `==`.
    ///
    /// [`f64::total_cmp`] ranks `-0.0` strictly below `0.0`. Since every comparison in
    /// the tree now goes through this ordering, adopting that behaviour would mean an
    /// envelope with a lower bound of `0.0` no longer contains the point `-0.0`. See
    /// `signed_zeroes_are_interchangeable_in_queries` in `aabb.rs`.
    #[test]
    fn signed_zeroes_compare_equal() {
        assert_eq!((-0.0f64).ord().cmp(&0.0f64.ord()), Ordering::Equal);
        assert_eq!((-0.0f32).ord().cmp(&0.0f32.ord()), Ordering::Equal);

        assert_eq!(f64::total_cmp(&-0.0, &0.0), Ordering::Less);
    }
}

/// Defines a point type that is compatible with rstar.
///
/// This trait should be used for interoperability with other point types, not to define custom objects
/// that can be inserted into r-trees. Use [`crate::RTreeObject`] or
/// [`crate::primitives::GeomWithData`] instead.
/// This trait defines points, not points with metadata.
///
/// `Point` is implemented out of the box for arrays like `[f32; 2]` or `[f64; 7]` (for any number of dimensions),
/// and for tuples like `(int, int)` and `(f64, f64, f64)` so tuples with only elements of the same type (up to dimension 9).
///
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
///   fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self
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
pub trait Point: Clone + PartialEq + Debug {
    /// The number type used by this point type.
    type Scalar: RTreeNum;

    /// The number of dimensions of this point type.
    const DIMENSIONS: usize;

    /// Creates a new point value with given values for each dimension.
    ///
    /// The value that each dimension should be initialized with is given by the parameter `generator`.
    /// Calling `generator(n)` returns the value of dimension `n`, `n` will be in the range `0 .. Self::DIMENSIONS`,
    /// and will be called with values of `n` in ascending order.
    fn generate(generator: impl FnMut(usize) -> Self::Scalar) -> Self;

    /// Returns a single coordinate of this point.
    ///
    /// Returns the coordinate indicated by `index`. `index` is always smaller than `Self::DIMENSIONS`.
    fn nth(&self, index: usize) -> Self::Scalar;

    /// Mutable variant of [nth](#methods.nth).
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar;
}

impl<T> PointExt for T where T: Point {}

/// Utility functions for Point
pub trait PointExt: Point {
    /// Returns a new Point with all components set to zero.
    fn new() -> Self {
        Self::from_value(Zero::zero())
    }

    /// Applies `f` to each pair of components of `self` and `other`.
    fn component_wise(
        &self,
        other: &Self,
        mut f: impl FnMut(Self::Scalar, Self::Scalar) -> Self::Scalar,
    ) -> Self {
        Self::generate(|i| f(self.nth(i), other.nth(i)))
    }

    /// Returns whether all pairs of components of `self` and `other` pass test closure `f`. Short circuits if any result is false.
    fn all_component_wise(
        &self,
        other: &Self,
        mut f: impl FnMut(Self::Scalar, Self::Scalar) -> bool,
    ) -> bool {
        (0..Self::DIMENSIONS).all(|i| f(self.nth(i), other.nth(i)))
    }

    /// Returns the dot product of `self` and `rhs`.
    fn dot(&self, rhs: &Self) -> Self::Scalar {
        self.component_wise(rhs, |l, r| l * r)
            .fold(Zero::zero(), |acc, val| acc + val)
    }

    /// Folds (aka reduces or injects) the Point component wise using `f` and returns the result.
    /// fold() takes two arguments: an initial value, and a closure with two arguments: an 'accumulator', and the value of the current component.
    /// The closure returns the value that the accumulator should have for the next iteration.
    ///
    /// The `start_value` is the value the accumulator will have on the first call of the closure.
    ///
    /// After applying the closure to every component of the Point, fold() returns the accumulator.
    fn fold<T>(&self, start_value: T, mut f: impl FnMut(T, Self::Scalar) -> T) -> T {
        (0..Self::DIMENSIONS).fold(start_value, |accumulated, i| f(accumulated, self.nth(i)))
    }

    /// Returns a Point with every component set to `value`.
    fn from_value(value: Self::Scalar) -> Self {
        Self::generate(|_| value)
    }

    /// Returns a Point with each component set to the smallest of each component pair of `self` and `other`.
    fn min_point(&self, other: &Self) -> Self {
        self.component_wise(other, min_inline)
    }

    /// Returns a Point with each component set to the biggest of each component pair of `self` and `other`.
    fn max_point(&self, other: &Self) -> Self {
        self.component_wise(other, max_inline)
    }

    /// Returns the squared length of this Point as if it was a vector.
    fn length_2(&self) -> Self::Scalar {
        self.fold(Zero::zero(), |acc, cur| cur * cur + acc)
    }

    /// Substracts `other` from `self` component wise.
    fn sub(&self, other: &Self) -> Self {
        self.component_wise(other, |l, r| l - r)
    }

    /// Adds `other` to `self` component wise.
    fn add(&self, other: &Self) -> Self {
        self.component_wise(other, |l, r| l + r)
    }

    /// Multiplies `self` with `scalar` component wise.
    fn mul(&self, scalar: Self::Scalar) -> Self {
        self.map(|coordinate| coordinate * scalar)
    }

    /// Applies `f` to `self` component wise.
    fn map(&self, mut f: impl FnMut(Self::Scalar) -> Self::Scalar) -> Self {
        Self::generate(|i| f(self.nth(i)))
    }

    /// Returns the squared distance between `self` and `other`.
    fn distance_2(&self, other: &Self) -> Self::Scalar {
        self.sub(other).length_2()
    }
}

#[inline]
pub fn min_inline<S>(a: S, b: S) -> S
where
    S: RTreeNum,
{
    if a.ord() < b.ord() {
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
    if a.ord() > b.ord() {
        a
    } else {
        b
    }
}

impl<S, const N: usize> Point for [S; N]
where
    S: RTreeNum,
{
    type Scalar = S;

    const DIMENSIONS: usize = N;

    fn generate(mut generator: impl FnMut(usize) -> S) -> Self {
        // The same implementation used in std::array::from_fn
        // Since this is a const generic it gets unrolled
        let mut idx = 0;
        [(); N].map(|_| {
            let res = generator(idx);
            idx += 1;
            res
        })
    }

    #[inline]
    fn nth(&self, index: usize) -> Self::Scalar {
        self[index]
    }

    #[inline]
    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        &mut self[index]
    }
}

macro_rules! count_exprs {
    () => (0);
    ($head:expr) => (1);
    ($head:expr, $($tail:expr),*) => (1 + count_exprs!($($tail),*));
}

macro_rules! fixed_type {
    ($expr:expr, $type:ty) => {
        $type
    };
}

macro_rules! impl_point_for_tuple {
    ($($index:expr => $name:ident),+) => {
        impl<S> Point for ($(fixed_type!($index, S),)+)
        where
            S: RTreeNum
        {
            type Scalar = S;

            const DIMENSIONS: usize = count_exprs!($($index),*);

            fn generate(mut generator: impl FnMut(usize) -> S) -> Self {
                ($(generator($index),)+)
            }

            #[inline]
            fn nth(&self, index: usize) -> Self::Scalar {
                let ($($name,)+) = self;

                match index {
                    $($index => *$name,)+
                    _ => unreachable!("index {} out of bounds for tuple", index),
                }
            }

            #[inline]
            fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
                let ($($name,)+) = self;

                match index {
                    $($index => $name,)+
                    _ => unreachable!("index {} out of bounds for tuple", index),
                }
            }
        }
    };
}

impl_point_for_tuple!(0 => a);
impl_point_for_tuple!(0 => a, 1 => b);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d, 4 => e);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d, 4 => e, 5 => f);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d, 4 => e, 5 => f, 6 => g);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d, 4 => e, 5 => f, 6 => g, 7 => h);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d, 4 => e, 5 => f, 6 => g, 7 => h, 8 => i);
impl_point_for_tuple!(0 => a, 1 => b, 2 => c, 3 => d, 4 => e, 5 => f, 6 => g, 7 => h, 8 => i, 9 => j);

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::Wrapping;

    #[test]
    fn test_types() {
        fn assert_impl_rtreenum<S: RTreeNum>() {}

        assert_impl_rtreenum::<i8>();
        assert_impl_rtreenum::<i16>();
        assert_impl_rtreenum::<i32>();
        assert_impl_rtreenum::<i64>();
        assert_impl_rtreenum::<i128>();
        assert_impl_rtreenum::<isize>();
        assert_impl_rtreenum::<Wrapping<i8>>();
        assert_impl_rtreenum::<Wrapping<i16>>();
        assert_impl_rtreenum::<Wrapping<i32>>();
        assert_impl_rtreenum::<Wrapping<i64>>();
        assert_impl_rtreenum::<Wrapping<i128>>();
        assert_impl_rtreenum::<Wrapping<isize>>();
        assert_impl_rtreenum::<f32>();
        assert_impl_rtreenum::<f64>();
    }

    macro_rules! test_tuple_configuration {
        ($($index:expr),*) => {
            let a = ($($index),*);
            $(assert_eq!(a.nth($index), $index));*
        }
    }

    #[test]
    fn test_tuples() {
        // Test a couple of simple cases
        let simple_int = (0, 1, 2);
        assert_eq!(simple_int.nth(2), 2);
        let simple_float = (0.5, 0.67, 1234.56);
        assert_eq!(simple_float.nth(2), 1234.56);
        let long_int = (0, 1, 2, 3, 4, 5, 6, 7, 8);
        assert_eq!(long_int.nth(8), 8);

        // Generate the code to test every nth function for every Tuple length
        test_tuple_configuration!(0, 1);
        test_tuple_configuration!(0, 1, 2);
        test_tuple_configuration!(0, 1, 2, 3);
        test_tuple_configuration!(0, 1, 2, 3, 4);
        test_tuple_configuration!(0, 1, 2, 3, 4, 5);
        test_tuple_configuration!(0, 1, 2, 3, 4, 5, 6);
        test_tuple_configuration!(0, 1, 2, 3, 4, 5, 6, 7);
        test_tuple_configuration!(0, 1, 2, 3, 4, 5, 6, 7, 8);
    }
}
