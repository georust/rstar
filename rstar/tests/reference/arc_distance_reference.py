#!/usr/bin/env python3
"""Independent ground truth for the ARC_GOLDEN table in geodetic_arc_property.rs.

Computes the great-circle distance from a query point to the shorter great-circle arc
A -> B on a sphere of radius 6_371_008.8 m, as a squared chord, with no shared code
with the rstar implementation or its in-test oracle. Correctness rests on three
independent legs:

  1. mpmath at 60 decimal digits, so floating-point conditioning is irrelevant.
  2. Two algorithmically distinct methods that must agree to < 1e-22 rad:
       A. project the query onto the arc's plane, take the foot if on the arc else an
          endpoint;
       B. parametrise the arc by slerp and minimise the angle by a 300-step ternary.
  3. s2sphere (an independent S2 port) cross-checks the endpoint angles.

Run with:

    uv run --with s2sphere --with mpmath rstar/tests/reference/arc_distance_reference.py

and paste the printed rows into ARC_GOLDEN. The case list deliberately includes every
degenerate counterexample Hegel shrank the f64 test oracle to (behind-A, pole-of-circle
~90deg cross-track, tiny arcs, an arc starting at a pole, near-antipodal queries, the
antimeridian) plus a seeded-random spread.
"""

import math
import random

import mpmath as mp
import s2sphere

mp.mp.dps = 60
R = mp.mpf("6371008.8")


def to_vec(lon, lat):
    la = mp.radians(mp.mpf(repr(lat)))
    lo = mp.radians(mp.mpf(repr(lon)))
    return (mp.cos(la) * mp.cos(lo), mp.cos(la) * mp.sin(lo), mp.sin(la))


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def cross(a, b):
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def norm(a):
    return mp.sqrt(dot(a, a))


def unit(a):
    n = norm(a)
    return (a[0] / n, a[1] / n, a[2] / n)


def scale(a, s):
    return (a[0] * s, a[1] * s, a[2] * s)


def angle(a, b):  # central angle, well conditioned via atan2(|a x b|, a . b)
    return mp.atan2(norm(cross(a, b)), dot(a, b))


TINY = mp.mpf(10) ** -40


def method_projection(qv, av, bv):
    theta = angle(av, bv)
    candidates = [av, bv]
    n_unnorm = cross(av, bv)
    if norm(n_unnorm) > TINY:
        n = unit(n_unnorm)
        d = dot(qv, n)
        proj = (qv[0] - d * n[0], qv[1] - d * n[1], qv[2] - d * n[2])
        if norm(proj) > TINY:
            foot = unit(proj)
            for m in (foot, scale(foot, -1)):
                if abs(angle(av, m) + angle(m, bv) - theta) < mp.mpf(10) ** -25:
                    candidates.append(m)
    return angle(qv, max(candidates, key=lambda c: dot(qv, c)))


def method_slerp(qv, av, bv):
    theta = angle(av, bv)
    if theta < TINY:
        return angle(qv, av)
    s = mp.sin(theta)

    def point(t):
        w0, w1 = mp.sin(theta - t) / s, mp.sin(t) / s
        return (
            w0 * av[0] + w1 * bv[0],
            w0 * av[1] + w1 * bv[1],
            w0 * av[2] + w1 * bv[2],
        )

    lo, hi = mp.mpf(0), theta
    for _ in range(300):
        m1 = lo + (hi - lo) / 3
        m2 = hi - (hi - lo) / 3
        if angle(qv, point(m1)) < angle(qv, point(m2)):
            hi = m2
        else:
            lo = m1
    return angle(qv, point((lo + hi) / 2))


def ground_truth_angle(a_lon, a_lat, b_lon, b_lat, q_lon, q_lat):
    av, bv, qv = to_vec(a_lon, a_lat), to_vec(b_lon, b_lat), to_vec(q_lon, q_lat)
    a = method_projection(qv, av, bv)
    b = method_slerp(qv, av, bv)
    if abs(a - b) > mp.mpf(10) ** -22:
        raise RuntimeError(f"reference methods disagree by {mp.nstr(abs(a - b), 5)} rad")
    # s2sphere cross-check on the endpoint angles (its f64 internals agree to ~1e-9).
    for lon, lat in ((a_lon, a_lat), (b_lon, b_lat)):
        p1 = s2sphere.LatLng.from_degrees(lat, lon).to_point()
        p2 = s2sphere.LatLng.from_degrees(q_lat, q_lon).to_point()
        s2 = s2sphere.LatLng.from_point(p1).get_distance(
            s2sphere.LatLng.from_point(p2)
        )
        ref = angle(qv, to_vec(lon, lat))
        assert abs(mp.mpf(s2.radians) - ref) < mp.mpf("1e-8")
    return (a + b) / 2


CURATED = [
    (0, 0, 0, 80, 0, 40),
    (0, 1, 0, 2, 0, 0),
    (10, 0, 40, 0, 0, 90),
    (-30, 0, 30, 0, 0, -10),
    (0, 0, 0, 1.192092896e-07, 0, 1),
    (0, 90, -180, -1, 0, 0),
    (-180, 0, -179.99999992312644, -9.111154769854667e-08, 0, 1.192092896e-07),
    (0, 42, 79.70606382642427, 77.44802220072441, -101.56168723473104, 12.549002721292226),
    (170, 0, -170, 0, 179, 5),
    (0, 0, 90, 0, 45, 0.0001),
    (0, 0, 10, 0, 5, 10),
    (-73, 40, 2, 48, -30, 55),
]


def random_cases(n):
    random.seed(20260603)
    out = []
    for _ in range(n):
        a_lon, a_lat = random.uniform(-180, 180), random.uniform(-90, 90)
        gamma, az = random.uniform(0, 170), random.uniform(-180, 180)
        phi1, lam1 = math.radians(a_lat), math.radians(a_lon)
        th, azr = math.radians(gamma), math.radians(az)
        phi2 = math.asin(
            math.sin(phi1) * math.cos(th) + math.cos(phi1) * math.sin(th) * math.cos(azr)
        )
        lam2 = lam1 + math.atan2(
            math.sin(azr) * math.sin(th) * math.cos(phi1),
            math.cos(th) - math.sin(phi1) * math.sin(phi2),
        )
        b_lon = ((math.degrees(lam2) + 540) % 360) - 180
        b_lat = math.degrees(phi2)
        q_lon, q_lat = random.uniform(-180, 180), random.uniform(-90, 90)
        out.append((a_lon, a_lat, b_lon, b_lat, q_lon, q_lat))
    return out


def main():
    for case in CURATED + random_cases(40):
        c2 = float(2 - 2 * mp.cos(ground_truth_angle(*case)))
        cols = ", ".join(repr(float(x)) for x in case)
        print(f"    ({cols}, {c2!r}),")


if __name__ == "__main__":
    main()
