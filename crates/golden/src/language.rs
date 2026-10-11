//! The language of a scene: an `.md` with properties (front matter), a title
//! and a table of spans. It is the same language as the Gherkin tables of
//! 201–203 and R6; reading them is the same function.
//!
//! ```text
//! ---
//! format: 1           required: the language version (ABI)
//! cadence_s: 1        source seconds between samples (default 1)
//! dwell_s: 2          the dwell the expectations hold with (optional)
//! senses: detect, reinforce, faces   the senses it assumes (default detect)
//! ---
//!
//! # Title
//!
//! | samples | on floor | where | in bed | … | occupants | bed   |
//! |---------|----------|-------|--------|---|-----------|-------|
//! | 1-5     | 2        | door  | 0      | … | 2         | empty |
//! ```

use std::fmt;

/// The language version this library reads.
pub const FORMAT: u32 = 1;

/// A scene: the night, in spans.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub format: u32,
    pub title: String,
    /// Source seconds between one sample and the next.
    pub cadence_s: f64,
    /// The dwell the table's expectations hold with, if it says so.
    pub dwell_s: Option<f64>,
    /// The senses the expectations assume: the profile must have them.
    pub senses: Vec<Sense>,
    pub spans: Vec<Span>,
}

/// A sense a scene can assume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    Detect,
    Reinforce,
    Faces,
}

/// The samples `from..=to` of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Samples {
    pub from: u32,
    pub to: u32,
}

/// A span: consecutive samples in which the senses see the same.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub samples: Samples,
    /// How many the main detector sees off the bed.
    pub floor: u32,
    pub location: Location,
    pub bed: InBed,
    /// How many reinforcement sees in bed, if asked.
    pub reinforcement: u32,
    pub faces: Faces,
    pub expected: Expected,
}

/// Where those seen on the floor are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Location {
    /// Far from the bed: they come from the door.
    #[default]
    Door,
    /// Next to the bed.
    NearBed,
    /// The first `near` next to the bed and the rest at the door: a real night
    /// mixes them (`where: 1 near bed`).
    Mixed { near: u32 },
}

/// How many the main detector sees in bed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InBed {
    Fixed(u32),
    /// 0 and 1 alternating, starting with 0.
    Alternating,
}

/// Which faces the face detector sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Faces {
    #[default]
    None,
    OnSubject,
    AtHeadboard,
    Weak,
}

/// What the engine must believe at the end of the span, if the table says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Expected {
    pub occupants: Option<u32>,
    pub bed: Option<ExpectedBed>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedBed {
    Empty,
    /// Seen or held.
    Occupied,
    Seen,
    Held,
    Unknown,
}

/// A reading error, with the line if known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub line: Option<usize>,
    pub reason: String,
}

impl Error {
    fn at(line: usize, reason: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "line {line}: {}", self.reason),
            None => f.write_str(&self.reason),
        }
    }
}

impl std::error::Error for Error {}

impl Scene {
    /// Reads a scene. It does no IO: the caller brings the text.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let lines: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l)).collect();
        let mut rest = lines.iter().copied();

        // Properties.
        match rest.next() {
            Some((_, "---")) => {}
            _ => return Err(Error::at(1, "missing the properties header (---)")),
        }
        let mut format = None;
        let mut cadence_s = 1.0;
        let mut dwell_s = None;
        let mut senses = vec![Sense::Detect];
        loop {
            let Some((n, line)) = rest.next() else {
                return Err(Error::at(
                    lines.len(),
                    "the properties header is not closed (---)",
                ));
            };
            if line == "---" {
                break;
            }
            let (key, value) = line
                .split_once(':')
                .map(|(k, v)| (k.trim(), v.trim()))
                .ok_or_else(|| Error::at(n, format!("property without \"key: value\": {line}")))?;
            match key {
                "format" => format = Some(number::<u32>(n, key, value)?),
                "cadence_s" => {
                    let v = number::<f64>(n, key, value)?;
                    if !(v.is_finite() && v > 0.0) {
                        return Err(Error::at(n, "cadence_s must be positive"));
                    }
                    cadence_s = v;
                }
                "dwell_s" => {
                    let v = number::<f64>(n, key, value)?;
                    if !(v.is_finite() && v >= 0.0) {
                        return Err(Error::at(n, "dwell_s must not be negative"));
                    }
                    dwell_s = Some(v);
                }
                "senses" => {
                    senses = value
                        .split(',')
                        .map(str::trim)
                        .map(|s| match s {
                            "detect" => Ok(Sense::Detect),
                            "reinforce" => Ok(Sense::Reinforce),
                            "faces" => Ok(Sense::Faces),
                            other => Err(Error::at(n, format!("unknown sense: {other}"))),
                        })
                        .collect::<Result<_, _>>()?;
                    if !senses.contains(&Sense::Detect) {
                        return Err(Error::at(
                            n,
                            "every scene assumes the main detector (detect)",
                        ));
                    }
                }
                other => return Err(Error::at(n, format!("unknown property: {other}"))),
            }
        }
        let format = format.ok_or_else(|| Error {
            line: None,
            reason: "missing the format property".into(),
        })?;
        if format != FORMAT {
            return Err(Error {
                line: None,
                reason: format!("unknown format {format}; format {FORMAT} is read"),
            });
        }

