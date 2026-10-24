//! UI-independent authoring operations. Callers publish returned sections through
//! ReplaceSection, so one explicit operation is one undo transaction.
use crate::{Result, require};
use qx_geometry::{
    authoring::{ChannelKey, Domain, Section},
    transition::{Function, Transition},
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
                t.start /= ratio;
                t.end /= ratio;
                t.amplitude /= ratio;
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
    }
    result.validate()?;
    Ok(result)
}
/// Explicit insertion/reshape: split into two endpoint/slope-matched Hermite
/// transitions. Exact for cubic polynomials with zero amplitude; other functions
/// are deliberately reauthored, never advertised as shape-preserving.
pub fn split_hermite(
    section: &Section,
    key: ChannelKey,
    id: u64,
    fraction: f64,
) -> Result<Section> {
    require(
        fraction.is_finite() && (0.001..=0.999).contains(&fraction),
        "split fraction must be in [0.001,0.999]",
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
    let x = t.length * fraction;
    let a = t.evaluate(0.)?;
    let m = t.evaluate(x)?;
    let b = t.evaluate(t.length)?;
    let first = Transition {
        id: t.id,
        length: x,
        start: a.value,
        end: m.value,
        amplitude: 0.,
        function: Function::Hermite {
            start_slope: a.derivative,
            end_slope: m.derivative,
        },
    };
    let second = Transition {
        id: new_id,
        length: t.length - x,
        start: m.value,
        end: b.value,
        amplitude: 0.,
        function: Function::Hermite {
            start_slope: m.derivative,
            end_slope: b.derivative,
        },
    };
    channel.transitions.splice(i..=i, [first, second]);
    result.validate()?;
    Ok(result)
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
