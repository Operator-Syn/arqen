// SPDX-License-Identifier: MPL-2.0
use anyhow::{Context, Result, bail};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use uuid::Uuid;

const CALLBACK_PATH_PREFIX: &str = "/oauth2/callback?";
const LAUNCH_PATH_PREFIX: &str = "/oauth2/launch/";
const START_PATH_PREFIX: &str = "/oauth2/start/";

pub(crate) struct CallbackServer {
    redirect_uri: String,
    launcher_uri: String,
    authorization_url: Arc<Mutex<Option<String>>>,
    receiver: Receiver<Result<String, String>>,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

include!("listener.rs");
include!("lifecycle.rs");
include!("pages.rs");
include!("tests.rs");
