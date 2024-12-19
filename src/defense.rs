use log::{info, warn};
use screeps::{find, game, StructureTower};
use screeps::game::get_object_by_id_typed;
use screeps::StructureType::Tower;
use crate::kernel::sleep::sleep;
use crate::room_states::room_states::{for_each_owned_room};
use crate::u;
use crate::utils::result_utils::ResultUtils;

pub async fn defend_rooms() {
    loop {
        for_each_owned_room(|room_name, room_state| {
            let room = u!(game::rooms().get(room_name));
                
            let enemies = room.find(find::HOSTILE_CREEPS, None);
            
            if let Some(enemy) = enemies.first() {
                info!("{} enemies present in room {}.", enemies.len(), room_name);
                
                for (_, id) in room_state.structures_with_type::<StructureTower>(Tower) {
                    if let Some(tower) = get_object_by_id_typed(&id) {
                        tower.attack(enemy).warn_if_err("Failed to attack the enemy.");
                    } else {
                        warn!("Failed to get the tower object.");
                    }
                }
            }
            
            
        });
        
        sleep(1).await;
    }
}