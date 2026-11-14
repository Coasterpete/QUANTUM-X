//! Geometry shared by stepping and the opt-in velocity initializer. No impulses,
//! force laws, stabilization, or integration belong here.
use crate::{Body, Result, TrackAttitude, angular::roll_constraint};
use qx_geometry::math::{Frame, Vec3};

pub(crate) const BALL_AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];

pub(crate) fn line_normals(direction: Vec3) -> Result<[Vec3; 2]> {
    let axis = if direction.y.abs() < 0.9 {
        Vec3::Y
    } else {
        Vec3::X
    };
    let n1 = direction
        .cross(axis)
        .normalized()
        .map_err(|e| e.to_string())?;
    Ok([n1, direction.cross(n1)])
}

pub(crate) fn track_normals(frame: Frame) -> [Vec3; 2] {
    [frame.normal(), frame.lateral()]
}

pub(crate) struct RailTwist {
    pub arm: Vec3,
    pub axis: Vec3,
    pub error: f64,
    pub linear: Vec3,
    pub angular: Vec3,
}

pub(crate) fn rail_twist(
    b: &Body,
    a: &TrackAttitude,
    target: Frame,
    target_position: Vec3,
    derivative: Vec3,
) -> Result<RailTwist> {
    let rail = Frame::new(b.pose.orientation.compose(a.local_frame)).map_err(|e| e.to_string())?;
    let (axis, error) = roll_constraint(target, rail)?;
    let arm = b.frame().local_to_world(a.local_point_m);
    let delta = b.pose.position_m + arm - target_position;
    let projection = 1. - delta.dot(derivative.cross(target.tangent()));
    if !projection.is_finite() || projection <= 1e-6 {
        return Err("rail attitude station projection is singular".into());
    }
    let linear = target.tangent() * (-axis.dot(derivative) / projection);
    Ok(RailTwist {
        arm,
        axis,
        error,
        linear,
        angular: axis + arm.cross(linear),
    })
}
