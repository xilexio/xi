use log::debug;
use screeps::{look, MaybeHasId, Position, RawObjectId};
use crate::construction::place_construction_sites::ConstructionSiteData;
use crate::geometry::position_utils::PositionUtils;
use crate::kernel::sleep::sleep;
use crate::kernel::wait_until_some::wait_until_some;
use crate::room_states::room_states::with_room_state;
use crate::travel::nearest_room::find_nearest_owned_room;
use crate::utils::get_object_by_id::erased_object_by_id;

pub async fn forced_build(construction_site_pos: Position) {
    loop {
        let room_name = construction_site_pos.room_name();
        debug!("Trying to build a construction site at {}.", construction_site_pos.f());

        // Waiting until able to determine the construction site at given position.
        // In particular this requires vision.
        let construction_site_data = wait_until_some(|| {
            let mut cs = construction_site_pos.look_for(look::CONSTRUCTION_SITES).ok()?;
            let construction_site_obj = cs.pop()?;
            Some(ConstructionSiteData {
                id: construction_site_obj.try_id()?,
                structure_type: construction_site_obj.structure_type(),
                pos: construction_site_pos,
            })
        }).await;
        
        let cs_id = RawObjectId::from(construction_site_data.id);
        debug!("Found construction site {} at {}.", cs_id, construction_site_pos.f());
        
        if let Some(creeps_provider_room_name) = find_nearest_owned_room(room_name, 3) {
            debug!("Registering the construction site {} at room {}.", cs_id, creeps_provider_room_name);
            with_room_state(creeps_provider_room_name, |room_state| {
                room_state.extra_construction_sites.push(construction_site_data);
            });
        
            // TODO Vision is required for this to work correctly.
            wait_until_some(|| {
                debug!("Waiting until the construction site {} disappears.", cs_id);
                match erased_object_by_id(&cs_id).ok() {
                    None => Some(()),
                    Some(_) => None,
                }
            }).await;
            
            debug!("The construction site {} disappeared. Unregistering it.", cs_id);
            with_room_state(creeps_provider_room_name, |room_state| {
                room_state.extra_construction_sites.retain(|cs| cs.id != cs_id);
            });
        } else {
            // Waiting until the task becomes possible.
            debug!("Waiting 100 ticks until next attempt.");
            sleep(100).await;
        }
    }
}