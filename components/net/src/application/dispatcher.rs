// Copyright (c) 2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::marker::PhantomData;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use protobuf;
use protocol;
use zmq;

use super::ApplicationState;
use config::{DispatcherCfg, RouterCfg};
use conn::{ConnErr, RouteClient};
use error::{Error, Result};
use socket::DEFAULT_CONTEXT;

/// Dispatchers connect to Message Queue Servers
pub trait Dispatcher: Sized + Send + 'static {
    type Config: DispatcherCfg + RouterCfg;
    type State: ApplicationState;

    fn dispatch(request: &mut protocol::Message, conn: &mut RouteClient, state: &mut Self::State);

    /// Callback to perform dispatcher initialization.
    ///
    /// The default implementation will take your initial state and convert it into the actual
    /// state of the worker. Override this function if you need to perform additional steps to
    /// initialize your worker state.
    #[allow(unused_mut)]
    fn init(mut state: Self::State) -> Self::State {
        state
    }
}

pub struct DispatcherPool<T: Dispatcher> {
    state: T::State,
    queue: Arc<String>,
    worker_count: usize,
    workers: Vec<mpsc::Receiver<()>>,
    marker: PhantomData<T>,
}

impl<T> DispatcherPool<T>
where
    T: Dispatcher,
{
    pub fn new<C>(queue: String, config: &C, state: T::State) -> Self
    where
        C: DispatcherCfg,
    {
        DispatcherPool {
            state: state,
            queue: Arc::new(queue),
            worker_count: config.worker_count(),
            workers: Vec::with_capacity(config.worker_count()),
            marker: PhantomData,
        }
    }

    /// Start a pool of message dispatchers.
    pub fn run(mut self) {
        for worker_id in 0..self.worker_count {
            self.spawn_dispatcher(worker_id);
        }
        thread::spawn(move || loop {
            for i in 0..self.worker_count {
                // Refactor this if/when the standard library ever stabilizes select for mpsc
                // https://doc.rust-lang.org/std/sync/mpsc/struct.Select.html
                match self.workers[i].try_recv() {
                    Err(mpsc::TryRecvError::Disconnected) => {
                        info!("Worker[{}] restarting...", i);
                        self.spawn_dispatcher(i);
                    }
                    Ok(msg) => warn!("Worker[{}] sent unexpected msg: {:?}", i, msg),
                    Err(mpsc::TryRecvError::Empty) => continue,
                }
            }
            thread::sleep(Duration::from_millis(500));
        });
    }

    fn spawn_dispatcher(&mut self, worker_id: usize) {
        let (tx, rx) = mpsc::sync_channel(1);
        let state = T::init(self.state.clone());
        let queue = self.queue.clone();
        thread::spawn(move || worker_run::<T>(tx, queue, state));
        if rx.recv().is_ok() {
            info!("Worker[{}] ready", worker_id);
            self.workers.insert(worker_id, rx);
        } else {
            error!("Worker[{}] failed to start", worker_id);
            self.workers.remove(worker_id);
        }
    }
}

fn worker_run<T>(rz: mpsc::SyncSender<()>, queue: Arc<String>, mut state: T::State)
where
    T: Dispatcher,
{
    debug_assert!(
        state.is_initialized(),
        "Dispatcher state not initialized! wrongfully \
        implements the `init()` callback or omits an override implementation where the default \
        implementation isn't enough to initialize the dispatcher's state?"
    );
    let mut message = protocol::Message::default();
    let mut conn = RouteClient::new().unwrap();
    conn.connect(&*queue).unwrap();
    rz.send(()).unwrap();
    loop {
        message.reset();
        match conn.wait_recv(&mut message, -1) {
            Ok(()) => trace!("dispatch, {:?}", message),
            Err(ConnErr::Shutdown(signal)) => break,
            Err(err) => {
                warn!("{}", err);
                continue;
            }
        }
        T::dispatch(&mut message, &mut conn, &mut state);
    }
}
