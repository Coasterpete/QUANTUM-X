//! Transactional region operations shared by pointer and precision controls.
use crate::{Result, require};
use qx_geometry::{
    authoring::{ChannelKey, Section},
    profile::Continuity,
    transition::{Function, Transition},
};

fn order(rule: Continuity) -> usize {
    match rule {
        Continuity::C2 => 2,
        Continuity::C1 => 1,
        _ => 0,
    }
}

/// Split at a section station (metres, seconds, or normalized distance), never
/// at a control-net abscissa. Exact joints resolve to the left region, as in the
/// reference editor; splitting that joint again is an error rather than a sliver.
pub fn split_at(section: &Section, key: ChannelKey, station: f64) -> Result<(Section, usize)> {
    require(station.is_finite(), "split station must be finite")?;
    let mut source = section.clone();
    let c = source.channel_mut(key)?;
    let mut start = 0.;
    for (index, t) in c.transitions.iter().enumerate() {
        if (start..=start + t.length).contains(&station) {
            let next =
                crate::editing::split_transition(section, key, t.id, (station - start) / t.length)?;
            return Ok((next, index + 1));
        }
        start += t.length;
    }
    Err(crate::DocumentError(
        "split station is outside this channel".into(),
    ))
}

/// Routine local shape editing protects both endpoint jets. The same refined
/// net drives numeric and graphical controls; viewing it never changes a draft.
pub fn shape_control(
    section: &Section,
    key: ChannelKey,
    index: usize,
    control: usize,
    value: f64,
) -> Result<Section> {
    let mut source = section.clone();
    let c = source.channel_mut(key)?;
    require(index < c.transitions.len(), "region index out of range")?;
    let mut t = c.transitions[index].to_nurbs()?;
    let Function::Nurbs { profile } = &mut t.function else {
        return Err(crate::DocumentError(
            "shape controls require a NURBS region".into(),
        ));
    };
    profile.refine_for_edit()?;
    require(
        control >= 3 && control < profile.values.len().saturating_sub(3),
        "shape control must preserve both endpoint jets",
    )?;
    if profile.values[control] == value {
        return Ok(section.clone());
    }
    profile.values[control] = value;
    replace(section, key, index, t)
}

/// Explicit opt-in repair of an already-authored discontinuity. Keep the selected
/// start jet and both outer jets; reshape only these two named participants.
/// Legacy cosine changes representation only under this explicit reshape command.
pub fn join_incoming(section: &Section, key: ChannelKey, index: usize) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(
        index > 0 && index < c.transitions.len(),
        "select a region with an incoming neighbor",
    )?;
    let shared = c.transitions[index].evaluate(0.)?;
    for i in [index - 1, index] {
        let source = &c.transitions[i];
        let a = source.evaluate(0.)?;
        let b = source.evaluate(source.length)?;
        let incoming = source.incoming_continuity();
        let mut t = match source.to_nurbs() {
            Ok(t) => t,
            Err(_) => {
                let mut t = Transition::nurbs(source.id, source.length, source.start, source.end);
                t.set_endpoint_jet(false, a, 2)?;
                t.set_endpoint_jet(true, b, 2)?;
                if let Function::Nurbs { profile } = &mut t.function {
                    profile.incoming = incoming;
                }
                t
            }
        };
        t.set_endpoint_jet(i < index, shared, 2)?;
        if i == index
            && let Function::Nurbs { profile } = &mut t.function
        {
            profile.incoming = Continuity::C2;
        }
        c.transitions[i] = t;
    }
    next.validate()?;
    Ok(next)
}

