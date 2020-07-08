use kiss3d::camera::ArcBall;
use kiss3d::event::{Action, MouseButton, WindowEvent};
use kiss3d::light::Light;
use kiss3d::planar_camera::{PlanarCamera, Sidescroll};
use kiss3d::text::Font;
use kiss3d::window::Window;
use nalgebra::{Point2, Point3, Vector2};
use rand::distributions::Uniform;
use rand::Rng;
use rstar::{Point, RStarInsertionStrategy, RTree, RTreeNode, RTreeParams, AABB};

mod three_d;
mod two_d;

mod buttons {
    use kiss3d::event::Key;
    pub const ADD_BUTTON: Key = Key::A;
    pub const ADD_MANY_BUTTON: Key = Key::B;
    pub const SWITCH_RENDER_MODE: Key = Key::F;
    pub const RESET_EMPTY: Key = Key::R;
    pub const RESET_BULK_LOAD: Key = Key::T;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum RenderMode {
    ThreeD,
    TwoD,
}

impl RenderMode {
    fn get_next_mode(self) -> Self {
        if self == RenderMode::ThreeD {
            RenderMode::TwoD
        } else {
            RenderMode::ThreeD
        }
    }
}

pub struct Scene {
    render_mode: RenderMode,
    last_cursor_position: TreePointType2D,
    tree_3d: DemoTree3D,
    tree_2d: DemoTree2D,
    camera_3d: ArcBall,
    camera_2d: Sidescroll,
}

impl Scene {
    fn new(window_width: u32, window_height: u32) -> Self {
        Scene {
            render_mode: RenderMode::ThreeD,
            last_cursor_position: Default::default(),
            camera_3d: create_default_camera_3d(),
            camera_2d: create_default_camera_2d(),
            tree_3d: Self::bulk_load_tree_3d(),
            tree_2d: Self::bulk_load_tree_2d(window_width, window_height),
        }
    }

    fn reset_to_bulk_loaded_tree(&mut self, window_width: u32, window_height: u32) {
        match self.render_mode {
            RenderMode::TwoD => self.tree_2d = Self::bulk_load_tree_2d(window_width, window_height),
            RenderMode::ThreeD => self.tree_3d = Self::bulk_load_tree_3d(),
        }
    }

    fn reset_to_empty(&mut self) {
        match self.render_mode {
            RenderMode::TwoD => self.tree_2d = Default::default(),
            RenderMode::ThreeD => self.tree_3d = Default::default(),
        }
    }

    fn bulk_load_tree_3d() -> DemoTree3D {
        let points_3d = create_random_points(500);
        DemoTree3D::bulk_load_with_params(points_3d)
    }

