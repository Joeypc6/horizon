//! The selection table on its own: what a mesh job's plan answers, and that an untouched world
//! gives no plan at all.
use super::*;

fn selection(slots: &[u32]) -> Selection {
    Arc::from(slots)
}

#[test]
fn an_empty_table_gives_no_plan_and_the_mesher_keeps_its_own_path() {
    let table = ModelDataSections::default();
    assert!(table.is_empty());
    assert!(table.plan(0, 4, 0).is_none());
    assert_eq!(table.generation(), 0);
}

#[test]
fn a_plan_answers_the_cells_selection_before_its_states_default() {
    let mut table = ModelDataSections::default();
    let mut defaults = vec![None; 8];
    defaults[5] = Some(selection(&[10]));
    table.reset(3, defaults.into());
    assert_eq!(table.generation(), 3);
    let plan = table.plan(0, 4, 0).expect("defaults alone give a plan");
    assert_eq!(plan.at(5, [1, 2, 3]), Some(&[10][..]), "state 5's default");
    assert_eq!(plan.at(6, [1, 2, 3]), None, "a state without one");
    // The block at (1, 66, 3) is cell (1, 2, 3) of section index 4 with the level's bottom at 0.
    assert!(table.set([1, 66, 3], 4, Entry::Selected(Some(selection(&[11])))));
    assert!(!table.set([1, 66, 3], 4, Entry::Selected(Some(selection(&[11])))), "unchanged");
    let plan = table.plan(0, 4, 0).unwrap();
    assert_eq!(plan.at(5, [1, 2, 3]), Some(&[11][..]), "its evaluated selection");
    assert_eq!(plan.at(5, [2, 2, 3]), Some(&[10][..]), "a neighbour keeps the default");
    assert!(table.set([1, 66, 3], 4, Entry::Selected(None)));
    assert_eq!(table.plan(0, 4, 0).unwrap().at(5, [1, 2, 3]), None, "its state's own");
    assert!(table.set([1, 66, 3], 4, Entry::Default));
    assert_eq!(table.plan(0, 4, 0).unwrap().at(5, [1, 2, 3]), Some(&[10][..]));
    assert!(!table.set([1, 66, 3], 4, Entry::Default), "already the default");
}

#[test]
fn a_frozen_plan_keeps_what_it_was_given_while_the_table_changes() {
    let mut table = ModelDataSections::default();
    table.reset(1, Arc::from([]));
    assert!(table.set([-1, 0, -17], 0, Entry::Selected(Some(selection(&[2])))));
    let frozen = table.plan(-1, 0, -2).expect("a cell gives a plan");
    assert!(table.set([-1, 0, -17], 0, Entry::Selected(Some(selection(&[3])))));
    assert_eq!(frozen.at(0, [15, 0, 15]), Some(&[2][..]), "the job's copy");
    assert_eq!(
        table.plan(-1, 0, -2).unwrap().at(0, [15, 0, 15]),
        Some(&[3][..])
    );
    table.remove_column(-1, -2);
    assert!(table.is_empty(), "no defaults and no cells left");
    table.set([-1, 0, -17], 0, Entry::Selected(Some(selection(&[2]))));
    table.reset(2, Arc::from([]));
    assert!(table.is_empty(), "a resource load drops every selection");
}
