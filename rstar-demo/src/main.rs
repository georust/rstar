// Copyright 2017 The Spade Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*
 * This example is an interactive demo showing the features of spade's delaunay
 * triangulation and R-Tree. Press h for help.
 */

extern crate rand;
extern crate rstar;
#[macro_use]
extern crate glium;
extern crate num_traits;

mod graphics;

use graphics::RenderData;
use rstar::RTree;
use rstar::RTreeNum;
use glium::DisplayBuild;
use glium::glutin::{ElementState, Event, MouseButton};
use glium::glutin::VirtualKeyCode;
use rand::{Rng, SeedableRng, XorShiftRng};
use rand::distributions::{Range, Distribution};
use rand::distributions::range::{SampleRange};

pub type Point = [f64; 2];

#[derive(Clone, Copy)]
pub enum LookupMode {
    Nearest,
}

impl ::std::fmt::Display for LookupMode {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                &LookupMode::Nearest => "Nearest neighbor",
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

    let mut rtree: RTree<Point> = RTree::new();

    let mut render_data = RenderData::new(&display);
    render_data.update_rtree_buffers(&display, &rtree);

    let mut last_point = [0.0, 0.0];
    let lookup_mode = LookupMode::Nearest;

    let seed = b"criminalisations";
    let mut rng = XorShiftRng::from_seed(*seed);

    println!("Interactive Demo");
    print_help();
    loop {
        let events: Vec<_> = display.poll_events().collect();

        let mut dirty = false;
        for event in events.into_iter() {
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
                            let new_points = ::random_points_with_seed(num, seed);
                            for point in new_points.into_iter() {
                                rtree.insert(point);
                            }
                            rtree.root().sanity_check();
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
                    last_point = [x as f64, y as f64];
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

fn get_selected_vertices(tree: &RTree<Point>, point: Point, lookup_mode: LookupMode) -> Vec<Point> {
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

fn random_points_in_range<S: RTreeNum + SampleRange>(
    range: S,
    size: usize,
    seed: [u8; 16],
) -> Vec<[S; 2]> {
    let mut rng = XorShiftRng::from_seed(seed);
    let range = Range::new(-range, range);
    let mut points = Vec::with_capacity(size);
    for _ in 0..size {
        let x = range.sample(&mut rng);
        let y = range.sample(&mut rng);
        points.push([x, y]);
    }
    points
}

fn random_points_with_seed<S: RTreeNum + SampleRange>(
    size: usize,
    seed: [u8; 16],
) -> Vec<[S; 2]> {
    random_points_in_range(S::one(), size, seed)
}
