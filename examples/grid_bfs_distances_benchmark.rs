// [1:22:56 AM]xi: avg dist (0, 0) - (25, 25): 111
// [1:22:56 AM]xi::profiler: bfs_distances completed in 84.30271500000163ms.
// [1:22:57 AM]xi: avg dist (0, 0) - (25, 25): 106.57
// [1:22:57 AM]xi::profiler: bfs_distances completed in 82.13927300000068ms.
// [1:22:58 AM]xi: avg dist (0, 0) - (25, 25): 108.82
// [1:22:58 AM]xi::profiler: bfs_distances completed in 83.25449500000104ms.
// [1:23:00 AM]xi: avg dist (0, 0) - (25, 25): 99.69
// [1:23:00 AM]xi::profiler: bfs_distances completed in 79.68064800000138ms.
// [1:23:01 AM]xi: avg dist (0, 0) - (25, 25): 118
// [1:23:01 AM]xi::profiler: bfs_distances completed in 83.07870600000024ms.
// [1:23:03 AM]xi: avg dist (0, 0) - (25, 25): 101.92
// [1:23:03 AM]xi::profiler: bfs_distances completed in 81.0615289999987ms.
// [1:23:05 AM]xi: avg dist (0, 0) - (25, 25): 97.56
// [1:23:05 AM]xi::profiler: bfs_distances completed in 83.25729900000078ms.
// [1:23:06 AM]xi: avg dist (0, 0) - (25, 25): 108.63
// [1:23:06 AM]xi::profiler: bfs_distances completed in 78.8240709999991ms.
// [1:23:08 AM]xi: avg dist (0, 0) - (25, 25): 106.67
// [1:23:08 AM]xi::profiler: bfs_distances completed in 82.60799399999814ms.

use nanorand::{Rng, WyRand};

fn benchmark() {
    if game::cpu::bucket() > 500 {
        measure_time("grid_bfs_distances", || {
            let n = 100;
            let number_of_obstacles = 1000;
            let mut total = 0.0;
            let mut rng = WyRand::new_seed(game::time() as u64);
            for i in 0..n {
                let obstacles: Vec<RoomXY> = (0..number_of_obstacles).map(|_| RoomXY::try_from((rng.generate_range(1..ROOM_SIZE), rng.generate_range(1..ROOM_SIZE))).unwrap()).collect();
                let start = [RoomXY::try_from((0, 0)).unwrap()];
                let result = grid_bfs_distances(start.iter(), obstacles.iter());
                total += result.get(RoomXY::try_from((25, 25)).unwrap()) as f64;
            }
            debug!("avg dist (0, 0) - (25, 25): {}", total / (n as f64));
        });
    }
}