    fn bulk_load_tree_2d(window_width: u32, window_height: u32) -> DemoTree2D {
        let mut points_2d = create_random_points::<TreePointType2D>(1000);
        for &mut [ref mut x, ref mut y] in &mut points_2d {
            *x = *x * window_width as f32 * 0.5;
            *y = *y * window_height as f32 * 0.5;
        }
        DemoTree2D::bulk_load_with_params(points_2d)
    }
}

type DemoTree3D = RTree<TreePointType3D, Params>;
type TreePointType3D = [f32; 3];

type DemoTree2D = RTree<TreePointType2D>;
type TreePointType2D = [f32; 2];

pub struct Params;
impl RTreeParams for Params {
    const MIN_SIZE: usize = 5;
    const MAX_SIZE: usize = 9;
    const REINSERTION_COUNT: usize = 3;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}

pub enum RenderData {
    ThreeD(
        Vec<three_d::LineRenderData3D>,
        Vec<three_d::PointRenderData3D>,
    ),
    TwoD(Vec<two_d::LineRenderData2D>),
}

fn main() {
    const WINDOW_WIDTH: u32 = 1024;
    const WINDOW_HEIGHT: u32 = 768;
    let mut window = Window::new_with_size("RStar demo", WINDOW_WIDTH, WINDOW_HEIGHT);
    window.set_background_color(1.0, 1.0, 1.0);
    window.set_light(Light::StickToCamera);
    window.set_point_size(4.);

    let mut scene = Scene::new(WINDOW_WIDTH, WINDOW_HEIGHT);

    let mut render_data = create_render_data_from_scene(&scene);

    while window.render_with_cameras(&mut scene.camera_3d, &mut scene.camera_2d) {
        render_data = handle_input(&window, &mut scene).unwrap_or(render_data);
        draw_tree(&mut window, &render_data);
        draw_help(&mut window, scene.render_mode);
    }
}

fn create_default_camera_3d() -> ArcBall {
    ArcBall::new(Point3::new(3.0, 2.5, 2.0), Point3::origin())
}

fn create_default_camera_2d() -> Sidescroll {
    Sidescroll::new()
}

pub fn create_random_points<P: Point<Scalar = f32>>(num_points: usize) -> Vec<P> {
    let mut result = Vec::with_capacity(num_points);
    let mut rng = rand::thread_rng();
    let distribution = Uniform::new(-1.0f32, 1.0);
    for _ in 0..num_points {
        let new_point = P::generate(|_| rng.sample(distribution));
        result.push(new_point);
    }
    result
}

fn handle_input(window: &Window, scene: &mut Scene) -> Option<RenderData> {
    let mut points_to_add = Vec::new();
    let mut is_dirty = false;
    for event in window.events().iter() {
        match event.value {
            WindowEvent::Key(buttons::ADD_BUTTON, Action::Press, _) => {
                points_to_add = create_random_points(10);
            }
            WindowEvent::Key(buttons::RESET_EMPTY, Action::Press, _) => {
                if scene.render_mode == RenderMode::TwoD {
                    scene.reset_to_empty();
                    is_dirty = true;
                }
            }
            WindowEvent::Key(buttons::RESET_BULK_LOAD, Action::Press, _) => {
                scene.reset_to_bulk_loaded_tree(window.width(), window.height());
                is_dirty = true;
            }
            WindowEvent::Key(buttons::ADD_MANY_BUTTON, Action::Press, _) => {
                points_to_add = create_random_points(100);
            }
            WindowEvent::Key(buttons::SWITCH_RENDER_MODE, Action::Press, _) => {
                scene.render_mode = scene.render_mode.get_next_mode();
                scene.camera_3d = create_default_camera_3d();
                scene.camera_2d = Sidescroll::new();
                is_dirty = true;
            }
            WindowEvent::Scroll(_, second, _) => {
                let current_zoom = scene.camera_2d.zoom();
                let factor = if second > 0. { 1.1 } else { 0.9 };
                scene.camera_2d.set_zoom(current_zoom * factor);
            }
            WindowEvent::MouseButton(MouseButton::Button1, Action::Press, _)
                if scene.render_mode == RenderMode::TwoD =>
            {
                let &[width, height] = window.size().as_ref();
                let unprojected = scene.camera_2d.unproject(
                    &scene.last_cursor_position.into(),
                    &Vector2::new(width as f32, height as f32),
                );

                scene.tree_2d.insert(unprojected.coords.into());
                is_dirty = true;
            }
            WindowEvent::CursorPos(x, y, _) if scene.render_mode == RenderMode::TwoD => {
                scene.last_cursor_position = [x as f32, y as f32];
            }
            _ => (),
        }
    }
    is_dirty |= !points_to_add.is_empty();
    if is_dirty {
        for point in points_to_add {
            if scene.render_mode == RenderMode::ThreeD {
                scene.tree_3d.insert(point);
            } else {
                scene.tree_2d.insert([
                    point[0] * window.width() as f32 * 0.5,
                    point[1] * window.height() as f32 * 0.5,
                ]);
            }
        }
        create_render_data_from_scene(scene).into()
    } else {
        None
    }
}

fn draw_tree(window: &mut Window, render_data: &RenderData) {
    match render_data {
        RenderData::ThreeD(ref lines, ref points) => {
            for (from, to, color) in lines {
                window.draw_line(&from, &to, &color);
            }

            for (point, color) in points {
                window.draw_point(&point, &color);
            }
        }
        RenderData::TwoD(lines) => {
            for (from, to, color) in lines {
                window.draw_planar_line(from, to, &color);
            }
        }
    }
}

fn draw_help(window: &mut Window, render_mode: RenderMode) {
    let font = Font::default();
    window.draw_text(
        &get_help_text(render_mode),
        &Point2::new(5.0, 5.0),
        60.0,
        &font,
        &Point3::new(0.0, 0.0, 0.0),
    );
}

fn get_help_text(render_mode: RenderMode) -> String {
    match render_mode {
        RenderMode::ThreeD => format!(
            "{:?}: Add 10 points\n\
             {:?}: Add 100 points\n\
             {:?}: Reset tree\n\
             {:?}: Change 2D/3D\n\
             LMB: Rotate view\n\
             RMB: Pan view\n\
             Mouse wheel: Zoom",
            buttons::ADD_BUTTON,
            buttons::ADD_MANY_BUTTON,
            buttons::RESET_BULK_LOAD,
            buttons::SWITCH_RENDER_MODE,
        ),
        RenderMode::TwoD => format!(
            "{:?}: Add 10 points\n\
             {:?}: Add 100 points\n\
             {:?}/{:?}: Reset tree\n\
             {:?}: Change 2D/3D\n\
             LMB: Add point\n\
             RMB: Pan view\n\
             Mouse wheel: Zoom\n\
             ",
            buttons::ADD_BUTTON,
            buttons::ADD_MANY_BUTTON,
            buttons::RESET_EMPTY,
            buttons::RESET_BULK_LOAD,
            buttons::SWITCH_RENDER_MODE,
        ),
    }
}

fn create_render_data_from_scene(scene: &Scene) -> RenderData {
    if scene.render_mode == RenderMode::TwoD {
        crate::two_d::create_render_data_for_tree_2d(&scene.tree_2d)
    } else {
        crate::three_d::create_render_data_for_tree_3d(&scene.tree_3d)
    }
}

fn get_color_for_depth(depth: usize) -> Point3<f32> {
    match depth {
        0 => Point3::new(0., 0., 0.),
        1 => Point3::new(0.85, 0., 0.85),
        2 => Point3::new(0., 0.85, 0.85),
        3 => Point3::new(0., 0., 0.55),
        4 => Point3::new(0.85, 0., 0.),
        _ => Point3::new(0., 0.85, 0.85),
    }
}
