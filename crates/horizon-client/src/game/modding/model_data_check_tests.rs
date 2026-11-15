//! The checks of `model_data_check` (#861 part 2) on their own: the design's acceptance values
//! for selection, the answer limits and the registration refusals.
use super::model_data_check::{Registration, check};
use mod_api::{
    Answered, BlockModelData, Identifier, ModelDataEntry, ModelDataKind, ModelDataProperty,
    ModelDataValue, ModelDataVariant,
};

fn id(text: &str) -> Identifier {
    Identifier::parse(text).unwrap()
}

/// A lamp whose `lit` (default false) selects variant 0 (`off`) or 1 (`on`), with a `glow`
/// vector of up to 256 numbers and a `level` from 0 to 3.
fn lamp() -> BlockModelData {
    BlockModelData::new(
        id("lamps:lamp"),
        vec![
            ModelDataProperty::new("lit", ModelDataKind::Bool, ModelDataValue::Bool(false)),
            ModelDataProperty::new(
                "glow",
                ModelDataKind::Floats { max_len: 256 },
                ModelDataValue::Floats(Vec::new()),
            ),
            ModelDataProperty::new(
                "level",
                ModelDataKind::Int { min: 0, max: 3 },
                ModelDataValue::Int(0),
            ),
        ],
        vec![
            ModelDataVariant::new(
                vec![ModelDataEntry::new("lit", ModelDataValue::Bool(false))],
                vec![id("lamps:block/lamp_off")],
            ),
            ModelDataVariant::new(
                vec![ModelDataEntry::new("lit", ModelDataValue::Bool(true))],
                vec![id("lamps:block/lamp_on")],
            ),
        ],
    )
}

fn answered(owner: &'static str, data: BlockModelData) -> Answered<BlockModelData> {
    mod_api::__private::built::answered(data, owner)
}

fn checked(data: BlockModelData) -> Vec<Registration> {
    check(vec![answered("lamps", data)], |block| block == "lamps:lamp")
}

#[test]
fn the_default_selects_the_first_variant_and_an_answer_the_second() {
    let [lamp] = &checked(lamp())[..] else {
        panic!("the lamp's model data is kept");
    };
    assert_eq!(lamp.select(&lamp.defaults()), Some(0), "default false");
    let lit = lamp
        .values(&[ModelDataEntry::new("lit", ModelDataValue::Bool(true))])
        .unwrap();
    assert_eq!(lamp.select(&lit), Some(1), "updated true");
    assert_eq!(lamp.values(&[]).unwrap(), lamp.defaults(), "nothing answered");
}

#[test]
fn an_answer_over_a_limit_or_not_finite_is_refused_whole() {
    let [lamp] = &checked(lamp())[..] else {
        panic!("the lamp's model data is kept");
    };
    let lit = ModelDataEntry::new("lit", ModelDataValue::Bool(true));
    for (answer, why) in [
        (
            vec![
                lit.clone(),
                ModelDataEntry::new("glow", ModelDataValue::Floats(vec![0.5; 257])),
            ],
            "257 elements",
        ),
        (
            vec![
                lit.clone(),
                ModelDataEntry::new("glow", ModelDataValue::Floats(vec![f32::NAN])),
            ],
            "isn't finite",
        ),
        (
            vec![
                lit.clone(),
                ModelDataEntry::new("glow", ModelDataValue::Floats(vec![f32::INFINITY])),
            ],
            "isn't finite",
        ),
        (
            vec![lit.clone(), ModelDataEntry::new("level", ModelDataValue::Int(4))],
            "outside 0..=3",
        ),
        (
            vec![lit.clone(), ModelDataEntry::new("level", ModelDataValue::Bool(true))],
            "another type",
        ),
        (vec![lit.clone(), lit.clone()], "twice"),
        (
            vec![lit.clone(), ModelDataEntry::new("colour", ModelDataValue::Int(1))],
            "unknown property",
        ),
        (
            vec![
                lit.clone(),
                ModelDataEntry::new("x".repeat(16_400), ModelDataValue::Int(0)),
            ],
            "bytes, over 16384",
        ),
    ] {
        let refused = lamp.values(&answer).unwrap_err();
        assert!(refused.contains(why), "{refused} names {why}");
    }
    // A refused answer draws the defaults: variant 0, not the true it asked for.
    assert_eq!(lamp.select(&lamp.defaults()), Some(0));
}

#[test]
fn an_answer_of_exactly_the_limits_is_kept() {
    let [lamp] = &checked(lamp())[..] else {
        panic!("the lamp's model data is kept");
    };
    let values = lamp
        .values(&[ModelDataEntry::new(
            "glow",
            ModelDataValue::Floats(vec![1.0; 256]),
        )])
        .unwrap();
    assert_eq!(values[1], ModelDataValue::Floats(vec![1.0; 256]));
}

#[test]
fn a_registration_breaking_its_schema_is_refused_whole() {
    let mut cases = Vec::new();
    let mut twice = lamp();
    twice.properties.push(twice.properties[0].clone());
    cases.push(twice);
    let mut wide = lamp();
    wide.properties[1].kind = ModelDataKind::Floats { max_len: 257 };
    cases.push(wide);
    let mut range = lamp();
    range.properties[2].kind = ModelDataKind::Int { min: 3, max: 0 };
    cases.push(range);
    let mut default = lamp();
    default.properties[0].default = ModelDataValue::Int(1);
    cases.push(default);
    let mut unknown = lamp();
    unknown.variants[0].when[0].property = "colour".into();
    cases.push(unknown);
    let mut floats = lamp();
    floats.variants[0].when[0] =
        ModelDataEntry::new("glow", ModelDataValue::Floats(Vec::new()));
    cases.push(floats);
    let mut empty = lamp();
    empty.variants[1].models.clear();
    cases.push(empty);
    let mut many = lamp();
    many.properties = (0..65)
        .map(|i| {
            ModelDataProperty::new(format!("p{i}"), ModelDataKind::Bool, ModelDataValue::Bool(false))
        })
        .collect();
    many.variants.clear();
    cases.push(many);
    for case in cases {
        assert!(checked(case.clone()).is_empty(), "{case:?} is refused");
    }
}

#[test]
fn an_unknown_block_or_two_mods_declaring_one_block_keep_nothing() {
    assert!(check(vec![answered("lamps", lamp())], |_| false).is_empty());
    let both = check(
        vec![answered("lamps", lamp()), answered("rival", lamp())],
        |block| block == "lamps:lamp",
    );
    assert!(both.is_empty(), "neither owner's model data is used");
}
