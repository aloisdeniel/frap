// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-statement resource budget (ARCHITECTURE §12 "Limits"): the `--stmt-timeout` deadline and
//! the `--max-elements` cap, visible to every crate that runs long loops (primitive generation,
//! mesh and BMesh ops, glTF export, render-scene building, rendering).
//!
//! It lives in the lowest layer (L0) because the loops that must observe it are spread over
//! every layer, including `frap-geom` (polyfill, normals), which depends only on this crate.
//!
//! The session (`frap-session`) opens a [`scope`] around every statement. The budget is
//! **thread-local**: several sessions in one process (tests, `frap serve` workers) never see
//! each other's deadline, and code that runs outside any scope is unlimited.
//!
//! **Cancellation is cooperative.** Long loops call [`check`] (cheap: it reads the clock once
//! every [`CHECK_EVERY`] calls). When the deadline has passed, `check` unwinds with
//! [`std::panic::resume_unwind`] carrying an [`Interrupted`] payload; the session's
//! transaction catches it, restores the snapshot taken before the statement (rollback) and
//! answers `RuntimeError` code `TIMEOUT`. `resume_unwind` does not run the panic hook, so a
//! timeout prints nothing on stderr. Code between a `check` and the session must therefore be
//! unwind-safe in the same sense the transaction already requires for panics: every mutation
//! happens on state the transaction can drop.
//!
//! **Element counts are checked before building.** Operators that can create unbounded
//! geometry from a few numbers (`mesh.primitive_grid_add(x_subdivisions=10**7, …)`) estimate
//! the counts from their properties and call [`reserve_corners`] first, so an absurd request
//! is rejected in microseconds instead of allocating gigabytes.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// [`check`] reads the clock once every this many calls.
pub const CHECK_EVERY: u32 = 1024;

/// Largest element count a Blender mesh can index (`int`): vertex, edge, face and corner
/// counts above it cannot exist in Blender, whatever `--max-elements` says.
pub const MAX_MESH_ELEMENTS: u64 = i32::MAX as u64;

/// The unwind payload of an expired deadline (see the module docs).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Interrupted {
    /// The statement's time limit.
    pub limit: Duration,
}

/// Limits of one statement.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    /// `--stmt-timeout` (`None`: no deadline).
    pub timeout: Option<Duration>,
    /// `--max-elements`, in face corners (`u64::MAX`: unlimited).
    pub max_elements: u64,
}

impl Default for Budget {
    /// Unlimited.
    fn default() -> Self {
        Budget {
            timeout: None,
            max_elements: u64::MAX,
        }
    }
}

#[derive(Copy, Clone, Debug)]
struct Active {
    deadline: Option<(Instant, Duration)>,
    max_elements: u64,
}

thread_local! {
    static ACTIVE: Cell<Option<Active>> = const { Cell::new(None) };
    static COUNTER: Cell<u32> = const { Cell::new(0) };
}

/// Restores the previous budget of this thread when dropped.
#[must_use = "the budget is active only while the guard lives"]
#[derive(Debug)]
pub struct Scope {
    prev: Option<Option<Active>>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(prev) = self.prev.take() {
            ACTIVE.with(|a| a.set(prev));
        }
    }
}

/// Activates `budget` on this thread until the returned guard is dropped. A nested scope
/// (an operator called inside an operator's arguments) keeps the outer statement's budget:
/// the deadline is never extended.
pub fn scope(budget: Budget) -> Scope {
    ACTIVE.with(|a| {
        if a.get().is_some() {
            return Scope { prev: None };
        }
        let now = Instant::now();
        a.set(Some(Active {
            deadline: budget.timeout.map(|t| {
                (
                    now.checked_add(t)
                        .unwrap_or(now + Duration::from_secs(86_400 * 365)),
                    t,
                )
            }),
            max_elements: budget.max_elements,
        }));
        COUNTER.with(|c| c.set(0));
        Scope { prev: Some(None) }
    })
}

/// Whether a budget is active on this thread.
pub fn is_active() -> bool {
    ACTIVE.with(|a| a.get().is_some())
}

/// The active `--max-elements` (`u64::MAX` outside a scope).
pub fn max_elements() -> u64 {
    ACTIVE.with(|a| a.get().map_or(u64::MAX, |b| b.max_elements))
}

/// The active time limit, if any.
pub fn timeout() -> Option<Duration> {
    ACTIVE.with(|a| a.get().and_then(|b| b.deadline.map(|(_, t)| t)))
}

/// Whether the active deadline has passed (reads the clock).
pub fn expired() -> bool {
    ACTIVE.with(|a| {
        a.get()
            .and_then(|b| b.deadline)
            .is_some_and(|(d, _)| Instant::now() >= d)
    })
}

/// Cooperative cancellation point for long loops: cheap (the clock is read once every
/// [`CHECK_EVERY`] calls); unwinds with [`Interrupted`] once the deadline has passed.
#[inline]
pub fn check() {
    let n = COUNTER.with(|c| {
        let n = c.get().wrapping_add(1);
        c.set(n);
        n
    });
    if n.is_multiple_of(CHECK_EVERY) {
        check_now();
    }
}

