// HashMap + std RandomState completed in 3.42158900000004ms.
// HashMap + AHash completed in 4.735211000000049ms.
// IndexMap completed in 5.182648000000029ms.
// FxHashMap completed in 1.5896880000000237ms.
// FnvHashMap completed in 2.862468999999976ms.
// ahash = { version = "0.8.3", default-features = false, features = ["compile-time-rng"] }
// indexmap = "1.9.3"
// rustc-hash = "1.1.0"
// fnv = "1.0.7"

fn benchmark() {
    measure_time("HashMap + std RandomState", || {
        let mut hmap = HashMap::new();
        for i in 0..100000 {
            hmap.insert(i % 1743, i);
        }
        let mut hmap2 = HashMap::new();
        for (x, y) in hmap.into_iter() {
            hmap2.insert(x, y);
        }
        let mut a = 0;
        for &k in hmap2.keys() {
            a += k;
        }
        debug!("{}", a);
    });

    measure_time("HashMap + AHash", || {
        let mut hmap: HashMap<i32, i32, ahash::RandomState> = HashMap::default();
        for i in 0..100000 {
            hmap.insert(i % 1743, i);
        }
        let mut hmap2: HashMap<i32, i32, ahash::RandomState> = HashMap::default();
        for (x, y) in hmap.into_iter() {
            hmap2.insert(x, y);
        }
        let mut a = 0;
        for &k in hmap2.keys() {
            a += k;
        }
        debug!("{}", a);
    });

    measure_time("IndexMap", || {
        let mut hmap = IndexMap::new();
        for i in 0..100000 {
            hmap.insert(i % 1743, i);
        }
        let mut hmap2 = IndexMap::new();
        for (x, y) in hmap.into_iter() {
            hmap2.insert(x, y);
        }
        let mut a = 0;
        for &k in hmap2.keys() {
            a += k;
        }
        debug!("{}", a);
    });

    measure_time("FxHashMap", || {
        let mut hmap = FxHashMap::default();
        for i in 0..100000 {
            hmap.insert(i % 1743, i);
        }
        let mut hmap2 = FxHashMap::default();
        for (x, y) in hmap.into_iter() {
            hmap2.insert(x, y);
        }
        let mut a = 0;
        for &k in hmap2.keys() {
            a += k;
        }
        debug!("{}", a);
    });

    measure_time("FnvHashMap", || {
        let mut hmap = FnvHashMap::default();
        for i in 0..100000 {
            hmap.insert(i % 1743, i);
        }
        let mut hmap2 = FnvHashMap::default();
        for (x, y) in hmap.into_iter() {
            hmap2.insert(x, y);
        }
        let mut a = 0;
        for &k in hmap2.keys() {
            a += k;
        }
        debug!("{}", a);
    });
}