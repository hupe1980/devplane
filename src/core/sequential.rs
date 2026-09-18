//! When to believe a probe whose oracle is a model.
//!
//! # The error this replaced was bounded in the wrong direction
//!
//! The differential harness used to re-ask a disagreement a fixed number of
//! times — five — and report it only if it survived all five. The reasoning was
//! real and is worth keeping: some shapes the model runs about two times in
//! three whatever the rules say, so three consecutive absences come up once in
//! twenty-seven, and a matrix of a few hundred rows finds that twice. Both were
//! reported as the loudest thing this harness can say, and both were the model.
//!
//! **But look at which error five asks bounds.** It bounds the chance of *crying
//! wolf* — roughly one in 243 for a two-in-three shape. It bounds nothing at all
//! in the other direction: a real widening whose evidence happens not to appear
//! five times running was printed as agreement and disappeared.
//!
//! So the harness spent its whole statistical budget on the error this product
//! can afford, and left unbounded the error it cannot have. That asymmetry is
//! not expressible with one retry count: there is a single knob and turning it
//! moves both errors together.
//!
//! # What this is instead
//!
//! A differential harness with a noisy oracle is a sequential hypothesis test,
//! and that has a name. Accumulate the log-likelihood ratio of the observations
//! under two hypotheses — *the two sides agree* against *they disagree* — and
//! stop when it crosses either threshold, approximately `(1-β)/α` and
//! `β/(1-α)`. Three outcomes follow, and the third is the one the old rule had
//! nowhere to put.
//!
//! Three properties earn it:
//!
//! 1. **Both errors are bounded by numbers somebody chose.** `beta` — the chance
//!    of missing a real disagreement — is set far tighter than `alpha`, because a
//!    false alarm costs an afternoon and a missed widening is the failure this
//!    product cannot have.
//! 2. **`Inconclusive` is what the test produces**, not a variant bolted on. The
//!    indeterminate region is there by construction.
//! 3. **It spends the fewest asks for the bounds it gives.** Wald and Wolfowitz
//!    proved in 1948 that this minimises the expected number of observations
//!    under *both* hypotheses — among all tests **with the same error bounds**.
//!
//! # What it costs, said plainly, because the first draft of this got it wrong
//!
//! An earlier version of this file claimed the new rule is cheaper than the old
//! one. **It is not, on the case that dominates.** The old rule accepted
//! agreement on a *single* ask; this needs four at the default miss rate. Most
//! rows in a matrix agree, so a full run gets several times more expensive, not
//! less.
//!
//! The old rule was cheap precisely *because* its miss rate was unbounded:
//! accepting agreement on one observation, when a real difference shows up about
//! six times in seven, is roughly a **15 % chance of missing a real
//! disagreement, per row**. Wald–Wolfowitz does not license the comparison
//! either — its optimality is among tests with the same bounds, and the old rule
//! has no `beta` bound at all, so it is not in that class.
//!
//! Against the old rule, honestly: **cheaper on disagreeing rows (2 asks against
//! 5), dearer on agreeing rows (4 against 1), and the increase is exactly the
//! price of a guarantee that did not previously exist.**
//!
//! That price is a dial, and [`ErrorBudget::asks_to_conclude_agreement`] is the
//! arithmetic for setting it:
//!
//! | `beta` | asks to conclude agreement | miss rate |
//! |---|---|---|
//! | `5e-2` | 2 | 1 in 20 |
//! | `1e-2` | 3 | 1 in 100 |
//! | `1e-3` | 4 | 1 in 1 000 — the default |
//! | `1e-6` | 8 | 1 in a million |
//!
//! Concluding a disagreement costs 2 asks at every one of those.
//!
//! # The limit, which is not small
//!
//! Wald's thresholds are an **approximation**. The statistic overshoots them at
//! the stopping time, so when `beta > 0` they do not guarantee error control at
//! exactly `alpha` and `beta`, nor optimality. The rates here are therefore
//! **targets**, said as targets, and where a choice has to be made this rounds
//! toward asking again — because the error being traded away is the one the
//! narrower-never-wider principle forbids.

/// How often the test may be wrong, in each direction. Policy, chosen by a
/// person, rather than something that emerges from a retry count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorBudget {
    /// Reporting a disagreement that is not real. Costs somebody an afternoon.
    pub alpha: f64,
    /// Missing a disagreement that is real. **This is the failure this product
    /// cannot have**, so it is set far tighter than `alpha`.
    pub beta: f64,
    /// How many times one probe may be asked before the answer is given up on.
    /// The budget is money; this is what stops one ambiguous row eating it.
    pub max_asks: u32,
}

impl Default for ErrorBudget {
    /// The asymmetry is the whole point, and it is two orders of magnitude.
    ///
    /// At these values a row that agrees costs four asks and a row that
    /// disagrees costs two. The four is the price of bounding the miss, and it
    /// is the number to lower if a full matrix has to fit a budget — see the
    /// table at the top of this file.
    fn default() -> Self {
        Self {
            alpha: 5e-2,
            beta: 1e-3,
            max_asks: 12,
        }
    }
}

