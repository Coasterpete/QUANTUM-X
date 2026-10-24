use qx_document::{
    command::{ChannelEdit, Command, Editor},
    persistence,
    sample::foundation_project,
};
use qx_geometry::{
    authoring::{ChannelKey, Section, SectionId},
    solver::GenerationOptions,
    transition::{Function, Transition},
};
use qx_style::{Catalog, StyleSelection};
use std::path::PathBuf;
fn temporary() -> PathBuf {
    std::env::temp_dir().join(format!(
        "qx-document-test-{}-{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn save_reopen_and_regenerate_identically_including_overwrite() {
    let catalog = Catalog::built_in().unwrap();
    let project = foundation_project();
    let path = temporary();
    let before = project
        .generate(&catalog, GenerationOptions::default())
        .unwrap();
    persistence::save(&path, &project, &catalog).unwrap();
    let reopened = persistence::load(&path, &catalog).unwrap();
    assert_eq!(project, reopened);
    assert_eq!(
        before,
        reopened
            .generate(&catalog, GenerationOptions::default())
            .unwrap()
    );
    let mut changed = project.clone();
    changed.metadata.name = "Updated".into();
    persistence::save(&path, &changed, &catalog).unwrap();
    assert_eq!(changed, persistence::load(&path, &catalog).unwrap());
    let mut invalid = changed;
    invalid.sections[0].extent = -1.0;
    assert!(persistence::save(&path, &invalid, &catalog).is_err());
    assert_eq!(
        persistence::load(&path, &catalog).unwrap().metadata.name,
        "Updated"
    );
    std::fs::remove_file(path).unwrap();
}
#[test]
fn compatibility_rejects_unknown_fields_versions_and_bad_references() {
    let catalog = Catalog::built_in().unwrap();
    let project = foundation_project();
    let mut value = serde_json::to_value(&project).unwrap();
    value["future_data"] = true.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
    let mut value = serde_json::to_value(&project).unwrap();
    value["schema_version"] = 2.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
    let mut value = serde_json::to_value(&project).unwrap();
    value["sections"][0]["roll_rate"]["future_channel_data"] = 3.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
    let mut p = project.clone();
    p.sections[1].id = p.sections[0].id;
    assert!(p.validate(&catalog).is_err());
    let mut p = project;
    p.style.style_id = "unknown".into();
    assert!(p.validate(&catalog).is_err());
    assert!(persistence::from_json("{ truncated", &catalog).is_err());
}
#[test]
fn command_transactions_undo_redo_and_style_independence() {
    let catalog = Catalog::built_in().unwrap();
    let project = foundation_project();
    let original = project
        .generate(&catalog, GenerationOptions::default())
        .unwrap();
    let mut editor = Editor::new(project.clone(), catalog.clone(), 8).unwrap();
    editor
        .execute([Command::SetStyle(StyleSelection {
            family_id: "monorail".into(),
            style_id: "monorail-200".into(),
        })])
        .unwrap();
    assert_eq!(
        original,
        editor
            .project()
            .generate(&catalog, GenerationOptions::default())
            .unwrap()
    );
    assert!(editor.undo());
    assert_eq!(editor.project(), &project);
    assert!(editor.redo());
    let checkpoint = editor.project().clone();
    let result = editor.execute([
        Command::RemoveSection { id: SectionId(1) },
        Command::RemoveSection { id: SectionId(999) },
    ]);
    assert!(result.is_err());
    assert_eq!(editor.project(), &checkpoint);
    let replacement = Transition {
        id: 1,
        length: 12.0,
        start: 0.0,
        end: 0.01,
        amplitude: 0.0,
        function: Function::CubicSmoothstep {},
    };
    editor
        .execute([Command::EditChannel {
            section_id: SectionId(1),
            channel: ChannelKey::Roll,
            edit: ChannelEdit::Replace {
                transition: replacement,
            },
        }])
        .unwrap();
    let edited = editor.project().clone();
    assert_ne!(edited, checkpoint);
    assert!(editor.undo());
    assert_eq!(editor.project(), &checkpoint);
    assert!(editor.redo());
    assert_eq!(editor.project(), &edited);
    editor
        .execute([Command::InsertSection {
            after: Some(SectionId(1)),
            section: Section::straight(80, "Inserted", 5.0),
        }])
        .unwrap();
    assert_eq!(editor.project().sections[1].id, SectionId(80));
    assert!(!editor.redo());
}
#[test]
fn coordinated_transition_insertion_and_removal_is_one_undo_step() {
    let catalog = Catalog::built_in().unwrap();
    let original = foundation_project();
    let mut editor = Editor::new(original.clone(), catalog, 4).unwrap();
    let mut first = Transition::constant(1, 6.0, 0.0);
    first.function = Function::Linear {};
    editor
        .execute([
            Command::EditChannel {
                section_id: SectionId(1),
                channel: ChannelKey::Roll,
                edit: ChannelEdit::Replace { transition: first },
            },
            Command::EditChannel {
                section_id: SectionId(1),
                channel: ChannelKey::Roll,
                edit: ChannelEdit::Insert {
                    index: 1,
                    transition: Transition::constant(2, 6.0, 0.0),
                },
            },
        ])
        .unwrap();
    assert_eq!(editor.project().sections[0].roll_rate.transitions.len(), 2);
    assert!(editor.undo());
    assert_eq!(editor.project(), &original);
    assert!(editor.redo());
    editor
        .execute([
            Command::EditChannel {
                section_id: SectionId(1),
                channel: ChannelKey::Roll,
                edit: ChannelEdit::Remove { id: 2 },
            },
            Command::EditChannel {
                section_id: SectionId(1),
                channel: ChannelKey::Roll,
                edit: ChannelEdit::Replace {
                    transition: Transition::constant(1, 12.0, 0.0),
                },
            },
        ])
        .unwrap();
    assert_eq!(editor.project(), &original);
}

#[test]
fn version_one_fixture_remains_compatible() {
    let catalog = Catalog::built_in().unwrap();
    let fixture = persistence::from_json(
        include_str!("../../../examples/foundation.qx.json"),
        &catalog,
    )
    .unwrap();
    assert_eq!(fixture, foundation_project());
    let mut value = serde_json::to_value(fixture).unwrap();
    value["sections"][0]["kind"]["future_kind_field"] = true.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
    let mut value = serde_json::to_value(foundation_project()).unwrap();
    value["sections"][0]["roll_rate"]["transitions"][0]["function"]["future_function_field"] =
        true.into();
    assert!(persistence::from_json(&value.to_string(), &catalog).is_err());
}
