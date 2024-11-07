use screeps::RoomName;
use crate::kernel::sleep::sleep;

pub async fn upgrade_controller(room_name: RoomName) {
    loop {
        // TODO create upgrading creeps
        // TODO schedule hauling of the energy
        // TODO before upgrading, build a container
        // TODO handle prioritizing energy for the upgrading - always upgrade enough to prevent
        //      the room from downgrading, but only upgrade more if there is energy to spare
        
        sleep(1).await;
    }
}