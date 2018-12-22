extern crate rand;
extern crate rstar;
#[macro_use]
extern crate glium;
extern crate num_traits;
extern crate rand_hc;

mod graphics;

use crate::graphics::RenderData;
use glium::glutin::VirtualKeyCode;
use glium::glutin::{ElementState, Event, MouseButton};
use glium::DisplayBuild;
use rand::distributions::uniform::*;
use rand::distributions::Distribution;
use rand::{Rng, SeedableRng};
use rand_hc::Hc128Rng;
use rstar::root;
use rstar::RTree;
use rstar::{RStarInsertionStrategy, RTreeNum, RTreeParams};

pub type Point = [f64; 2];

#[derive(Clone, Copy)]
pub enum LookupMode {
    Nearest,
}

struct Params;
impl RTreeParams for Params {
    const MIN_SIZE: usize = 4;
    const MAX_SIZE: usize = 9;
    type DefaultInsertionStrategy = RStarInsertionStrategy;
}

impl ::std::fmt::Display for LookupMode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                LookupMode::Nearest => "Nearest neighbor",
            }
        )
    }
}

pub fn main() {
    let display = ::glium::glutin::WindowBuilder::new()
        .with_dimensions(800, 800)
        .with_title("Interactive Demo".to_string())
        .build_glium()
        .unwrap();

    let points = random_points_with_seed(10000, *b"abcdefghijklm1+nnonrestrictively");
    let mut rtree: RTree<Point, Params> = RTree::bulk_load_with_params(points);

    let mut render_data = RenderData::new(&display);
    render_data.update_rtree_buffers(&display, &rtree);

    let mut last_point = [0.0, 0.0];
    let lookup_mode = LookupMode::Nearest;

    let seed = b"seminationalizedcriminalisations";
    let mut rng = Hc128Rng::from_seed(*seed);

    println!("Interactive Demo");
    print_help();
    loop {
        let events: Vec<_> = display.poll_events().collect();

        let mut dirty = false;
        for event in events {
            match event {
                Event::Refresh => render_data.draw(&display),
                Event::Closed => return,
                Event::KeyboardInput(ElementState::Pressed, _, Some(key)) => {
                    match key {
                        VirtualKeyCode::Escape => return,
                        VirtualKeyCode::H => {
                            print_help();
                        }
                        VirtualKeyCode::F => {
                            render_data.update_rtree_buffers(&display, &rtree);
                            dirty = true;
                        }
                        VirtualKeyCode::A | VirtualKeyCode::B => {
                            // Insert some random points
                            let num = if key == VirtualKeyCode::A {
                                10usize
                            } else {
                                100
                            };
                            // let seed = ::rand::thread_rng().gen();
                            let mut seed = rng.gen();
                            for _ in 0..num {
                                seed = rng.gen();
                            }
                            let new_points = crate::random_points_with_seed(num, seed);
                            for point in new_points {
                                rtree.insert(point);
                            }
                            root(&rtree).sanity_check::<Params>();
                            render_data.update_rtree_buffers(&display, &rtree);
                            dirty = true;
                        }
                        _ => (),
                    }
                }
                Event::MouseInput(ElementState::Pressed, MouseButton::Left) => {
                    rtree.insert(last_point);
                    render_data.update_rtree_buffers(&display, &rtree);
                    dirty = true;
                }
                // Event::MouseInput(ElementState::Pressed, MouseButton::Right) => {
                //     let nn = rtree.nearest_neighbor(&last_point).cloned();
                //     if let Some(p) = nn {
                //         rtree.remove(&p);
                //         render_data.update_rtree_buffers(&display, &rtree);
                //         let selection = get_selected_vertices(&rtree, last_point, lookup_mode);
                //         render_data.update_selection(&display, &selection);
                //         dirty = true;
                //     }
                // },
                Event::MouseMoved(x, y) => {
                    let (w, h) = display.get_framebuffer_dimensions();
                    // Transform x, y into the range [-1 , 1]
                    let y = h as i32 - y;
                    let x = (x as f32 / w as f32) * 2. - 1.;
                    let y = (y as f32 / h as f32) * 2. - 1.;
                    last_point = [f64::from(x), f64::from(y)];
                    let selection = get_selected_vertices(&rtree, last_point, lookup_mode);
                    render_data.update_selection(&display, &selection);
                    dirty = true;
                }
                _ => (),
            }
        }
        if dirty {
            render_data.draw(&display);
        }
    }
}

fn get_selected_vertices<Params>(
    tree: &RTree<Point, Params>,
    point: Point,
    lookup_mode: LookupMode,
) -> Vec<Point>
where
    Params: RTreeParams,
{
    let mut points = Vec::new();
    match lookup_mode {
        LookupMode::Nearest => {
            points.extend(tree.nearest_neighbor(&point).iter().cloned());
        }
    }
    points
}

fn print_help() {
    println!("H - print this help dialog");
    println!("M - change lookup mode");
    println!("A - add 10 random points.");
    println!("B - add 100 random points.");
    println!("--------------------------");
    println!("Left click: Add single point.");
    println!("Right click: Delete closest point.");
}

fn random_points_in_range<S: RTreeNum + SampleUniform>(
    range: S,
    size: usize,
    seed: [u8; 32],
) -> Vec<[S; 2]> {
    let mut rng = Hc128Rng::from_seed(seed);
    let range = Uniform::new(-range, range);
    let mut points = Vec::with_capacity(size);
    for _ in 0..size {
        let x = range.sample(&mut rng);
        let y = range.sample(&mut rng);
        points.push([x, y]);
    }
    points
}

fn random_points_with_seed<S: RTreeNum + SampleUniform>(
    size: usize,
    seed: [u8; 32],
) -> Vec<[S; 2]> {
    random_points_in_range(S::one(), size, seed)
}
