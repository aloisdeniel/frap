// SPDX-License-Identifier: GPL-3.0-or-later
//! Golden case format: header directives, `#@frap` / `#@blender` lines (§20.2). WP 1.8.
//!
//! A case is a Python file that both tools run. The header is one or more comment lines
//! `# golden: key=value key=value …` before the first statement:
//!
//! | Key | Values | Default |
//! |---|---|---|
//! | `startup` | `factory` \| `empty` | `factory` |
//! | `compare` | comma list of `objects`, `mesh:<name>`, `evaluated:<name>`, `materials`, `anim:<name>`, `glb:<path>`, `image:<path>`, `context` | (line outcomes only) |
//! | `tier` | `T0` … `T4` | `T0` |
//! | `tol` | float | `1e-5` |
//! | `order` | `exact` \| `canonical` (mesh comparator override) | from the tier |
//! | `ssim` | minimum SSIM for `image:` (T4) | `0.995` |
//! | `check` | T3 invariants, comma list (see [`crate::compare::invariants`]) | none |
//! | `skip_blender` | flag: frap-only case, never blessed | off |
//! | `glb_structure` | flag: `glb:` also compares the numbers of accessors and buffer views | off |
//!
//! Line outcomes are always compared. Lines prefixed `#@frap` run only in frap, `#@blender`
//! only in Blender; both keep their line numbers, so the two sides report the same `line` for
//! every shared statement.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

/// The default float tolerance (§20.2).
pub const DEFAULT_TOL: f32 = 1e-5;
/// The default SSIM threshold for `image:` comparisons against blessed images (§20.4).
pub const DEFAULT_SSIM: f64 = 0.995;

/// Initial state of a case.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Startup {
    /// `blender -b --factory-startup` / frap's factory startup.
    #[default]
    Factory,
    /// `wm.read_homefile(use_empty=True)`.
    Empty,
}

impl Startup {
    /// Header spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Startup::Factory => "factory",
            Startup::Empty => "empty",
        }
    }
}

/// Fidelity tier (ARCHITECTURE §10.1, §20.4).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    /// Line outcomes and RNA values.
    #[default]
    T0,
    /// Index-exact meshes.
    T1,
    /// Canonical (order-free) meshes.
    T2,
    /// Invariants.
    T3,
    /// Images.
    T4,
}

impl Tier {
    /// `T0` … `T4`.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::T0 => "T0",
            Tier::T1 => "T1",
            Tier::T2 => "T2",
            Tier::T3 => "T3",
            Tier::T4 => "T4",
        }
    }
    fn parse(s: &str) -> Option<Tier> {
        Some(match s {
            "T0" => Tier::T0,
            "T1" => Tier::T1,
            "T2" => Tier::T2,
            "T3" => Tier::T3,
            "T4" => Tier::T4,
            _ => return None,
        })
    }
}

/// Mesh comparison order.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Order {
    /// Index by index (`mesh_exact`).
    Exact,
    /// Order-free (`mesh_canonical`).
    Canonical,
}

/// One `compare=` item.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CompareSpec {
    /// Every object (`rna_values`).
    Objects,
    /// One original mesh by datablock name.
    Mesh(String),
    /// One evaluated mesh by object name.
    Evaluated(String),
    /// Every material's node graph (`rna_values`; `Dots Stroke` excluded, D-11).
    Materials,
    /// One action (M6).
    Anim(String),
    /// A glTF/GLB output file, case-relative (`gltf_canonical`).
    Glb(String),
    /// An image output file, case-relative (`image_ssim`).
    Image(String),
    /// The context (`rna_values`).
    Context,
}

