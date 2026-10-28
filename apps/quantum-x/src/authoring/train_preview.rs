//! Transient development references derived from the same poses used by inspection.
use qx_document::train::{ComponentKind, Train};
use qx_geometry::{
    math::{Frame, Vec3},
    solver::GeneratedTrack,
};
use qx_physics::consist::ConsistInspection;
use qx_rendering::OverlayLine;

pub(super) fn populate(
    train: &Train,
    track: &GeneratedTrack,
    inspection: &ConsistInspection,
    state: &mut super::train_assets::State,
    display: &mut qx_rendering::DisplayOptions,
) {
    display.overlay_lines = lines(train, track, inspection, state);
    let selected = if state
        .selected
        .as_ref()
        .is_some_and(|(id, _)| id != &train.id)
    {
        None
    } else {
        state
            .selected
            .as_ref()
            .filter(|(t, _)| t == &train.id)
            .and_then(|(_, id)| {
                train
                    .cars
                    .iter()
                    .find(|car| car.body_id == *id || car.bogie_ids.contains(id))
            })
            .or_else(|| train.cars.first())
    };
    let mut all = None;
    let mut focused = None;
    for (c, pose) in train.components.iter().zip(&inspection.components) {
        state.errors.remove(&format!("Correction {}", c.id));
        let loaded = c.asset.as_ref().filter(|a| {
            if super::train_assets::renderable(a){true}else{state.errors.insert(format!("Correction {}",c.id),"Visual correction exceeds renderer limits; development box shown. Revise the correction.".into());false}
        }).and_then(|a| {
            state
                .library
                .get(&a.path)
                .map(|e| (a.clone(), e.model.clone()))
        });
        let (correction, model) = loaded.unwrap_or_else(|| {
            let mut a = super::train_assets::identity(String::new());
            a.scale = c.dimensions_m;
            (a, state.fallback.clone())
        });
        if !super::train_assets::renderable(&correction) {
            state.errors.insert(
                format!("Dimensions {}", c.id),
                "Development box dimensions exceed the renderer range; mechanical data retained."
                    .into(),
            );
            continue;
        }
        let instance = super::train_assets::instance(
            model,
            c.id.clone(),
            pose.pose.position_m,
            pose.pose.orientation,
            &correction,
        );
        let bounds = instance.bounds();
        if !bounds.0.is_finite()
            || !bounds.1.is_finite()
            || instance
                .transform
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e30)
        {
            state.errors.insert(
                format!("Transform {}", c.id),
                "Visual transform exceeds rendering range; revise the local correction".into(),
            );
            continue;
        }
        state.errors.remove(&format!("Transform {}", c.id));
        extend_bounds(&mut all, bounds);
        if selected.is_some_and(|car| car.body_id == c.id || car.bogie_ids.contains(&c.id)) {
            extend_bounds(&mut focused, bounds);
        }
        if c.kind != ComponentKind::Bogie || state.bogies {
            display.asset_instances.push(instance);
        }
    }
    display.train_bounds = all;
    display.selected_car_bounds = focused;
}
fn extend_bounds(target: &mut Option<(Vec3, Vec3)>, (a, b): (Vec3, Vec3)) {
    *target = Some(if let Some((min, max)) = *target {
        (
            Vec3::new(min.x.min(a.x), min.y.min(a.y), min.z.min(a.z)),
            Vec3::new(max.x.max(b.x), max.y.max(b.y), max.z.max(b.z)),
        )
    } else {
        (a, b)
    });
}

