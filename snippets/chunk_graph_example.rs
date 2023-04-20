fn example() {
    let spawn = game::spawns().values().next().unwrap_throw();
    let room_name = spawn.room().unwrap_throw().name();
    scan(room_name).unwrap();

    let cg = measure_time("chunk_graph", || {
        with_room_state(room_name, |state| {
            chunk_graph(&state.terrain.to_obstacle_matrix(0), 5)
        }).unwrap()
    });
    visualize(
        room_name,
        Visualization::Matrix(Box::new(cg.xy_chunks.map(|_, ix| ix.index() as u8))),
    );
    visualize(room_name, Visualization::Graph(cg.graph));
}