impl CompareSpec {
    fn parse(s: &str) -> Option<CompareSpec> {
        let (k, v) = match s.split_once(':') {
            Some((k, v)) => (k, Some(v)),
            None => (s, None),
        };
        let named = |v: Option<&str>| v.filter(|v| !v.is_empty()).map(str::to_owned);
        Some(match (k, v) {
            ("objects", None) => CompareSpec::Objects,
            ("materials", None) => CompareSpec::Materials,
            ("context", None) => CompareSpec::Context,
            ("mesh", v) => CompareSpec::Mesh(named(v)?),
            ("evaluated", v) => CompareSpec::Evaluated(named(v)?),
            ("anim", v) => CompareSpec::Anim(named(v)?),
            ("glb", v) => CompareSpec::Glb(named(v)?),
            ("image", v) => CompareSpec::Image(named(v)?),
            _ => return None,
        })
    }
    /// Header spelling (`mesh:Cube`).
    pub fn to_header(&self) -> String {
        match self {
            CompareSpec::Objects => "objects".into(),
            CompareSpec::Materials => "materials".into(),
            CompareSpec::Context => "context".into(),
            CompareSpec::Mesh(n) => format!("mesh:{n}"),
            CompareSpec::Evaluated(n) => format!("evaluated:{n}"),
            CompareSpec::Anim(n) => format!("anim:{n}"),
            CompareSpec::Glb(n) => format!("glb:{n}"),
            CompareSpec::Image(n) => format!("image:{n}"),
        }
    }
}

/// Which tool runs a line.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Side {
    /// Both (the normal case).
    Both,
    /// `#@frap` lines.
    Frap,
    /// `#@blender` lines.
    Blender,
}

/// One physical source line of a case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseLine {
    /// 1-based line number.
    pub line: u32,
    /// Who runs it.
    pub side: Side,
    /// The text without its `#@…` prefix.
    pub text: String,
}

/// A parse error in a case file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseError {
    /// The file.
    pub path: PathBuf,
    /// 1-based line (0 when not line-specific).
    pub line: u32,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for CaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.path.display(), self.line, self.message)
    }
}

impl std::error::Error for CaseError {}

/// One comparison failure.
#[derive(Clone, Debug, PartialEq)]
pub struct Mismatch {
    /// Comparator (`lines`, `rna_values`, `mesh_exact`, …).
    pub comparator: &'static str,
    /// Where (`objects['Cube'].location[2]`, `line 4`).
    pub path: String,
    /// What differs (`frap 1.0 != blender 2.0`).
    pub detail: String,
}

impl Mismatch {
    /// New mismatch.
    pub fn new(
        comparator: &'static str,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Mismatch {
            comparator,
            path: path.into(),
            detail: detail.into(),
        }
    }
}

impl fmt::Display for Mismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.comparator, self.path, self.detail)
    }
}

/// A parsed case.
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// The file.
    pub path: PathBuf,
    /// Directory relative to the cases root (`primitives/cube`, `/`-separated; empty at the
    /// root).
    pub area: String,
    /// File stem (`cube_add`).
    pub name: String,
    /// `startup=`.
    pub startup: Startup,
    /// `compare=`.
    pub compare: Vec<CompareSpec>,
    /// `tier=`.
    pub tier: Tier,
    /// `tol=`.
    pub tol: f32,
    /// `order=` (None: from the tier).
    pub order: Option<Order>,
    /// `ssim=`.
    pub ssim: f64,
    /// `check=` (T3 invariants).
    pub checks: Vec<String>,
    /// `skip_blender`.
    pub skip_blender: bool,
    /// `glb_structure`: `glb:` comparisons keep the canonical `structure` entry (accessor and
    /// buffer-view counts: data the add-on shares by content must be shared).
    pub glb_structure: bool,
    /// Every physical line (comments included).
    pub lines: Vec<CaseLine>,
}

impl Case {
    /// Reads and parses a case file. `cases_root` gives the area (`<root>/<area>/<name>.py`).
    pub fn load(path: &Path, cases_root: &Path) -> Result<Case, CaseError> {
        let text = std::fs::read_to_string(path).map_err(|e| CaseError {
            path: path.to_owned(),
            line: 0,
            message: format!("cannot read: {e}"),
        })?;
        let mut case = Case::parse(path, &text)?;
        if let Ok(rel) = path.strip_prefix(cases_root)
            && let Some(parent) = rel.parent()
        {
            case.area = parent
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
        }
        Ok(case)
    }

