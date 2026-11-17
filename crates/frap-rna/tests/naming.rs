// SPDX-License-Identifier: GPL-3.0-or-later
//! `unique_name` / `unique_id_name` / `unique_subname` reproduce every case verified on
//! Blender 5.2.4 (`bpy.data.meshes.new(name)` for IDs, `object.modifiers.new(name, …)` and
//! `vertex_groups.new(name=…)` for sub-data, `-b --factory-startup`). Each case replays the
//! creations in order and compares every resulting name.

use std::cell::RefCell;

use frap_rna::naming::{unique_id_name, unique_name, unique_subname};

const ID_MAX: usize = 255;
const SUB_MAX: usize = 63;

/// A namespace that applies one of the algorithms on every `new`.
struct Space {
    names: RefCell<Vec<String>>,
    exact: bool,
}

impl Space {
    fn new(exact: bool) -> Self {
        Space {
            names: RefCell::new(Vec::new()),
            exact,
        }
    }
    fn new_id(&self, req: &str, default: &str) -> String {
        let got = if self.exact {
            let names = self.names.borrow();
            unique_id_name(names.iter().map(String::as_str), req, ID_MAX, default).to_string()
        } else {
            let names = self.names.borrow();
            unique_name(&|n| names.iter().any(|x| x == n), req, ID_MAX, default).to_string()
        };
        self.names.borrow_mut().push(got.clone());
        got
    }
    fn new_sub(&self, req: &str, default: &str) -> String {
        let got = {
            let names = self.names.borrow();
            unique_subname(&|n| names.iter().any(|x| x == n), req, SUB_MAX, default).to_string()
        };
        self.names.borrow_mut().push(got.clone());
        got
    }
    fn remove(&self, name: &str) {
        self.names.borrow_mut().retain(|n| n != name);
    }
}

fn id_cases(exact: bool) {
    let sp = Space::new(exact);
    let new = |n: &str| sp.new_id(n, "Mesh");
    assert_eq!(
        [new("Foo"), new("Foo"), new("Foo")],
        ["Foo", "Foo.001", "Foo.002"]
    );
    assert_eq!(new("Foo.001"), "Foo.003");
    assert_eq!([new("Bar.5"), new("Bar.5")], ["Bar.5", "Bar.001"]);
    assert_eq!([new("Baz.999"), new("Baz.999")], ["Baz.999", "Baz.001"]);
    assert_eq!([new(""), new("")], ["Mesh", "Mesh.001"]);
    // `Qux.1` occupies number 1 of base `Qux`.
    assert_eq!(
        [new("Qux"), new("Qux.1"), new("Qux")],
        ["Qux", "Qux.1", "Qux.002"]
    );
    assert_eq!([new("Q2.0001"), new("Q2.0001")], ["Q2.0001", "Q2.002"]);
    // Too long with a suffix: one character is dropped from the base, which is free.
    let a255 = "A".repeat(255);
    assert_eq!(new(&a255), a255);
    assert_eq!(new(&a255), "A".repeat(254));
    let b253 = "B".repeat(253);
    assert_eq!(new(&b253), b253);
    assert_eq!(new(&b253), "B".repeat(252));
    let c251 = "C".repeat(251);
    assert_eq!(new(&c251), c251);
    assert_eq!(new(&c251), format!("{c251}.001"));
    // UTF-8: 128 × "é" (256 bytes) is truncated to 127 characters.
    let e = new(&"é".repeat(128));
    assert_eq!((e.chars().count(), e.len()), (127, 254));
    // `é` × 127 now exists: a new one becomes 126 × `é`, the next one 125 × `é`.
    assert_eq!(new(&"é".repeat(127)).len(), 252);
    assert_eq!(new(&"é".repeat(127)).len(), 250);
    // Suffixes that are not numbers.
    assert_eq!([new("Dot."), new("Dot.")], ["Dot.", "Dot..001"]);
    assert_eq!([new("Abc.abc"), new("Abc.abc")], ["Abc.abc", "Abc.abc.001"]);
    assert_eq!(
        [new("Ov.12345678901"), new("Ov.12345678901")],
        ["Ov.12345678901", "Ov.12345678901.001"]
    );
    assert_eq!([new("Num.1500"), new("Num.1500")], ["Num.1500", "Num.001"]);
    assert_eq!(
        [new("Big"), new("Big.5000"), new("Big")],
        ["Big", "Big.5000", "Big.001"]
    );
    // A removed name frees its number.
    sp.remove("Foo.001");
    assert_eq!(new("Foo"), "Foo.001");
    // Past .999 the suffix grows to four digits.
    let many: Vec<String> = (0..1100).map(|_| new("Many")).collect();
    assert_eq!(many[999], "Many.999");
    assert_eq!(many[1000], "Many.1000");
    assert_eq!(many[1001], "Many.1001");
    assert_eq!(many[1099], "Many.1099");
}

