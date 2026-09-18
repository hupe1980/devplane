//! Whether a release counts as measured, and how far a floor may move.
//!
//! Pure on purpose, and in `core` on purpose. The run that produces this data
//! spawns a vendor binary and spends money; the question *what does this data
//! mean* must not need either, or it can only be answered by paying.
//!
//! **Four outcomes, never a boolean.** *This release announced nothing that
//! could change a verdict* and *this release was measured and found clean* are
//! different sentences, and a surface that renders them alike is wrong — a
//! constitutional constraint, so it is a type here rather than a paragraph
//! somewhere.

/// What one probe came back as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    /// The gate and the vendor reached the same verdict.
    Agreed,
    /// They differ, and the difference is declared with its reason.
    Declared,
    /// The oracle would not run it here. **Unmeasured, not clean.**
    Skipped,
    /// They differ, undeclared. The one failure this product cannot have.
    Disagreed,
}

/// How one changelog row is disposed of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// It cannot change a verdict here, and says why. Needs no vendor.
    Declined,
    /// A test over *our own matcher* covers it. Proves Devplane behaves as we
    /// read the row; says nothing about the vendor, so it owes the measured
    /// floor.
    Case,
    /// It names a probe, and this is what that probe came back as.
    Probe(ProbeOutcome),
}

/// The verdict for one release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseOutcome {
    /// It announced nothing that could change a verdict. **Not the same as
    /// clean**, and the two never render alike.
    NoRows,
    /// Every row is accounted for by something that asked the vendor.
    Measured,
    /// At least one row has no probe, or its probe was skipped.
    Owed,
    /// At least one probe disagreed, undeclared.
    Disagreed,
}

impl ReleaseOutcome {
    /// The words a report uses. Distinct strings, deliberately: the guard
    /// against collapsing `NoRows` into `Measured` is that they do not share a
    /// sentence anywhere.
    pub fn says(self) -> &'static str {
        match self {
            ReleaseOutcome::NoRows => "announced nothing that could change a verdict",
            ReleaseOutcome::Measured => "every announced row was probed and the vendor agreed",
            ReleaseOutcome::Owed => "at least one announced row has no probe that ran",
            ReleaseOutcome::Disagreed => "a probe disagreed, undeclared",
        }
    }

    /// Whether a floor may move *through* this release.
    pub fn advances(self) -> bool {
        matches!(self, ReleaseOutcome::NoRows | ReleaseOutcome::Measured)
    }
}

/// The outcome for one release, from its rows.
///
/// Order matters: a disagreement outranks an owed row, because a red result is
/// more urgent than an incomplete one and reporting the milder of the two would
/// be the understatement this whole layer exists to prevent.
pub fn release_outcome(rows: &[Row]) -> ReleaseOutcome {
    if rows.is_empty() {
        return ReleaseOutcome::NoRows;
    }
    if rows
        .iter()
        .any(|r| matches!(r, Row::Probe(ProbeOutcome::Disagreed)))
    {
        return ReleaseOutcome::Disagreed;
    }
    // A `Case` row and a skipped probe are the same kind of gap: something the
    // vendor was never actually asked. A skip is never a pass.
    if rows
        .iter()
        .any(|r| matches!(r, Row::Case | Row::Probe(ProbeOutcome::Skipped)))
    {
        return ReleaseOutcome::Owed;
    }
    ReleaseOutcome::Measured
}

/// How far the measured floor may move across a span, oldest release first.
///
/// Returns the newest release preceded by an unbroken run of advancing
/// outcomes. **The first release that does not advance stops it, and every
/// release above that one is unmeasured regardless of its own outcome** — the
/// invariant most likely to be implemented as "skip and continue", which would
/// advance a floor over a gap.
pub fn floor_after<'a>(before: &'a str, span: &[(&'a str, ReleaseOutcome)]) -> &'a str {
    let mut floor = before;
    for (release, outcome) in span {
        if !outcome.advances() {
            break;
        }
        floor = release;
    }
    floor
}