    /// Parses case text. The area is empty; [`Case::load`] fills it.
    pub fn parse(path: &Path, text: &str) -> Result<Case, CaseError> {
        let err = |line: u32, message: String| CaseError {
            path: path.to_owned(),
            line,
            message,
        };
        let mut case = Case {
            path: path.to_owned(),
            area: String::new(),
            name: path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            startup: Startup::Factory,
            compare: Vec::new(),
            tier: Tier::T0,
            tol: DEFAULT_TOL,
            order: None,
            ssim: DEFAULT_SSIM,
            checks: Vec::new(),
            skip_blender: false,
            glb_structure: false,
            lines: Vec::new(),
        };
        let mut seen_header = false;
        let mut in_header_block = true;
        for (i, raw) in text.lines().enumerate() {
            let line = i as u32 + 1;
            let trimmed = raw.trim_start();
            let (side, body) = if let Some(rest) = trimmed.strip_prefix("#@frap") {
                (Side::Frap, strip_one_space(rest))
            } else if let Some(rest) = trimmed.strip_prefix("#@blender") {
                (Side::Blender, strip_one_space(rest))
            } else {
                (Side::Both, raw)
            };
            if side == Side::Both {
                if let Some(h) = trimmed.strip_prefix("# golden:") {
                    if !in_header_block {
                        return Err(err(
                            line,
                            "`# golden:` header after the first statement".into(),
                        ));
                    }
                    seen_header = true;
                    case.apply_header(h).map_err(|m| err(line, m))?;
                } else if !(trimmed.is_empty() || trimmed.starts_with('#')) {
                    in_header_block = false;
                }
            } else {
                in_header_block = false;
            }
            case.lines.push(CaseLine {
                line,
                side,
                text: body.to_owned(),
            });
        }
        if !seen_header {
            return Err(err(0, "missing `# golden:` header".into()));
        }
        Ok(case)
    }

    fn apply_header(&mut self, h: &str) -> Result<(), String> {
        for tok in h.split_whitespace() {
            let (k, v) = match tok.split_once('=') {
                Some((k, v)) => (k, Some(v)),
                None => (tok, None),
            };
            match (k, v) {
                ("skip_blender", None) | ("skip_blender", Some("1" | "true")) => {
                    self.skip_blender = true
                }
                ("glb_structure", None) | ("glb_structure", Some("1" | "true")) => {
                    self.glb_structure = true
                }
                ("startup", Some("factory")) => self.startup = Startup::Factory,
                ("startup", Some("empty")) => self.startup = Startup::Empty,
                ("compare", Some(list)) => {
                    for item in list.split(',').filter(|s| !s.is_empty()) {
                        let spec = CompareSpec::parse(item)
                            .ok_or_else(|| format!("unknown compare item `{item}`"))?;
                        if !self.compare.contains(&spec) {
                            self.compare.push(spec);
                        }
                    }
                }
                ("tier", Some(t)) => {
                    self.tier = Tier::parse(t).ok_or_else(|| format!("unknown tier `{t}`"))?
                }
                ("tol", Some(t)) => {
                    self.tol = t
                        .parse::<f32>()
                        .ok()
                        .filter(|t| t.is_finite() && *t >= 0.0)
                        .ok_or_else(|| format!("bad tol `{t}`"))?
                }
                ("order", Some("exact")) => self.order = Some(Order::Exact),
                ("order", Some("canonical")) => self.order = Some(Order::Canonical),
                ("ssim", Some(t)) => {
                    self.ssim = t
                        .parse::<f64>()
                        .ok()
                        .filter(|t| (0.0..=1.0).contains(t))
                        .ok_or_else(|| format!("bad ssim `{t}`"))?
                }
                ("check", Some(list)) => self
                    .checks
                    .extend(list.split(',').filter(|s| !s.is_empty()).map(str::to_owned)),
                _ => return Err(format!("unknown header directive `{tok}`")),
            }
        }
        Ok(())
    }

