use qx_document::Project;
use qx_geometry::{
    authoring::{Domain, SectionKind},
    solver::GeneratedTrack,
    transition::Unit,
};
use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

fn writer(dir: &Path, name: &str) -> io::Result<BufWriter<File>> {
    Ok(BufWriter::new(File::create(dir.join(name))?))
}
fn cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}
fn row(w: &mut impl Write, values: impl IntoIterator<Item = String>) -> io::Result<()> {
    writeln!(
        w,
        "{}",
        values
            .into_iter()
            .map(|v| cell(&v))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn optional(v: Option<f64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_default()
}
fn domain(d: Domain) -> &'static str {
    match d {
        Domain::Time => "seconds",
        Domain::Distance => "metres",
        Domain::Normalized => "normalized_arc_distance",
    }
}
fn unit(u: Unit) -> &'static str {
    match u {
        Unit::G => "g",
        Unit::RadiansPerSecond => "rad/s",
        Unit::RadiansPerMetre => "rad/m",
        Unit::RadiansPerNormalized => "rad/normalized_distance",
    }
}
fn kind(s: &SectionKind) -> &'static str {
    match s {
        SectionKind::Spline { curve } => curve.method(),
        SectionKind::Straight {} => "straight",
        SectionKind::Arc { .. } => "arc",
        SectionKind::Force { .. } => "force",
        SectionKind::Geometric { .. } => "world_geometric",
        SectionKind::RiderLocal { .. } => "rider_local",
    }
}
fn xml(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' && c != '\r' {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub enum Refinement {
    Complete(f64),
    Unavailable(String),
}
impl Refinement {
    pub fn description(&self) -> String {
        match self {
            Self::Complete(delta) => format!(
                "Step-halving maximum position difference: {delta:.12e} m (not an error bound)."
            ),
            Self::Unavailable(reason) => format!(
                "Step-halving comparison unavailable: {reason}. Requested-quality generation succeeded."
            ),
        }
    }
}
pub fn write_all(
    dir: &Path,
    project: &Project,
    track: &GeneratedTrack,
    step: f64,
    refinement: &Refinement,
) -> io::Result<()> {
    centerline(dir, track)?;
    graphs(dir, project, track)?;
    transitions(dir, project)?;
    obj(dir, track)?;
    plots(dir, project, track)?;
    let mut sections = writer(dir, "sections.csv")?;
    writeln!(
        sections,
        "section_id,name,method,start_sample,end_sample,start_distance_m,end_distance_m"
    )?;
    for s in &track.sections {
        let section = project
            .sections
            .iter()
            .find(|section| section.id == s.id)
            .expect("generated section");
        row(
            &mut sections,
            [
                s.id.0.to_string(),
                section.name.clone(),
                kind(&section.kind).into(),
                s.start_sample.to_string(),
                s.end_sample.to_string(),
                s.start_distance_m.to_string(),
                s.end_distance_m.to_string(),
            ],
        )?;
    }
    sections.flush()?;
    let mut curve_joins = writer(dir, "curve-joins.csv")?;
    writeln!(
        curve_joins,
        "section_id,native_parameter,position_gap_m,tangent_angle_rad,curvature_vector_gap_per_m,first_derivative_gap,second_derivative_gap,g0,g1,g2,c1,c2"
    )?;
    for section in &project.sections {
        if let SectionKind::Spline { curve } = &section.kind {
            for join in curve
                .prepare()
                .map_err(io::Error::other)?
                .continuity()
                .map_err(io::Error::other)?
            {
                row(
                    &mut curve_joins,
                    [
                        section.id.0.to_string(),
                        join.parameter.to_string(),
                        join.position_gap_m.to_string(),
                        join.tangent_angle_rad.to_string(),
                        join.curvature_vector_gap_per_m.to_string(),
                        join.first_derivative_gap.to_string(),
                        join.second_derivative_gap.to_string(),
                        join.g0(1e-8).to_string(),
                        join.g1(1e-8).to_string(),
                        join.g2(1e-8).to_string(),
                        join.c1(1e-8).to_string(),
                        join.c2(1e-8).to_string(),
                    ],
                )?;
            }
        }
    }
    curve_joins.flush()?;
    let mut connections = writer(dir, "connections.csv")?;
    writeln!(
        connections,
        "section_id,position_gap_m,tangent_angle_rad,orientation_angle_rad,curvature_vector_gap_per_m,twist_rate_gap_rad_per_m,reference_speed_gap_mps"
    )?;
    for c in &track.connections {
        writeln!(
            connections,
            "{},{},{},{},{},{},{}",
            c.section_id.0,
            c.position_gap_m,
            c.tangent_angle_rad,
            c.orientation_angle_rad,
            c.curvature_vector_gap_per_m,
            c.twist_rate_gap_rad_per_m,
            c.reference_speed_gap_mps
        )?;
    }
    connections.flush()?;
    let mut numerical = writer(dir, "numerical.csv")?;
    writeln!(
        numerical,
        "section_id,length_m,estimated_quadrature_error_m,quadrature_evaluations,table_stations"
    )?;
    for n in &track.numerical {
        writeln!(
            numerical,
            "{},{},{},{},{}",
            n.section_id.0,
            n.length_m,
            n.estimated_quadrature_error_m,
            n.quadrature_evaluations,
            n.table_stations
        )?;
    }
    numerical.flush()?;
    let mut warnings = writer(dir, "warnings.csv")?;
    writeln!(warnings, "section_id,input,code,message")?;
    for d in &track.diagnostics {
        row(
            &mut warnings,
            [
                d.section_id.0.to_string(),
                optional(d.input),
                d.code.into(),
                d.message.clone(),
            ],
        )?;
    }
    warnings.flush()?;
    let mut summary = writer(dir, "summary.txt")?;
    writeln!(
        summary,
        "QUANTUM-X development-only geometry diagnostics\nProject: {}\nSections: {}\nLength: {:.12} m\nCenterline samples: {}\nMax integration step: {} m\nSave/reopen regeneration: identical on this machine/build\n{}\nAll forces are design-speed geometry diagnostics, not train simulation.\nOBJ is a centerline polyline, not a rail mesh.\n",
        project.metadata.name,
        project.sections.len(),
        track.length_m(),
        track.samples.len(),
        step,
        refinement.description()
    )?;
    if let Some(p) = track.samples.last() {
        writeln!(
            summary,
            "End position (m): {}, {}, {}",
            p.position_m.x, p.position_m.y, p.position_m.z
        )?;
    }
    for s in &track.sections {
        writeln!(
            summary,
            "Section {}: samples {}..={}, distance {}..{} m",
            s.id.0, s.start_sample, s.end_sample, s.start_distance_m, s.end_distance_m
        )?;
    }
    for n in &track.numerical {
        writeln!(
            summary,
            "Spline section {}: length {} m; estimated quadrature error {} m; {} speed evaluations; {} table stations (estimate, not certified bound)",
            n.section_id.0,
            n.length_m,
            n.estimated_quadrature_error_m,
            n.quadrature_evaluations,
            n.table_stations
        )?;
    }
    writeln!(summary, "\nWarnings: {}", track.diagnostics.len())?;
    for d in &track.diagnostics {
        writeln!(
            summary,
            "[{}] section {} input {:?}: {}",
            d.code, d.section_id.0, d.input, d.message
        )?;
    }
    summary.flush()
}
fn centerline(dir: &Path, track: &GeneratedTrack) -> io::Result<()> {
    let mut w = writer(dir, "centerline.csv")?;
    writeln!(
        w,
        "distance_m,design_time_s,x_m,y_m,z_m,tangent_x,tangent_y,tangent_z,rider_normal_x,rider_normal_y,rider_normal_z,lateral_x,lateral_y,lateral_z,quaternion_w,quaternion_x,quaternion_y,quaternion_z"
    )?;
    for s in &track.samples {
        let p = s.position_m;
        let t = s.frame.tangent();
        let n = s.frame.normal();
        let l = s.frame.lateral();
        let q = s.frame.orientation();
        row(
            &mut w,
            [
                s.distance_m,
                s.design_time_s,
                p.x,
                p.y,
                p.z,
                t.x,
                t.y,
                t.z,
                n.x,
                n.y,
                n.z,
                l.x,
                l.y,
                l.z,
                q.w,
                q.x,
                q.y,
                q.z,
            ]
            .map(|v| v.to_string()),
        )?;
    }
    w.flush()
}
fn graphs(dir: &Path, project: &Project, track: &GeneratedTrack) -> io::Result<()> {
    let mut w = writer(dir, "measurements.csv")?;
    writeln!(
        w,
        "section_id,section_kind,domain,input,distance_m,design_time_s,elevation_m,bank_rad,pitch_rad,yaw_rad,curvature_per_m,derived_normal_g,derived_lateral_g,derived_twist_rad_per_m,derived_world_pitch_rate_rad_per_m,derived_world_yaw_rate_rad_per_m,reference_speed_mps,authored_normal_g,authored_lateral_g,authored_pitch_rate,authored_yaw_rate,authored_roll_rate,authored_angular_rate_unit"
    )?;
    for g in &track.graphs {
        let section = project
            .sections
            .iter()
            .find(|s| s.id == g.section_id)
            .expect("generated section exists");
        row(
            &mut w,
            [
                g.section_id.0.to_string(),
                kind(&section.kind).into(),
                domain(g.domain).into(),
                g.input.to_string(),
                g.distance_m.to_string(),
                g.design_time_s.to_string(),
                g.elevation_m.to_string(),
                optional(g.bank_rad),
                g.pitch_rad.to_string(),
                g.yaw_rad.to_string(),
                g.curvature_per_m.to_string(),
                g.normal_g.to_string(),
                g.lateral_g.to_string(),
                g.roll_rate_rad_per_m.to_string(),
                optional(g.pitch_rate_rad_per_m),
                optional(g.yaw_rate_rad_per_m),
                g.reference_speed_mps.to_string(),
                optional(g.authored_normal_g),
                optional(g.authored_lateral_g),
                optional(g.authored_pitch_rate),
                optional(g.authored_yaw_rate),
                g.authored_roll_rate.to_string(),
                unit(g.domain.rate_unit()).into(),
            ],
        )?;
    }
    w.flush()
}
fn transitions(dir: &Path, project: &Project) -> io::Result<()> {
    let mut w = writer(dir, "transitions.csv")?;
    writeln!(
        w,
        "section_id,section_name,channel,input_unit,output_unit,transition_id,input,value,derivative,endpoint_side"
    )?;
    for section in &project.sections {
        for (name, channel) in section.channels() {
            let mut start = 0.0;
            for t in &channel.transitions {
                for i in 0..=64 {
                    let local = t.length * i as f64 / 64.0;
                    let e = t.evaluate(local).map_err(io::Error::other)?;
                    row(
                        &mut w,
                        [
                            section.id.0.to_string(),
                            section.name.clone(),
                            name.into(),
                            domain(section.domain).into(),
                            unit(channel.unit).into(),
                            t.id.to_string(),
                            (start + local).to_string(),
                            e.value.to_string(),
                            e.derivative.to_string(),
                            if i == 0 {
                                "right"
                            } else if i == 64 {
                                "left"
                            } else {
                                "interior"
                            }
                            .into(),
                        ],
                    )?;
                }
                start += t.length;
            }
        }
    }
    w.flush()
}
fn obj(dir: &Path, track: &GeneratedTrack) -> io::Result<()> {
    let mut w = writer(dir, "centerline.obj")?;
    writeln!(
        w,
        "# QUANTUM-X development centerline polyline; metres, Y up; no rail surface\no centerline"
    )?;
    for s in &track.samples {
        writeln!(
            w,
            "v {} {} {}",
            s.position_m.x, s.position_m.y, s.position_m.z
        )?;
    }
    // Separate line segments avoid enormous single OBJ records.
    for i in 1..track.samples.len() {
        writeln!(w, "l {} {}", i, i + 1)?;
    }
    w.flush()
}

