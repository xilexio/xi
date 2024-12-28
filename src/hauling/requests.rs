use std::cell::RefCell;
use std::cmp::{max, min};
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
    /// Request to withdraw or pickup some resource from the target to the hauler.
    WithdrawRequest,
    /// Request to transfer some resource from the hauler to the target.
    DepositRequest,
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum HaulRequestTargetKind {
    /// Permanent storage.
    StorageTarget,
    /// A creep that can move, so the position is approximate.
    CreepTarget,
    /// A resource pile.
    PickupTarget,
    /// A regular immovable room object, e.g., structure, tombstone.
    /// It can also be a permanent storage when the storage is low on some resource.
    RegularTarget,
}

#[derive(Default)]
pub struct RoomHaulRequests {
    pub withdraw_requests: FxHashMap<HaulRequestId, HaulRequestRef>,
    pub deposit_requests: FxHashMap<HaulRequestId, HaulRequestRef>,
}

/// There can be only one haul request per withdrawal/deposit, per object, per resource type.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct HaulRequestId(RawObjectId, ResourceType);

impl Display for HaulRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}

/// Generic haul request, both for withdrawing and storing.
#[derive(Debug)]
pub struct HaulRequest {
    pub kind: HaulRequestKind,
    /// Name of the room responsible for providing the hauler.
    pub room_name: RoomName,
    pub target: RawObjectId,
    pub target_kind: HaulRequestTargetKind,
    pub limited_transfer: bool,
    pub resource_type: ResourceType,
    /// Best effort information on the position of the target.
    /// May change if the target is moving (e.g., creep).
    pub pos: Position,
    /// The amount of resource to be withdrawn or deposited. When zero, the request is fulfilled.
    pub amount: u32,
    /// How will the amount change in the near future in terms of change per tick.
    /// Zero if the change is unpredictable (e.g., storage).
    pub change: i32,
    /// Maximum amount sufficient number of ticks has passed.
    pub max_amount: u32,
    /// Priority.
    pub priority: Priority,
    /// The amount that is reserved to be withdrawn or deposited.
    /// May exceed `amount` if the `amount` has decreased.
    pub reserved_amount: u32,
}

/// Haul request identifier that cancels the request on drop.
#[derive(Debug)]
pub struct HaulRequestHandle {
    pub request: HaulRequestRef,
    pub droppable: bool,
}

#[derive(Debug)]
pub struct ReservedHaulRequest {
    pub request: HaulRequestRef,
    pub amount: u32,
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
            DepositRequest => {
                write!(
                    f,
                    "({},{},{}), {}/{} {} ({:?}), {}, to {}",
                    self.pos.room_name(),
                    self.pos.x(),
                    self.pos.y(),
                    self.reserved_amount,
                    self.amount,
                    self.resource_type,
                    self.change,
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
                    self.change,
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
        target_kind: HaulRequestTargetKind,
        limited_transfer: bool,
        pos: Position
    ) -> Self {
        HaulRequest {
            kind,
            room_name,
            target: target.into(),
            target_kind,
            limited_transfer,
            pos,
            resource_type,
            amount: 0,
            change: 0,
            max_amount: u32::MAX,
            priority: Priority(100),
            reserved_amount: 0,
        }
    }
    
    pub fn id(&self) -> HaulRequestId {
        HaulRequestId(self.target, self.resource_type)
    }

    pub fn unreserved_amount(&self) -> i32 {
        self.amount as i32 - self.reserved_amount as i32
    }
    
    pub fn predicted_amount(&self, ticks: u32) -> u32 {
        min(self.max_amount, max(0, self.amount as i32 + self.change * ticks as i32) as u32)
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
    pub fn new(request: HaulRequestRef, amount: u32) -> Self {
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

    pub fn complete(&mut self) {
        let mut borrowed_request = self.request.borrow_mut();
        borrowed_request.amount -= self.amount;
        borrowed_request.reserved_amount -= self.amount;
        // Preventing the drop from changing anything.
        self.amount = 0;
    }
}