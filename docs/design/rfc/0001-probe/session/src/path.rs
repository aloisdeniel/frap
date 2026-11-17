// SPDX-License-Identifier: GPL-3.0-or-later
//! Minimal path parser for the probe (`bpy.data.x['k'].y[0].z`, `bpy.context.object.location`).
//! The real one is `frap-lang` (§11); this exists only to drive the resolver from strings.

#[derive(Clone, Debug, PartialEq)]
pub enum PSeg {
    Attr(String),
    Key(String),
    Index(i64),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PRoot {
    Data,
    Context,
}

pub fn parse(src: &str) -> Result<(PRoot, Vec<PSeg>), String> {
    let (root, mut rest) = if let Some(r) = src.strip_prefix("bpy.data") {
        (PRoot::Data, r)
    } else if let Some(r) = src.strip_prefix("bpy.context") {
        (PRoot::Context, r)
    } else {
        return Err(format!("bad root: {src}"));
    };
    let mut segs = vec![];
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('.') {
            let end = r.find(['.', '[']).unwrap_or(r.len());
            segs.push(PSeg::Attr(r[..end].to_string()));
            rest = &r[end..];
        } else if let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']').ok_or("unclosed [")?;
            let inner = r[..end].trim();
            if let Some(q) = inner
                .strip_prefix('\'')
                .and_then(|x| x.strip_suffix('\''))
                .or_else(|| inner.strip_prefix('"').and_then(|x| x.strip_suffix('"')))
            {
                segs.push(PSeg::Key(q.to_string()));
            } else {
                segs.push(PSeg::Index(inner.parse().map_err(|_| format!("bad index {inner}"))?));
            }
            rest = &r[end + 1..];
        } else {
            return Err(format!("unexpected {rest}"));
        }
    }
    Ok((root, segs))
}
