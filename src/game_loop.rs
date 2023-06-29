use crate::config::{FIRST_MEMORY_SAVE_TICK, LOG_LEVEL, MEMORY_SAVE_INTERVAL};
use crate::construction::construct_structures;
use crate::game_time::{first_tick, game_tick};
use crate::global_state::{load_global_state, save_global_state};
use crate::maintenance::maintain_rooms;
use crate::priorities::{CONSTRUCTING_STRUCTURES_PRIORITY, MOVE_CREEPS_PRIORITY, ROOM_MAINTENANCE_PRIORITY, ROOM_PLANNING_PRIORITY, ROOM_SCANNING_PRIORITY, VISUALIZATIONS_PRIORITY};
use crate::room_planner::plan_rooms::plan_rooms;
use crate::room_state::scan_rooms::scan_rooms;
use crate::visualization::show_visualizations::show_visualizations;
use log::info;
use screeps::game;
use crate::{kernel, logging};
use crate::travel::move_creeps;

pub fn setup() {
    logging::init_logging(LOG_LEVEL);

    load_global_state();

    drop(kernel::schedule("scan_rooms", ROOM_SCANNING_PRIORITY, scan_rooms()));
    drop(kernel::schedule("plan_rooms", ROOM_PLANNING_PRIORITY, plan_rooms()));
    drop(kernel::schedule(
        "construct_structures",
        CONSTRUCTING_STRUCTURES_PRIORITY,
        construct_structures(),
    ));
    drop(kernel::schedule(
        "maintain_rooms",
        ROOM_MAINTENANCE_PRIORITY,
        maintain_rooms(),
    ));
    drop(kernel::schedule(
        "move_creeps",
        MOVE_CREEPS_PRIORITY,
        move_creeps()
    ));
    drop(kernel::schedule(
        "show_visualizations",
        VISUALIZATIONS_PRIORITY,
        show_visualizations(),
    ));
}

// pub static mut S_PLANNER: Option<RoomPlanner> = None;

