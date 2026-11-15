use super::*;
use mod_api::prelude::*;
use mod_api::{
    BlockModelData, Identifier, ModelDataEntry, ModelDataKind, ModelDataProperty,
    ModelDataRefreshError, ModelDataValue, ModelDataVariant,
};
use std::cell::RefCell;

fn lamp() -> BlockModelData {
    BlockModelData::new(
        Identifier::parse("lamps:lamp").unwrap(),
        vec![
            ModelDataProperty::new("lit", ModelDataKind::Bool, ModelDataValue::Bool(false)),
            ModelDataProperty::new(
                "level",
                ModelDataKind::Int { min: -2, max: 9 },
                ModelDataValue::Int(-2),
            ),
            ModelDataProperty::new(
                "glow",
                ModelDataKind::Floats { max_len: 4 },
                ModelDataValue::Floats(vec![0.25, -1.5]),
            ),
        ],
        vec![ModelDataVariant::new(
            vec![ModelDataEntry::new("lit", ModelDataValue::Bool(true))],
            vec![Identifier::parse("lamps:block/lamp_on").unwrap()],
        )],
    )
}

#[test]
fn block_model_data_round_trips_every_kind_and_value() {
    let data = lamp();
    let mut out = Writer::new();
    out.put(&data);
    assert_eq!(Reader::decode::<BlockModelData>(out.bytes()).unwrap(), data);
    let answer = vec![
        ModelDataEntry::new("lit", ModelDataValue::Bool(true)),
        ModelDataEntry::new("glow", ModelDataValue::Floats(vec![f32::MAX, 0.0])),
    ];
    let mut out = Writer::new();
    out.put(&answer);
    assert_eq!(Reader::decode::<Vec<ModelDataEntry>>(out.bytes()).unwrap(), answer);
    let mut out = Writer::new();
    out.put(&ModelDataRefreshError::NoModelData);
    assert_eq!(out.bytes(), 0u32.to_le_bytes());
}

/// The client: keeps each refresh asked for and has model data only at the origin.
#[derive(Default)]
struct Client(RefCell<Vec<[i32; 3]>>);
impl mod_api::Host for Client {}
mod_api::impl_host_areas!(Client);
impl mod_api::ClientHost for Client {}
mod_api::impl_client_host_areas!(Client, except(client_render));
impl ClientRenderClientHost for Client {
    fn refresh_model_data(&mut self, pos: [i32; 3]) -> Result<(), ModelDataRefreshError> {
        self.0.borrow_mut().push(pos);
        if pos == [0, 64, 0] {
            Ok(())
        } else {
            Err(ModelDataRefreshError::NoModelData)
        }
    }
}

#[test]
fn wire_loopback_a_guest_refresh_reaches_the_client_and_its_refusal_comes_back() {
    mod_api::__private::initialize().unwrap();
    let _phase = mod_api::__private::PhaseOwner::scope(mod_api::__private::ModPhase::World);
    let mut client = Client::default();
    let mut frame = boundary::Frame::default();
    frame.set_client_host_mut(&mut client);
    let mut cx = boundary::Cx::default();
    cx.frames.push(frame);
    let cx = std::cell::RefCell::new(cx);
    guest::set_transport(Some(Rc::new(move |id, bytes| {
        let mut out = Writer::new();
        // SAFETY: the client and its frame remain live for this synchronous transport.
        unsafe { boundary::serve(id, &mut *cx.borrow_mut(), bytes, &mut out) }.unwrap();
        out.into_bytes()
    })));
    assert_eq!(guest::Guest.refresh_model_data([0, 64, 0]), Ok(()));
    assert_eq!(
        guest::Guest.refresh_model_data([-5, 3, 7]),
        Err(ModelDataRefreshError::NoModelData)
    );
    guest::set_transport(None);
    assert_eq!(*client.0.borrow(), [[0, 64, 0], [-5, 3, 7]]);
}
