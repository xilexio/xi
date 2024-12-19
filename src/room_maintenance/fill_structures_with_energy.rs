use log::debug;
use rustc_hash::FxHashMap;
use crate::room_states::room_states::with_room_state;
use screeps::{ObjectId, Position, RawObjectId, ResourceType, RoomName, RoomXY, Structure};
use screeps::StructureType::{Extension, Spawn, Tower};
use crate::geometry::room_xy::RoomXYUtils;
use crate::hauling::requests::{HaulRequest, HaulRequestHandle};
use crate::hauling::requests::HaulRequestKind::DepositRequest;
use crate::hauling::requests::HaulRequestTargetKind::RegularTarget;
use crate::hauling::scheduling_hauls::schedule_haul;
use crate::hauling::transfers::get_free_capacity_with_object;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;
use crate::room_states::utils::loop_until_structures_change;
use crate::utils::get_object_by_id::structure_object_by_id;
use crate::utils::priority::Priority;

/// Keeps spawns filled by requesting haulers to fill them.
pub async fn fill_structures_with_energy(room_name: RoomName) {
    loop {
        // TODO Maybe don't drop all store requests on change, just the ones that changed?
        let mut deposit_request_handles: FxHashMap<_, _> = FxHashMap::default();
        
        loop_until_structures_change(room_name, 4, || {
            with_room_state(room_name, |room_state| {
                for structure_type in [Spawn, Extension, Tower] {
                    schedule_missing_energy_deposit_for_structure_type(
                        room_name,
                        room_state.structures.get(&structure_type),
                        &mut deposit_request_handles
                    );
                }
            });

            true
        }).await;
    }
}

pub fn schedule_missing_energy_deposit_for_structure_type(
    room_name: RoomName,
    structures: Option<&FxHashMap<RoomXY, ObjectId<Structure>>>,
    deposit_request_handles: &mut FxHashMap<ObjectId<Structure>, HaulRequestHandle>
) {
    for (&xy, &id) in structures.iter().flat_map(|spawns| spawns.iter()) {
        let handle = schedule_missing_energy_deposit(
            room_name,
            RawObjectId::from(id).into(),
            xy.to_pos(room_name),
            deposit_request_handles.remove(&id)
        );
        if let Some(handle) = handle {
            deposit_request_handles.insert(id, handle);
        }
    }
}

pub fn schedule_missing_energy_deposit(
    room_name: RoomName,
    id: ObjectId<Structure>,
    pos: Position,
    replaced_request_handle: Option<HaulRequestHandle>
) -> Option<HaulRequestHandle> {
    // It might have been destroyed.
    let obj = structure_object_by_id(id).ok()?;
    let missing_energy = get_free_capacity_with_object(obj.as_has_store()?, id.into(), Some(ResourceType::Energy), AfterAllTransfers);
    
    if missing_energy > 0 {
        debug!("Scheduling haul of missing {missing_energy} energy for {id} in {room_name}.");
        // The previous store request is replaced by this one.
        let mut store_request = HaulRequest::new(
            DepositRequest,
            room_name,
            ResourceType::Energy,
            id,
            RegularTarget,
            false,
            pos
        );
        store_request.amount = missing_energy;
        // TODO Far away extensions less important.
        store_request.priority = Priority(100);
        Some(schedule_haul(store_request, replaced_request_handle))
    } else {
        None
    }
}
