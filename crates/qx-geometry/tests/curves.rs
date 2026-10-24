use qx_geometry::{
    arc_length::{ArcLengthOptions, ArcLengthTable},
    curve::{ControlPoint, Curve, Parameterization, clamped_knots},
    curve_edit::{BezierContinuity, CurveEdit},
    math::Vec3,
    transition::Side,
};
fn points(values: &[[f64; 3]]) -> Vec<ControlPoint> {
    values
        .iter()
        .enumerate()
        .map(|(i, p)| ControlPoint {
            id: i as u64 + 1,
            position_m: Vec3::new(p[0], p[1], p[2]),
        })
        .collect()
}
fn arc() -> Curve {
    Curve::Nurbs {
        degree: 2,
        points: points(&[[0., 0., 0.], [10., 0., 0.], [10., 10., 0.]]),
        knots: vec![0., 0., 0., 1., 1., 1.],
        weights: vec![1., std::f64::consts::FRAC_1_SQRT_2, 1.],
    }
}
fn families() -> Vec<Curve> {
    vec![
        Curve::Bezier {
            points: points(&[[0., 0., 0.], [2., 0., 0.], [6., 3., 1.], [10., 4., 2.]]),
        },
        Curve::BSpline {
            degree: 3,
            points: points(&[
                [0., 0., 0.],
                [2., 0., 0.],
                [5., 4., 0.],
                [8., 3., 2.],
                [12., 2., 3.],
            ]),
            knots: clamped_knots(5, 3).unwrap(),
        },
        arc(),
        Curve::CatmullRom {
            points: points(&[[0., 0., 0.], [2., 0., 0.], [7., 4., 0.], [14., 2., 3.]]),
            parameterization: Parameterization::Centripetal,
        },
    ]
}
#[test]
fn bezier_endpoints_and_independent_derivatives() {
    let curve = families().remove(0);
    let p = curve.points();
    let a = curve.evaluate(0.).unwrap();
    let b = curve.evaluate(1.).unwrap();
    assert_eq!(a.position_m, p[0].position_m);
    assert_eq!(b.position_m, p[3].position_m);
    assert_eq!(a.first, (p[1].position_m - p[0].position_m) * 3.);
    assert_eq!(b.first, (p[3].position_m - p[2].position_m) * 3.);
    assert_eq!(
        a.second,
        (p[2].position_m - p[1].position_m * 2. + p[0].position_m) * 6.
    );
}
#[test]
fn analytic_derivatives_match_independent_central_differences() {
    for curve in families() {
        let p = curve.prepare().unwrap();
        let (lo, hi) = p.domain();
        for f in [0.13, 0.27, 0.61, 0.83] {
            let u = lo + (hi - lo) * f;
            let h = (hi - lo) * 1e-5;
            let a = p.evaluate(u - h, Side::Right).unwrap();
            let b = p.evaluate(u + h, Side::Right).unwrap();
            let e = p.evaluate(u, Side::Right).unwrap();
            assert!(
                ((b.position_m - a.position_m) / (2. * h) - e.first).length() < 2e-6,
                "{}",
                curve.method()
            );
            assert!(
                ((b.first - a.first) / (2. * h) - e.second).length() < 2e-5,
                "{}",
                curve.method()
            );
        }
    }
}
// Cox-de Boor basis recursion is independent of the production de Boor evaluator.
fn basis(i: usize, p: usize, u: f64, k: &[f64]) -> f64 {
    if p == 0 {
        return if k[i] <= u && u < k[i + 1] { 1. } else { 0. };
    }
    let a = if k[i + p] > k[i] {
        (u - k[i]) / (k[i + p] - k[i]) * basis(i, p - 1, u, k)
    } else {
        0.
    };
    let b = if k[i + p + 1] > k[i + 1] {
        (k[i + p + 1] - u) / (k[i + p + 1] - k[i + 1]) * basis(i + 1, p - 1, u, k)
    } else {
        0.
    };
    a + b
}
#[test]
fn bspline_nonuniform_and_uniform_basis_reference() {
    for knots in [
        vec![0., 0., 0., 0.2, 0.7, 1., 1., 1.],
        vec![0., 1., 2., 3., 4., 5., 6., 7.],
    ] {
        let points = points(&[
            [0., 0., 0.],
            [1., 3., 0.],
            [4., -1., 2.],
            [7., 4., 0.],
            [9., 0., 1.],
        ]);
        let c = Curve::BSpline {
            degree: 2,
            points: points.clone(),
            knots: knots.clone(),
        };
        let prepared = c.prepare().unwrap();
        let (lo, hi) = prepared.domain();
        for j in 1..100 {
            let u = lo + (hi - lo) * j as f64 / 100.;
            let expected = points.iter().enumerate().fold(Vec3::ZERO, |v, (i, p)| {
                v + p.position_m * basis(i, 2, u, &knots)
            });
            assert!((expected - c.evaluate(u).unwrap().position_m).length() < 1e-12);
            assert!(
                ((0..points.len())
                    .map(|i| basis(i, 2, u, &knots))
                    .sum::<f64>()
                    - 1.)
                    .abs()
                    < 1e-12
            );
        }
        prepared.evaluate(lo, Side::Right).unwrap();
        prepared.evaluate(hi, Side::Left).unwrap();
    }
}
#[test]
fn repeated_knots_have_explicit_one_sided_derivatives_and_continuity() {
    let c = Curve::BSpline {
        degree: 2,
        points: points(&[
            [0., 0., 0.],
            [1., 0., 0.],
            [2., 0., 0.],
            [2., 1., 0.],
            [2., 2., 0.],
        ]),
        knots: vec![0., 0., 0., 0.5, 0.5, 1., 1., 1.],
    };
    let p = c.prepare().unwrap();
    let a = p.evaluate(0.5, Side::Left).unwrap();
    let b = p.evaluate(0.5, Side::Right).unwrap();
    assert_eq!(a.position_m, b.position_m);
    assert!((a.tangent().unwrap() - Vec3::X).length() < 1e-12);
    assert!((b.tangent().unwrap() - Vec3::Y).length() < 1e-12);
    let join = &p.continuity().unwrap()[0];
    assert!(join.g0(1e-10));
    assert!(!join.g1(1e-10));
}
#[test]
fn rational_circle_has_correct_radius_curvature_length_and_inversion() {
    let p = arc().prepare().unwrap();
    let table = ArcLengthTable::build(&p, ArcLengthOptions::default()).unwrap();
    assert!((table.length_m() - 5. * std::f64::consts::PI).abs() < 1e-8);
    for i in 0..=100 {
        let s = table.length_m() * i as f64 / 100.;
        let u = table.parameter_at_distance(&p, s).unwrap();
        let e = p.evaluate(u, Side::Right).unwrap();
        assert!(((e.position_m - Vec3::new(0., 10., 0.)).length() - 10.).abs() < 1e-12);
        assert!((e.curvature().unwrap() - 0.1).abs() < 1e-12);
        let theta = s / 10.;
        let exact = Vec3::new(10. * theta.sin(), 10. * (1. - theta.cos()), 0.);
        assert!((e.position_m - exact).length() < 3e-8);
        assert!((table.distance_at_parameter(&p, u).unwrap() - s).abs() < 3e-8);
    }
    assert!(
        table
            .stations
            .windows(2)
            .all(|w| w[1].distance_m > w[0].distance_m)
    );
    let mut nonrational = arc();
    if let Curve::Nurbs { weights, .. } = &mut nonrational {
        *weights = vec![1.; 3];
    }
    assert!(
        (nonrational.evaluate(0.5).unwrap().position_m
            - p.evaluate(0.5, Side::Right).unwrap().position_m)
            .length()
            > 0.5
    );
}
#[test]
fn nonuniform_line_arc_inversion_and_near_zero_endpoint_derivative() {
    let c = Curve::Bezier {
        points: points(&[[0., 0., 0.], [0., 0., 0.], [0., 0., 0.], [20., 0., 0.]]),
    };
    let p = c.prepare().unwrap();
    let t = ArcLengthTable::build(&p, ArcLengthOptions::default()).unwrap();
    assert!((t.length_m() - 20.).abs() < 1e-12);
    for s in [1e-6, 0.1, 1., 8., 19.9, 20.] {
        let u = t.parameter_at_distance(&p, s).unwrap();
        assert!((20. * u.powi(3) - s).abs() < 3e-8);
    }
    assert!(p.evaluate(0., Side::Right).unwrap().tangent().is_err());
    assert!(t.parameter_at_distance(&p, -1.).is_err());
}
#[test]
fn knot_insertion_preserves_rational_and_nonrational_geometry() {
    for mut c in [arc(), families().remove(1)] {
        let before = c.clone();
        let old_ids: Vec<_> = c.points().iter().map(|p| p.id).collect();
        c.insert_knot(0.37, 99).unwrap();
        c.insert_knot(0.63, 100).unwrap();
        assert!(
            old_ids
                .iter()
                .all(|id| c.points().iter().any(|p| p.id == *id))
        );
        for i in 0..=100 {
            let u = i as f64 / 100.;
            let a = before.evaluate(u).unwrap();
            let b = c.evaluate(u).unwrap();
            assert!((a.position_m - b.position_m).length() < 1e-12);
            assert!((a.first - b.first).length() < 1e-11);
        }
        let saved = c.clone();
        assert!(c.insert_knot(0.5, 99).is_err());
        assert_eq!(c, saved);
    }
}
#[test]
fn catmull_rom_interpolates_and_parameterizations_remain_distinct() {
    let points = points(&[[0., 0., 0.], [1., 0., 0.], [4., 3., 0.], [19., 1., 0.]]);
    let mut middle = vec![];
    for parameterization in [
        Parameterization::Uniform,
        Parameterization::Chordal,
        Parameterization::Centripetal,
    ] {
        let c = Curve::CatmullRom {
            points: points.clone(),
            parameterization,
        };
        let p = c.prepare().unwrap();
        for (&u, cp) in p.breaks().iter().zip(&points) {
            assert!(
                (p.evaluate(u, Side::Right).unwrap().position_m - cp.position_m).length() < 1e-12
            );
        }
        assert!(p.continuity().unwrap().iter().all(|j| j.c1(1e-10)));
        middle.push(
            p.evaluate((p.breaks()[1] + p.breaks()[2]) * 0.5, Side::Right)
                .unwrap()
                .position_m,
        );
    }
    assert!((middle[0] - middle[1]).length() > 0.01);
    assert!((middle[1] - middle[2]).length() > 0.01);
}
#[test]
fn explicit_alignment_and_bezier_c2_fit_are_mathematically_defined() {
    let mut c = Curve::Bezier {
        points: points(&[
            [3., 4., 0.],
            [3., 5., 0.],
            [3., 6., 0.],
            [3., 7., 0.],
            [4., 8., 0.],
            [5., 8., 0.],
            [6., 9., 0.],
        ]),
    };
    CurveEdit::FitBezierJoin {
        endpoint_id: 4,
        continuity: BezierContinuity::C2,
    }
    .apply(&mut c)
    .unwrap();
    assert!(c.prepare().unwrap().continuity().unwrap()[0].c2(1e-12));
    let old = c.clone();
    c.align_start().unwrap();
    assert!(c.evaluate(0.).unwrap().position_m.length() < 1e-12);
    assert!((c.evaluate(0.).unwrap().tangent().unwrap() - Vec3::X).length() < 1e-12);
    assert!(
        (c.points()[6].position_m.length()
            - (old.points()[6].position_m - old.points()[0].position_m).length())
        .abs()
            < 1e-12
    );
}
#[test]
fn invalid_knots_weights_ids_and_degeneracy_are_rejected() {
    for mut c in families() {
        c.points_mut()[0].position_m.x = f64::NAN;
        assert!(c.validate().is_err());
    }
    let mut c = arc();
    if let Curve::Nurbs { weights, .. } = &mut c {
        weights[1] = 0.;
    }
    assert!(c.validate().is_err());
    let mut c = arc();
    if let Curve::Nurbs { knots, .. } = &mut c {
        knots[3] = -1.;
    }
    assert!(c.validate().is_err());
    let mut c = arc();
    c.points_mut()[1].id = 1;
    assert!(c.validate().is_err());
    let c = Curve::CatmullRom {
        points: points(&[[0., 0., 0.], [0., 0., 0.]]),
        parameterization: Parameterization::default(),
    };
    assert!(c.validate().is_err());
    let c = Curve::Bezier {
        points: points(&[[0., 0., 0.], [0., 0., 0.], [0., 0., 0.], [0., 0., 0.]]),
    };
    assert!(ArcLengthTable::build(&c.prepare().unwrap(), ArcLengthOptions::default()).is_err());
    assert!(arc().evaluate(1.01).is_err());
    assert!(arc().evaluate(f64::NAN).is_err());
}

#[test]
fn quadrature_refinement_and_explicit_budgets() {
    let p = arc().prepare().unwrap();
    let exact = 5. * std::f64::consts::PI;
    let loose = ArcLengthTable::build(
        &p,
        ArcLengthOptions {
            absolute_tolerance_m: 1e-3,
            relative_tolerance: 1e-8,
            ..Default::default()
        },
    )
    .unwrap();
    let fine = ArcLengthTable::build(
        &p,
        ArcLengthOptions {
            absolute_tolerance_m: 1e-11,
            relative_tolerance: 1e-13,
            ..Default::default()
        },
    )
    .unwrap();
    assert!((fine.length_m() - exact).abs() < (loose.length_m() - exact).abs());
    assert!((fine.length_m() - exact).abs() < 1e-10);
    assert!(
        ArcLengthTable::build(
            &p,
            ArcLengthOptions {
                absolute_tolerance_m: 1e-12,
                relative_tolerance: 1e-14,
                max_leaves: 4,
                max_depth: 4
            }
        )
        .is_err()
    );
}
