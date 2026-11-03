//! Appearance edits use the existing stable construction slots and component IDs.
//! No geometry, family classification, or persisted fields are introduced here.
use crate::{Result, construction::TrackConstruction, materials::Binding, require};

impl TrackConstruction {
    /// Carry the complete appearance of matching components across a style
    /// change, including private rail slots, tints, UV transforms and finishes.
    pub fn reuse_component_appearances(&mut self, previous: &Self) -> Result<()> {
        let mut reused = std::collections::BTreeMap::<String, String>::new();
        let mut materials = self.materials.clone();
        for (id, slot) in self
            .continuous
            .iter_mut()
            .map(|c| (&c.id, &mut c.material))
            .chain(self.repeated.iter_mut().map(|c| (&c.id, &mut c.material)))
        {
            let Some(source) = previous.material_slot(id) else {
                continue;
            };
            let Some(material) = previous.materials.iter().find(|m| m.id == source) else {
                continue;
            };
            let next = reused.entry(source.to_owned()).or_insert_with(|| {
                let mut n = 1;
                while materials.iter().any(|m| m.id == format!("retained-{n}")) {
                    n += 1;
                }
                let mut m = material.clone();
                m.id = format!("retained-{n}");
                let id = m.id.clone();
                materials.push(m);
                id
            });
            slot.clone_from(next);
        }
        materials.retain(|m| {
            self.continuous.iter().any(|c| c.material == m.id)
                || self.repeated.iter().any(|c| c.material == m.id)
        });
        self.materials = materials;
        self.validate()
    }
    pub fn material_slot(&self, component: &str) -> Option<&str> {
        self.continuous
            .iter()
            .find(|c| c.id == component)
            .map(|c| c.material.as_str())
            .or_else(|| {
                self.repeated
                    .iter()
                    .find(|c| c.id == component)
                    .map(|c| c.material.as_str())
            })
    }

    /// Give one component its own appearance without changing other consumers of
    /// a shared slot. An existing private slot is reused, keeping edits bounded.
    pub fn override_component_material(&mut self, component: &str) -> Result<()> {
        let source = self
            .material_slot(component)
            .ok_or_else(|| crate::CatalogError("Unknown track component".into()))?
            .to_owned();
        let users = self
            .continuous
            .iter()
            .map(|c| &c.material)
            .chain(self.repeated.iter().map(|c| &c.material))
            .filter(|id| **id == source)
            .count();
        if users == 1 {
            return Ok(());
        }
        require(
            self.materials.len() < 64,
            "Material slot limit reached; reuse an existing appearance",
        )?;
        let mut material = self
            .materials
            .iter()
            .find(|m| m.id == source)
            .ok_or_else(|| crate::CatalogError("Component material slot is missing".into()))?
            .clone();
        let mut n = 1;
        while self
            .materials
            .iter()
            .any(|m| m.id == format!("appearance-{n}"))
        {
            n += 1;
        }
        material.id = format!("appearance-{n}");
        let id = material.id.clone();
        self.materials.push(material);
        self.share_component_material(component, &id)
    }

    /// Explicitly join a shared appearance. Unused slots are retained so no
    /// authored fallback/binding is silently removed from a project.
    pub fn share_component_material(&mut self, component: &str, slot: &str) -> Result<()> {
        require(
            self.materials.iter().any(|m| m.id == slot),
            "Unknown material slot",
        )?;
        if let Some(c) = self.continuous.iter_mut().find(|c| c.id == component) {
            c.material = slot.into();
            return Ok(());
        }
        if let Some(c) = self.repeated.iter_mut().find(|c| c.id == component) {
            c.material = slot.into();
            return Ok(());
        }
        Err(crate::CatalogError("Unknown track component".into()))
    }

    /// Reuse explicitly registered bindings on compatible destination slots.
    /// Unmatched slots retain the new style's defaults; geometry is untouched.
    pub fn reuse_material_bindings(&mut self, previous: &Self) {
        for slot in &mut self.materials {
            if let Some(binding) = previous
                .materials
                .iter()
                .find(|m| m.id == slot.id)
                .and_then(|m| m.library.as_ref())
            {
                slot.library = Some(binding.clone());
            }
        }
    }

    pub fn assign_material(&mut self, slot: &str, binding: Option<Binding>) -> Result<()> {
        if let Some(binding) = &binding {
            binding.validate()?;
        }
        let material = self
            .materials
            .iter_mut()
            .find(|m| m.id == slot)
            .ok_or_else(|| crate::CatalogError("Unknown material slot".into()))?;
        material.library = binding;
        Ok(())
    }
}