/// [`check`] for a step that cost about `work` loop iterations (a walk over an n-gon): the
/// clock is read once every [`CHECK_EVERY`] units of work, so loops whose steps grow with the
/// mesh (quadratic algorithms on large faces) are still cancelled promptly.
#[inline]
pub fn check_n(work: u32) {
    let (old, new) = COUNTER.with(|c| {
        let old = c.get();
        let new = old.wrapping_add(work.max(1));
        c.set(new);
        (old, new)
    });
    if old / CHECK_EVERY != new / CHECK_EVERY || new < old {
        check_now();
    }
}

/// Like [`check`], reading the clock every time (between coarse steps: per object, per
/// render pass, per exported mesh).
pub fn check_now() {
    let hit = ACTIVE.with(|a| {
        a.get()
            .and_then(|b| b.deadline)
            .filter(|(d, _)| Instant::now() >= *d)
            .map(|(_, t)| t)
    });
    if let Some(limit) = hit {
        std::panic::resume_unwind(Box::new(Interrupted { limit }));
    }
}

/// The [`Interrupted`] payload of a caught unwind, if it is one.
pub fn interrupted(payload: &(dyn std::any::Any + Send)) -> Option<Interrupted> {
    payload.downcast_ref::<Interrupted>().copied()
}

/// A request that does not fit the budget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementsExceeded {
    /// Face corners the request would create (saturated at `u64::MAX` on overflow).
    pub corners: u64,
    /// The limit it exceeds: `--max-elements`, or [`MAX_MESH_ELEMENTS`].
    pub limit: u64,
    /// `true` when the limit is Blender's `int` range rather than `--max-elements`.
    pub index_range: bool,
    /// `Some(what)` for a buffer that is not a mesh ([`reserve_buffer`]): `corners` and `limit`
    /// are then bytes, and `what` names the buffer ("image buffer").
    pub buffer: Option<&'static str>,
}

impl std::fmt::Display for ElementsExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(what) = self.buffer {
            write!(
                f,
                "frap: limit exceeded (max_elements): the {what} would need {} bytes (limit {} \
                 bytes, {BUFFER_BYTES_PER_ELEMENT} per --max-elements unit)",
                self.corners, self.limit
            )
        } else if self.index_range {
            write!(
                f,
                "frap: limit exceeded (max_elements): the mesh would need more than {} elements, \
                 more than a Blender mesh can index",
                self.limit
            )
        } else {
            write!(
                f,
                "frap: limit exceeded (max_elements): the statement would create {} face corners \
                 (limit {})",
                self.corners, self.limit
            )
        }
    }
}

/// Pre-check before building geometry: `corners` (face corners the request creates) and
/// `elements` (its largest per-domain count: vertices, edges, faces or corners). `None` means
/// the estimate itself overflowed. Fails when the request exceeds `--max-elements` or Blender's
/// `int` index range.
pub fn reserve_corners(
    corners: Option<u64>,
    elements: Option<u64>,
) -> Result<(), ElementsExceeded> {
    let (Some(corners), Some(elements)) = (corners, elements) else {
        return Err(ElementsExceeded {
            corners: u64::MAX,
            limit: MAX_MESH_ELEMENTS,
            index_range: true,
            buffer: None,
        });
    };
    if elements > MAX_MESH_ELEMENTS || corners > MAX_MESH_ELEMENTS {
        return Err(ElementsExceeded {
            corners,
            limit: MAX_MESH_ELEMENTS,
            index_range: true,
            buffer: None,
        });
    }
    let max = max_elements();
    if corners > max {
        return Err(ElementsExceeded {
            corners,
            limit: max,
            index_range: false,
            buffer: None,
        });
    }
    Ok(())
}

/// Running cap for code that builds geometry one element at a time (BMesh loop allocation):
/// once a mesh holds more than `--max-elements` face corners, unwinds with an
/// [`ElementsExceeded`] payload, which the session's transaction turns into a rollback and
/// `LIMIT_EXCEEDED` (like [`Interrupted`] for the deadline). It is the backstop for growth no
/// operator pre-check ([`reserve_corners`]) estimated: runaway geometry stops at the limit
/// instead of exhausting memory. Unlimited outside a scope; a single thread-local read.
#[inline]
pub fn check_corners(corners: u64) {
    let max = max_elements();
    if corners > max {
        exceed_corners(corners, max);
    }
}

#[cold]
#[inline(never)]
fn exceed_corners(corners: u64, limit: u64) -> ! {
    std::panic::resume_unwind(Box::new(ElementsExceeded {
        corners,
        limit,
        index_range: false,
        buffer: None,
    }))
}

/// Bytes of memory one `--max-elements` unit allows for a buffer that is not a mesh (image
/// pixels): the default 50,000,000 allows 1.6 GB, an 8192 × 8192 float RGBA image is 1 GiB.
pub const BUFFER_BYTES_PER_ELEMENT: u64 = 32;

