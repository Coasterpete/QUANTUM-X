use crate::{Metadata, Project, Result, require};
use qx_geometry::{
    authoring::{ChannelKey, InitialConditions, Section, SectionId},
    curve_edit::CurveEdit,
    transition::Transition,
};
use qx_style::{Catalog, StyleSelection};
use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub enum ChannelEdit {
    Insert {
        index: usize,
        transition: Transition,
    },
    Replace {
        transition: Transition,
    },
    Remove {
        id: u64,
    },
}
#[derive(Clone, Debug)]
pub enum Command {
    EditSpline {
        section_id: SectionId,
        edit: CurveEdit,
    },
    /// Explicit generation check of all inherited boundary constraints.
    ValidateConnections {
        options: qx_geometry::solver::GenerationOptions,
    },
    InsertSection {
        after: Option<SectionId>,
        section: Section,
    },
    RemoveSection {
        id: SectionId,
    },
    ReplaceSection {
        section: Section,
    },
    EditChannel {
        section_id: SectionId,
        channel: ChannelKey,
        edit: ChannelEdit,
    },
    SetMetadata(Metadata),
    SetInitial(InitialConditions),
    SetStyle(StyleSelection),
}
impl Command {
    fn apply(self, project: &mut Project) -> Result<()> {
        let index = |id: SectionId, sections: &[Section]| {
            sections
                .iter()
                .position(|s| s.id == id)
                .ok_or_else(|| crate::DocumentError(format!("section ID {} not found", id.0)))
        };
        match self {
            Self::EditSpline { section_id, edit } => {
                let i = index(section_id, &project.sections)?;
                let qx_geometry::authoring::SectionKind::Spline { curve } =
                    &mut project.sections[i].kind
                else {
                    return Err(crate::DocumentError(
                        "section is not spline-authored".into(),
                    ));
                };
                edit.apply(curve)?;
            }
            Self::ValidateConnections { options } => {
                qx_geometry::solver::generate(&project.initial, &project.sections, options)?;
            }
            Self::InsertSection { after, section } => {
                let i = after
                    .map(|id| index(id, &project.sections).map(|n| n + 1))
                    .transpose()?
                    .unwrap_or(0);
                project.sections.insert(i, section);
            }
            Self::RemoveSection { id } => {
                let i = index(id, &project.sections)?;
                project.sections.remove(i);
            }
            Self::ReplaceSection { section } => {
                let i = index(section.id, &project.sections)?;
                project.sections[i] = section;
            }
            Self::EditChannel {
                section_id,
                channel,
                edit,
            } => {
                let i = index(section_id, &project.sections)?;
                let c = project.sections[i].channel_mut(channel)?;
                match edit {
                    ChannelEdit::Insert { index, transition } => c.insert(index, transition)?,
                    ChannelEdit::Replace { transition } => c.replace(transition.id, transition)?,
                    ChannelEdit::Remove { id } => c.remove(id)?,
                }
            }
            Self::SetMetadata(metadata) => project.metadata = metadata,
            Self::SetInitial(initial) => project.initial = initial,
            Self::SetStyle(style) => project.style = style,
        }
        Ok(())
    }
}
/// Bounded snapshot history is deliberate for this first document format.
/// Execute validates the whole batch once; failed commands change no state.
pub struct Editor {
    project: Project,
    catalog: Catalog,
    undo: VecDeque<Project>,
    redo: Vec<Project>,
    capacity: usize,
}
impl Editor {
    pub fn new(project: Project, catalog: Catalog, capacity: usize) -> Result<Self> {
        require(
            (1..=256).contains(&capacity),
            "history capacity must be in [1,256]",
        )?;
        project.validate(&catalog)?;
        Ok(Self {
            project,
            catalog,
            undo: VecDeque::new(),
            redo: vec![],
            capacity,
        })
    }
    pub fn project(&self) -> &Project {
        &self.project
    }
    pub fn execute(&mut self, commands: impl IntoIterator<Item = Command>) -> Result<()> {
        let mut candidate = self.project.clone();
        let mut count = 0;
        for command in commands {
            count += 1;
            require(count <= 1024, "command batch exceeds 1024 operations")?;
            command.apply(&mut candidate)?;
        }
        candidate.validate(&self.catalog)?;
        if candidate != self.project {
            if self.undo.len() == self.capacity {
                self.undo.pop_front();
            }
            self.undo
                .push_back(std::mem::replace(&mut self.project, candidate));
            self.redo.clear();
        }
        Ok(())
    }
    pub fn undo(&mut self) -> bool {
        if let Some(previous) = self.undo.pop_back() {
            self.redo
                .push(std::mem::replace(&mut self.project, previous));
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo
                .push_back(std::mem::replace(&mut self.project, next));
            true
        } else {
            false
        }
    }
}
