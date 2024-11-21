use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;
use log::trace;
use rustc_hash::FxHashMap;
use screeps::{ObjectId, Position, RawObjectId, ResourceType, RoomName};
use crate::utils::priority::Priority;
use crate::hauling::scheduling_hauls::cancel_haul_request;
use crate::a;
use HaulRequestKind::*;

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum HaulRequestKind {
    WithdrawRequest,
    PickupRequest,
    StoreRequest,
}

#[derive(Default)]
pub(crate) struct RoomHaulRequests {
    pub withdraw_requests: FxHashMap<HaulRequestId, HaulRequestRef>,
    pub store_requests: FxHashMap<HaulRequestId, HaulRequestRef>,
}

/// There can be only one haul request per withdrawal/store, per object, per resource type.
pub type HaulRequestId = (RawObjectId, ResourceType);

/// Generic haul request, both for withdrawing and storing.
#[derive(Debug)]
pub(crate) struct HaulRequest {
    pub(crate) kind: HaulRequestKind,
    /// Name of the room responsible for providing the hauler.
    pub(crate) room_name: RoomName,
    pub(crate) target: RawObjectId,
    pub(crate) resource_type: ResourceType,
    /// Best effort information on the position of the target.
    /// May change if the target is moving (e.g., creep).
    pub pos: Position,
    /// The amount of resource to be withdrawn of stored. When zero, the request is fulfilled.
    pub amount: u32,
    /// How will the amount change in the near future.
    pub amount_change: RequestAmountChange,
    /// How many units of the resource are lost to decay per tick.
    pub decay: u32,
    /// Priority 
    pub priority: Priority,
    /// The amount that is reserved to be withdrawn or stored. May exceed `amount` if the `amount`
    /// has decreased.
    pub(crate) reserved_amount: u32,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum RequestAmountChange {
    /// The amount will not change until the request is fulfilled.
    NoChange,
    /// The amount may change unpredictably.
    UnknownChange,
    /// The amount will be increasing until the request is fulfilled, although possibly erratically.
    Increase,
    /// The amount will decrease until it disappears or the request is fulfilled.
    Decrease,
}

/// Haul request identifier that cancels the request on drop.
#[derive(Debug)]
pub struct HaulRequestHandle {
    pub(crate) request: HaulRequestRef,
    pub(crate) droppable: bool,
}

#[derive(Debug)]
pub(crate) struct ReservedHaulRequest {
    pub(crate) request: HaulRequestRef,
    pub(crate) amount: u32,
}

pub type HaulRequestRef = Rc<RefCell<HaulRequest>>;

thread_local! {
    static HAUL_REQUESTS: RefCell<FxHashMap<RoomName, RoomHaulRequests>> = RefCell::new(FxHashMap::default());
}

pub(super) fn with_haul_requests<F, R>(room_name: RoomName, f: F) -> R
where
    F: FnOnce(&mut RoomHaulRequests) -> R,
{
    HAUL_REQUESTS.with(|states| {
        let mut borrowed_states = states.borrow_mut();
        let room_spawn_schedule = borrowed_states
            .entry(room_name)
            .or_default();
        f(room_spawn_schedule)
    })
}

impl Drop for HaulRequestHandle {
    fn drop(&mut self) {
        if self.droppable {
            cancel_haul_request(self.request.clone());
        }
    }
}

impl Display for HaulRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            StoreRequest => {
                write!(
                    f,
                    "({},{},{}), {}/{} {} ({:?}), {}, to {}",
                    self.pos.room_name(),
                    self.pos.x(),
                    self.pos.y(),
                    self.reserved_amount,
                    self.amount,
                    self.resource_type,
                    self.amount_change,
                    self.priority,
                    self.target
                )
            }
            _ => {
                write!(
                    f,
                    "({},{},{}), {}/{} {} ({:?}), {}, from {} ({:?})",
                    self.pos.room_name(),
                    self.pos.x(),
                    self.pos.y(),
                    self.reserved_amount,
                    self.amount,
                    self.resource_type,
                    self.amount_change,
                    self.priority,
                    self.target,
                    self.kind
                )
            }
        }
    }
}

impl HaulRequest {
    pub fn new<T>(
        kind: HaulRequestKind,
        room_name: RoomName,
        resource_type: ResourceType,
        target: ObjectId<T>,
        pos: Position
    ) -> Self {
        HaulRequest {
            kind,
            room_name,
            target: target.into(),
            pos,
            resource_type,
            amount: 0,
            amount_change: RequestAmountChange::NoChange,
            decay: 0,
            priority: Priority::default(),
            reserved_amount: 0,
        }
    }
    
    pub fn id(&self) -> HaulRequestId {
        (self.target, self.resource_type)
    }

    pub fn unreserved_amount(&self) -> i32 {
        self.amount as i32 - self.reserved_amount as i32
    }
}

impl Drop for ReservedHaulRequest {
    fn drop(&mut self) {
        trace!(
            "Releasing {} reserved amount from the request {}.",
            self.amount,
            self.request.borrow()
        );
        self.request.borrow_mut().reserved_amount -= self.amount;
    }
}

impl ReservedHaulRequest {
    pub(crate) fn new(request: HaulRequestRef, amount: u32) -> Self {
        // Cannot reserve an empty haul.
        a!(amount > 0);
        let mut borrowed_request = request.borrow_mut();
        borrowed_request.reserved_amount += amount;
        // While the reserved amount may exceed the total amount if the total amount decreases,
        // it cannot do so when creating a new request.
        a!(borrowed_request.reserved_amount <= borrowed_request.amount);
        drop(borrowed_request);
        ReservedHaulRequest {
            request,
            amount
        }
    }

    pub(crate) fn complete(&mut self) {
        let mut borrowed_request = self.request.borrow_mut();
        borrowed_request.amount -= self.amount;
        borrowed_request.reserved_amount -= self.amount;
        // Preventing the drop from changing anything.
        self.amount = 0;
    }
}