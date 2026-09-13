//! Placeholder binary that reserves the `vibeplane` crate name.
//!
//! Vibeplane is a local-first control plane for AI coding agents: one board
//! across all projects and providers, an attention inbox, work items with
//! verification gates, and Git/GitHub integration. See the repository for
//! the concept and roadmap.

fn main() {
    println!("Vibeplane {} — the control plane for vibe engineering.", env!("CARGO_PKG_VERSION"));
    println!("This is a name-reserving placeholder; the product is under development.");
    println!("Concept and roadmap: {}", env!("CARGO_PKG_REPOSITORY"));
}
