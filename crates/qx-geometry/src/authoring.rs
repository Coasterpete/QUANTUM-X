use crate::{
    Result, ensure,
    math::{Quaternion, Vec3},
    transition::{Channel, Unit},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Time,
    Distance,
    /// Spline roll input xi = physical arc distance / regenerated curve length.
    Normalized,
}
impl Domain {
    pub fn rate_unit(self) -> Unit {
        match self {
            Self::Time => Unit::RadiansPerSecond,
            Self::Distance => Unit::RadiansPerMetre,
            Self::Normalized => Unit::RadiansPerNormalized,
        }
    }
    pub fn metres_per_input(self, speed: f64) -> f64 {
        match self {
            Self::Time => speed,
            Self::Distance => 1.0,
            Self::Normalized => 1.0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryPolicy {
    /// Inherit the previous pose exactly; curvature/twist/speed jumps warn.
    AllowRateJump,
    /// Reject a change in world curvature, twist, or reference speed.
    RequireContinuousRates,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum SectionKind {
    /// Controls are section-local. Start must be zero with tangent +X.
    Spline {
        curve: crate::curve::Curve,
    },
    Straight {},
    /// Constant rider-frame curvature; plane angle is from normal toward lateral.
    Arc {
        radius_m: f64,
        plane_angle_rad: f64,
    },
    Force {
        normal_g: Channel,
        lateral_g: Channel,
    },
    /// Pitch toward gravity-referenced up; yaw about world -Y.
    Geometric {
        pitch_rate: Channel,
        yaw_rate: Channel,
    },
    /// Pitch toward rider normal; yaw toward rider lateral.
    RiderLocal {
        pitch_rate: Channel,
        yaw_rate: Channel,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub id: SectionId,
    pub name: String,
    pub domain: Domain,
    /// Explicit local boundaries are [0, extent] in the domain's units.
    pub extent: f64,
    pub incoming_boundary: BoundaryPolicy,
    /// None inherits the initial design speed, not a simulated velocity.
    pub reference_speed_mps: Option<f64>,
    pub roll_rate: Channel,
    pub kind: SectionKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelKey {
    Roll,
    Normal,
    Lateral,
    Pitch,
    Yaw,
}
impl Section {
    pub fn spline(id: u64, name: impl Into<String>, curve: crate::curve::Curve) -> Self {
        let mut section = Self::straight(id, name, 1.0);
        section.domain = Domain::Normalized;
        section.roll_rate = Channel::constant(Unit::RadiansPerNormalized, 1.0, 0.0);
        section.kind = SectionKind::Spline { curve };
        section
    }
    pub fn straight(id: u64, name: impl Into<String>, length_m: f64) -> Self {
        Self {
            id: SectionId(id),
            name: name.into(),
            domain: Domain::Distance,
            extent: length_m,
            incoming_boundary: BoundaryPolicy::AllowRateJump,
            reference_speed_mps: None,
            roll_rate: Channel::constant(Unit::RadiansPerMetre, length_m, 0.0),
            kind: SectionKind::Straight {},
        }
    }
    pub fn channels(&self) -> Vec<(&'static str, &Channel)> {
        let mut result = vec![("roll", &self.roll_rate)];
        match &self.kind {
            SectionKind::Force {
                normal_g,
                lateral_g,
            } => result.extend([("normal", normal_g), ("lateral", lateral_g)]),
            SectionKind::Geometric {
                pitch_rate,
                yaw_rate,
            }
            | SectionKind::RiderLocal {
                pitch_rate,
                yaw_rate,
            } => result.extend([("pitch", pitch_rate), ("yaw", yaw_rate)]),
            _ => {}
        }
        result
    }
    pub fn channel_mut(&mut self, key: ChannelKey) -> Result<&mut Channel> {
        match (&mut self.kind, key) {
            (_, ChannelKey::Roll) => Ok(&mut self.roll_rate),
            (SectionKind::Force { normal_g, .. }, ChannelKey::Normal) => Ok(normal_g),
            (SectionKind::Force { lateral_g, .. }, ChannelKey::Lateral) => Ok(lateral_g),
            (
                SectionKind::Geometric { pitch_rate, .. }
                | SectionKind::RiderLocal { pitch_rate, .. },
                ChannelKey::Pitch,
            ) => Ok(pitch_rate),
            (
                SectionKind::Geometric { yaw_rate, .. } | SectionKind::RiderLocal { yaw_rate, .. },
                ChannelKey::Yaw,
            ) => Ok(yaw_rate),
            _ => Err(crate::GeometryError(
                "channel is not applicable to this section type".into(),
            )),
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure(self.id.0 > 0, "section ID must be nonzero")?;
        ensure(
            !self.name.trim().is_empty() && self.name.len() <= 1024,
            "section name must contain 1..1024 bytes",
        )?;
        ensure(
            self.extent.is_finite() && (1e-8..=1e6).contains(&self.extent),
            "section extent must be in [1e-8,1e6]",
        )?;
        if let Some(v) = self.reference_speed_mps {
            validate_speed(v)?;
        }
        if let SectionKind::Spline { curve } = &self.kind {
            ensure(
                self.domain == Domain::Normalized && self.extent == 1.0,
                "spline sections require normalized domain [0,1]",
            )?;
            let prepared = curve.prepare()?;
            let start = prepared.evaluate(prepared.domain().0, crate::transition::Side::Right)?;
            ensure(
                start.position_m.length() <= 1e-8 && (start.tangent()? - Vec3::X).length() <= 1e-8,
                "spline start must be local zero with tangent +X; use explicit AlignStart",
            )?;
            prepared
                .evaluate(prepared.domain().1, crate::transition::Side::Left)?
                .tangent()?;
            for join in prepared.continuity()? {
                ensure(
                    join.g1(1e-8),
                    format!(
                        "spline has a cusp/reversed/discontinuous tangent at parameter {}; track sections require G1",
                        join.parameter
                    ),
                )?;
            }
        } else {
            ensure(
                self.domain != Domain::Normalized,
                "normalized domain is reserved for spline sections",
            )?;
        }
        if let SectionKind::Arc {
            radius_m,
            plane_angle_rad,
        } = self.kind
        {
            ensure(
                radius_m.is_finite()
                    && (1e-4..=1e9).contains(&radius_m)
                    && plane_angle_rad.is_finite(),
                "invalid arc radius or plane angle",
            )?;
        }
        for (name, channel) in self.channels() {
            channel.validate().map_err(|e| {
                crate::GeometryError(format!("section {} {name} channel: {e}", self.id.0))
            })?;
            let expected = if name == "normal" || name == "lateral" {
                Unit::G
            } else {
                self.domain.rate_unit()
            };
            ensure(
                channel.unit == expected,
                format!("section {}: wrong units for {name}", self.id.0),
            )?;
            ensure(
                crate::same_input(channel.length(), self.extent),
                format!(
                    "section {}: {name} channel must cover [0, extent]",
                    self.id.0
                ),
            )?;
        }
        Ok(())
    }
}
pub fn validate_speed(v: f64) -> Result<()> {
    ensure(
        v.is_finite() && (0.1..=500.0).contains(&v),
        "reference speed must be in [0.1,500] m/s",
    )
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialConditions {
    pub position_m: Vec3,
    pub orientation: Quaternion,
    pub reference_speed_mps: f64,
    pub gravity_mps2: f64,
}
impl Default for InitialConditions {
    fn default() -> Self {
        Self {
            position_m: Vec3::ZERO,
            orientation: Quaternion::IDENTITY,
            reference_speed_mps: 20.0,
            gravity_mps2: 9.80665,
        }
    }
}
impl InitialConditions {
    pub fn validate(&self) -> Result<()> {
        ensure(
            self.position_m.is_finite() && self.position_m.length() <= 1e9,
            "invalid initial position (limit 1e9 m)",
        )?;
        self.orientation.normalized()?;
        validate_speed(self.reference_speed_mps)?;
        ensure(
            self.gravity_mps2.is_finite() && (0.01..=100.0).contains(&self.gravity_mps2),
            "gravity must be in [0.01,100] m/s²",
        )
    }
}
pub fn validate_track(initial: &InitialConditions, sections: &[Section]) -> Result<()> {
    initial.validate()?;
    ensure(
        !sections.is_empty() && sections.len() <= 4096,
        "track must contain 1..4096 sections",
    )?;
    let mut ids = std::collections::HashSet::new();
    let mut count = 0;
    let mut controls = 0;
    for s in sections {
        s.validate()?;
        ensure(ids.insert(s.id), "duplicate section ID")?;
        count += s
            .channels()
            .iter()
            .map(|(_, c)| c.transitions.len())
            .sum::<usize>();
        if let SectionKind::Spline { curve } = &s.kind {
            controls += curve.points().len();
        }
    }
    ensure(count <= 65536, "track exceeds 65536 total transitions")?;
    ensure(
        controls <= 65536,
        "track exceeds 65536 total spline controls",
    )
}