#[test]
fn id_names_exact() {
    id_cases(true);
}

#[test]
fn id_names_from_predicate() {
    id_cases(false);
}

#[test]
fn renaming_to_a_taken_name() {
    // `m2.name = "R1"` while `R1` exists: the assigned ID becomes `R1.001`; renaming it to
    // `R1` again keeps `R1.001`; an empty name becomes the type default.
    let existing = ["R1", "Mesh", "Mesh.001"];
    assert_eq!(unique_id_name(existing, "R1", ID_MAX, "Mesh"), "R1.001");
    let existing = ["R1", "Mesh", "Mesh.001"];
    assert_eq!(unique_id_name(existing, "", ID_MAX, "Mesh"), "Mesh.002");
}

#[test]
fn subdata_names() {
    let sp = Space::new(false);
    let new = |n: &str, d: &str| sp.new_sub(n, d);
    assert_eq!(
        [
            new("Bevel", "Bevel"),
            new("Bevel", "Bevel"),
            new("Bevel", "Bevel")
        ],
        ["Bevel", "Bevel.001", "Bevel.002"]
    );
    // Counting continues from the requested number.
    assert_eq!(
        [new("Bevel.005", "Bevel"), new("Bevel.005", "Bevel")],
        ["Bevel.005", "Bevel.006"]
    );
    sp.remove("Bevel.001");
    assert_eq!(new("Bevel", "Bevel"), "Bevel.001");
    assert_eq!(
        [new("", "Subdivision"), new("", "Subdivision")],
        ["Subdivision", "Subdivision.001"]
    );
    // 63-byte limit: the base is cut so that base + ".001" fits.
    let m63 = "M".repeat(63);
    assert_eq!(new(&m63, "Bevel"), m63);
    let b = new(&m63, "Bevel");
    assert_eq!((b.len(), &b[b.len() - 6..]), (63, "MM.001"));
    // 40 × "é" is cut to 31 characters (62 bytes); duplicates cut the base on a character.
    let e40 = new(&"é".repeat(40), "Bevel");
    assert_eq!(e40.len(), 62);
    let e31 = "é".repeat(31);
    let a = new(&e31, "Bevel");
    let b = new(&e31, "Bevel");
    assert_eq!((a.len(), b.len()), (62, 62));
    assert_eq!(a, format!("{}.001", "é".repeat(29)));
    assert_eq!(b, format!("{}.002", "é".repeat(29)));
    assert_eq!(
        [
            new("Ov.99999999999", "Bevel"),
            new("Ov.99999999999", "Bevel")
        ],
        ["Ov.99999999999", "Ov.99999999999.001"]
    );
    // Vertex groups use the same rule with default "Group".
    let vg = Space::new(false);
    assert_eq!(
        [
            vg.new_sub("G", "Group"),
            vg.new_sub("G", "Group"),
            vg.new_sub("", "Group")
        ],
        ["G", "G.001", "Group"]
    );
}
