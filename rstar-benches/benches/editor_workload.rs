//! Benchmarks modelling an interactive editor's spatial index: 2D `f64` rectangles with an id,
//! default tree parameters, a bulk load followed by `remove` + `insert` churn, viewport queries
//! (`locate_in_envelope_intersecting`) and k-nearest lookups.

#[macro_use]
extern crate criterion;

use std::hint::black_box;

use criterion::{BatchSize, Criterion};
use rand::{RngExt, SeedableRng};
use rand_hc::Hc128Rng;
use rstar::{PointDistance, RTree, RTreeObject, AABB};

const SEED: &[u8; 32] = b"Gv0aHMtHkBGsUXNspGU9fLRuCWkZWHZx";

/// Side of the square the items are scattered over: about half of it is covered by groups.
fn world(n: usize) -> f64 {
    220.0 * (n as f64).sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Item {
    id: u128,
    env: AABB<[f64; 2]>,
}

impl RTreeObject for Item {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.env
    }
}

impl PointDistance for Item {
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        self.env.distance_2(point)
    }
}

/// Groups of small rectangles, each group followed by a container covering it (a container is
/// indexed with the bounds of its subtree).
fn create_items(n: usize, rng: &mut Hc128Rng) -> Vec<Item> {
    const GROUP: usize = 8;
    let world = world(n);
    let mut items = Vec::with_capacity(n);
    while items.len() < n {
        let [cx, cy]: [f64; 2] = rng.random();
        let (cx, cy) = (cx * world, cy * world);
        let start = items.len();
        let mut hull = AABB::from_point([cx, cy]);
        for _ in 0..GROUP.min(n - start) {
            let [dx, dy, w, h]: [f64; 4] = rng.random();
            let (x, y) = (cx + dx * 400.0, cy + dy * 400.0);
            let env = AABB::from_corners([x, y], [x + 10.0 + w * 110.0, y + 10.0 + h * 60.0]);
            hull = AABB::from_corners(
                [hull.lower()[0].min(x), hull.lower()[1].min(y)],
                [
                    hull.upper()[0].max(env.upper()[0]),
                    hull.upper()[1].max(env.upper()[1]),
                ],
            );
            items.push(Item {
                id: items.len() as u128,
                env,
            });
        }
        if items.len() < n {
            items.push(Item {
                id: items.len() as u128,
                env: hull,
            });
        }
    }
    items
}

fn moved(item: &Item, rng: &mut Hc128Rng) -> Item {
    let [dx, dy]: [f64; 2] = rng.random();
    let (dx, dy) = ((dx - 0.5) * 300.0, (dy - 0.5) * 300.0);
    let (l, u) = (item.env.lower(), item.env.upper());
    Item {
        id: item.id,
        env: AABB::from_corners([l[0] + dx, l[1] + dy], [u[0] + dx, u[1] + dy]),
    }
}

fn bulk_load(c: &mut Criterion) {
    // 28_000 is a count for which OMT used to produce leaves at different depths.
    for n in [2_000usize, 28_000, 200_000] {
        let items = create_items(n, &mut Hc128Rng::from_seed(*SEED));
        c.bench_function(&format!("editor/bulk_load/{n}"), |b| {
            b.iter_batched(|| items.clone(), RTree::bulk_load, BatchSize::LargeInput);
        });
    }
}

fn churn(c: &mut Criterion) {
    for n in [2_000usize, 200_000] {
        let mut rng = Hc128Rng::from_seed(*SEED);
        let mut items = create_items(n, &mut rng);
        let mut tree = RTree::bulk_load(items.clone());
        let mut next = 0;
        c.bench_function(&format!("editor/move (remove + insert)/{n}"), |b| {
            b.iter(|| {
                let i = (next * 7919) % n;
                next += 1;
                let new = moved(&items[i], &mut rng);
                assert!(tree.remove(&items[i]).is_some());
                tree.insert(new);
                items[i] = new;
            });
        });

        let mut tree = RTree::bulk_load(items.clone());
        let mut next = 0;
        c.bench_function(&format!("editor/remove + reinsert same/{n}"), |b| {
            b.iter(|| {
                let i = (next * 7919) % n;
                next += 1;
                let it = tree.remove(&items[i]).unwrap();
                tree.insert(it);
            });
        });
    }
}

fn queries(c: &mut Criterion) {
    for n in [2_000usize, 200_000] {
        let mut rng = Hc128Rng::from_seed(*SEED);
        let items = create_items(n, &mut rng);
        // What queries see right after opening a document.
        let tree = RTree::bulk_load(items);
        let world = world(n);
        let mut views = |w: f64, h: f64| -> Vec<AABB<[f64; 2]>> {
            (0..64)
                .map(|_| {
                    let [x, y]: [f64; 2] = rng.random();
                    let (x, y) = (x * (world - w), y * (world - h));
                    AABB::from_corners([x, y], [x + w, y + h])
                })
                .collect()
        };
        let viewports = views(1920.0, 1080.0);
        let zoomed_out = views(1920.0 * 4.0, 1080.0 * 4.0);
        let points: Vec<[f64; 2]> = (0..64)
            .map(|_| {
                let [x, y]: [f64; 2] = rng.random();
                [x * world, y * world]
            })
            .collect();

        let mut out: Vec<u128> = Vec::new();
        for (name, views) in [
            ("viewport", &viewports),
            ("zoomed out viewport", &zoomed_out),
        ] {
            c.bench_function(&format!("editor/{name} query/{n}"), |b| {
                b.iter(|| {
                    for v in views {
                        out.clear();
                        out.extend(tree.locate_in_envelope_intersecting(*v).map(|i| i.id));
                        black_box(&out);
                    }
                });
            });
        }

        c.bench_function(&format!("editor/point query/{n}"), |b| {
            b.iter(|| {
                for p in &points {
                    let env =
                        AABB::from_corners([p[0] - 4.0, p[1] - 4.0], [p[0] + 4.0, p[1] + 4.0]);
                    out.clear();
                    out.extend(tree.locate_in_envelope_intersecting(env).map(|i| i.id));
                    black_box(&out);
                }
            });
        });

        c.bench_function(&format!("editor/nearest 8/{n}"), |b| {
            b.iter(|| {
                for p in &points {
                    for (item, d2) in tree.nearest_neighbor_iter_with_distance_2(*p).take(8) {
                        black_box((item.id, d2));
                    }
                }
            });
        });
    }
}

criterion_group!(benches, bulk_load, churn, queries);
criterion_main!(benches);
