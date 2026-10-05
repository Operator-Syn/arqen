// SPDX-License-Identifier: MPL-2.0
use super::*;
use std::{sync::mpsc, thread, time::Duration};
#[test]
fn bulk_executor_overlaps_and_preserves_order() {
    let (entered, events) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let r = release.clone();
    let worker = thread::spawn(move || {
        bounded_map(&[3, 2, 1, 0], |i| {
            entered.send(*i).unwrap();
            let (lock, cv) = &*r;
            let mut go = lock.lock().unwrap();
            while !*go {
                go = cv.wait(go).unwrap();
            }
            *i
        })
    });
    events.recv_timeout(Duration::from_secs(3)).unwrap();
    let overlap = events.recv_timeout(Duration::from_millis(200)).is_ok();
    {
        let (lock, cv) = &*release;
        *lock.lock().unwrap() = true;
        cv.notify_all();
    }
    assert_eq!(worker.join().unwrap(), vec![3, 2, 1, 0]);
    assert!(overlap, "independent provider work must overlap");
}
#[test]
fn bulk_executor_global_gate_bounds_and_releases() {
    let gate = Arc::new(ProviderGate::default());
    let held: Vec<_> = (0..8).map(|_| gate.acquire()).collect();
    let (send, recv) = mpsc::channel();
    let g = gate.clone();
    let worker = thread::spawn(move || {
        let _permit = g.acquire();
        send.send(()).unwrap();
    });
    let leaked = recv.recv_timeout(Duration::from_millis(100)).is_ok();
    drop(held);
    worker.join().unwrap();
    assert!(!leaked, "ninth request must wait");
    assert_eq!(*gate.active.lock().unwrap(), 0);
}