pub fn game_loop() {
    let ticks_since_restart = game_tick() - first_tick();

    info!(
        "[ξ] Tick: {} / {} -- CPU: {}/{}",
        ticks_since_restart,
        game::time(),
        game::cpu::tick_limit(),
        game::cpu::bucket()
    );

    if ticks_since_restart == 0 {
        info!("Initialization used {}CPU.", game::cpu::get_used());
    }

    kernel::wake_up_sleeping_processes();
    kernel::run_processes();

    if ticks_since_restart >= FIRST_MEMORY_SAVE_TICK && ticks_since_restart % MEMORY_SAVE_INTERVAL == 0 {
        save_global_state();
    }

    // if game::cpu::bucket() > 1000 {
    //     measure_time("test", || {
    //         let spawn = game::spawns().values().next().unwrap_throw();
    //         let room_name = spawn.room().unwrap_throw().name();
    //         scan(room_name).unwrap_throw();

    //     let mut matrix = RoomMatrix::new(1u8);
    //     with_room_state(room_name, |state| {
    //         for (xy, t) in state.terrain.iter() {
    //             if t == Wall {
    //                 matrix.set(xy, obstacle_cost());
    //             }
    //         }
    //     });
    //
    //     let dt = distance_transform_from_obstacles(matrix.find_xy(obstacle_cost()));
    //
    //     let empty = dt.iter().filter_map(|(xy, dist)| (xy.exit_distance() >= 6 && dist >= 2).then_some(xy)).collect::<Vec<_>>();
    //     let xy = empty[(game::time() / 10).wrapping_mul(game::time() / 10 + 328647) as usize % empty.len()];
    //     let xy_dists = count_restricted_distance_matrix(matrix.find_xy(obstacle_cost()).chain(room_rect().iter().filter(|xy| xy.exit_distance() <= 5)), xy, 120);
    //     for (xy, dist) in xy_dists.iter() {
    //         if dist < unreachable_cost() {
    //             matrix.set(xy, 0);
    //         }
    //     }
    //
    //     with_room_state(room_name, |state| {
    //         for resource_xy in once(state.controller.unwrap().xy).chain(state.sources.iter().copied().map(|source| source.xy)) {
    //             let dm = distance_matrix(matrix.find_xy(obstacle_cost()), once(resource_xy));
    //             let path = shortest_path_by_matrix(&dm, xy, 1);
    //             for path_xy in path {
    //                 matrix.set(path_xy, 0);
    //             }
    //         }
    //     });
    //
    //     let init_dists = distance_matrix(matrix.find_xy(obstacle_cost()), matrix.find_xy(0));
    //     let mut min_cut_matrix = init_dists.map(|xy, dist| {
    //         if dist == 0 || dist == obstacle_cost::<u8>() {
    //             dist
    //         } else if dist < 5 {
    //             10
    //         } else {
    //             5 + dist
    //         }
    //     });
    //
    //     let min_cut = measure_time("grid_min_cut", || { grid_min_cut(&min_cut_matrix) });
    //     for xy in min_cut.iter().copied() {
    //         min_cut_matrix.set(xy, 200);
    //     }
    //
    //     visualize(room_name, Visualization::Matrix(Box::new(min_cut_matrix)));

    // with_room_state(room_name, |state| {
    //     let r = 6;
    //     let r2 = 1;
    //     let mut area_transform = RoomMatrix::new(obstacle_cost());
    //     for xy in state.terrain.not_walls() {
    //         let b = ball(xy, r);
    //         let xy_walls = b.iter().filter(|&p| state.terrain.get(p) == Wall);
    //         let xy_dm = rect_restricted_distance_matrix(xy_walls, once(xy), b, r);
    //         let space_around_dm = min(xy_dm.iter().filter(|&(xy, dist)| dist < unreachable_cost()).count(), 250) as u8;
    //         area_transform.set(xy, space_around_dm);
    //     }
    //     let max_area_transform = area_transform.map(|xy, v| {
    //         if v > 250 {
    //             v
    //         } else {
    //             let b = ball(xy, r2);
    //             unwrap!(b.iter().map(|b_xy| area_transform.get(b_xy)).filter(|&v| v <= 250).max())
    //         }
    //     });
    //     visualize(
    //         room_name,
    //         Visualization::Matrix(Box::new(
    //             max_area_transform
    //         )),
    //     );
    // });

    // with_room_state(room_name, |state| {
    //     let mut wall_distances = RoomMatrix::new(obstacle_cost());
    //     for xy in state.terrain.not_walls() {
    //         let mut dx1 = 1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((dx1, 0));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dx1 += 1;
    //             }
    //         }
    //
    //         let mut dx2 = -1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((dx2, 0));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dx2 -= 1;
    //             }
    //         }
    //
    //         let mut dy1 =1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((0, dy1));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dy1 += 1;
    //             }
    //         }
    //
    //         let mut dy2 = -1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((0, dy2));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dy2 -= 1;
    //             }
    //         }
    //
    //         // TODO propagate corner rampart distances
    //         // TODO exits should count for a large distance
    //
    //         let mut dists = [dx1.abs(), dx2.abs(), dy1.abs(), dy2.abs()];
    //         dists.sort();
    //         wall_distances.set(xy, (dists[0] + dists[1] - 1) as u8);
    //
    //         // let mut dists = [dx1.abs() + dx2.abs(), dy1.abs() + dy2.abs()];
    //         // dists.sort();
    //         // wall_distances.set(xy, (dists[0] - 1) as u8);
    //     }
    //     visualize(
    //         room_name,
    //         Visualization::Matrix(Box::new(
    //             wall_distances
    //         )),
    //     );
    // });

    // with_room_state(room_name, |state| {
    //     let mut wall_distances = RoomMatrix::new(obstacle_cost());
    //     for xy in state.terrain.not_walls() {
    //         let mut dx1 = 1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((dx1, 0));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dx1 += 1;
    //             }
    //         }
    //
    //         let mut dx2 = -1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((dx2, 0));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dx2 -= 1;
    //             }
    //         }
    //
    //         let mut dy1 =1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((0, dy1));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dy1 += 1;
    //             }
    //         }
    //
    //         let mut dy2 = -1i8;
    //         loop {
    //             let xy1 = xy.try_add_diff((0, dy2));
    //             if xy1.is_err() || state.terrain.get(xy1.unwrap()) == Wall {
    //                 break;
    //             } else {
    //                 dy2 -= 1;
    //             }
    //         }
    //
    //         // TODO propagate corner rampart distances
    //         // TODO exits should count for a large distance
    //
    //         let mut dists_and_dirs = [(dy2.abs(), Top), (dx1.abs(), Right), (dy1.abs(), Bottom), (dx2.abs(), Left)];
    //         dists_and_dirs.sort_by_key(|&(d, _)| d);
    //
    //         if (dists_and_dirs[0].1 == Bottom || dists_and_dirs[1].1 == Bottom) && (dists_and_dirs[0].1 == Right || dists_and_dirs[1].1 == Right) {
    //             if let Ok(rect_bottom_right) = xy.try_add_diff((dx1 + 4, dy1 + 4)) {
    //                 let rect = unwrap!(Rect::new(xy, rect_bottom_right));
    //                 let mut rampart_xys = vec![xy];
    //                 for dy in 1..dy1.abs() {
    //                     rampart_xys.push(unsafe { xy.add_diff((0, dy)) });
    //                 }
    //                 for dx in 1..dx1.abs() {
    //                     rampart_xys.push(unsafe { xy.add_diff((dx, 0)) });
    //                 }
    //                 let rrdm = rect_restricted_distance_matrix(rect.iter().filter(|&xy| state.terrain.get(xy) == Wall), rampart_xys.into_iter(), rect, 6);
    //                 let enclosed = rrdm.iter().filter(|&(_, d)| d < unreachable_cost()).count();
    //                 // wall_distances.set(xy, min(enclosed, 50) as u8);
    //             }
    //         } else if (dists_and_dirs[0].1 == Bottom || dists_and_dirs[1].1 == Bottom) && (dists_and_dirs[0].1 == Top || dists_and_dirs[1].1 == Top) {
    //             let rect_top_left = xy.saturated_add_diff((-6, dy2 - 4));
    //             let rect_bottom_right = xy.saturated_add_diff((0, dy1 + 4));
    //
    //             let rect = unwrap!(Rect::new(rect_top_left, rect_bottom_right));
    //             let mut rampart_xys = vec![xy];
    //             for dy in 1..dy1.abs() {
    //                 rampart_xys.push(unsafe { xy.add_diff((0, dy)) });
    //             }
    //             for dy in 1..dy2.abs() {
    //                 rampart_xys.push(unsafe { xy.add_diff((-dy, 0)) });
    //             }
    //             let rrdm = rect_restricted_distance_matrix(rect.iter().filter(|&xy| state.terrain.get(xy) == Wall), rampart_xys.into_iter(), rect, 6);
    //             let enclosed = rrdm.iter().filter(|&(_, d)| d < unreachable_cost()).count();
    //             wall_distances.set(xy, min(enclosed, 50) as u8);
    //             // TODO make this search custom for optimization and allow for early stop if another chunk is found
    //         }
    //     }
    //     visualize(
    //         room_name,
    //         Visualization::Matrix(Box::new(
    //             wall_distances
    //         )),
    //     );
    // });

    // with_room_state(room_name, |state| {
    //     let cg = measure_time("chunk_graph", || chunk_graph(&state.terrain.to_obstacle_matrix(0), 7));
    //     let cm = measure_time("chokepoint_matrix", || {
    //         chokepoint_matrix(&cg, Direction::BottomRight, 15, 49)
    //     });
    //
    //     let displayed_matrix = cm.map(|_, (width, size)| {
    //         if game::time() % 6 == 0 {
    //             if width == 0 {
    //                 obstacle_cost()
    //             } else {
    //                 width
    //             }
    //         } else if game::time() % 6 == 3 {
    //             size
    //         } else if width <= 15 && size >= 49 {
    //             width
    //         } else {
    //             obstacle_cost()
    //         }
    //     });
    //
    //     visualize(room_name, Visualization::Matrix(Box::new(displayed_matrix)));
    // });

    // let cg = measure_time("chunk_graph", || {
    //     with_room_state(room_name, |state| {
    //         chunk_graph(&state.terrain.to_obstacle_matrix(0), 4)
    //     }).unwrap()
    // });
    //
    // let hard_chokepoints = cg.hard_chokepoints();
    // let enclosed = cg.enclosed();
    //
    // let mut node_values = FxHashMap::default();
    // for node in cg.graph.node_indices() {
    //     let mut near_nodes = FxHashSet::default();
    //     let mut near_nodes_size = *unwrap!(cg.chunk_sizes.get(&node));
    //     near_nodes.insert(node);
    //     for near_node in cg.graph.neighbors(node) {
    //         near_nodes.insert(near_node);
    //         near_nodes_size += unwrap!(cg.chunk_sizes.get(&near_node));
    //     }
    //     let mut near_near_nodes = FxHashSet::default();
    //     for near_node in cg.graph.neighbors(node) {
    //         for near_near_node in cg.graph.neighbors(near_node) {
    //             if !near_nodes.contains(&near_near_node) {
    //                 near_near_nodes.insert(near_near_node);
    //             }
    //         }
    //     }
    //
    //     node_values.insert(node, format!("{} / {} / {}", node.index(), near_nodes_size, if hard_chokepoints.contains(&node) { "chokepoint" } else if enclosed.contains(&node) { "enclosure" } else { "-" }));
    // }
    //
    // visualize(room_name, Visualization::Graph(cg.graph.clone()));
    // visualize(room_name, Visualization::NodeLabels(cg.graph, node_values));

    // let mut cost_matrix = with_room_state(room_name, |state| state.terrain.to_cost_matrix(1)).unwrap();
    // let core_rect = Rect::new_unordered((15, 23).try_into().unwrap(), (19, 27).try_into().unwrap());
    // for xy in core_rect.iter() {
    //     cost_matrix.set(xy, obstacle_cost());
    //     RoomVisualExt::new(room_name).structure_roomxy(xy, Extension, 1.0);
    // }
    // let mut vis = RoomVisualExt::new(room_name);
    // vis.structure(2.0, 1.0, Spawn, 1.0);
    // vis.structure(10.0, 0.0, Link, 1.0);
    // vis.structure(34.0, 3.0, Tower, 1.0);
    // vis.structure(40.0, 11.0, Storage, 1.0);
    // let preference_matrix = RoomMatrix::new(0).map(|xy, _| (xy.x.u8() + xy.y.u8()) % 2);
    // let path_specs = vec![
    //     PathSpec::new(vec![(17, 23).try_into().unwrap(), (19, 25).try_into().unwrap(), (15, 25).try_into().unwrap()], (2, 1).try_into().unwrap(), 1),
    //     PathSpec::new(vec![(17, 23).try_into().unwrap(), (19, 25).try_into().unwrap(), (15, 25).try_into().unwrap()], (34, 3).try_into().unwrap(), 3),
    //     PathSpec::new(vec![(19, 27).try_into().unwrap()], (40, 11).try_into().unwrap(), 1),
    //     PathSpec::new(vec![(17, 23).try_into().unwrap(), (19, 25).try_into().unwrap(), (15, 25).try_into().unwrap()], (10, 0).try_into().unwrap(), 1),
    // ];
    // if let Some(paths) = minimal_shortest_paths_tree(&cost_matrix, &preference_matrix, &path_specs) {
    //     for path in paths {
    //         for xy in path.path {
    //             vis.structure_roomxy(xy, Road, 1.0);
    //         }
    //         vis.circle(path.target.x.u8() as f32, path.target.y.u8() as f32, None);
    //     }
    // }

    //     if unsafe { S_PLANNER.is_none() } {
    //         let maybe_planner = measure_time("RoomPlanner::new", || {
    //             with_room_state(room_name, |state| RoomPlanner::new(state, true)).unwrap()
    //         });
    //         match maybe_planner {
    //             Ok(new_planner) => unsafe {
    //                 S_PLANNER = Some(new_planner);
    //             },
    //             Err(e) => debug!("{}", e),
    //         }
    //     }
    //     unsafe {
    //         if let Some(planner) = S_PLANNER.as_mut() {
    //             if /* planner.best_plan.is_some() */ planner.is_finished() || game::time() % 4 != 0 {
    //                 if planner.is_finished() && game::time() % 4 == 3 {
    //                     debug!("Restarting the planner.");
    //                     S_PLANNER = None;
    //                 }
    //                 if let Some(plan) = planner.best_plan.clone() {
    //                     visualize(room_name, Visualization::Structures(plan.tiles.to_structures_map()));
    //                     visualize(
    //                         room_name,
    //                         Visualization::Text(format!(
    //                             "{:.2} E/t,   {:.3} CPU/t,   {:.0} DEF,   {:.2} SCORE",
    //                             plan.score.energy_balance,
    //                             plan.score.cpu_cost,
    //                             plan.score.def_score,
    //                             plan.score.total_score
    //                         )),
    //                     );
    //                 }
    //             } else {
    //                 let plan_result = measure_time("RoomPlanner::plan", || planner.plan());
    //                 // if ticks_since_restart < 2 {
    //                 //     planner.best_plan = None;
    //                 // } else {
    //                     match plan_result {
    //                         Ok(plan) => {
    //                             visualize(room_name, Visualization::Structures(plan.tiles.to_structures_map()));
    //                             visualize(
    //                                 room_name,
    //                                 Visualization::Text(format!(
    //                                     "{:.2} E/t,   {:.3} CPU/t,   {:.0} DEF,   {:.2} SCORE",
    //                                     plan.score.energy_balance,
    //                                     plan.score.cpu_cost,
    //                                     plan.score.def_score,
    //                                     plan.score.total_score
    //                                 )),
    //                             );
    //                         }
    //                         Err(e) => debug!("{}", e),
    //                     };
    //                 // }
    //             }
    //         } else {
    //             debug!("Planner not found.");
    //         }
    //     }
    // });
    // }
}
