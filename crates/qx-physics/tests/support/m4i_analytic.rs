//! Closed-form references from authored constants only. No constraint, projection,
//! impulse, Jacobian, sampled-track or source-evaluation calls belong here.
use qx_geometry::math::Vec3;
pub const R: f64 = 20.;
pub const THETA: f64 = 0.1;
pub const BANK: f64 = 0.6;
pub const S0: f64 = 5.;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    Incline,
    Circle,
    Banked,
}
impl Case {
    pub fn name(self) -> &'static str {
        match self {
            Self::Incline => "incline",
            Self::Circle => "circle",
            Self::Banked => "banked",
        }
    }
    pub fn speed(self) -> f64 {
        if self == Self::Incline { 5. } else { 15. }
    }
    pub fn frame(self, s: f64) -> (Vec3, Vec3, Vec3) {
        if self == Self::Incline {
            let t = Vec3::new(THETA.cos(), -THETA.sin(), 0.);
            (t * s, t, Vec3::new(THETA.sin(), THETA.cos(), 0.))
        } else {
            let a = s / R;
            let inward = Vec3::new(-a.sin(), 0., a.cos());
            let bank = if self == Self::Banked { BANK } else { 0. };
            (
                Vec3::new(R * a.sin(), 0., R * (1. - a.cos())),
                Vec3::new(a.cos(), 0., a.sin()),
                Vec3::Y * bank.cos() + inward * bank.sin(),
            )
        }
    }
    pub fn station(self, p: Vec3) -> f64 {
        if self == Self::Incline {
            p.dot(self.frame(0.).1)
        }
        // These reference arcs occupy [0, 110] m, less than one revolution.
        // atan2's principal branch alone would jump after pi*R at six seconds.
        else {
            R * p.x.atan2(R - p.z).rem_euclid(std::f64::consts::TAU)
        }
    }
    pub fn radius(self, half_chord: f64) -> f64 {
        if self == Self::Incline {
            f64::INFINITY
        } else {
            (R * R - half_chord * half_chord).sqrt()
        }
    }
    pub fn position(self, s: f64, radius: f64) -> Vec3 {
        let p = self.frame(s).0;
        if self == Self::Incline {
            p
        } else {
            let c = Vec3::new(0., 0., R);
            c + (p - c) * (radius / R)
        }
    }
    pub fn speed_at(self, s: f64) -> f64 {
        if self == Self::Incline {
            (25. + 2. * 9.81 * THETA.sin() * (s - S0)).sqrt()
        } else {
            self.speed()
        }
    }
    pub fn station_at(self, time: f64, radius: f64) -> f64 {
        if self == Self::Incline {
            S0 + 5. * time + 0.5 * 9.81 * THETA.sin() * time * time
        } else {
            S0 + R * self.speed() * time / radius
        }
    }
    pub fn omega(self, speed: f64, radius: f64) -> Vec3 {
        if self == Self::Incline {
            Vec3::ZERO
        } else {
            Vec3::new(0., -speed / radius, 0.)
        }
    }
    pub fn reaction(self, s: f64, speed: f64, radius: f64) -> Vec3 {
        if self == Self::Incline {
            (self.frame(s).1 * (9.81 * THETA.sin()) + Vec3::Y * 9.81) * 100.
        } else {
            (Vec3::new(-(s / R).sin(), 0., (s / R).cos()) * (speed * speed / radius)
                + Vec3::Y * 9.81)
                * 100.
        }
    }
}
pub fn energy(p: Vec3, v: Vec3, w: Vec3) -> f64 {
    50. * v.dot(v) + 10. * w.dot(w) + 981. * p.y
}