/// Pre-check before allocating a large non-mesh buffer of `bytes` bytes (`None`: the size
/// overflowed): fails above `--max-elements` × [`BUFFER_BYTES_PER_ELEMENT`] bytes. Unlimited
/// outside a scope, except for sizes that do not fit the address space.
pub fn reserve_buffer(what: &'static str, bytes: Option<u64>) -> Result<(), ElementsExceeded> {
    let limit = max_elements()
        .saturating_mul(BUFFER_BYTES_PER_ELEMENT)
        .min(isize::MAX as u64);
    match bytes {
        Some(b) if b <= limit => Ok(()),
        b => Err(ElementsExceeded {
            corners: b.unwrap_or(u64::MAX),
            limit,
            index_range: false,
            buffer: Some(what),
        }),
    }
}

/// [`reserve_buffer`] that unwinds with the [`ElementsExceeded`] payload instead of returning
/// it (the session answers `LIMIT_EXCEEDED`, like [`check_corners`]), for code that cannot
/// return an error (property getters).
pub fn check_buffer(what: &'static str, bytes: Option<u64>) {
    if let Err(e) = reserve_buffer(what, bytes) {
        std::panic::resume_unwind(Box::new(e));
    }
}

/// The [`ElementsExceeded`] payload of a caught unwind ([`check_corners`]), if it is one.
pub fn elements_exceeded(payload: &(dyn std::any::Any + Send)) -> Option<ElementsExceeded> {
    payload.downcast_ref::<ElementsExceeded>().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn unlimited_outside_a_scope() {
        assert!(!is_active());
        assert_eq!(max_elements(), u64::MAX);
        assert!(!expired());
        for _ in 0..10 * CHECK_EVERY {
            check();
        }
        check_now();
        assert!(reserve_corners(Some(1 << 30), Some(1 << 30)).is_ok());
    }

    #[test]
    fn deadline_unwinds_with_the_payload() {
        let r = catch_unwind(AssertUnwindSafe(|| {
            let _s = scope(Budget {
                timeout: Some(Duration::from_millis(1)),
                max_elements: 10,
            });
            std::thread::sleep(Duration::from_millis(5));
            assert!(expired());
            loop {
                check();
            }
        }));
        let p = r.expect_err("unwinds");
        assert_eq!(
            interrupted(&*p),
            Some(Interrupted {
                limit: Duration::from_millis(1)
            })
        );
        // The guard was dropped during the unwind.
        assert!(!is_active());
    }

    #[test]
    fn weighted_checks_read_the_clock_by_work() {
        let r = catch_unwind(AssertUnwindSafe(|| {
            let _s = scope(Budget {
                timeout: Some(Duration::from_millis(1)),
                max_elements: u64::MAX,
            });
            std::thread::sleep(Duration::from_millis(5));
            // One step worth a full check interval reads the clock at once.
            check_n(CHECK_EVERY);
        }));
        assert!(interrupted(&*r.expect_err("unwinds")).is_some());
        assert!(!is_active());
        // Unlimited outside a scope, whatever the work.
        check_n(u32::MAX);
        check_n(0);
    }

    #[test]
    fn nested_scopes_keep_the_outer_budget() {
        let _a = scope(Budget {
            timeout: None,
            max_elements: 100,
        });
        {
            let _b = scope(Budget {
                timeout: None,
                max_elements: 5,
            });
            assert_eq!(max_elements(), 100);
        }
        assert!(is_active());
        assert_eq!(max_elements(), 100);
        let e = reserve_corners(Some(101), Some(101)).unwrap_err();
        assert!(!e.index_range);
        assert_eq!(
            e.to_string(),
            "frap: limit exceeded (max_elements): the statement would create 101 face corners (limit 100)"
        );
        assert!(reserve_corners(Some(100), Some(100)).is_ok());
        assert!(reserve_corners(None, Some(1)).unwrap_err().index_range);
        assert!(
            reserve_corners(Some(1), Some(MAX_MESH_ELEMENTS + 1))
                .unwrap_err()
                .index_range
        );
        assert!(reserve_buffer("image buffer", Some(3200)).is_ok());
        let e = reserve_buffer("image buffer", Some(3201)).unwrap_err();
        assert_eq!(
            e.to_string(),
            "frap: limit exceeded (max_elements): the image buffer would need 3201 bytes (limit \
             3200 bytes, 32 per --max-elements unit)"
        );
        assert!(reserve_buffer("image buffer", None).is_err());
        let r = catch_unwind(AssertUnwindSafe(|| {
            check_buffer("image buffer", Some(1 << 20))
        }));
        let p = r.expect_err("unwinds");
        assert_eq!(elements_exceeded(&*p).unwrap().buffer, Some("image buffer"));
    }

    #[test]
    fn buffers_are_unlimited_outside_a_scope() {
        assert!(reserve_buffer("image buffer", Some(1 << 40)).is_ok());
        assert!(reserve_buffer("image buffer", Some(u64::MAX)).is_err());
    }
}
