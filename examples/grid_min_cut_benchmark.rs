fn benchmark() {
    if game::cpu::bucket() > 500 {
        let spawn = game::spawns().values().next().unwrap_throw();
        let room_name = spawn.room().unwrap_throw().name();
        scan(room_name).unwrap_throw();
        with_room_state(room_name, |state| {
            let mut room_visual_ext = RoomVisualExt::new(room_name);
            let mut costs = RoomMatrix::new_custom_filled(10);
            for y in 0..ROOM_SIZE {
                for x in 0..ROOM_SIZE {
                    costs.set((x, y).try_into().unwrap_throw(), (1.0 + random() * 10.0) as u8);
                }
            }
            for y in 10..17 {
                for x in 10..35 {
                    costs.set((x, y).try_into().unwrap_throw(), 0);
                }
            }
            for i in 0..10 {
                let x = 3 + (random() * 44.0) as u8;
                let y = 3 + (random() * 44.0) as u8;
                costs.set((x, y).try_into().unwrap_throw(), 0);
                room_visual_ext.circle(x as f32, y as f32, None);
            }
            for xy in state.terrain.walls() {
                costs.set(xy, OBSTACLE_COST);
            }
            let min_cut = measure_time("grid_min_cut", || grid_min_cut(costs));
            debug!("Min cut size: {}", min_cut.len());
            for xy in min_cut.iter() {
                room_visual_ext.structure_roomxy(*xy, StructureType::Rampart, 1.0);
            }
        });
    }
}