        // Title and table.
        let mut title = None;
        let mut table: Vec<(usize, &str)> = Vec::new();
        let mut table_closed = false;
        for (n, line) in rest {
            let line = line.trim();
            if line.starts_with('|') {
                if table_closed {
                    return Err(Error::at(n, "a scene has a single table"));
                }
                table.push((n, line));
            } else {
                if !table.is_empty() {
                    table_closed = true;
                }
                if title.is_none()
                    && let Some(t) = line.strip_prefix("# ")
                {
                    title = Some(t.trim().to_owned());
                }
            }
        }
        let title = title.ok_or_else(|| Error {
            line: None,
            reason: "missing the title (# …)".into(),
        })?;
        let table_cells: Vec<(usize, Vec<&str>)> =
            table.iter().map(|(n, r)| (*n, cells(r))).collect();
        let spans = read_spans(&table_cells)?;
        contiguous(&table, &spans)?;
        Ok(Self {
            format,
            title,
            cadence_s,
            dwell_s,
            senses,
            spans,
        })
    }
}

/// Reads the rows of a span table (Gherkin or Markdown): the first is the
/// header. Samples may start at any number: a Gherkin scenario can be set up
/// with a `Given` before the table.
pub fn spans(rows: &[&str]) -> Result<Vec<Span>, Error> {
    let numbered: Vec<(usize, Vec<&str>)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (i + 1, cells(r)))
        .collect();
    read_spans(&numbered)
}

/// The same, with the cells already split: a Gherkin step table as the
/// cucumber parser delivers it (`step.table.rows`).
pub fn spans_from_cells<S: AsRef<str>>(rows: &[Vec<S>]) -> Result<Vec<Span>, Error> {
    let numbered: Vec<(usize, Vec<&str>)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (i + 1, r.iter().map(|c| c.as_ref().trim()).collect()))
        .collect();
    read_spans(&numbered)
}

const SAMPLES: &str = "samples";
const ON_FLOOR: &str = "on floor";
const WHERE: &str = "where";
const IN_BED: &str = "in bed";
const REINFORCEMENT: &str = "reinforcement in bed";
const FACES: &str = "faces";
const OCCUPANTS: &str = "occupants";
const BED: &str = "bed";
const REQUIRED: [&str; 4] = [SAMPLES, ON_FLOOR, WHERE, IN_BED];
const OPTIONAL: [&str; 4] = [REINFORCEMENT, FACES, OCCUPANTS, BED];

fn cells(row: &str) -> Vec<&str> {
    row.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

/// `|---|:--:|`: the row that separates the header in Markdown.
fn is_separator(cells: &[&str]) -> bool {
    cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':')))
}