impl ErrorBudget {
    /// `ln((1-β)/α)` — cross this and the disagreement is reported.
    fn upper(&self) -> f64 {
        ((1.0 - self.beta) / self.alpha).ln()
    }

    /// `ln(β/(1-α))` — cross this and the two sides are called agreed.
    fn lower(&self) -> f64 {
        (self.beta / (1.0 - self.alpha)).ln()
    }
}

/// What the evidence so far licenses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// The evidence supports a real disagreement. Report it.
    Disagree,
    /// The evidence supports agreement. Report that.
    Agree,
    /// Neither threshold reached and asks remain. Ask again.
    Continue,
    /// Neither threshold reached and the budget is spent. **Never rounded to
    /// agreement**: *the disagreement did not reproduce* and *the two sides
    /// agree* are different sentences.
    Inconclusive,
}

/// The evidence for one probe, across however many asks it has taken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Evidence {
    /// Asks where the two sides gave the same answer.
    pub agreements: u32,
    /// Asks where they did not.
    pub disagreements: u32,
}

impl Evidence {
    pub fn asks(&self) -> u32 {
        self.agreements + self.disagreements
    }

    pub fn saw_disagreement(&mut self) {
        self.disagreements += 1;
    }

    pub fn saw_agreement(&mut self) {
        self.agreements += 1;
    }
}

/// How likely a disagreement is to show up on any one ask when the two sides
/// genuinely differ.
///
/// Not 1.0, and that is the whole reason this file exists: the oracle is a
/// model, and a real difference can fail to produce its evidence. Measured
/// rather than assumed — the shapes that flaked ran about two times in three.
const P_UNDER_DISAGREE: f64 = 0.85;

/// And when they genuinely agree. Not 0.0, for the mirror reason: a model can
/// decline, stop early or rewrite a command, and produce an apparent difference
/// where there is none.
const P_UNDER_AGREE: f64 = 0.10;

/// Where the evidence stands.
///
/// The log-likelihood ratio of what has been seen under *they disagree* against
/// *they agree*, compared with Wald's two thresholds.
pub fn decide(e: Evidence, budget: &ErrorBudget) -> Decision {
    if e.asks() == 0 {
        return Decision::Continue;
    }
    let llr = e.disagreements as f64 * (P_UNDER_DISAGREE / P_UNDER_AGREE).ln()
        + e.agreements as f64 * ((1.0 - P_UNDER_DISAGREE) / (1.0 - P_UNDER_AGREE)).ln();

    if llr >= budget.upper() {
        return Decision::Disagree;
    }
    if llr <= budget.lower() {
        return Decision::Agree;
    }
    match e.asks() >= budget.max_asks {
        true => Decision::Inconclusive,
        false => Decision::Continue,
    }
}

impl ErrorBudget {
    /// How many consecutive agreements this budget needs before it will call
    /// two sides agreed.
    ///
    /// **This is the cost dial.** Most rows in a matrix agree, so this number
    /// multiplied by the row count is very nearly what a run costs.
    pub fn asks_to_conclude_agreement(&self) -> u32 {
        let per = ((1.0 - P_UNDER_DISAGREE) / (1.0 - P_UNDER_AGREE)).ln();
        (self.lower() / per).ceil() as u32
    }

    /// And how many disagreements before it will report one.
    pub fn asks_to_conclude_disagreement(&self) -> u32 {
        let per = (P_UNDER_DISAGREE / P_UNDER_AGREE).ln();
        (self.upper() / per).ceil() as u32
    }
}