struct Series {
    label: String,
    points: Vec<Option<(f64, f64)>>,
}
fn series(label: &str, points: impl Iterator<Item = (f64, f64)>) -> Series {
    Series {
        label: label.into(),
        points: points.map(Some).collect(),
    }
}
fn optional_series(label: &str, points: impl Iterator<Item = (f64, Option<f64>)>) -> Series {
    Series {
        label: label.into(),
        points: points.map(|(x, y)| y.map(|y| (x, y))).collect(),
    }
}
fn svg(
    dir: &Path,
    name: &str,
    title: &str,
    x_label: &str,
    panels: Vec<(String, Vec<Series>)>,
) -> io::Result<()> {
    let mut w = writer(dir, name)?;
    let height = 90 + panels.len() * 280;
    writeln!(
        w,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1100\" height=\"{height}\" viewBox=\"0 0 1100 {height}\"><rect width=\"1100\" height=\"{height}\" fill=\"#ffffff\"/><g font-family=\"sans-serif\" fill=\"#17212b\"><text x=\"24\" y=\"28\" font-size=\"20\">{}</text><text x=\"24\" y=\"52\" font-size=\"13\">Development diagnostics; design-speed geometry; angles in radians; not train simulation</text>",
        xml(title)
    )?;
    let colors = ["#185c9b", "#a53b22", "#367129", "#713a91"];
    for (index, (label, lines)) in panels.into_iter().enumerate() {
        let ytop = 100.0 + index as f64 * 280.0;
        let points: Vec<_> = lines
            .iter()
            .flat_map(|s| s.points.iter())
            .flatten()
            .filter(|(x, y)| x.is_finite() && y.is_finite())
            .collect();
        let (mut xmin, mut xmax, mut ymin, mut ymax) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for (x, y) in points {
            xmin = xmin.min(*x);
            xmax = xmax.max(*x);
            ymin = ymin.min(*y);
            ymax = ymax.max(*y);
        }
        if !xmin.is_finite() {
            xmin = 0.0;
            xmax = 1.0;
            ymin = 0.0;
            ymax = 1.0;
        }
        if xmax - xmin < 1e-12 {
            xmax = xmin + 1.0;
        }
        let pad = ((ymax - ymin) * 0.08).max(0.01);
        ymin -= pad;
        ymax += pad;
        writeln!(
            w,
            "<text x=\"24\" y=\"{}\" font-size=\"15\">{}</text><path d=\"M 100 {} V {} H 1050\" fill=\"none\" stroke=\"#63717e\"/>",
            ytop - 14.0,
            xml(&label),
            ytop,
            ytop + 170.0
        )?;
        for tick in 0..=4 {
            let u = tick as f64 / 4.0;
            let x = 100.0 + 950.0 * u;
            let y = ytop + 170.0 - 170.0 * u;
            writeln!(
                w,
                "<path d=\"M {x} {} v 4 M 96 {y} H 1050\" stroke=\"#d6dce1\" fill=\"none\"/><text x=\"{x}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\">{:.3}</text><text x=\"90\" y=\"{}\" text-anchor=\"end\" font-size=\"11\">{:.4}</text>",
                ytop + 170.0,
                ytop + 188.0,
                xmin + u * (xmax - xmin),
                y + 4.0,
                ymin + u * (ymax - ymin)
            )?;
        }
        for (j, line) in lines.into_iter().enumerate() {
            let color = colors[j % colors.len()];
            // Preserve missing-data gaps instead of inventing connecting curves.
            for run in line.points.split(Option::is_none).filter(|r| !r.is_empty()) {
                let points = run
                    .iter()
                    .flatten()
                    .filter(|(x, y)| x.is_finite() && y.is_finite())
                    .map(|(x, y)| {
                        format!(
                            "{:.3},{:.3}",
                            100.0 + (x - xmin) / (xmax - xmin) * 950.0,
                            ytop + 170.0 - (y - ymin) / (ymax - ymin) * 170.0
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                // Dash patterns distinguish overlays independently of color.
                let dash = if j % 2 == 1 { "7 4" } else { "none" };
                writeln!(
                    w,
                    "<polyline points=\"{points}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.7\" stroke-dasharray=\"{dash}\"/>"
                )?;
            }
            writeln!(
                w,
                "<text x=\"{}\" y=\"{}\" fill=\"{color}\" font-size=\"12\">{}</text>",
                100 + j * 230,
                ytop + 225.0,
                xml(&line.label)
            )?;
        }
        writeln!(
            w,
            "<text x=\"575\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\">{}</text>",
            ytop + 207.0,
            xml(x_label)
        )?;
    }
    writeln!(w, "</g></svg>")?;
    w.flush()
}
fn plots(dir: &Path, project: &Project, track: &GeneratedTrack) -> io::Result<()> {
    let mut panels = vec![
        (
            "Elevation (m)".into(),
            vec![series(
                "generated elevation",
                track.graphs.iter().map(|g| (g.distance_m, g.elevation_m)),
            )],
        ),
        (
            "Normal load (g)".into(),
            vec![
                series(
                    "derived from frames",
                    track.graphs.iter().map(|g| (g.distance_m, g.normal_g)),
                ),
                optional_series(
                    "authored (force sections)",
                    track
                        .graphs
                        .iter()
                        .map(|g| (g.distance_m, g.authored_normal_g)),
                ),
            ],
        ),
        (
            "Lateral load (g)".into(),
            vec![
                series(
                    "derived from frames",
                    track.graphs.iter().map(|g| (g.distance_m, g.lateral_g)),
                ),
                optional_series(
                    "authored (force sections)",
                    track
                        .graphs
                        .iter()
                        .map(|g| (g.distance_m, g.authored_lateral_g)),
                ),
            ],
        ),
        (
            "Bank (rad, wrapped; undefined at vertical tangents)".into(),
            vec![optional_series(
                "gravity-referenced bank",
                track.graphs.iter().map(|g| (g.distance_m, g.bank_rad)),
            )],
        ),
        (
            "Curvature (1/m)".into(),
            vec![series(
                "derived curvature",
                track
                    .graphs
                    .iter()
                    .map(|g| (g.distance_m, g.curvature_per_m)),
            )],
        ),
        (
            "Twist (rad/m)".into(),
            vec![series(
                "derived rider-frame twist",
                track
                    .graphs
                    .iter()
                    .map(|g| (g.distance_m, g.roll_rate_rad_per_m)),
            )],
        ),
        (
            "World pitch/yaw rate (rad/m)".into(),
            vec![
                optional_series(
                    "pitch",
                    track
                        .graphs
                        .iter()
                        .map(|g| (g.distance_m, g.pitch_rate_rad_per_m)),
                ),
                optional_series(
                    "yaw about -Y",
                    track
                        .graphs
                        .iter()
                        .map(|g| (g.distance_m, g.yaw_rate_rad_per_m)),
                ),
            ],
        ),
        (
            "Reference speed (m/s)".into(),
            vec![series(
                "design speed",
                track
                    .graphs
                    .iter()
                    .map(|g| (g.distance_m, g.reference_speed_mps)),
            )],
        ),
    ];
    svg(
        dir,
        "measurements.svg",
        "QUANTUM-X generated measurements",
        "Arc distance (m)",
        std::mem::take(&mut panels),
    )?;
    let mut time_panels = Vec::new();
    for section in &project.sections {
        for (name, channel) in section.channels() {
            let mut start = 0.0;
            let mut points = vec![];
            for t in &channel.transitions {
                for i in 0..=64 {
                    let x = t.length * i as f64 / 64.0;
                    points.push(Some((
                        start + x,
                        t.evaluate(x).map_err(io::Error::other)?.value,
                    )));
                }
                start += t.length;
            }
            time_panels.push((
                format!(
                    "Section {}: {} / {} ({}); local input in {}",
                    section.id.0,
                    section.name.chars().take(60).collect::<String>(),
                    name,
                    unit(channel.unit),
                    domain(section.domain)
                ),
                vec![Series {
                    label: name.into(),
                    points,
                }],
            ));
        }
    }
    svg(
        dir,
        "transitions.svg",
        "QUANTUM-X authored transition channels",
        "Local section input (units shown above)",
        time_panels,
    )?;
    svg(
        dir,
        "trajectory.svg",
        "QUANTUM-X centerline projections",
        "World X (m)",
        vec![
            (
                "Side projection: world elevation Y (m)".into(),
                vec![series(
                    "centerline",
                    track
                        .samples
                        .iter()
                        .map(|s| (s.position_m.x, s.position_m.y)),
                )],
            ),
            (
                "Plan projection: world lateral Z (m)".into(),
                vec![series(
                    "centerline",
                    track
                        .samples
                        .iter()
                        .map(|s| (s.position_m.x, s.position_m.z)),
                )],
            ),
        ],
    )
}