/// A constrained endpoint is a shared boundary. This explicit command edits both
/// participants; the UI identifies that scope before dragging or numeric editing.
/// Other boundaries and every other channel remain unchanged.
pub fn endpoint(
    section: &Section,
    key: ChannelKey,
    index: usize,
    end: bool,
    value: f64,
) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(index < c.transitions.len(), "region index out of range")?;
    let t = &c.transitions[index];
    if matches!(t.function, Function::Constant {}) {
        return plateau_level(section, key, index, value);
    }
    if (if end { t.end } else { t.start }) == value {
        next.validate()?;
        return Ok(next);
    }
    let mut jet = t.evaluate(if end { t.length } else { 0. })?;
    jet.value = value;
    if matches!(t.function, Function::Nurbs { .. }) {
        c.transitions[index].set_endpoint_jet(end, jet, 2)?;
    } else {
        let t = &mut c.transitions[index];
        require(
            !matches!(t.function, Function::CosineSpan { .. }),
            "Legacy cosine restriction is exact and read-only; select an explicit cubic reshape to edit it",
        )?;
        if end {
            t.end = value;
        } else {
            t.start = value;
        }
        if matches!(t.function, Function::Constant {}) {
            t.start = value;
            t.end = value;
        }
    }
    let neighbor = if end {
        (index + 1 < c.transitions.len()).then_some(index + 1)
    } else {
        index.checked_sub(1)
    };
    if let Some(j) = neighbor {
        let rule = c.transitions[if end { j } else { index }].incoming_continuity();
        if rule != Continuity::Free {
            let length = c.transitions[j].length;
            let mut other = c.transitions[j].evaluate(if end { 0. } else { length })?;
            other.value = value;
            if order(rule) >= 1 {
                other.derivative = jet.derivative;
            }
            if order(rule) >= 2 {
                other.second_derivative = jet.second_derivative;
            }
            c.transitions[j].set_endpoint_jet(!end, other, 2)?;
        }
    }
    next.validate()?;
    Ok(next)
}

/// Move the shared station, maintaining the two outer endpoint jets and the
/// shared physical-domain jet. No downstream stations or other channels move.
pub fn boundary(section: &Section, key: ChannelKey, index: usize, length: f64) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(
        index + 1 < c.transitions.len(),
        "only internal boundaries can move",
    )?;
    if c.transitions[index].length == length {
        return Ok(section.clone());
    }
    let total = c.transitions[index].length + c.transitions[index + 1].length;
    require(
        length.is_finite() && length >= 1e-8 && total - length >= 1e-8,
        "boundary must leave both regions at least 1e-8 domain units",
    )?;
    for (i, new_length) in [(index, length), (index + 1, total - length)] {
        let t = &mut c.transitions[i];
        let plateau = is_plateau(t);
        let a = t.evaluate(0.)?;
        let b = t.evaluate(t.length)?;
        t.length = new_length;
        if !plateau && matches!(t.function, Function::Nurbs { .. }) {
            t.set_endpoint_jet(false, a, 2)?;
            t.set_endpoint_jet(true, b, 2)?;
        }
    }
    next.validate()?;
    Ok(next)
}

/// Exact concatenation retains every span, with no endpoint fitting or smoothing.
pub fn merge(section: &Section, key: ChannelKey, index: usize) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(
        index + 1 < c.transitions.len(),
        "select a region with a following neighbor",
    )?;
    let a = c.transitions[index].to_nurbs()?;
    let b = c.transitions[index + 1].to_nurbs()?;
    let (Function::Nurbs { profile: mut p }, Function::Nurbs { profile: q }) =
        (a.function, b.function)
    else {
        unreachable!()
    };
    require(
        p.degree == q.degree,
        "exact merge currently requires equal NURBS degrees",
    )?;
    require(
        a.end == b.start,
        "cannot merge a value step; retain the explicit boundary or edit its shared value first",
    )?;
    require(
        p.values.len() + q.values.len() - 1 <= 256,
        "merged profile exceeds 256 controls",
    )?;
    let length = a.length + b.length;
    let rank = |c: Continuity| match c {
        Continuity::Free => 0,
        Continuity::C0 => 1,
        Continuity::C1 => 2,
        Continuity::C2 => 3,
    };
    p.interior = [p.interior, q.interior, q.incoming]
        .into_iter()
        .min_by_key(|c| rank(*c))
        .unwrap();
    let fraction = a.length / length;
    let weight_scale = p.weights.last().unwrap() / q.weights[0];
    for k in &mut p.knots {
        *k *= fraction;
    }
    p.knots.pop();
    p.knots.extend(
        q.knots[p.degree + 1..]
            .iter()
            .map(|k| fraction + (1. - fraction) * k),
    );
    p.values.extend_from_slice(&q.values[1..]);
    p.weights
        .extend(q.weights[1..].iter().map(|w| w * weight_scale));
    let merged = Transition::from_profile(a.id, length, p);
    merged.validate()?;
    c.transitions.splice(index..=index + 1, [merged]);
    next.validate()?;
    Ok(next)
}

