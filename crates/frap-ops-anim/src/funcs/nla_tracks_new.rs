// SPDX-License-Identifier: GPL-3.0-or-later
//! `AnimData.nla_tracks.new(prev=None) -> NlaTrack` (`rna_NlaTrack_new`,
//! blender/source/blender/makesrna/intern/rna_animation.cc, Blender 5.2.4): a new track
//! (`NlaTrack`, `NlaTrack.001`, …, selected) on top of the stack, or right above `prev`; it
//! becomes the active track. A `prev` of another datablock's AnimData is refused with
//! `NotImplementedError` (Blender links the track into that other list).
//!
//! [`adt`] holds what the `NlaTracks` / `NlaStrips` functions share.

use frap_rna::{PyExc, RnaError, RnaErrorKind, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use crate::keying::nla;

/// Locating the `AnimData` (and its tracks and strips) a function's pointers name.
pub(crate) mod adt {
    use frap_anim::AnimData;
    use frap_anim::rna::anim_data::{AdtLoc, locate};
    use frap_rna::{PyExc, RnaError, RnaErrorKind, RnaPtr, Seg};
    use frap_scene::Host;

    use crate::keying::{adt_slot_mut, report_error};

    /// `RuntimeError` for a pointer whose struct is gone.
    pub(crate) fn removed(what: &str) -> RnaError {
        RnaError::new(
            RnaErrorKind::Other,
            PyExc::RuntimeError,
            format!("StructRNA of type {what} has been removed"),
        )
    }

    /// The path of `p` below `animation_data`.
    pub(crate) fn below(p: &RnaPtr) -> Option<&[Seg]> {
        let i = p
            .path
            .iter()
            .position(|s| *s == Seg::Prop("animation_data"))?;
        Some(&p.path[i + 1..])
    }

    /// Where `p` points inside the AnimData of its root (`None`: elsewhere).
    pub(crate) fn loc_of(h: &dyn Host, p: &RnaPtr) -> Option<(AdtLoc, Option<String>)> {
        let adt = frap_eval::animate::anim_data(h.main(), p.root)?;
        let (loc, coll) = locate(adt, below(p)?)?;
        Some((loc, coll.map(str::to_owned)))
    }

    /// Runs `f` on the AnimData `this` belongs to, mutably.
    pub(crate) fn with_adt<R>(
        h: &mut dyn Host,
        this: &RnaPtr,
        f: impl FnOnce(&mut AnimData) -> Result<R, RnaError>,
    ) -> Result<R, RnaError> {
        let (main, log) = h.main_mut();
        let adt = adt_slot_mut(main, log, this.root)
            .and_then(Option::as_mut)
            .ok_or_else(|| removed("AnimData"))?;
        f(adt)
    }

    /// `BKE_report(RPT_ERROR, …)`
    pub(crate) fn error(msg: impl std::fmt::Display) -> RnaError {
        report_error(msg)
    }
}

#[rna_func(srna = "NlaTracks", func = "new")]
fn nla_tracks_new(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let prev = match args.get("prev") {
        Some(Value::Ptr(p)) => match adt::loc_of(h, p) {
            Some((frap_anim::rna::anim_data::AdtLoc::Track(t), None)) if p.root == this.root => {
                Some(t)
            }
            // Blender links the new track into the list `prev` belongs to (the other
            // datablock's), named after this one's tracks (verified on 5.2.4); frap does not
            // reproduce that and says so.
            Some((frap_anim::rna::anim_data::AdtLoc::Track(_), None)) => {
                return Err(RnaError::new(
                    RnaErrorKind::NotImplemented,
                    PyExc::NotImplementedError,
                    "frap: NlaTracks.new(prev=…): prev is a track of another datablock's \
                     animation data; pass a track of this AnimData or None",
                ));
            }
            _ => return Err(adt::removed("NlaTrack")),
        },
        _ => None,
    };
    let name = adt::with_adt(h, this, |adt| {
        let t = match prev {
            Some(p) => nla::track_new_after(adt, Some(p)),
            None => nla::track_new_tail(adt),
        };
        nla::track_set_active(adt, t);
        Ok(adt.nla_tracks[t].name.clone())
    })?;
    Ok(Value::Ptr(Box::new(
        this.child(Seg::Key(name), &frap_spec_rna::NLA_TRACK),
    )))
}

// `AnimData.nla_tracks.active` (`rna_NlaTrack_active_get/set`,
// blender/source/blender/makesrna/intern/rna_animation.cc, with `BKE_nlatrack_find_active` /
// `BKE_nlatrack_set_active`, Blender 5.2.4, GPL-2.0-or-later): the first track flagged active,
// `None` without one. Assigning clears the flag on every track of this AnimData (nothing
// happens when it has no tracks), then flags the given track; a track of another datablock's
// AnimData is flagged in its own list (verified on 5.2.4: this AnimData then has no active
// track). Here because it shares `adt` with `NlaTracks.new`.

/// `NlaTracks` is the API struct of `AnimData.nla_tracks` (no binding of its own).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static ACTIVE_DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "NlaTracks",
    prop: "active",
    coll: false,
};

#[frap_rna::rna_hook(srna = "NlaTracks", prop = "active")]
mod active_hook {
    use frap_anim::rna::anim_data::AdtLoc;
    use frap_rna::{Field, PropDef, RnaError, RnaPtr, Seg, Value};
    use frap_scene::{Host, HostRead};

    use super::adt;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let data = frap_eval::animate::anim_data(h.main(), this.root)
            .ok_or_else(|| adt::removed("AnimData"))?;
        Ok(Field::Value(
            match data.nla_tracks.iter().find(|t| t.active) {
                Some(t) => Value::Ptr(Box::new(
                    this.child(Seg::Key(t.name.clone()), &frap_spec_rna::NLA_TRACK),
                )),
                None => Value::None,
            },
        ))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let target = match &v {
            Value::Ptr(p) => match adt::loc_of(h, p) {
                Some((AdtLoc::Track(t), None)) => Some((p.root, t)),
                _ => return Err(adt::removed("NlaTrack")),
            },
            _ => None,
        };
        adt::with_adt(h, this, |data| {
            if data.nla_tracks.is_empty() {
                return Ok(false);
            }
            for t in &mut data.nla_tracks {
                t.active = false;
            }
            Ok(true)
        })
        .and_then(|cleared| {
            let Some((root, t)) = target.filter(|_| cleared) else {
                return Ok(());
            };
            let other = RnaPtr::new(root, this.srna);
            adt::with_adt(h, &other, |data| {
                if let Some(track) = data.nla_tracks.get_mut(t) {
                    track.active = true;
                }
                Ok(())
            })
        })
    }
}
