//! Contains primitives ready for insertion into an r-tree.

mod line;
mod point_with_data;
mod rectangle;

pub use self::{line::Line, point_with_data::PointWithData, rectangle::Rectangle};