/// Relaxing a join never changes its curve. Tightening it validates the existing
/// data and fails with numeric gaps rather than silently fitting adjacent regions.
pub fn continuity(
    section: &Section,
    key: ChannelKey,
    index: usize,
    rule: Continuity,
) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(
        index > 0 && index < c.transitions.len(),
        "select a region with an incoming boundary",
    )?;
    let t = c.transitions[index].to_nurbs()?;
    c.transitions[index] = t;
    if let Function::Nurbs { profile } = &mut c.transitions[index].function {
        profile.incoming = rule;
    }
    next.validate()?;
    Ok(next)
}

/// Control/weight edits affect only the selected profile. Constrained joins are
/// checked before publication; violating edits retain the previous draft.
pub fn replace(
    section: &Section,
    key: ChannelKey,
    index: usize,
    mut t: Transition,
) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(
        index < c.transitions.len() && c.transitions[index].id == t.id,
        "replacement must retain region ID",
    )?;
    let rule = if index > 0 {
        c.transitions[index].incoming_continuity()
    } else {
        Continuity::Free
    };
    if rule != Continuity::Free && t.incoming_continuity() != rule {
        t = t.to_nurbs()?;
        if let Function::Nurbs { profile } = &mut t.function {
            profile.incoming = rule;
        }
    }
    c.transitions[index] = t;
    next.validate()?;
    Ok(next)
}

/// Reordering retains the continuity rule at the affected boundary. A split's
/// C2 contract cannot disappear merely because its left region moves right.
pub fn reorder(section: &Section, key: ChannelKey, index: usize, other: usize) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(
        index < c.transitions.len() && other < c.transitions.len(),
        "region reorder index out of range",
    )?;
    let rules: Vec<_> = c
        .transitions
        .iter()
        .map(Transition::incoming_continuity)
        .collect();
    c.transitions.swap(index, other);
    for (i, t) in c.transitions.iter_mut().enumerate().skip(1) {
        if rules[i] != Continuity::Free && !matches!(t.function, Function::Nurbs { .. }) {
            *t = t.to_nurbs()?;
        }
        if let Function::Nurbs { profile } = &mut t.function {
            profile.incoming = rules[i];
        }
    }
    next.validate()?;
    Ok(next)
}

/// Exact constant detection, independent of screen sampling or zoom.
pub fn is_plateau(t: &Transition) -> bool {
    if t.start != t.end || t.amplitude != 0. {
        return false;
    }
    match &t.function {
        Function::Nurbs { profile } => profile.values.iter().all(|v| *v == t.start),
        Function::Hermite {
            start_slope,
            end_slope,
        } => *start_slope == 0. && *end_slope == 0.,
        Function::CosineSpan { .. } => false,
        _ => true,
    }
}

