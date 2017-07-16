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

mod dispatcher;

use std::sync::Arc;

use core::os::process;
use protocol::{self, routesrv};
use uuid::Uuid;
use zmq::{self, Error as ZError};

pub use self::dispatcher::Dispatcher;
use self::dispatcher::DispatcherPool;
use config::{RouterCfg, ToAddrString};
use conn::{self, ConnErr};
use error::{Error, Result};
use socket::DEFAULT_CONTEXT;

enum RecvResult {
    /// Signals to the main loop which sockets have pending messages to be processed. `.0` signals
    /// that the router socket has messages while `.1` signals the dispatcher queue.
    OnMessage((bool, bool)),
    Shutdown,
    Timeout,
}

/// Apply to a struct containing worker state that will be passed as a mutable reference on each
/// call of `dispatch()` to an implementer of `Dispatcher`.
pub trait ApplicationState: Clone + Send {
    fn is_initialized(&self) -> bool;
}

pub struct Application<T: Dispatcher> {
    config: T::Config,
    /// Listening socket for `Dispatcher`s to connect to. Messages are proxied from `router_sock`
    /// to this socket and vice versa.
    dispatcher_sock: zmq::Socket,
    msg_buf: zmq::Message,
    /// Connection socket connected to one or more `RouteSrv`s. Message are proxied from this
    /// socket to `dispatcher_sock` and vice versa.
    router_sock: zmq::Socket,
}

impl<T> Application<T>
where
    T: Dispatcher,
{
    fn new(config: T::Config) -> Result<Self> {
        let net_ident = net_ident();
        let router_sock = (**DEFAULT_CONTEXT).as_mut().socket(zmq::DEALER)?;
        router_sock.set_identity(net_ident.as_bytes())?;
        router_sock.set_probe_router(true)?;
        let dispatcher_sock = (**DEFAULT_CONTEXT).as_mut().socket(zmq::DEALER).unwrap();
        Ok(Application {
            config: config,
            dispatcher_sock: dispatcher_sock,
            msg_buf: zmq::Message::new()?,
            router_sock: router_sock,
        })
    }

    fn run(mut self, state: T::State) -> Result<()> {
        for addr in self.config.route_addrs() {
            self.router_sock.connect(&addr.to_addr_string())?;
        }
        let ipc_addr = format!("inproc://net.dispatcher.{}", Uuid::new_v4());
        self.dispatcher_sock.bind(&ipc_addr)?;
        let mut heartbeat = protocol::Message::default();
        heartbeat.set_body(&routesrv::Heartbeat::new()).unwrap();
        DispatcherPool::<T>::new(ipc_addr, &self.config, state).run();
        info!("application is ready to go.");
        loop {
            trace!("waiting for message");
            match self.wait_recv() {
                RecvResult::OnMessage((router, dispatcher)) => {
                    trace!(
                        "received messages, router={}, dispatcher={}",
                        router,
                        dispatcher
                    );
                    // Handle completed work before new work
                    if dispatcher {
                        proxy_message(
                            &mut self.dispatcher_sock,
                            &mut self.router_sock,
                            &mut self.msg_buf,
                        )?;
                    }
                    if router {
                        proxy_message(
                            &mut self.router_sock,
                            &mut self.dispatcher_sock,
                            &mut self.msg_buf,
                        )?;
                    }
                }
                RecvResult::Timeout => {
                    trace!("recv timeout, sending heartbeat");
                    conn::send(&self.router_sock, &heartbeat)?;
                }
                RecvResult::Shutdown => {
                    info!("received shutdown signal, shutting down...");
                    break;
                }
            }
        }
        Ok(())
    }

    fn wait_recv(&mut self) -> RecvResult {
        let mut items = [
            self.router_sock.as_poll_item(zmq::POLLIN),
            self.dispatcher_sock.as_poll_item(zmq::POLLIN),
        ];
        match zmq::poll(&mut items, 30_000) {
            Ok(count) if count < 0 => unreachable!("zmq::poll, returned with a negative count"),
            Ok(count) if count == 0 => {
                println!("JW TODO: CASE 1");
                return RecvResult::Timeout;
            }
            Ok(_) => (),
            Err(ZError::EAGAIN) => {
                println!("JW TODO: CASE 2");
                return RecvResult::Timeout;
            }
            Err(ZError::EINTR) |
            Err(ZError::ETERM) => return RecvResult::Shutdown,
            Err(ZError::EFAULT) => panic!("zmq::poll, the provided _items_ was not valid (NULL)"),
            Err(err) => unreachable!("zmq::poll, returned an unexpected error, {:?}", err),
        }
        RecvResult::OnMessage((items[0].is_readable(), items[1].is_readable()))
    }
}

pub fn start<T>(cfg: T::Config, state: T::State) -> Result<()>
where
    T: Dispatcher,
{
    let app = Application::<T>::new(cfg)?;
    app.run(state)
}

fn net_ident() -> String {
    let hostname = super::hostname().unwrap();
    let pid = process::current_pid();
    format!("{}@{}", pid, hostname)
}

/// Proxy messages from one socket to another.
fn proxy_message(
    source: &mut zmq::Socket,
    destination: &mut zmq::Socket,
    buf: &mut zmq::Message,
) -> Result<()> {
    loop {
        match source.recv(buf, zmq::DONTWAIT) {
            Ok(()) => {
                let flags = if buf.get_more() { zmq::SNDMORE } else { 0 };
                destination.send(&*buf, flags).map_err(ConnErr::Socket)?;
            }
            Err(ZError::EAGAIN) => break,
            Err(err) => return Err(Error::from(ConnErr::Socket(err))),
        }
    }
    Ok(())
}