    /// The mesh comparison order: `order=` if given, else exact up to T1, canonical above.
    pub fn mesh_order(&self) -> Order {
        self.order.unwrap_or(if self.tier <= Tier::T1 {
            Order::Exact
        } else {
            Order::Canonical
        })
    }

    /// The source one tool runs: lines of the other tool become empty lines, so line numbers
    /// stay identical on both sides.
    pub fn source_for(&self, side: Side) -> String {
        let mut out = String::new();
        for l in &self.lines {
            if l.side == Side::Both || l.side == side {
                out.push_str(&l.text);
            }
            out.push('\n');
        }
        out
    }

    /// Lines that only one tool runs (their outcomes are checked for success on that side
    /// only).
    pub fn side_only_lines(&self) -> BTreeSet<u32> {
        self.lines
            .iter()
            .filter(|l| l.side != Side::Both)
            .map(|l| l.line)
            .collect()
    }

    /// `<area>/<name>` (the case id used by filters and fixture paths).
    pub fn id(&self) -> String {
        if self.area.is_empty() {
            self.name.clone()
        } else {
            format!("{}/{}", self.area, self.name)
        }
    }

    /// `<fixtures_root>/<area>/<name>.blender.json` (§20.5).
    pub fn fixture_path(&self, fixtures_root: &Path) -> PathBuf {
        let mut p = fixtures_root.to_owned();
        for c in self.area.split('/').filter(|c| !c.is_empty()) {
            p.push(c);
        }
        p.push(format!("{}.blender.json", self.name));
        p
    }

    /// The header line that reproduces this case's directives.
    pub fn header(&self) -> String {
        let mut h = format!("# golden: startup={}", self.startup.as_str());
        if !self.compare.is_empty() {
            let items: Vec<String> = self.compare.iter().map(CompareSpec::to_header).collect();
            h.push_str(&format!(" compare={}", items.join(",")));
        }
        h.push_str(&format!(" tier={} tol={}", self.tier.as_str(), self.tol));
        if let Some(o) = self.order {
            h.push_str(match o {
                Order::Exact => " order=exact",
                Order::Canonical => " order=canonical",
            });
        }
        if self.ssim != DEFAULT_SSIM {
            h.push_str(&format!(" ssim={}", self.ssim));
        }
        if !self.checks.is_empty() {
            h.push_str(&format!(" check={}", self.checks.join(",")));
        }
        if self.skip_blender {
            h.push_str(" skip_blender");
        }
        if self.glb_structure {
            h.push_str(" glb_structure");
        }
        h
    }

    /// Operators called by this case (`mesh.primitive_cube_add`), in first-use order, from
    /// lines both tools run (for the coverage report).
    pub fn operators(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in self.lines.iter().filter(|l| l.side == Side::Both) {
            let code = l.text.split('#').next().unwrap_or("");
            let mut rest = code;
            while let Some(i) = rest.find("bpy.ops.") {
                rest = &rest[i + "bpy.ops.".len()..];
                let id: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
                    .collect();
                let id = id.trim_end_matches('.');
                if id.split('.').count() == 2 && !out.iter().any(|o| o == id) {
                    out.push(id.to_owned());
                }
            }
        }
        out
    }
}

fn strip_one_space(s: &str) -> &str {
    s.strip_prefix(' ').unwrap_or(s).trim_start()
}