fn read_spans(rows: &[(usize, Vec<&str>)]) -> Result<Vec<Span>, Error> {
    let mut rows = rows.iter();
    let Some((header_line, columns)) = rows.next() else {
        return Err(Error {
            line: None,
            reason: "missing the span table".into(),
        });
    };
    let header_line = *header_line;
    for c in columns {
        if !REQUIRED.contains(c) && !OPTIONAL.contains(c) {
            return Err(Error::at(header_line, format!("unknown column: {c}")));
        }
    }
    for c in REQUIRED {
        if !columns.contains(&c) {
            return Err(Error::at(
                header_line,
                format!("missing the \"{c}\" column"),
            ));
        }
    }
    let mut spans = Vec::new();
    for (n, values) in rows {
        let n = *n;
        if is_separator(values) {
            continue;
        }
        if values.len() != columns.len() {
            return Err(Error::at(
                n,
                format!(
                    "the row has {} cells and the header {}",
                    values.len(),
                    columns.len()
                ),
            ));
        }
        let cell = |name: &str| {
            columns
                .iter()
                .position(|c| *c == name)
                .and_then(|i| values.get(i).copied())
        };
        let required = |name: &str| {
            cell(name).ok_or_else(|| Error::at(n, format!("missing the \"{name}\" cell")))
        };
        let span = Span {
            samples: samples(n, required(SAMPLES)?)?,
            floor: number(n, ON_FLOOR, required(ON_FLOOR)?)?,
            location: location(n, required(WHERE)?)?,
            bed: in_bed(n, required(IN_BED)?)?,
            reinforcement: match cell(REINFORCEMENT) {
                None | Some("") => 0,
                Some(v) => number(n, REINFORCEMENT, v)?,
            },
            faces: faces(n, cell(FACES).unwrap_or_default())?,
            expected: Expected {
                occupants: match cell(OCCUPANTS) {
                    None | Some("") => None,
                    Some(v) => Some(number(n, OCCUPANTS, v)?),
                },
                bed: expected_bed(n, cell(BED).unwrap_or_default())?,
            },
        };
        if let Location::Mixed { near } = span.location
            && (near == 0 || near >= span.floor)
        {
            return Err(Error::at(
                n,
                format!(
                    "\"{near} near bed\" with {} on the floor: use \"door\" or \"near bed\"",
                    span.floor
                ),
            ));
        }
        spans.push(span);
    }
    Ok(spans)
}

/// In a scene, the night starts at sample 1 and has no gaps.
fn contiguous(table: &[(usize, &str)], spans: &[Span]) -> Result<(), Error> {
    let lines: Vec<usize> = table
        .iter()
        .filter(|(_, r)| !is_separator(&cells(r)))
        .skip(1)
        .map(|(n, _)| *n)
        .collect();
    let mut next = 1;
    for (i, span) in spans.iter().enumerate() {
        if span.samples.from != next {
            let line = lines.get(i).copied().unwrap_or_default();
            return Err(Error::at(
                line,
                format!(
                    "the span starts at sample {} and sample {next} was expected",
                    span.samples.from
                ),
            ));
        }
        next = span.samples.to + 1;
    }
    Ok(())
}

fn number<T: std::str::FromStr>(n: usize, name: &str, value: &str) -> Result<T, Error> {
    value
        .parse()
        .map_err(|_| Error::at(n, format!("\"{name}\" is not a number: {value}")))
}

fn samples(n: usize, value: &str) -> Result<Samples, Error> {
    let (from, to) = match value.split_once('-') {
        Some((a, b)) => (number(n, SAMPLES, a.trim())?, number(n, SAMPLES, b.trim())?),
        None => {
            let m = number(n, SAMPLES, value)?;
            (m, m)
        }
    };
    if from == 0 || to < from {
        return Err(Error::at(n, format!("invalid samples: {value}")));
    }
    Ok(Samples { from, to })
}

fn location(n: usize, value: &str) -> Result<Location, Error> {
    match value {
        "" | "door" => Ok(Location::Door),
        "near bed" => Ok(Location::NearBed),
        other if other.ends_with(" near bed") => {
            let near = other.trim_end_matches(" near bed").trim();
            number(n, WHERE, near).map(|near| Location::Mixed { near })
        }
        other => Err(Error::at(n, format!("unknown \"where\": {other}"))),
    }
}

fn in_bed(n: usize, value: &str) -> Result<InBed, Error> {
    match value {
        "0 and 1" => Ok(InBed::Alternating),
        v => number(n, IN_BED, v).map(InBed::Fixed),
    }
}

fn faces(n: usize, value: &str) -> Result<Faces, Error> {
    match value {
        "" => Ok(Faces::None),
        "on subject" => Ok(Faces::OnSubject),
        "at headboard" => Ok(Faces::AtHeadboard),
        "weak" => Ok(Faces::Weak),
        other => Err(Error::at(n, format!("unknown \"faces\": {other}"))),
    }
}

