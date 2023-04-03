fn example() {
    let spawn = game::spawns().values().next().unwrap_throw();
    let room_name = spawn.room().unwrap_throw().name();
    scan(room_name).unwrap_throw();
    let visualizer = Visualizer {};
    // Takes around 3ms on the pserver.
    let cg = measure_time("chunk_graph", || {
        with_room_state(room_name, |state| {
            chunk_graph(&state.terrain.to_obstacle_matrix(), 5)
        }).unwrap()
    });
    visualizer.visualize(
        room_name,
        &Visualization::Matrix(cg.xy_chunks.map(|ix| ix.index() as u8)),
    );
    visualizer.visualize(room_name, &Visualization::Graph(cg.graph));
}