/// Every `*.py` case below `cases_root` (recursive, sorted by path), optionally filtered by a
/// glob over the case id (`primitives/*`, `**/cube*`).
pub fn discover(cases_root: &Path, filter: Option<&str>) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "py") {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(cases_root, &mut out);
    out.sort();
    if let Some(f) = filter {
        out.retain(|p| {
            let id = p
                .strip_prefix(cases_root)
                .unwrap_or(p)
                .with_extension("")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            glob_match(f, &id)
        });
    }
    out
}

/// Glob match: `*` (any run without `/`), `**` (any run), `?` (one character).
pub fn glob_match(pattern: &str, text: &str) -> bool {
    fn go(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') if p.get(1) == Some(&'*') => {
                let rest = &p[2..];
                let rest = rest.strip_prefix(&['/']).unwrap_or(rest);
                (0..=t.len()).any(|i| go(rest, &t[i..]))
            }
            Some('*') => {
                let mut i = 0;
                loop {
                    if go(&p[1..], &t[i..]) {
                        return true;
                    }
                    if i == t.len() || t[i] == '/' {
                        return false;
                    }
                    i += 1;
                }
            }
            Some('?') => !t.is_empty() && t[0] != '/' && go(&p[1..], &t[1..]),
            Some(c) => t.first() == Some(c) && go(&p[1..], &t[1..]),
        }
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    go(&p, &t)
}

