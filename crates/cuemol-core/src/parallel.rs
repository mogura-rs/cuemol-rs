use std::sync::OnceLock;

pub fn parallel_enabled() -> bool {
    true
}

pub fn parallel_max_concurrency() -> usize {
    static CONCURRENCY: OnceLock<usize> = OnceLock::new();
    *CONCURRENCY.get_or_init(|| {
        let available = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1);
        std::env::var("CUEMOL_TBB_THREADS")
            .ok()
            .and_then(|value| value.parse::<isize>().ok())
            .filter(|value| *value >= 1)
            .map_or(available, |value| available.min(value as usize))
            .max(1)
    })
}

pub fn parallel_for<F>(begin: usize, end: usize, function: F)
where
    F: Fn(usize) + Sync,
{
    if begin >= end {
        return;
    }
    let workers = parallel_max_concurrency().min(end - begin);
    if workers <= 1 {
        for index in begin..end {
            function(index);
        }
        return;
    }
    let chunk_size = (end - begin).div_ceil(workers);
    std::thread::scope(|scope| {
        for worker in 0..workers {
            let chunk_begin = begin + worker * chunk_size;
            let chunk_end = (chunk_begin + chunk_size).min(end);
            if chunk_begin >= chunk_end {
                break;
            }
            let function = &function;
            scope.spawn(move || {
                for index in chunk_begin..chunk_end {
                    function(index);
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn covers_each_index_once() {
        assert!(parallel_enabled());
        let hits = (0..257).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
        parallel_for(0, hits.len(), |index| {
            hits[index].fetch_add(1, Ordering::Relaxed);
        });
        assert!(hits.iter().all(|count| count.load(Ordering::Relaxed) == 1));
    }
    #[test]
    fn empty_ranges_call_nothing() {
        let count = AtomicUsize::new(0);
        parallel_for(5, 5, |_| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        parallel_for(9, 2, |_| {
            count.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn worker_panics_propagate() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            parallel_for(0, 8, |index| assert_ne!(index, 3, "worker failure"))
        }));
        assert!(result.is_err());
    }
}