/// Propagate only explicitly constrained shared jets; preserve the neighbor's
/// opposite jet and never fit an unconstrained legacy step automatically.
fn share_jet(c: &mut qx_geometry::transition::Channel, index: usize, end: bool) -> Result<()> {
    let neighbor = if end {
        (index + 1 < c.transitions.len()).then_some(index + 1)
    } else {
        index.checked_sub(1)
    };
    if let Some(j) = neighbor {
        let rule = c.transitions[if end { j } else { index }].incoming_continuity();
        if rule != Continuity::Free {
            let t = &c.transitions[index];
            let jet = t.evaluate(if end { t.length } else { 0. })?;
            let other = &mut c.transitions[j];
            let mut target = other.evaluate(if end { 0. } else { other.length })?;
            let outer = other.evaluate(if end { other.length } else { 0. })?;
            let old_value = target.value;
            target.value = jet.value;
            if order(rule) >= 1 {
                target.derivative = jet.derivative;
            }
            if order(rule) >= 2 {
                target.second_derivative = jet.second_derivative;
            }
            // Retain normalized easing when lifting a plateau. Merely solving
            // the last three controls makes a newly flat entrance stay flat
            // for most of its length. A flat neighbor becomes a full quintic
            // ramp; an existing ramp keeps its relative interior shape.
            let mut ramp = if is_plateau(other) {
                Transition {
                    id: other.id,
                    length: other.length,
                    start: if end { target.value } else { outer.value },
                    end: if end { outer.value } else { target.value },
                    amplitude: 0.,
                    function: Function::QuinticSmoothstep {},
                }
                .to_nurbs()?
            } else {
                let mut ramp = other.to_nurbs()?;
                if old_value != outer.value {
                    let scale = (target.value - outer.value) / (old_value - outer.value);
                    ramp.scale_values(scale);
                    let offset = outer.value * (1. - scale);
                    ramp.start += offset;
                    ramp.end += offset;
                    if let Function::Nurbs { profile } = &mut ramp.function {
                        for v in &mut profile.values {
                            *v += offset;
                        }
                    }
                }
                ramp
            };
            if let Function::Nurbs { profile } = &mut ramp.function {
                profile.incoming = other.incoming_continuity();
            }
            ramp.set_endpoint_jet(end, outer, 2)?;
            ramp.set_endpoint_jet(!end, target, 2)?;
            *other = ramp;
        }
    }
    Ok(())
}

/// Change a hold's level without changing its length or flattening either ramp.
pub fn plateau_level(
    section: &Section,
    key: ChannelKey,
    index: usize,
    value: f64,
) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(index < c.transitions.len(), "transition index out of range")?;
    let t = &mut c.transitions[index];
    require(is_plateau(t), "select a constant plateau")?;
    if value == t.start {
        return Ok(section.clone());
    }
    require(value.is_finite(), "plateau level must be finite")?;
    if let Function::Nurbs { profile } = &mut t.function {
        profile.values.fill(value);
    }
    t.start = value;
    t.end = value;
    share_jet(c, index, false)?;
    share_jet(c, index, true)?;
    next.validate()?;
    Ok(next)
}

/// Move an authored endpoint in both axes atomically. Outer stations are fixed;
/// internal movement transfers extent between neighbors, retaining total length.
pub fn point(
    section: &Section,
    key: ChannelKey,
    index: usize,
    end: bool,
    station: f64,
    value: f64,
) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(index < c.transitions.len(), "transition index out of range")?;
    require(station.is_finite(), "point station must be finite")?;
    let boundary_index = if end {
        (index + 1 < c.transitions.len()).then_some(index)
    } else {
        index.checked_sub(1)
    };
    let current: f64 = c.transitions[..index + usize::from(end)]
        .iter()
        .map(|t| t.length)
        .sum();
    if let Some(i) = boundary_index {
        let start: f64 = c.transitions[..i].iter().map(|t| t.length).sum();
        next = boundary(section, key, i, station - start)?;
    } else {
        require(
            (station - current).abs() <= 1e-10,
            "outer endpoint station is fixed; edit the Region length",
        )?;
    }
    let c = next.channel_mut(key)?;
    let neighbor = if end {
        (index + 1 < c.transitions.len()).then_some(index + 1)
    } else {
        index.checked_sub(1)
    };
    // Graph seam points preserve a connected interior hold. Ordinary numeric
    // Start/End fields keep their existing independent endpoint semantics;
    // explicit plateau bars/precision fields always edit the entire level.
    if let Some(hold) = [Some(index), neighbor].into_iter().flatten().find(|i| {
        *i > 0
            && *i + 1 < c.transitions.len()
            && is_plateau(&c.transitions[*i])
            && c.transitions[*i].incoming_continuity() != Continuity::Free
            && c.transitions[*i + 1].incoming_continuity() != Continuity::Free
    }) {
        return plateau_level(&next, key, hold, value);
    }
    endpoint(&next, key, index, end, value)
}

