// SPDX-License-Identifier: MPL-2.0
use std::sync::{Arc, Condvar, Mutex};
#[derive(Debug, Default)]
pub(crate) struct ProviderGate {
    active: Mutex<usize>,
    changed: Condvar,
}
impl ProviderGate {
    pub(crate) fn acquire(self: &Arc<Self>) -> Permit {
        let mut active = self.active.lock().unwrap_or_else(|p| p.into_inner());
        while *active >= 8 {
            active = self.changed.wait(active).unwrap_or_else(|p| p.into_inner());
        }
        *active += 1;
        Permit(self.clone())
    }
}
pub(crate) struct Permit(Arc<ProviderGate>);
impl Drop for Permit {
    fn drop(&mut self) {
        let mut active = self.0.active.lock().unwrap_or_else(|p| p.into_inner());
        *active -= 1;
        self.0.changed.notify_one();
    }
}
pub(crate) fn bounded_map<T: Sync, R: Send>(
    inputs: &[T],
    operation: impl Fn(&T) -> R + Sync,
) -> Vec<R> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let results: Vec<_> = (0..inputs.len()).map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..inputs.len().min(4) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(input) = inputs.get(index) else {
                        break;
                    };
                    *results[index].lock().unwrap_or_else(|p| p.into_inner()) =
                        Some(operation(input));
                }
            });
        }
    });
    results
        .into_iter()
        .map(|r| {
            r.into_inner()
                .unwrap_or_else(|p| p.into_inner())
                .expect("worker completed without a result")
        })
        .collect()
}
#[cfg(test)]
#[path = "../../tests/unit/gmail_concurrency.rs"]
mod tests;
