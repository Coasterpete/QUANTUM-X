use qx_geometry::{
    authoring::{InitialConditions, Section},
    construction,
    diagnostic_import::*,
    math::{Frame, Quaternion, Vec3},
    solver::{GenerationOptions, generate},
};
const CSV: &str = include_str!("import-fixtures/terrain.csv");
const XML: &str = include_str!("import-fixtures/terrain-supports.xml");
#[test]
fn native_round_trip_preserves_real_generated_samples() {
    let t = generate(
        &InitialConditions::default(),
        &[Section::straight(1, "Native authored", 20.)],
        GenerationOptions::default(),
    )
    .unwrap();
    let mut bytes = vec![];
    write_native_csv(&t.samples, &mut bytes).unwrap();
    let imported = parse_csv(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert_eq!(imported.format, CsvFormat::Native);
    for (a, b) in t.samples.iter().zip(&imported.samples) {
        assert_eq!(a.position_m, b.position_m);
        assert_eq!(a.distance_m, b.distance_m);
        assert_eq!(a.design_time_s, b.design_time_s);
        assert!((a.frame.normal() - b.frame.normal()).length() < 1e-14);
    }
    assert!(imported.construction_track().sections.is_empty());
    assert!(imported.construction_track().graphs.is_empty());
}
#[test]
fn nl2_positions_coordinate_basis_banking_and_elevations_survive() {
    let t = parse_csv(CSV).unwrap();
    assert_eq!(t.samples.len(), 4);
    assert_eq!(t.samples[0].position_m, Vec3::new(100., 40., -70.));
    assert_eq!(t.bounds.max.y, 41.5);
    assert_eq!(t.bounds.min.y, 40.);
    assert!((t.polyline_length_m - 3. * 1.25_f64.sqrt()).abs() < 1e-12);
    assert!((t.samples[0].frame.lateral() - Vec3::Z).length() < 1e-12);
    // A quarter-turn bank around +X keeps NL2 world XYZ unchanged.
    let banked = format!(
        "{NL2_HEADER}\n1\t0\t50\t0\t1\t0\t0\t0\t1\t0\t0\t0\t1\n2\t1\t50\t0\t1\t0\t0\t0\t1\t0\t0\t0\t1\n"
    );
    let t = parse_csv(&banked).unwrap();
    assert!((t.samples[0].frame.normal() - Vec3::Z).length() < 1e-14);
    assert!((t.samples[0].frame.lateral() + Vec3::Y).length() < 1e-14);
    let inverse = format!(
        "{NL2_HEADER}\n1\t0\t20\t0\t-1\t0\t0\t0\t0\t-1\t0\t-1\t0\n2\t-1\t20\t0\t-1\t0\t0\t0\t0\t-1\t0\t-1\t0\n"
    );
    assert!(parse_csv(&inverse).is_ok());
}
#[test]
fn nested_beam_node_connectivity_extensions_rotations_and_footers() {
    let s = parse_support_xml(XML).unwrap();
    assert_eq!(s.nodes.len(), 6);
    assert_eq!(s.beams.len(), 3);
    let a = s.nodes["rail-A"].position_m;
    let b = s.nodes["foundation-A"].position_m;
    let mid = (a + b) * 0.5;
    assert!((s.nodes["joint-A"].position_m - mid).length() < 1e-12);
    assert!(
        (s.nodes["nested-B"].position_m - (mid + (s.nodes["free-A"].position_m - mid) * 0.25))
            .length()
            < 1e-12
    );
    assert_eq!(b.y, 25.);
    assert_eq!(s.footers[0].rotation_rad, 0.3);
    assert_eq!(s.footers[0].height_above_terrain_m, 0.1524);
    let beam = &s.beams[0];
    assert_eq!(beam.rotation_rad, 0.2);
    assert!(beam.vertical_rotation);
    let (x, y) = beam.endpoints(&s.nodes).unwrap();
    assert!(((y - x).length() - ((b - a).length() + 0.1)).abs() < 1e-12);
    assert_eq!(beam.diameter_m, Some(0.5));
    assert_eq!(s.source_xml, XML);
    let track = parse_csv(CSV).unwrap().construction_track();
    let mut mesh = construction::build(&track, &Default::default(), Default::default()).unwrap();
    s.append_mesh(&mut mesh).unwrap();
    assert_eq!(
        mesh.components
            .iter()
            .filter(|c| c.id.starts_with("support/"))
            .count(),
        3
    );
    assert!(mesh.bounds.min.y > 24.);
    for c in &mesh.components {
        c.mesh.validate().unwrap();
    }
}
#[test]
fn alignment_reports_real_offsets_and_missing_track_without_mutation() {
    let t = parse_csv(CSV).unwrap();
    let s = parse_support_xml(XML).unwrap();
    let before = (t.clone(), s.clone());
    let r = alignment(&t, &s).unwrap();
    assert!((r.rms_nearest_distance_m - 0.5).abs() < 1e-6);
    assert!(r.residuals[0].local_offset_t_up_right_m.y < -0.49999);
    assert!(r.residuals[0].connector_orientation_residual_rad.is_none());
    assert_eq!((t.clone(), s.clone()), before);
    let s = parse_support_xml(&XML.replace("custom_track_index=\"0\"", "custom_track_index=\"1\""))
        .unwrap();
    let r = alignment(&t, &s).unwrap();
    assert_eq!(r.unavailable_track_connectors, 1);
    assert!(r.residuals[0].station_position_distance_m.is_none());
}
#[test]
fn malformed_csv_and_inconsistent_native_frames_are_rejected() {
    for bad in [
        CSV.replace("2\t101", "1\t101"),
        CSV.replace("101\t40.5", "100\t40"),
        CSV.replace("101\t40.5", "NaN\t40.5"),
        CSV.replace("0.894427", "2.894427"),
        CSV.replace("\t", ","),
        CSV.replace("-0.447214\t0.894427\t0", "0.447214\t-0.894427\t0"),
    ] {
        assert!(parse_csv(&bad).is_err(), "{bad}");
    }
    let mut t = parse_csv(CSV).unwrap();
    let mut bytes = vec![];
    write_native_csv(&t.samples, &mut bytes).unwrap();
    let bad = String::from_utf8(bytes)
        .unwrap()
        .replacen(",40,", ",inf,", 1);
    assert!(parse_csv(&bad).is_err());
    t.samples[1].distance_m = 0.;
    let mut bytes = vec![];
    write_native_csv(&t.samples, &mut bytes).unwrap();
    assert!(parse_csv(std::str::from_utf8(&bytes).unwrap()).is_err());
    t.samples[1].distance_m = 1.2;
    t.samples[1].frame = Frame::new(Quaternion::IDENTITY).unwrap();
    let mut bytes = vec![];
    write_native_csv(&t.samples, &mut bytes).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let bad = text.replacen(",1,0,0,0\n", ",0,0,0,0\n", 1);
    assert!(parse_csv(&bad).is_err());
}
#[test]
fn malformed_connectivity_entities_limits_and_unknown_properties() {
    for bad in [XML.replace("end=\"free-A\"","end=\"missing\""),XML.replace("id=\"free-A\"","id=\"rail-A\""),XML.replace("pos=\"0.5\"","pos=\"1.5\""),XML.replace("size1=\"0.5\"","size1=\"NaN\""),"<!DOCTYPE root [<!ENTITY ex SYSTEM 'file:///secret'>]><root><supports>&ex;</supports></root>".into(),"<root><supports>".into(),XML.replace("25 -70","inf -70")] {
        assert!(parse_support_xml(&bad).is_err(),"{bad}");
    }
    let unknown = XML.replace("<vertical/>", "<vertical/><new_property>4</new_property>");
    let s = parse_support_xml(&unknown).unwrap();
    assert_eq!(s.unsupported["beam/new_property"], 1);
    assert!(s.source_xml.contains("new_property"));
    let prefab=XML.replace("</supports>","<unknown_transform><rotation>1</rotation><freenode id=\"untransformed\"><pos>0 0 0</pos></freenode></unknown_transform></supports>");
    let s = parse_support_xml(&prefab).unwrap();
    assert!(!s.nodes.contains_key("untransformed"));
    assert!(
        s.unsupported
            .keys()
            .any(|k| k.contains("unknown_transform"))
    );
    let long = " ".repeat(MAX_INPUT_BYTES + 1);
    assert!(parse_csv(&long).is_err());
    assert!(parse_support_xml(&long).is_err());
    let deep = format!(
        "<root><supports>{}{}{}",
        "<x>".repeat(40),
        "</x>".repeat(40),
        "</supports></root>"
    );
    assert!(parse_support_xml(&deep).is_err());
}
#[test]
fn nested_prefab_atomizations_preserve_explicit_world_geometry() {
    let xml = r#"<root><supports>
      <prefab path="intern:unused.nl2prefab" center_rails_coord="7" custom_track_index="0">
        <atomization><supports>
          <freenode id="outer"><pos>500 45 -700</pos></freenode>
          <prefab path="not-loaded.nl2prefab" center_rails_coord="8" custom_track_index="0">
            <atomization><supports>
              <freenode id="inner"><pos>502 35 -703</pos></freenode>
              <beam start="outer" end="inner" type="1" size1="0.4"/>
            </supports></atomization>
          </prefab>
        </supports></atomization>
      </prefab>
    </supports></root>"#;
    let s = parse_support_xml(xml).unwrap();
    assert_eq!(s.prefabs.len(), 2);
    assert_eq!(s.beams.len(), 1);
    assert!(s.prefabs.iter().all(|p| p.atomized));
    assert_eq!(s.nodes["inner"].position_m, Vec3::new(502., 35., -703.));
    assert_eq!(s.nodes["outer"].position_m, Vec3::new(500., 45., -700.));
    let missing = xml
        .replace("<atomization><supports>", "<unknown><supports>")
        .replace("</supports></atomization>", "</supports></unknown>");
    let s = parse_support_xml(&missing).unwrap();
    assert!(s.nodes.is_empty());
    assert!(
        s.unsupported
            .contains_key("prefab without explicit atomization omitted")
    );
}
#[test]
fn optional_private_brobdingnagian_compatibility() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/nl2/BROBDINGNAGIAN");
    let csv = root.join("BROBDINGNAGIAN.csv");
    let xml = root.join("BROBDINGNAGIAN_supports.xml");
    if !csv.is_file() || !xml.is_file() {
        eprintln!("SKIP: private BROBDINGNAGIAN exports are absent (optional local integration)");
        return;
    }
    let t = parse_csv(&read_text(&csv).unwrap()).unwrap();
    let s = parse_support_xml(&read_text(&xml).unwrap()).unwrap();
    assert_eq!(t.samples.len(), 5655);
    assert_eq!(s.nodes.len(), 2475);
    assert_eq!(s.beams.len(), 914);
    assert_eq!(s.footers.len(), 290);
    assert!((t.polyline_length_m - 2826.9881997604757).abs() < 1e-7);
    let r = alignment(&t, &s).unwrap();
    assert_eq!(r.matched_track_connectors, 174);
    assert_eq!(r.unavailable_track_connectors, 5);
    assert!(r.max_nearest_distance_m < 1.082);
    assert!(t.bounds.max.y - t.bounds.min.y > 146.);
    println!(
        "private compatibility: {} samples, {} nodes, {} beams; connector RMS {} m, max {} m",
        t.samples.len(),
        s.nodes.len(),
        s.beams.len(),
        r.rms_nearest_distance_m,
        r.max_nearest_distance_m
    );
}
