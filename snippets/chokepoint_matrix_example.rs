fn example() {
    with_room_state(room_name, |state| {
        let cg = measure_time("chunk_graph", || chunk_graph(&state.terrain.to_obstacle_matrix(0), 7));
        let cm = measure_time("chokepoint_matrix", || {
            chokepoint_matrix(&cg, Direction::TopRight, 15, 49)
        });

        let displayed_matrix = cm.map(|_, (width, size)| {
            if game::time() % 6 == 0 {
                if width == 0 {
                    obstacle_cost()
                } else {
                    width
                }
            } else if game::time() % 6 == 3 {
                size
            } else if width <= 15 && size >= 49 {
                width
            } else {
                obstacle_cost()
            }
        });

        visualize(room_name, Visualization::Matrix(Box::new(displayed_matrix)));
    });
}