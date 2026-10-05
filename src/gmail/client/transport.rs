// SPDX-License-Identifier: MPL-2.0
use super::*;
use crate::gmail::concurrency::{Permit, ProviderGate};
use std::sync::Arc;
thread_local! { static WRITES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
pub(in crate::gmail) fn dispatched_writes() -> u64 {
    WRITES.with(|n| n.get())
}
#[derive(Debug, Clone)]
pub(super) struct GatedClient {
    inner: Client,
    gate: Arc<ProviderGate>,
}
impl From<Client> for GatedClient {
    fn from(inner: Client) -> Self {
        Self {
            inner,
            gate: Arc::new(ProviderGate::default()),
        }
    }
}
impl GatedClient {
    fn request(&self, method: reqwest::Method, url: Url) -> GatedRequest {
        let write = method != reqwest::Method::GET;
        GatedRequest {
            inner: self.inner.request(method, url),
            gate: self.gate.clone(),
            write,
        }
    }
    pub(super) fn get(&self, url: Url) -> GatedRequest {
        self.request(reqwest::Method::GET, url)
    }
    pub(super) fn post(&self, url: Url) -> GatedRequest {
        self.request(reqwest::Method::POST, url)
    }
    pub(super) fn delete(&self, url: Url) -> GatedRequest {
        self.request(reqwest::Method::DELETE, url)
    }
}
pub(super) struct GatedRequest {
    inner: reqwest::blocking::RequestBuilder,
    gate: Arc<ProviderGate>,
    write: bool,
}
impl GatedRequest {
    pub(super) fn bearer_auth(mut self, token: &str) -> Self {
        self.inner = self.inner.bearer_auth(token);
        self
    }
    pub(super) fn query<T: Serialize + ?Sized>(mut self, query: &T) -> Self {
        self.inner = self.inner.query(query);
        self
    }
    pub(super) fn json<T: Serialize + ?Sized>(mut self, value: &T) -> Self {
        self.inner = self.inner.json(value);
        self
    }
    pub(super) fn body(mut self, body: impl Into<reqwest::blocking::Body>) -> Self {
        self.inner = self.inner.body(body);
        self
    }
    pub(super) fn send(self) -> std::result::Result<GatedResponse, reqwest::Error> {
        let permit = self.gate.acquire();
        if self.write {
            WRITES.with(|n| n.set(n.get().wrapping_add(1)));
        }
        self.inner.send().map(|inner| GatedResponse {
            inner,
            _permit: permit,
        })
    }
}
pub(super) struct GatedResponse {
    inner: reqwest::blocking::Response,
    _permit: Permit,
}
impl GatedResponse {
    pub(super) fn status(&self) -> StatusCode {
        self.inner.status()
    }
    pub(super) fn content_length(&self) -> Option<u64> {
        self.inner.content_length()
    }
}
impl Read for GatedResponse {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}
