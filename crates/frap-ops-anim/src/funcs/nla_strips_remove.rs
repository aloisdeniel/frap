// SPDX-License-Identifier: GPL-3.0-or-later
//! `NlaTrack.strips.remove(strip)` (`rna_NlaStrip_remove`,
//! blender/source/blender/makesrna/intern/rna_nla.cc, Blender 5.2.4): removes the strip from
//! the track. A strip of another track raises `RuntimeError: Error: NLA strip '…' not found in
//! track '…'`.

use frap_anim::rna::anim_data::AdtLoc;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::nla_tracks_new::adt;

#[rna_func(srna = "NlaStrips", func = "remove")]
fn nla_strips_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let Some((AdtLoc::Track(t), Some(_))) = adt::loc_of(h, this) else {
        return Err(adt::removed("NlaTrack"));
    };
    let Some(Value::Ptr(p)) = args.get("strip") else {
        return Err(adt::removed("NlaStrip"));
    };
    let target = match adt::loc_of(h, p) {
        Some((AdtLoc::Strip(pt, s), None)) if p.root == this.root && pt == t => Some(s),
        _ => None,
    };
    let Some(s) = target else {
        let key = |seg: Option<&frap_rna::Seg>| match seg {
            Some(frap_rna::Seg::Key(k)) => k.to_string(),
            _ => String::new(),
        };
        let strip = key(p.path.last());
        let track = frap_eval::animate::anim_data(h.main(), this.root)
            .and_then(|a| a.nla_tracks.get(t))
            .map(|x| x.name.to_string())
            .unwrap_or_default();
        return Err(adt::error(format!(
            "NLA strip '{strip}' not found in track '{track}'"
        )));
    };
    adt::with_adt(h, this, |a| {
        a.nla_tracks[t].strips.remove(s);
        Ok(Value::None)
    })
}
