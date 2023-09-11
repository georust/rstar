//! [`mint`](https://crates.io/crates/mint) is a library for
//! interoperability between maths crates, for example, you may want
//! to use [nalgebra](https://crates.io/crates/nalgebra) types for
//! representing your points and _also_ use the same points with the
//! `rstar` library.
//!
//! Here is an example of how you might do that using `mint` types for
//! compatibility between the two libraries. Make sure to enable the
//! `mint` features on both the `nalgebra` and the `rstar` crates for
//! this to work. You will also need to depend on the
//! [`mint`](https://crates.io/crates/mint) crate.
//!
//! ```
//! use rstar::RTree;
//!
//! let point1 = nalgebra::Point2::new(0.0, 0.0);
//! let point2 = nalgebra::Point2::new(1.0, 1.0);
//!
//! // First we have to convert the foreign points into the mint
//! // compatibility types before we can store them in the rtree
//!
//! let mint_point1: mint::Point2<f64> = point1.into();
//! let mint_point2: mint::Point2<f64> = point2.into();
//!
//! // Now we can use them with rtree structs and methods
//! let mut rtree = RTree::new();
//!
//! rtree.insert(mint_point2);
//!
//! assert_eq!(rtree.nearest_neighbor(&mint_point1), Some(&mint_point2));
//! ```

use crate::{Point, RTreeNum};

impl<T: RTreeNum> Point for mint::Point2<T> {
    type Scalar = T;

    const DIMENSIONS: usize = 2;

    fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self {
        mint::Point2 {
            x: generator(0),
            y: generator(1),
        }
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        match index {
            0 => self.x,
            1 => self.y,
            _ => unreachable!(),
        }
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            _ => unreachable!(),
        }
    }
}

impl<T: RTreeNum> Point for mint::Point3<T> {
    type Scalar = T;

    const DIMENSIONS: usize = 3;

    fn generate(mut generator: impl FnMut(usize) -> Self::Scalar) -> Self {
        mint::Point3 {
            x: generator(0),
            y: generator(1),
            z: generator(2),
        }
    }

    fn nth(&self, index: usize) -> Self::Scalar {
        match index {
            0 => self.x,
            1 => self.y,
            2 => self.z,
            _ => unreachable!(),
        }
    }

    fn nth_mut(&mut self, index: usize) -> &mut Self::Scalar {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            _ => unreachable!(),
        }
    }
}