pub(super) fn lines(
    train: &Train,
    track: &GeneratedTrack,
    inspection: &ConsistInspection,
    visibility: &super::train_assets::State,
) -> Vec<OverlayLine> {
    let mut lines = Vec::new();
    let mut add = |start_m, end_m, color| {
        lines.push(OverlayLine {
            start_m,
            end_m,
            color,
        })
    };
    for (component, evaluated) in train.components.iter().zip(&inspection.components) {
        if component.kind == ComponentKind::Bogie && !visibility.bogies {
            continue;
        }
        // Inspection emits components in definition order. These boxes are only
        // visual references; dimensions never modify the physical mass/inertia.
        let Ok(frame) = Frame::new(evaluated.pose.orientation) else {
            continue;
        };
        let point = |p| evaluated.pose.position_m + frame.local_to_world(p);
        let d = component.dimensions_m * 0.5;
        let corners: Vec<_> = (0..8)
            .map(|i| {
                point(Vec3::new(
                    if i & 1 == 0 { -d.x } else { d.x },
                    if i & 2 == 0 { -d.y } else { d.y },
                    if i & 4 == 0 { -d.z } else { d.z },
                ))
            })
            .collect();
        let color = if component.kind == ComponentKind::CarBody {
            [0.95, 0.75, 0.3]
        } else {
            [0.35, 0.85, 0.95]
        };
        if visibility.wireframe {
            for i in 0..8 {
                for bit in [1, 2, 4] {
                    if i & bit == 0 {
                        add(corners[i], corners[i | bit], color);
                    }
                }
            }
        }
        let axis_length = component
            .dimensions_m
            .x
            .min(component.dimensions_m.y)
            .min(component.dimensions_m.z)
            * 0.75;
        if visibility.wireframe {
            for (axis, color) in [
                (Vec3::X, [1., 0.25, 0.2]),
                (Vec3::Y, [0.25, 1., 0.35]),
                (Vec3::Z, [0.3, 0.55, 1.]),
            ] {
                add(point(Vec3::ZERO), point(axis * axis_length), color);
            }
        }
        // Schema 5 has wheel reference positions, not wheel radii. Crosses are
        // reference markers scaled to the component, not physical wheel meshes.
        let marker = axis_length * 0.25;
        if visibility.wheels {
            for wheel in &component.wheel_references_m {
                for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                    add(
                        point(*wheel - axis * marker),
                        point(*wheel + axis * marker),
                        [1., 0.65, 0.9],
                    );
                }
            }
        }
        if visibility.wireframe
            && let Ok(sample) = track.sample_at_distance(evaluated.station_m)
        {
            for guide in &component.guide_references_m {
                let p = point(*guide);
                add(p, sample.position_m, [1., 0.35, 0.35]);
                for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                    add(
                        point(*guide - axis * marker),
                        point(*guide + axis * marker),
                        [1., 0.35, 0.35],
                    );
                }
            }
        }
    }
    for joint in &inspection.joints {
        let connection = train.connections.iter().find(|c| c.id == joint.id).unwrap();
        let kinds: Vec<_> = [&connection.component_a, &connection.component_b]
            .iter()
            .filter_map(|id| {
                train
                    .components
                    .iter()
                    .find(|c| &c.id == *id)
                    .map(|c| c.kind)
            })
            .collect();
        let coupler = kinds.iter().all(|k| *k == ComponentKind::CarBody);
        if (coupler && !visibility.couplers)
            || (!coupler && (!visibility.joints || !visibility.bogies))
        {
            continue;
        }
        // Distinct anchor crosses and their connecting segment expose residuals,
        // including couplers, without assuming any joint's type from its name.
        add(joint.anchor_a_m, joint.anchor_b_m, [1., 0.3, 0.3]);
        for (p, color) in [
            (joint.anchor_a_m, [1., 0.85, 0.2]),
            (joint.anchor_b_m, [1., 0.35, 0.2]),
        ] {
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                add(p - axis * 0.12, p + axis * 0.12, color);
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use qx_geometry::{
        authoring::{InitialConditions, Section},
        solver::{GenerationOptions, generate},
    };
    #[test]
    fn overlay_follows_component_pose_and_dimensions() {
        let track = generate(
            &InitialConditions::default(),
            &[Section::straight(1, "fixture", 100.)],
            GenerationOptions::default(),
        )
        .unwrap();
        let mut train = Train::development_fixture("fixture".into());
        let first = qx_physics::consist::inspect(&train, &track, 30.).unwrap();
        let visibility = super::super::train_assets::State::default();
        let original = lines(&train, &track, &first, &visibility);
        assert!(original.len() >= 6 * 15 + 5 * 7);
        train.components[0].dimensions_m.x *= 2.;
        let resized = lines(&train, &track, &first, &visibility);
        assert_ne!(original[0], resized[0]);
        let moved = qx_physics::consist::inspect(&train, &track, 40.).unwrap();
        assert_ne!(resized[0], lines(&train, &track, &moved, &visibility)[0]);
    }
}