/// Coverage report (`cargo xtask golden report`, §20.5): operator × tier matrix over the given
/// cases, with the number of cases per cell and whether every case is blessed.
pub fn coverage_report(cases: &[Case], fixtures_root: &Path) -> String {
    use std::collections::BTreeMap;
    let tiers = [Tier::T0, Tier::T1, Tier::T2, Tier::T3, Tier::T4];
    let mut m: BTreeMap<String, [usize; 5]> = BTreeMap::new();
    let mut unblessed = Vec::new();
    for c in cases {
        if !c.skip_blender && !c.fixture_path(fixtures_root).is_file() {
            unblessed.push(c.id());
        }
        for op in c.operators() {
            m.entry(op).or_default()[c.tier as usize] += 1;
        }
    }
    let mut s = String::from("| operator |");
    for t in tiers {
        s.push_str(&format!(" {} |", t.as_str()));
    }
    s.push_str("\n|---|---|---|---|---|---|\n");
    for (op, counts) in &m {
        s.push_str(&format!("| `{op}` |"));
        for n in counts {
            if *n == 0 {
                s.push_str(" |");
            } else {
                s.push_str(&format!(" {n} |"));
            }
        }
        s.push('\n');
    }
    s.push_str(&format!(
        "\n{} cases, {} operators, {} not blessed{}\n",
        cases.len(),
        m.len(),
        unblessed.len(),
        if unblessed.is_empty() {
            String::new()
        } else {
            format!(": {}", unblessed.join(", "))
        }
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "\
# tests/golden/cases/edit/extrude_region_move_top.py
# golden: startup=factory compare=objects,mesh:Cube tier=T1 tol=1e-5
import bpy
bpy.ops.mesh.primitive_cube_add()
#@frap    bpy.ops.frap.select_elements(type='FACE', indices=[5])
#@blender import bmesh; bm=bmesh.from_edit_mesh(bpy.context.object.data)
bpy.ops.mesh.primitive_cube_add(sizee=2)        # expected TypeError on both sides (T0)
";

    #[test]
    fn parses_the_architecture_example() {
        let c = Case::parse(Path::new("x/extrude.py"), EXAMPLE).unwrap();
        assert_eq!(c.startup, Startup::Factory);
        assert_eq!(
            c.compare,
            vec![CompareSpec::Objects, CompareSpec::Mesh("Cube".into())]
        );
        assert_eq!(c.tier, Tier::T1);
        assert_eq!(c.tol, 1e-5);
        assert_eq!(c.mesh_order(), Order::Exact);
        assert_eq!(c.name, "extrude");
        assert_eq!(c.side_only_lines(), BTreeSet::from([5, 6]));
        let f = c.source_for(Side::Frap);
        let b = c.source_for(Side::Blender);
        assert_eq!(f.lines().count(), 7);
        assert_eq!(b.lines().count(), 7);
        assert_eq!(
            f.lines().nth(4).unwrap(),
            "bpy.ops.frap.select_elements(type='FACE', indices=[5])"
        );
        assert_eq!(f.lines().nth(5).unwrap(), "");
        assert_eq!(b.lines().nth(4).unwrap(), "");
        assert!(b.lines().nth(5).unwrap().starts_with("import bmesh;"));
        assert_eq!(c.operators(), vec!["mesh.primitive_cube_add".to_string()]);
    }

    #[test]
    fn header_directives() {
        let c = Case::parse(
            Path::new("a.py"),
            "# golden: startup=empty compare=glb:out/a.glb,image:o.png,context,evaluated:Cube\n\
             # golden: tier=T2 order=exact ssim=0.97 check=manifold,euler:2 skip_blender glb_structure\nimport bpy\n",
        )
        .unwrap();
        assert_eq!(c.startup, Startup::Empty);
        assert_eq!(c.tier, Tier::T2);
        assert_eq!(c.mesh_order(), Order::Exact);
        assert_eq!(c.ssim, 0.97);
        assert_eq!(c.checks, vec!["manifold", "euler:2"]);
        assert!(c.skip_blender);
        assert!(c.glb_structure);
        assert_eq!(
            c.compare,
            vec![
                CompareSpec::Glb("out/a.glb".into()),
                CompareSpec::Image("o.png".into()),
                CompareSpec::Context,
                CompareSpec::Evaluated("Cube".into()),
            ]
        );
        // The header round-trips.
        let again = Case::parse(Path::new("a.py"), &format!("{}\n", c.header())).unwrap();
        assert_eq!(again.compare, c.compare);
        assert_eq!(again.tier, c.tier);
        assert_eq!(again.skip_blender, c.skip_blender);
        assert_eq!(again.glb_structure, c.glb_structure);
    }

    #[test]
    fn header_errors() {
        for bad in [
            "import bpy\n",
            "# golden: compare=nope\n",
            "# golden: tier=T9\n",
            "# golden: tol=abc\n",
            "# golden: frobnicate=1\n",
            "# golden: compare=mesh:\n",
            "# golden: tier=T0\nimport bpy\n# golden: tol=1\n",
        ] {
            assert!(Case::parse(Path::new("a.py"), bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn default_mesh_order_follows_tier() {
        let c = Case::parse(Path::new("a.py"), "# golden: tier=T2\n").unwrap();
        assert_eq!(c.mesh_order(), Order::Canonical);
        let c = Case::parse(Path::new("a.py"), "# golden: tier=T2 order=exact\n").unwrap();
        assert_eq!(c.mesh_order(), Order::Exact);
    }

    #[test]
    fn fixture_paths_and_ids() {
        let root = Path::new("/r/cases");
        let c = Case {
            area: "primitives/cube".into(),
            ..Case::parse(Path::new("/r/cases/primitives/cube/size.py"), "# golden:\n").unwrap()
        };
        assert_eq!(c.id(), "primitives/cube/size");
        assert_eq!(
            c.fixture_path(Path::new("/r/fixtures")),
            Path::new("/r/fixtures/primitives/cube/size.blender.json")
        );
        let _ = root;
    }

    #[test]
    fn globs() {
        assert!(glob_match("primitives/*", "primitives/cube"));
        assert!(!glob_match("primitives/*", "primitives/cube/size"));
        assert!(glob_match("primitives/**", "primitives/cube/size"));
        assert!(glob_match("**/size", "primitives/cube/size"));
        assert!(glob_match("**", "a"));
        assert!(glob_match("harness/cube_?dd", "harness/cube_add"));
        assert!(!glob_match("harness/x", "harness/cube_add"));
        assert!(glob_match("*cube*", "cube_add"));
    }
}
