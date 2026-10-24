//! Representation-aware edits; document transactions validate the final topology.
use crate::{
    Result,
    curve::{ControlPoint, Curve, Parameterization},
    ensure,
    math::Vec3,
};

#[derive(Clone, Copy, Debug)]
pub enum BezierContinuity {
    G1,
    C1,
    C2,
}
#[derive(Clone, Debug)]
pub enum CurveEdit {
    MovePoint {
        id: u64,
        position_m: Vec3,
    },
    InsertPoint {
        after: Option<u64>,
        point: ControlPoint,
        weight: f64,
    },
    RemovePoint {
        id: u64,
    },
    SetWeight {
        id: u64,
        weight: f64,
    },
    SetKnots(Vec<f64>),
    /// Reinterpret the controls with an explicitly supplied degree/knot vector.
    /// This is not shape-preserving degree elevation.
    SetDegree {
        degree: usize,
        knots: Vec<f64>,
    },
    SetParameterization(Parameterization),
    InsertKnot {
        parameter: f64,
        new_id: u64,
    },
    ConvertToNurbs,
    AlignStart,
    FitBezierJoin {
        endpoint_id: u64,
        continuity: BezierContinuity,
    },
    Replace(Curve),
}
impl CurveEdit {
    pub fn apply(self, curve: &mut Curve) -> Result<()> {
        let index = |c: &Curve, id: u64| {
            c.points()
                .iter()
                .position(|p| p.id == id)
                .ok_or_else(|| crate::GeometryError(format!("control ID {id} not found")))
        };
        match self {
            Self::MovePoint { id, position_m } => {
                let i = index(curve, id)?;
                curve.points_mut()[i].position_m = position_m;
            }
            Self::InsertPoint {
                after,
                point,
                weight,
            } => {
                let i = after
                    .map(|id| index(curve, id).map(|i| i + 1))
                    .transpose()?
                    .unwrap_or(0);
                if let Curve::Nurbs { weights, .. } = curve {
                    weights.insert(i, weight);
                }
                curve.points_mut().insert(i, point);
            }
            Self::RemovePoint { id } => {
                let i = index(curve, id)?;
                curve.points_mut().remove(i);
                if let Curve::Nurbs { weights, .. } = curve {
                    ensure(i < weights.len(), "weight index missing")?;
                    weights.remove(i);
                }
            }
            Self::SetWeight { id, weight } => {
                let i = index(curve, id)?;
                let Curve::Nurbs { weights, .. } = curve else {
                    return Err(crate::GeometryError("weights apply only to NURBS".into()));
                };
                ensure(i < weights.len(), "weight index missing")?;
                weights[i] = weight;
            }
            Self::SetKnots(new) => match curve {
                Curve::BSpline { knots, .. } | Curve::Nurbs { knots, .. } => *knots = new,
                _ => return Err(crate::GeometryError("knots require B-spline/NURBS".into())),
            },
            Self::SetDegree {
                degree: new,
                knots: new_knots,
            } => match curve {
                Curve::BSpline { degree, knots, .. } | Curve::Nurbs { degree, knots, .. } => {
                    *degree = new;
                    *knots = new_knots;
                }
                _ => {
                    return Err(crate::GeometryError(
                        "degree applies to B-spline/NURBS".into(),
                    ));
                }
            },
            Self::SetParameterization(new) => match curve {
                Curve::CatmullRom {
                    parameterization, ..
                } => *parameterization = new,
                _ => {
                    return Err(crate::GeometryError(
                        "parameterization applies to Catmull-Rom".into(),
                    ));
                }
            },
            Self::InsertKnot { parameter, new_id } => curve.insert_knot(parameter, new_id)?,
            Self::ConvertToNurbs => *curve = curve.to_nurbs()?,
            Self::AlignStart => curve.align_start()?,
            Self::Replace(new) => *curve = new,
            Self::FitBezierJoin {
                endpoint_id,
                continuity,
            } => {
                let i = index(curve, endpoint_id)?;
                let Curve::Bezier { points } = curve else {
                    return Err(crate::GeometryError("join fitting requires Bezier".into()));
                };
                ensure(
                    i >= 3 && i + 3 < points.len() && i.is_multiple_of(3),
                    "Bezier join ID must be an interior shared endpoint",
                )?;
                let p = points[i].position_m;
                let incoming = p - points[i - 1].position_m;
                let outgoing = match continuity {
                    BezierContinuity::G1 => {
                        incoming.normalized()? * (points[i + 1].position_m - p).length()
                    }
                    _ => incoming,
                };
                points[i + 1].position_m = p + outgoing;
                if matches!(continuity, BezierContinuity::C2) {
                    // Equal unit parameter spans: P''left = 6*(P-2*Hleft+Hleft2).
                    points[i + 2].position_m = points[i - 2].position_m
                        - points[i - 1].position_m * 2.0
                        + points[i + 1].position_m * 2.0;
                }
            }
        }
        Ok(())
    }
}
