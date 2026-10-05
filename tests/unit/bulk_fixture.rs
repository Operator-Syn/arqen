// SPDX-License-Identifier: MPL-2.0
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub struct Call {
    pub method: String,
    pub path: String,
    pub body: serde_json::Value,
}
pub struct Fixture {
    pub url: String,
    pub calls: Arc<Mutex<Vec<Call>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    pub fn new(
        handler: impl Fn(&Call) -> Option<(u16, serde_json::Value)> + Send + Sync + 'static,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let h = Arc::new(handler);
        let c = calls.clone();
        let s = stop.clone();
        let worker = thread::spawn(move || {
            let mut jobs = Vec::new();
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let h = h.clone();
                        let c = c.clone();
                        jobs.push(thread::spawn(move || serve(stream, &*h, &c)));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1))
                    }
                    Err(e) => panic!("fixture accept: {e}"),
                }
            }
            for job in jobs {
                job.join().unwrap();
            }
        });
        Self {
            url,
            calls,
            stop,
            worker: Some(worker),
        }
    }
    pub fn snapshot(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn serve(
    mut stream: TcpStream,
    handler: &impl Fn(&Call) -> Option<(u16, serde_json::Value)>,
    calls: &Mutex<Vec<Call>>,
) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    let (end, len) = loop {
        assert!(Instant::now() < deadline);
        let mut buf = [0; 4096];
        let n = stream.read(&mut buf).unwrap();
        assert_ne!(n, 0);
        bytes.extend_from_slice(&buf[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..i]);
            let len = header
                .lines()
                .find_map(|l| {
                    l.split_once(':')
                        .filter(|(n, _)| n.eq_ignore_ascii_case("content-length"))
                        .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            if bytes.len() >= i + 4 + len {
                break (i + 4, len);
            }
        }
    };
    let head = String::from_utf8_lossy(&bytes[..end]);
    let mut first = head.lines().next().unwrap().split_whitespace();
    let call = Call {
        method: first.next().unwrap().into(),
        path: first.next().unwrap().into(),
        body: if len == 0 {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes[end..end + len]).unwrap()
        },
    };
    calls.lock().unwrap().push(call.clone());
    if let Some((status, body)) = handler(&call) {
        let body = if body.is_null() {
            Vec::new()
        } else {
            serde_json::to_vec(&body).unwrap()
        };
        let header = format!(
            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(header.as_bytes());
        let _ = stream.write_all(&body);
    }
}
