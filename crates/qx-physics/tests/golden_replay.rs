#[path = "support/replay_baseline.rs"]
mod baseline;
use baseline::{Golden, compare_exact, fingerprint, run};

#[test]
fn production_solver_matches_committed_golden_replays() {
    let tolerant = match std::env::var("QX_REPLAY_COMPARISON").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("exact") => false,
        Ok("tolerant") => true,
        _ => panic!("QX_REPLAY_COMPARISON must be exact or tolerant"),
    };
    for bytes in [
        include_str!("fixtures/replay/level.json"),
        include_str!("fixtures/replay/grade.json"),
        include_str!("fixtures/replay/launch.json"),
        include_str!("fixtures/replay/brake_reverse.json"),
        include_str!("fixtures/replay/banked_curve.json"),
        include_str!("fixtures/replay/rotating_seats.json"),
        include_str!("fixtures/replay/circuit_seam.json"),
    ] {
        let golden: Golden = serde_json::from_str(bytes).unwrap();
        assert_eq!(golden.format_version, 1);
        assert_eq!(
            golden.fixed_step_seconds.to_bits(),
            qx_physics::FIXED_STEP_SECONDS.to_bits()
        );
        let actual = run(&golden.input);
        assert_eq!(golden.states.len(), actual.len());
        if golden.input.name == "launch" {
            for s in actual.iter().skip(1) {
                let occupied: usize = s.discrete["device[7].occupied_bodies"].parse().unwrap();
                assert!(
                    occupied > 0 && occupied < golden.input.train.components.len(),
                    "launch fixture must exercise partial occupancy"
                );
                assert_eq!(s.discrete["device[7].force_limited"], "true");
                assert!(
                    s.scalars["device[7].requested_force_n"]
                        > golden.input.operations.devices[0].maximum_force_n
                );
            }
        }
        if golden.input.name == "circuit_seam" {
            assert!(
                actual.last().unwrap().scalars["guide[0].station_m"]
                    < actual[0].scalars["guide[0].station_m"],
                "fixture must cross the circuit seam"
            );
        }
        for (expected, actual) in golden.states.iter().zip(&actual) {
            assert_eq!(
                expected.fingerprint,
                fingerprint(expected),
                "corrupt fixture"
            );
            println!(
                "REPLAY {} tick={} expected={} actual={} exact={}",
                golden.input.name,
                actual.tick,
                expected.fingerprint,
                actual.fingerprint,
                expected.fingerprint == actual.fingerprint
            );
            if tolerant {
                // Diagnostic comparison only: never used by CI. Explicit fixed
                // bounds measure portability; they are not solver tolerances.
                assert_eq!(expected.tick, actual.tick);
                assert_eq!(expected.discrete, actual.discrete);
                assert_eq!(
                    expected.scalars.keys().collect::<Vec<_>>(),
                    actual.scalars.keys().collect::<Vec<_>>()
                );
                let mut max_delta = 0.0_f64;
                let mut worst_key = "none";
                let mut different_bits = 0;
                for (key, a) in &expected.scalars {
                    let b = actual.scalars[key];
                    let delta = (a - b).abs();
                    if delta > max_delta {
                        max_delta = delta;
                        worst_key = key;
                    }
                    different_bits += usize::from(a.to_bits() != b.to_bits());
                    let absolute = if key.contains("force_n") || key.contains("torque_") {
                        1e-6
                    } else {
                        1e-10
                    };
                    assert!(
                        delta <= absolute + 1e-10 * a.abs().max(b.abs()),
                        "{} tick={} {key}: delta={delta:e}",
                        golden.input.name,
                        actual.tick
                    );
                }
                println!(
                    "TOLERANT_ONLY {} tick={} differing_scalars={different_bits} max_absolute_delta={max_delta:e} field={worst_key}",
                    golden.input.name, actual.tick
                );
            } else {
                compare_exact(expected, actual);
            }
        }
    }
}

#[test]
#[should_panic(expected = "body[0].position_m.x")]
fn exact_comparator_rejects_one_ulp_even_when_decimal_values_are_close() {
    let expected: Golden =
        serde_json::from_str(include_str!("fixtures/replay/level.json")).unwrap();
    let expected = &expected.states[0];
    let mut changed = expected.clone();
    let x = changed.scalars.get_mut("body[0].position_m.x").unwrap();
    *x = f64::from_bits(x.to_bits() + 1);
    changed.fingerprint = fingerprint(&changed);
    compare_exact(expected, &changed);
}

#[test]
#[should_panic(expected = "discrete state")]
fn exact_comparator_rejects_component_identity_changes() {
    let expected: Golden =
        serde_json::from_str(include_str!("fixtures/replay/level.json")).unwrap();
    let expected = &expected.states[0];
    let mut changed = expected.clone();
    changed
        .discrete
        .insert("body[0].id".into(), "unexpected-body".into());
    changed.fingerprint = fingerprint(&changed);
    compare_exact(expected, &changed);
}
