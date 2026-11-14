//! M4H offline experiments; original M4G test binary is unchanged.
//! Opt-in M4F experiment. Every mechanical equation is imported unchanged.
#![cfg(feature = "experimental-source-guidance")]
#[allow(dead_code)]
#[path = "support/m4b_contract.rs"]
mod contract;
#[path = "support/m4h_energy.rs"]
mod energy;
#[allow(dead_code)]
#[path = "support/m4h_dynamics.rs"]
mod energy_dynamics;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/m4e_c_extension.rs"]
mod fixtures;
#[allow(dead_code)]
#[path = "support/m4a_guidance.rs"]
mod guidance;
#[allow(dead_code)]
#[path = "support/m4a_matrix.rs"]
mod matrix;
#[allow(dead_code)]
#[path = "support/m4h_tracking.rs"]
mod measured_tracking;
#[allow(dead_code)]
#[path = "support/m4a_mechanisms.rs"]
mod mechanisms;
#[allow(dead_code)]
#[path = "support/m4b_dynamics.rs"]
mod original_dynamics;
#[allow(dead_code)]
#[path = "../../qx-geometry/tests/support/rail_fixtures.rs"]
mod rail_fixtures;
#[allow(dead_code)]
#[path = "support/m4d_dynamics.rs"]
mod refined_dynamics;
#[allow(dead_code)]
#[path = "support/m4f_surface.rs"]
mod surface;
#[allow(dead_code)]
#[path = "support/m4d_tracking.rs"]
mod tracking;
use contract::{Kind, Model};
use matrix::multiply;
use qx_geometry::{
    authoring::{InitialConditions, Section},
    heartline::AuthoredReference,
    math::Vec3,
    solver::{
        GenerationOptions,
        prepared::{SourceIdentity, dense::ContinuousGraph},
    },
    transition::Side,
};
use surface::Rail;
const ID: SourceIdentity = SourceIdentity {
    document: 0x4fb,
    generation: 1,
};
fn rail(initial: &InitialConditions, sections: &[Section]) -> Rail {
    Rail::new(
        ContinuousGraph::prepare(
            ID,
            AuthoredReference::TrackCenterline,
            initial,
            sections,
            GenerationOptions::default(),
        )
        .unwrap(),
        ID,
    )
    .unwrap()
}
fn model(r: &Rail, lead: f64, shared: bool) -> Model {
    // Reuse only M4B's declared mechanical topology and local dimensions.
    // Seed EVERY body from this source, never from the synthetic path geometry.
    let mut mechanism = mechanisms::Mechanism::new(
        mechanisms::Path::Straight,
        lead,
        false,
        Vec3::new(0.05, 0.12, 0.03),
        shared,
    );
    for p in &mut mechanism.poses {
        let local = Vec3::new(0., p.position_m.y, p.position_m.z);
        let e = r.source.evaluate(ID, p.position_m.x, Side::Right).unwrap();
        p.position_m = e.position_m + e.frame.local_to_world(local);
        p.orientation = e.frame.orientation();
    }
    Model::new(mechanism)
}
fn array(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}