/// Move a scalar shape handle's Greville station and ordinate. A monotone
/// piecewise affine warp redistributes the existing interior knots, retaining
/// multiplicities and continuity. This is an authored timing edit, not a 2D
/// positional spline. Both physical endpoint jets are restored afterwards.
pub fn shape_point(
    section: &Section,
    key: ChannelKey,
    index: usize,
    control: usize,
    station: f64,
    value: f64,
) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(index < c.transitions.len(), "transition index out of range")?;
    let start: f64 = c.transitions[..index].iter().map(|t| t.length).sum();
    let mut t = c.transitions[index].to_nurbs()?;
    let a = t.evaluate(0.)?;
    let b = t.evaluate(t.length)?;
    let Function::Nurbs { profile } = &mut t.function else {
        unreachable!()
    };
    profile.refine_for_edit()?;
    require(
        control >= 3 && control < profile.values.len().saturating_sub(3),
        "shape handle must preserve endpoint jets",
    )?;
    let u = profile.knots[control + 1..=control + profile.degree]
        .iter()
        .sum::<f64>()
        / profile.degree as f64;
    let target = (station - start) / t.length;
    require(
        target.is_finite() && target > 0. && target < 1.,
        "easing handle must stay inside the transition",
    )?;
    let timing_changed = (target - u).abs() > 8. * f64::EPSILON;
    if !timing_changed && profile.values[control] == value {
        return Ok(section.clone());
    }
    if timing_changed {
        let basis = |k: f64| if k <= u { k / u } else { (1. - k) / (1. - u) };
        let gain = profile.knots[control + 1..=control + profile.degree]
            .iter()
            .map(|k| basis(*k))
            .sum::<f64>()
            / profile.degree as f64;
        require(gain > 0., "this shape handle has no movable timing knots")?;
        let shift = (target - u) / gain;
        require(
            shift > -u + 1e-6 && shift < 1. - u - 1e-6,
            "easing handle would collapse a knot span; move it closer",
        )?;
        for k in &mut profile.knots {
            *k += shift * basis(*k);
        }
    }
    profile.values[control] = value;
    t.set_endpoint_jet(false, a, 2)?;
    t.set_endpoint_jet(true, b, 2)?;
    c.transitions[index] = t;
    next.validate()?;
    Ok(next)
}

/// Explicit user-invoked reshape into entrance / constant hold / exit. Retains
/// the selected transition's outer jets, ID and total extent; allocates only the
/// two new transition IDs. The middle level starts at the authored midpoint.
pub fn ramp_plateau(section: &Section, key: ChannelKey, index: usize) -> Result<Section> {
    let mut next = section.clone();
    let c = next.channel_mut(key)?;
    require(index < c.transitions.len(), "transition index out of range")?;
    let t = &c.transitions[index];
    let fresh = c
        .transitions
        .iter()
        .map(|t| t.id)
        .max()
        .unwrap()
        .checked_add(1)
        .ok_or_else(|| crate::DocumentError("transition IDs exhausted".into()))?;
    let last = fresh
        .checked_add(1)
        .ok_or_else(|| crate::DocumentError("transition IDs exhausted".into()))?;
    let level = t.evaluate(t.length * 0.5)?.value;
    let length = t.length / 3.;
    let ramp = |id, length, start, end, incoming| -> Result<Transition> {
        let mut ramp = Transition {
            id,
            length,
            start,
            end,
            amplitude: 0.,
            function: Function::QuinticSmoothstep {},
        }
        .to_nurbs()?;
        if let Function::Nurbs { profile } = &mut ramp.function {
            profile.incoming = incoming;
        }
        Ok(ramp)
    };
    let mut entrance = ramp(t.id, length, t.start, level, t.incoming_continuity())?;
    entrance.set_endpoint_jet(false, t.evaluate(0.)?, 2)?;
    let hold = ramp(fresh, length, level, level, Continuity::C2)?;
    let mut exit = ramp(last, t.length - 2. * length, level, t.end, Continuity::C2)?;
    exit.set_endpoint_jet(true, t.evaluate(t.length)?, 2)?;
    c.transitions.splice(index..=index, [entrance, hold, exit]);
    next.validate()?;
    Ok(next)
}
