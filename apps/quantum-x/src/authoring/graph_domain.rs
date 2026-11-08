//! Data-domain adapter for the shared Transition Editor. Device profiles are
//! authoritative in the operations draft; the temporary Section is only an
//! adapter for existing constrained region operations and never enters a project.
use super::session::Session;
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionId},
    transition::Unit,
};

impl Session {
    pub fn graph_device(&self) -> Option<u64> {
        self.expansion.curve_device.filter(|id| {
            self.expansion
                .operations
                .devices
                .iter()
                .any(|d| d.id == *id && d.acceleration_curve.is_some())
        })
    }
    pub fn graph_key(&self) -> ChannelKey {
        if self.graph_device().is_some() {
            ChannelKey::Roll
        } else {
            self.channel
        }
    }
    pub fn graph_id(&self) -> SectionId {
        self.graph_device().map_or(self.selected, SectionId)
    }
    pub fn graph_transition(&self) -> usize {
        if self.graph_device().is_some() {
            self.expansion.curve_transition
        } else {
            self.transition
        }
    }
    pub fn set_graph_transition(&mut self, value: usize) {
        if self.graph_device().is_some() {
            self.expansion.curve_transition = value;
        } else {
            self.transition = value;
        }
    }
    pub fn graph_canvas_id(&self) -> egui::Id {
        let id = egui::Id::unique(("graph canvas", self.graph_id().0, self.graph_key() as u8));
        if let Some(device) = self.graph_device() {
            id.with(("device", device))
        } else {
            id
        }
    }
    pub fn graph_section(&self) -> Section {
        if let Some(id) = self.graph_device() {
            let d = self
                .expansion
                .operations
                .devices
                .iter()
                .find(|d| d.id == id)
                .unwrap();
            let mut c = d.acceleration_curve.as_ref().unwrap().clone();
            // Region operations validate a Section's rate units. Only this
            // transient adapter carries rate units; the document stores m/s².
            c.unit = Unit::RadiansPerMetre;
            let mut section = Section::straight(id, &d.name, c.length());
            section.roll_rate = c;
            section
        } else {
            self.draft.clone()
        }
    }
    pub fn set_graph_section(&mut self, section: Section) -> Result<(), String> {
        self.set_graph_section_in(self.graph_device(), section)
    }
    pub fn set_graph_section_in(
        &mut self,
        device: Option<u64>,
        section: Section,
    ) -> Result<(), String> {
        if let Some(id) = device {
            let d = self
                .expansion
                .operations
                .devices
                .iter_mut()
                .find(|d| d.id == id)
                .ok_or("The device was removed during this gesture")?;
            let mut curve = section.roll_rate;
            curve.unit = Unit::MetresPerSecondSquared;
            qx_document::operations::validate_acceleration_curve(&curve)
                .map_err(|e| e.to_string())?;
            let old = d
                .acceleration_curve
                .as_ref()
                .ok_or("The device curve was removed during this gesture")?
                .length();
            if (curve.length() - old).abs() > 1e-7 {
                return Err("Keep the device-local extent fixed; move an internal boundary or explicitly fit the interval.".into());
            }
            d.acceleration_curve = Some(curve);
        } else if self.selected == section.id {
            self.draft = section;
        }
        Ok(())
    }
    pub fn accept_graph_section(&mut self, section: Section) {
        if let Err(e) = self.set_graph_section(section) {
            self.status = e.clone();
            self.graph_feedback = Some(e);
        } else {
            self.graph_feedback = None;
        }
    }
    pub fn apply_graph(&mut self) -> bool {
        if self.graph_device().is_some() {
            let applied = self.apply_expansion();
            if !applied {
                self.graph_feedback = self.expansion.message.clone();
            }
            applied
        } else {
            self.apply_draft()
        }
    }
    pub fn cancel_graph(&mut self) {
        self.graph_feedback = None;
        if self.graph_device().is_some() {
            self.cancel_expansion();
        } else {
            self.cancel_section();
        }
    }
    pub fn open_device_curve(&mut self, id: u64, span: f64) {
        if self.graph_drag.is_some() {
            return;
        }
        if let Some(d) = self
            .expansion
            .operations
            .devices
            .iter_mut()
            .find(|d| d.id == id)
        {
            let curve = d.editable_curve(span);
            if let Err(e) = qx_document::operations::validate_acceleration_curve(&curve) {
                self.status = e.to_string();
                return;
            }
            d.profile.clear();
            d.acceleration_curve = Some(curve);
            self.expansion.curve_device = Some(id);
            self.expansion.curve_transition = 0;
            self.expansion.engineering = false;
            self.workbench.layouts[0].dock(super::workbench::Pane::Transition);
            self.graph_feedback = None;
        }
    }
}
