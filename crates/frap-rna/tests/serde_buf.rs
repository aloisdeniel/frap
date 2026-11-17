// SPDX-License-Identifier: GPL-3.0-or-later
//! `serde_buf` round-trips (§15): inline without a sink, `$buf` references with a sink and a
//! source, the sink's inline choice, `Option` fields, and error cases.

use std::collections::HashMap;
use std::sync::Arc;

use frap_rna::serde_buf::{self, BufferSink, BufferSource, with_sink, with_source};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Mesh {
    #[serde(with = "frap_rna::serde_buf")]
    positions: Arc<Vec<[f32; 3]>>,
    #[serde(with = "frap_rna::serde_buf")]
    offsets: Arc<Vec<i32>>,
    #[serde(with = "frap_rna::serde_buf")]
    sharp: Arc<Vec<bool>>,
    #[serde(with = "frap_rna::serde_buf::option")]
    uv: Option<Arc<Vec<[f32; 2]>>>,
    #[serde(with = "frap_rna::serde_buf::option")]
    none: Option<Arc<Vec<u8>>>,
}

fn sample() -> Mesh {
    Mesh {
        positions: Arc::new((0..100).map(|i| [i as f32, -0.5, 1e-7]).collect()),
        offsets: Arc::new(vec![0, 4, 8, -3]),
        sharp: Arc::new(vec![true, false, true]),
        uv: Some(Arc::new((0..70).map(|i| [i as f32 * 0.25, 1.0]).collect())),
        none: None,
    }
}

/// Like frap-project's sink: content-addressed ids, arrays shorter than 64 items inline.
#[derive(Default)]
struct Sink {
    bufs: HashMap<String, Vec<u8>>,
    seen: Vec<(String, usize)>,
}

impl BufferSink for Sink {
    fn put(&mut self, dtype: &str, len: usize, bytes: Vec<u8>) -> Option<String> {
        self.seen.push((dtype.to_owned(), len));
        if len < 64 {
            return None;
        }
        let id = format!("buf{}", self.bufs.len());
        self.bufs.insert(id.clone(), bytes);
        Some(id)
    }
}

struct Source(HashMap<String, Vec<u8>>);
impl BufferSource for Source {
    fn get(&self, id: &str) -> Option<Vec<u8>> {
        self.0.get(id).cloned()
    }
}

#[test]
fn inline_without_sink() {
    let m = sample();
    let json = serde_json::to_string(&m).unwrap();
    assert!(json.contains("\"offsets\":[0,4,8,-3]"));
    assert!(json.contains("\"sharp\":[true,false,true]"));
    assert!(json.contains("\"none\":null"));
    // f32 values print in their shortest form.
    assert!(json.contains("[1.0,-0.5,1e-7]"), "{json}");
    let back: Mesh = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
}

#[test]
fn buffers_through_sink_and_source() {
    let m = sample();
    let (json, sink) = with_sink(Sink::default(), || serde_json::to_string(&m).unwrap());
    assert_eq!(
        sink.seen,
        [
            ("f32x3".to_owned(), 100),
            ("i32".to_owned(), 4),
            ("bool".to_owned(), 3),
            ("f32x2".to_owned(), 70)
        ]
    );
    assert!(json.contains("\"positions\":{\"$buf\":\"buf0\",\"dtype\":\"f32x3\",\"len\":100}"));
    assert!(json.contains("\"uv\":{\"$buf\":\"buf1\",\"dtype\":\"f32x2\",\"len\":70}"));
    assert!(
        json.contains("\"offsets\":[0,4,8,-3]"),
        "small arrays stay inline"
    );
    // Little-endian raw bytes.
    let b = &sink.bufs["buf0"];
    assert_eq!(b.len(), 100 * 12);
    assert_eq!(&b[12..16], &1.0f32.to_le_bytes());
    // Read back.
    let (back, _) = with_source(Source(sink.bufs), || {
        serde_json::from_str::<Mesh>(&json).unwrap()
    });
    assert_eq!(back, m);
    // The sink is uninstalled afterwards: inline again.
    assert!(!serde_json::to_string(&m).unwrap().contains("$buf"));
}

