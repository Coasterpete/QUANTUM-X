//! UI-independent authoring operations. Callers publish returned sections through
//! ReplaceSection, so one explicit operation is one undo transaction.
use crate::{Result, require};
use qx_geometry::{
    authoring::{ChannelKey, Domain, Section},
    transition::Function,
};

/// Explicitly scale every channel's domain; endpoint slopes scale inversely.
pub fn resize(section: &Section, extent: f64) -> Result<Section> {
    require(
        section.domain != Domain::Normalized,
        "spline extent is normalized; edit controls to change physical length",
    )?;
    require(
        extent.is_finite() && (1e-8..=1e6).contains(&extent),
        "section extent must be in [1e-8,1e6]",
    )?;
    let mut result = section.clone();
    let ratio = extent / result.extent;
    for key in channel_keys(section) {
        for t in &mut result.channel_mut(key)?.transitions {
            t.length *= ratio;
            if let Function::Hermite {
                start_slope,
                end_slope,
            } = &mut t.function
            {
                *start_slope /= ratio;
                *end_slope /= ratio;
            }
        }
    }
    result.extent = extent;
    result.validate()?;
    Ok(result)
}
pub fn channel_keys(section: &Section) -> Vec<ChannelKey> {
    use qx_geometry::authoring::SectionKind;
    let mut keys = vec![ChannelKey::Roll];
    match section.kind {
        SectionKind::Force { .. } => keys.extend([ChannelKey::Normal, ChannelKey::Lateral]),
        SectionKind::Geometric { .. } | SectionKind::RiderLocal { .. } => {
            keys.extend([ChannelKey::Pitch, ChannelKey::Yaw])
        }
        _ => {}
    }
    keys
}
/// Re-express time/distance at a stated constant design speed, preserving shape.
pub fn convert_domain(section: &Section, target: Domain, speed_mps: f64) -> Result<Section> {
    qx_geometry::authoring::validate_speed(speed_mps)?;
    require(
        target != Domain::Normalized && section.domain != Domain::Normalized,
        "normalized spline domains cannot be converted to time/distance",
    )?;
    if target == section.domain {
        return Ok(section.clone());
    }
    let ratio = section.domain.metres_per_input(speed_mps) / target.metres_per_input(speed_mps);
    let mut result = resize(section, section.extent * ratio)?;
    result.domain = target;
    for key in channel_keys(section) {
        let channel = result.channel_mut(key)?;
        if key != ChannelKey::Normal && key != ChannelKey::Lateral {
            channel.unit = target.rate_unit();
            for t in &mut channel.transitions {
                t.scale_values(1. / ratio);
            }
        }
    }
    result.validate()?;
    Ok(result)
}
/// Shape-preserving subdivision. First ID and all other authored regions survive.
pub fn split_transition(
    section: &Section,
    key: ChannelKey,
    id: u64,
    fraction: f64,
) -> Result<Section> {
    require(
        fraction.is_finite() && fraction > 0. && fraction < 1.,
        "split fraction must be inside (0,1)",
    )?;
    let mut result = section.clone();
    let channel = result.channel_mut(key)?;
    let i = channel
        .transitions
        .iter()
        .position(|t| t.id == id)
        .ok_or_else(|| crate::DocumentError("transition ID not found".into()))?;
    let t = channel.transitions[i].clone();
    let new_id = channel
        .transitions
        .iter()
        .map(|t| t.id)
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| crate::DocumentError("transition ID exhausted".into()))?;
    let (first, second) = t.split(fraction, new_id)?;
    channel.transitions.splice(i..=i, [first, second]);
    result.validate()?;
    Ok(result)
}
/// Compatibility API; now performs exact subdivision rather than Hermite fitting.
pub fn split_hermite(
    section: &Section,
    key: ChannelKey,
    id: u64,
    fraction: f64,
) -> Result<Section> {
    split_transition(section, key, id, fraction)
}
/// Remove a transition by extending its explicit neighbor into the vacated
/// interval. This is a visible, reversible reshape; section duration is retained.
pub fn remove_transition(section: &Section, key: ChannelKey, id: u64) -> Result<Section> {
    let mut result = section.clone();
    let channel = result.channel_mut(key)?;
    require(
        channel.transitions.len() > 1,
        "a channel must retain at least one transition",
    )?;
    let i = channel
        .transitions
        .iter()
        .position(|t| t.id == id)
        .ok_or_else(|| crate::DocumentError("transition ID not found".into()))?;
    let removed = channel.transitions.remove(i);
    let neighbor = if i == 0 { 0 } else { i - 1 };
    let t = &mut channel.transitions[neighbor];
    if matches!(t.function, Function::Nurbs { .. }) {
        // Removal is an explicit reshape. Keep the two surviving outer jets,
        // including the removed region's far boundary, so another constrained
        // neighbor is not disconnected by simply stretching the control net.
        let a = if i == 0 {
            removed.evaluate(0.)?
        } else {
            t.evaluate(0.)?
        };
        let b = if i == 0 {
            t.evaluate(t.length)?
        } else {
            removed.evaluate(removed.length)?
        };
        t.length += removed.length;
        t.set_endpoint_jet(false, a, 2)?;
        t.set_endpoint_jet(true, b, 2)?;
        result.validate()?;
        return Ok(result);
    }
    let ratio = (t.length + removed.length) / t.length;
    t.length += removed.length;
    if let Function::Hermite {
        start_slope,
        end_slope,
    } = &mut t.function
    {
        *start_slope /= ratio;
        *end_slope /= ratio;
    }
    result.validate()?;
    Ok(result)
}
