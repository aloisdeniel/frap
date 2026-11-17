// SPDX-License-Identifier: GPL-3.0-or-later
//! The pretty-printed JSON layout of `scene.json` and `manifest.json`: two-space indent,
//! `"key": value`, and arrays holding only scalars kept on one line (`"location": [0.0, 1.0, 0.0]`),
//! so a diff of a changed vector or a small inline array is one line, not one per component.
//!
//! The value is first serialized compactly by `serde_json` (which writes `f32` fields in
//! their shortest round-trip spelling), then laid out by [`layout`], which only inserts
//! whitespace: every token keeps its exact bytes.

use serde::Serialize;

/// Serializes `value` in the `.frap` JSON layout, with a trailing newline.
pub fn to_json_pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    Ok(layout(&serde_json::to_vec(value)?))
}

/// Lays out compact JSON (as written by `serde_json::to_vec`: no insignificant whitespace).
#[must_use]
pub fn layout(src: &[u8]) -> Vec<u8> {
    fn newline(out: &mut Vec<u8>, indent: usize) {
        out.push(b'\n');
        out.resize(out.len() + 2 * indent, b' ');
    }

    let mut out = Vec::with_capacity(src.len() + src.len() / 2);
    let mut indent = 0usize;
    let mut i = 0;
    while i < src.len() {
        let c = src[i];
        match c {
            b'"' => {
                let end = string_end(src, i);
                out.extend_from_slice(&src[i..end]);
                i = end;
            }
            b'{' | b'[' => {
                let close = if c == b'{' { b'}' } else { b']' };
                if src.get(i + 1) == Some(&close) {
                    out.extend_from_slice(&[c, close]);
                    i += 2;
                } else if let Some(end) = (c == b'[').then(|| scalar_array_end(src, i)).flatten() {
                    out.push(b'[');
                    let mut j = i + 1;
                    while j < end {
                        match src[j] {
                            b'"' => {
                                let e = string_end(src, j);
                                out.extend_from_slice(&src[j..e]);
                                j = e;
                            }
                            b',' => {
                                out.extend_from_slice(b", ");
                                j += 1;
                            }
                            b => {
                                out.push(b);
                                j += 1;
                            }
                        }
                    }
                    out.push(b']');
                    i = end + 1;
                } else {
                    out.push(c);
                    indent += 1;
                    newline(&mut out, indent);
                    i += 1;
                }
            }
            b'}' | b']' => {
                indent = indent.saturating_sub(1);
                newline(&mut out, indent);
                out.push(c);
                i += 1;
            }
            b',' => {
                out.push(b',');
                newline(&mut out, indent);
                i += 1;
            }
            b':' => {
                out.extend_from_slice(b": ");
                i += 1;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out.push(b'\n');
    out
}

/// Index just past the string starting at `src[start] == b'"'` (or `src.len()` if unterminated).
fn string_end(src: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    while j < src.len() {
        match src[j] {
            b'\\' => j += 2,
            b'"' => return j + 1,
            _ => j += 1,
        }
    }
    src.len()
}

/// Index of the `]` closing the array at `src[start] == b'['` if it holds no array or object.
fn scalar_array_end(src: &[u8], start: usize) -> Option<usize> {
    let mut j = start + 1;
    while j < src.len() {
        match src[j] {
            b'"' => j = string_end(src, j),
            b'[' | b'{' => return None,
            b']' => return Some(j),
            _ => j += 1,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn layout_shape() {
        let v = json!({
            "location": [0.0, 1.5, -2.0],
            "empty": [], "obj": {}, "names": ["a,b", "c]\"d", "[{"],
            "faces": [[0, 1, 2], [2, 3, 0]],
            "nested": {"k": [{"x": 1}], "s": "a:b,{c}"},
            "f32": 0.1f32,
        });
        let text = String::from_utf8(to_json_pretty(&v).unwrap()).unwrap();
        assert_eq!(
            text,
            r#"{
  "location": [0.0, 1.5, -2.0],
  "empty": [],
  "obj": {},
  "names": ["a,b", "c]\"d", "[{"],
  "faces": [
    [0, 1, 2],
    [2, 3, 0]
  ],
  "nested": {
    "k": [
      {
        "x": 1
      }
    ],
    "s": "a:b,{c}"
  },
  "f32": 0.10000000149011612
}
"#
        );
        // f32 struct fields keep their shortest spelling (serialized directly, not via Value).
        #[derive(Serialize)]
        struct S {
            x: f32,
            v: [f32; 2],
        }
        assert_eq!(
            String::from_utf8(
                to_json_pretty(&S {
                    x: 0.1,
                    v: [0.2, 1e-7]
                })
                .unwrap()
            )
            .unwrap(),
            "{\n  \"x\": 0.1,\n  \"v\": [0.2, 1e-7]\n}\n"
        );
        assert_eq!(to_json_pretty(&json!([])).unwrap(), b"[]\n");
        assert_eq!(to_json_pretty(&json!("x")).unwrap(), b"\"x\"\n");
    }

    fn arb_json() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            Just(Value::Null),
            any::<bool>().prop_map(Value::Bool),
            any::<i64>().prop_map(Value::from),
            any::<f64>()
                .prop_filter("finite", |f| f.is_finite())
                .prop_map(Value::from),
            "[ -~\u{e9}\u{1F600}\\\\\"]{0,8}".prop_map(Value::String),
        ];
        leaf.prop_recursive(4, 64, 6, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
                proptest::collection::vec(("[a-z:,\\[\\]{}\"]{0,5}", inner), 0..6)
                    .prop_map(|kv| Value::Object(kv.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn layout_only_adds_whitespace(v in arb_json()) {
            let pretty = to_json_pretty(&v).unwrap();
            let back: Value = serde_json::from_slice(&pretty).unwrap();
            prop_assert_eq!(&back, &v);
            prop_assert_eq!(serde_json::to_vec(&back).unwrap(), serde_json::to_vec(&v).unwrap());
            prop_assert!(pretty.ends_with(b"\n"));
        }
    }
}
