//! Contains primitives ready for insertion into an r-tree.

mod line;
mod line_with_data;
mod point_with_data;
mod rectangle;
mod rectangle_with_data;

pub use self::line::Line;
pub use self::line_with_data::LineWithData;
pub use self::point_with_data::PointWithData;
pub use self::rectangle::Rectangle;
pub use self::rectangle_with_data::RectangleWithData;
