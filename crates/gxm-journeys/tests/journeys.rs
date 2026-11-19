//! Every journey in the catalog, run against the real app, headless, in its own world.
//!
//! One test per journey, so a failure names the workflow that broke.

use gxm_journeys::{catalog, run};
use gxm_world::World;

fn journey(name: &str) {
    let j = catalog::find(name).unwrap_or_else(|| panic!("no journey named {name:?}"));
    let world = World::new();
    (j.world)(&world);
    if let Err(f) = run(&j, &world) {
        panic!("{f}");
    }
}

#[test]
fn get_started() {
    journey("get_started");
}

#[test]
fn first_look() {
    journey("first_look");
}

/// Every journey in the catalog has a test above.
#[test]
fn push_and_pull() {
    journey("push_and_pull");
}

#[test]
fn shape_history() {
    journey("shape_history");
}

#[test]
fn review_pull_request() {
    journey("review_pull_request");
}

#[test]
fn merge_pull_request() {
    journey("merge_pull_request");
}

#[test]
fn every_journey_is_tested() {
    let tested = include_str!("journeys.rs");
    for j in catalog::all() {
        assert!(tested.contains(&format!("fn {}()", j.name)), "journey {:?} has no test in tests/journeys.rs", j.name);
    }
}