#[test]
fn errors() {
    let m = sample();
    let (json, sink) = with_sink(Sink::default(), || serde_json::to_string(&m).unwrap());
    // No source installed.
    let e = serde_json::from_str::<Mesh>(&json).unwrap_err().to_string();
    assert!(e.contains("no BufferSource is installed"), "{e}");
    // Missing buffer.
    let (r, _) = with_source(Source(HashMap::new()), || {
        serde_json::from_str::<Mesh>(&json)
    });
    assert!(r.unwrap_err().to_string().contains("buffer buf0 not found"));
    // Wrong dtype.
    let bad = json.replace("\"dtype\":\"f32x3\"", "\"dtype\":\"f32x4\"");
    let (r, _) = with_source(Source(sink.bufs.clone()), || {
        serde_json::from_str::<Mesh>(&bad)
    });
    assert!(
        r.unwrap_err()
            .to_string()
            .contains("dtype f32x4, expected f32x3")
    );
    // Wrong length.
    let bad = json.replace("\"len\":100", "\"len\":99");
    let (r, _) = with_source(Source(sink.bufs), || serde_json::from_str::<Mesh>(&bad));
    assert!(r.unwrap_err().to_string().contains("expected 1188"));
}

#[test]
fn nested_scopes_restore_and_panics_uninstall() {
    struct Count(usize);
    impl BufferSink for Count {
        fn put(&mut self, _: &str, _: usize, _: Vec<u8>) -> Option<String> {
            self.0 += 1;
            Some(format!("c{}", self.0))
        }
    }
    let v = Arc::new(vec![1u16, 2, 3]);
    #[derive(Serialize)]
    struct W<'a>(#[serde(with = "serde_buf")] &'a Arc<Vec<u16>>);
    let ((inner, outer_json), outer) = with_sink(Count(0), || {
        let (j, inner) = with_sink(Count(100), || serde_json::to_string(&W(&v)).unwrap());
        assert_eq!(j, "{\"$buf\":\"c101\",\"dtype\":\"u16\",\"len\":3}");
        (inner, serde_json::to_string(&W(&v)).unwrap())
    });
    assert_eq!(inner.0, 101);
    assert_eq!(outer.0, 1);
    assert_eq!(outer_json, "{\"$buf\":\"c1\",\"dtype\":\"u16\",\"len\":3}");
    let r = std::panic::catch_unwind(|| {
        with_sink(Count(0), || panic!("boom"));
    });
    assert!(r.is_err());
    assert_eq!(serde_json::to_string(&W(&v)).unwrap(), "[1,2,3]");
}

#[test]
fn every_element_type_round_trips() {
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct All {
        #[serde(with = "serde_buf")]
        a: Arc<Vec<f64>>,
        #[serde(with = "serde_buf")]
        b: Arc<Vec<i8>>,
        #[serde(with = "serde_buf")]
        c: Arc<Vec<u64>>,
        #[serde(with = "serde_buf")]
        d: Arc<Vec<[u8; 4]>>,
        #[serde(with = "serde_buf")]
        e: Arc<Vec<[i32; 2]>>,
    }
    struct All64;
    impl BufferSink for All64 {
        fn put(&mut self, dtype: &str, _: usize, bytes: Vec<u8>) -> Option<String> {
            Some(format!(
                "{dtype}:{}",
                bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
            ))
        }
    }
    struct Hex;
    impl BufferSource for Hex {
        fn get(&self, id: &str) -> Option<Vec<u8>> {
            let hex = id.split_once(':')?.1;
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
                .collect()
        }
    }
    let v = All {
        a: Arc::new(vec![0.1, -2.5]),
        b: Arc::new(vec![-128, 127]),
        c: Arc::new(vec![u64::MAX]),
        d: Arc::new(vec![[1, 2, 3, 4]]),
        e: Arc::new(vec![[-1, 2]]),
    };
    let (json, _) = with_sink(All64, || serde_json::to_string(&v).unwrap());
    assert!(json.contains("\"dtype\":\"u8x4\""));
    assert!(json.contains("\"dtype\":\"i32x2\""));
    let (back, _) = with_source(Hex, || serde_json::from_str::<All>(&json).unwrap());
    assert_eq!(back, v);
}
