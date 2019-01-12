# rstar

A flexible, n-dimensional [r*-tree](https://en.wikipedia.org/wiki/R*_tree) implementation for the rust ecosystem.

# Features
 - A flexible r*-tree written in safe rust
 - Supports custom point types
 - Supports the insertion of user defined types
 - Supported operations:
   - Insertion
   - Rectangle queries
   - Nearest neighbor
   - Nearest neighbor iteration
   - Locate at point
   - Element removal
   - efficient bulk loading
 - Features geometric primitives that can readily be inserted into an r-tree:
   - Points (arrays with a constant size)
   - Lines
   - Rectangles
 - Small number of dependencies
 - Serde support with the `serde` feature

# Benchmarks
All benchmarks are performed on a i7-8550U CPU @ 1.80Ghz and with uniformly distributed points. The underlying point type is `[f64; 2]`.

| Benchmark                      | Tree size | Time      |
|-------------------------------------|-----:|----------:|
| bulk loading                        | 2000 | 229.82 us |
| sequentially loading                | 2000 | 1.4477 ms |
| nearest neighbor (bulk loaded tree) | 100k |   1.32 us |
| nearest neighbor (sequential tree)  | 100k |   1.56 us |
| successful point lookup             | 100k | 177.32 ns |
| unsuccessful point lookup           | 100k | 273.51 ns |

# Project state
The project is being actively developed, feature requests and PRs are welcome!

# Documentation
The documentation is hosted on [docs.rs](https://docs.rs/rstar/).

# License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