fn expected_bed(n: usize, value: &str) -> Result<Option<ExpectedBed>, Error> {
    Ok(Some(match value {
        "" => return Ok(None),
        "empty" => ExpectedBed::Empty,
        "occupied" => ExpectedBed::Occupied,
        "occupied, seen" => ExpectedBed::Seen,
        "occupied, held" => ExpectedBed::Held,
        "unknown" => ExpectedBed::Unknown,
        other => return Err(Error::at(n, format!("unknown \"bed\": {other}"))),
    }))
}

/// Writes a scene in the language [`Scene::parse`] reads: what distilling
/// produces. Optional columns appear only if some span uses them.
pub fn write(scene: &Scene, description: &str) -> String {
    let mut text = format!(
        "---\nformat: {}\ncadence_s: {}\n",
        scene.format, scene.cadence_s
    );
    if let Some(d) = scene.dwell_s {
        text.push_str(&format!("dwell_s: {d}\n"));
    }
    if scene.senses != [Sense::Detect] {
        let names: Vec<&str> = scene
            .senses
            .iter()
            .map(|s| match s {
                Sense::Detect => "detect",
                Sense::Reinforce => "reinforce",
                Sense::Faces => "faces",
            })
            .collect();
        text.push_str(&format!("senses: {}\n", names.join(", ")));
    }
    text.push_str(&format!("---\n\n# {}\n\n", scene.title));
    if !description.is_empty() {
        text.push_str(description.trim_end());
        text.push_str("\n\n");
    }
    let reinforcement = scene.spans.iter().any(|s| s.reinforcement > 0);
    let faces = scene.spans.iter().any(|s| s.faces != Faces::None);
    let mut columns = vec![SAMPLES, ON_FLOOR, WHERE, IN_BED];
    columns.extend(reinforcement.then_some(REINFORCEMENT));
    columns.extend(faces.then_some(FACES));
    columns.extend([OCCUPANTS, BED]);
    let rows: Vec<Vec<String>> = scene
        .spans
        .iter()
        .map(|s| {
            let Samples { from, to } = s.samples;
            let mut r = vec![
                if from == to {
                    from.to_string()
                } else {
                    format!("{from}-{to}")
                },
                s.floor.to_string(),
                match s.location {
                    Location::Door if s.floor == 0 => String::new(),
                    Location::Door => "door".into(),
                    Location::NearBed => "near bed".into(),
                    Location::Mixed { near } => format!("{near} near bed"),
                },
                match s.bed {
                    InBed::Fixed(c) => c.to_string(),
                    InBed::Alternating => "0 and 1".into(),
                },
            ];
            if reinforcement {
                r.push(if s.reinforcement > 0 {
                    s.reinforcement.to_string()
                } else {
                    String::new()
                });
            }
            if faces {
                r.push(
                    match s.faces {
                        Faces::None => "",
                        Faces::OnSubject => "on subject",
                        Faces::AtHeadboard => "at headboard",
                        Faces::Weak => "weak",
                    }
                    .into(),
                );
            }
            r.push(
                s.expected
                    .occupants
                    .map_or_else(String::new, |o| o.to_string()),
            );
            r.push(
                match s.expected.bed {
                    None => "",
                    Some(ExpectedBed::Empty) => "empty",
                    Some(ExpectedBed::Occupied) => "occupied",
                    Some(ExpectedBed::Seen) => "occupied, seen",
                    Some(ExpectedBed::Held) => "occupied, held",
                    Some(ExpectedBed::Unknown) => "unknown",
                }
                .into(),
            );
            r
        })
        .collect();
    let widths: Vec<usize> = (0..columns.len())
        .map(|i| {
            rows.iter()
                .filter_map(|r| r.get(i))
                .map(|c| c.chars().count())
                .chain(columns.get(i).map(|c| c.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    let line = |cells: Vec<String>| {
        let parts: Vec<String> = cells
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{c}{}", " ".repeat(w.saturating_sub(c.chars().count()))))
            .collect();
        format!("| {} |\n", parts.join(" | "))
    };
    text.push_str(&line(columns.iter().map(|c| (*c).to_owned()).collect()));
    text.push_str(&format!(
        "|{}|\n",
        widths
            .iter()
            .map(|w| "-".repeat(w + 2))
            .collect::<Vec<_>>()
            .join("|")
    ));
    for r in rows {
        text.push_str(&line(r));
    }
    text
}