/// The two rates, for a report that must say them as targets.
pub fn targets(budget: &ErrorBudget) -> String {
    format!(
        "error targets: alpha {:.0e} (report a disagreement that is not real) \
         beta {:.0e} (miss one that is) — targets, not achieved rates: Wald's \
         thresholds are an approximation and the statistic overshoots them",
        budget.alpha, budget.beta
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(pattern: &[bool], budget: &ErrorBudget) -> (Decision, u32) {
        let mut e = Evidence::default();
        for &disagreed in pattern {
            match disagreed {
                true => e.saw_disagreement(),
                false => e.saw_agreement(),
            }
            match decide(e, budget) {
                Decision::Continue => continue,
                d => return (d, e.asks()),
            }
        }
        (decide(e, budget), e.asks())
    }

    /// Consistent agreement is reported, and **cheaply** — this is the saving
    /// that makes the principled design the cheaper one. The rule it replaced
    /// paid for five asks on every disagreement.
    #[test]
    fn consistent_agreement_is_concluded_and_costs_what_the_budget_says() {
        let b = ErrorBudget::default();
        let (d, asks) = run(&[false; 12], &b);
        assert_eq!(d, Decision::Agree);
        assert_eq!(
            asks,
            b.asks_to_conclude_agreement(),
            "the dial does not predict the cost"
        );
        assert_eq!(
            asks, 4,
            "the default budget's agreement cost moved without a note"
        );
    }

    /// **The correction.** The first draft of this module claimed the new rule
    /// is cheaper than the fixed five asks it replaced. It is not, on the case
    /// that dominates a matrix: the old rule accepted agreement on one ask.
    ///
    /// The old rule was cheap *because* its miss rate was unbounded. This test
    /// exists so the trade stays visible to whoever next reads the cost.
    #[test]
    fn bounding_the_miss_costs_more_than_not_bounding_it() {
        let b = ErrorBudget::default();
        const OLD_ASKS_TO_ACCEPT_AGREEMENT: u32 = 1;
        assert!(
            b.asks_to_conclude_agreement() > OLD_ASKS_TO_ACCEPT_AGREEMENT,
            "if this ever passes trivially, the miss bound has been given away"
        );
        // And cheaper where the old rule was expensive.
        const OLD_ASKS_TO_CONFIRM_DISAGREEMENT: u32 = 5;
        assert!(b.asks_to_conclude_disagreement() < OLD_ASKS_TO_CONFIRM_DISAGREEMENT);
    }

    /// The dial is real: a looser miss rate buys a cheaper matrix, and the
    /// arithmetic says how much by.
    #[test]
    fn the_cost_dial_is_monotonic_and_predicts_the_run() {
        let mut last = 0;
        for beta in [5e-2, 1e-2, 1e-3, 1e-6] {
            let b = ErrorBudget {
                beta,
                ..Default::default()
            };
            let n = b.asks_to_conclude_agreement();
            assert!(n >= last, "a tighter miss rate got cheaper");
            let (d, asks) = run(&[false; 16], &b);
            assert_eq!(d, Decision::Agree);
            assert_eq!(asks, n, "beta {beta:e}: predicted {n}, cost {asks}");
            last = n;
        }
    }

    /// A consistent disagreement is still reported, which is the thing the old
    /// rule got right and must not be lost.
    #[test]
    fn consistent_disagreement_is_reported() {
        let b = ErrorBudget::default();
        let (d, asks) = run(&[true; 12], &b);
        assert_eq!(d, Decision::Disagree);
        assert!(asks <= 5, "a real finding took {asks} asks");
    }

    /// **The case the old rule got wrong.** A shape the model runs about two
    /// times in three produces an intermittent disagreement. Five consecutive
    /// absences printed `ok` — agreement — and the finding disappeared.
    #[test]
    fn an_intermittent_disagreement_is_inconclusive_and_never_agreement() {
        let b = ErrorBudget::default();
        let (d, _) = run(
            &[
                true, false, true, false, true, false, false, true, false, true, false, false,
            ],
            &b,
        );
        assert_eq!(
            d,
            Decision::Inconclusive,
            "an intermittent disagreement resolved to a verdict"
        );
        assert_ne!(d, Decision::Agree, "it was rounded to agreement");
    }

    /// Running out of budget is never agreement.
    #[test]
    fn a_spent_budget_is_inconclusive() {
        let b = ErrorBudget {
            max_asks: 2,
            ..Default::default()
        };
        let (d, asks) = run(&[true, false], &b);
        assert_eq!(d, Decision::Inconclusive);
        assert_eq!(asks, 2);
    }

    /// **The asymmetry is the point.** Missing a real disagreement is the
    /// failure this product cannot have, so it takes more evidence to conclude
    /// agreement than to conclude disagreement.
    #[test]
    fn it_is_harder_to_conclude_agreement_than_disagreement() {
        let b = ErrorBudget::default();
        let (_, to_disagree) = run(&[true; 12], &b);
        let (_, to_agree) = run(&[false; 12], &b);
        assert!(
            to_agree >= to_disagree,
            "agreement was cheaper to conclude ({to_agree}) than disagreement \
             ({to_disagree}), which inverts the error this gate exists to avoid"
        );
    }

    /// Tightening the miss rate costs more evidence before agreement, which is
    /// what a declared rate is supposed to buy.
    #[test]
    fn a_tighter_miss_rate_demands_more_evidence() {
        let loose = ErrorBudget {
            beta: 1e-1,
            ..Default::default()
        };
        let tight = ErrorBudget {
            beta: 1e-6,
            ..Default::default()
        };
        let (_, loose_asks) = run(&[false; 16], &loose);
        let (_, tight_asks) = run(&[false; 16], &tight);
        assert!(
            tight_asks > loose_asks,
            "a hundred-thousand-fold tighter miss rate bought nothing: {tight_asks} vs {loose_asks}"
        );
    }

    /// No evidence is not an answer.
    #[test]
    fn nothing_seen_is_never_a_verdict() {
        assert_eq!(
            decide(Evidence::default(), &ErrorBudget::default()),
            Decision::Continue
        );
    }

    /// The report must say *target*, because the thresholds are approximate.
    #[test]
    fn the_rates_are_reported_as_targets() {
        let s = targets(&ErrorBudget::default());
        assert!(s.contains("targets, not achieved"), "{s}");
        assert!(s.contains("overshoot"), "{s}");
    }
}
