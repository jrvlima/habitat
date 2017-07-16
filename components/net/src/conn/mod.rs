// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
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

//! Contains types and functions for sending and receiving messages to and from a message broker
//! connected to one or more `RouteSrv`. All messages are routed through a `RouteSrv` and forwarded
//! to the appropriate receiver of a message.

mod error;

use std::ops::{Deref, DerefMut};
use std::result;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use fnv::FnvHasher;
use protobuf::{self, parse_from_bytes};
use protocol::{self, Message, ProtocolError, Routable, RouteKey};
use protocol::net::{ErrCode, NetError};
use zmq::{self, Error as ZError};

pub use self::error::ConnErr;
use config::{RouterAddr, ToAddrString};
use error::Error;
use socket::DEFAULT_CONTEXT;

pub type NetResult<T> = result::Result<T, NetError>;

/// Time to wait before timing out a message receive for a `RouteConn`.
const RECV_TIMEOUT_MS: i32 = 5_000;
/// Time to wait before timing out a message send for a `RouteBroker` to a router.
const SEND_TIMEOUT_MS: i32 = 5_000;
// ZeroMQ address for the application's RouteBroker's queue.
const ROUTE_INPROC_ADDR: &'static str = "inproc://route-broker";

/// A messaging RouteBroker for proxying messages from clients to one or more `RouteSrv` and vice
/// versa.
pub struct RouteBroker {
    client_sock: zmq::Socket,
    router_sock: zmq::Socket,
}

impl RouteBroker {
    /// Create a new `RouteBroker`
    ///
    /// # Errors
    ///
    /// * A socket cannot be created within the given `zmq::Context`
    /// * A socket cannot be configured
    ///
    /// # Panics
    ///
    /// * Could not read `zmq::Context` due to deadlock or poisoning
    fn new(net_ident: String) -> Result<Self, ConnErr> {
        let fe = (**DEFAULT_CONTEXT).as_mut().socket(zmq::ROUTER)?;
        let be = (**DEFAULT_CONTEXT).as_mut().socket(zmq::DEALER)?;
        fe.set_identity(net_ident.as_bytes())?;
        be.set_rcvtimeo(RECV_TIMEOUT_MS)?;
        be.set_sndtimeo(SEND_TIMEOUT_MS)?;
        be.set_immediate(true)?;
        Ok(RouteBroker {
            client_sock: fe,
            router_sock: be,
        })
    }

    /// Helper function for creating a new `RouteClient`.
    ///
    /// # Errors
    ///
    /// * Could not connect to `RouteBroker`
    /// * Could not create socket
    ///
    /// # Panics
    ///
    /// * Could not read `zmq::Context` due to deadlock or poisoning
    pub fn connect() -> Result<RouteClient, ConnErr> {
        let mut conn = RouteClient::new()?;
        conn.connect(ROUTE_INPROC_ADDR)?;
        Ok(conn)
    }

    /// Create a new `RouteBroker` and run it in a separate thread. This function will block the
    /// calling thread until the new broker has successfully started.
    ///
    /// # Panics
    ///
    /// * RouteBroker crashed during startup
    pub fn run(net_ident: String, routers: &Vec<RouterAddr>) -> JoinHandle<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        let addrs = routers.iter().map(|a| a.to_addr_string()).collect();
        let handle = thread::Builder::new()
            .name("router-broker".to_string())
            .spawn(move || {
                let mut broker = Self::new(net_ident).unwrap();
                broker.start(tx, addrs).unwrap();
            })
            .unwrap();
        match rx.recv() {
            Ok(()) => handle,
            Err(e) => panic!("router-broker thread startup error, err={}", e),
        }
    }

    // Main loop for `RouteBroker`.
    //
    // Binds front-end socket to ZeroMQ inproc address and connects to all routers. Sends a message
    // back to the caller over the given rendezvous channel to signal when ready.
    fn start(&mut self, rz: mpsc::SyncSender<()>, routers: Vec<String>) -> Result<(), ConnErr> {
        self.client_sock.bind(ROUTE_INPROC_ADDR)?;
        for addr in routers {
            self.router_sock.connect(&addr)?;
        }
        rz.send(()).unwrap();
        zmq::proxy(&mut self.client_sock, &mut self.router_sock)?;
        Ok(())
    }
}

/// Client connection for sending and receiving messages to and from the service cluster through
/// a running `RouteBroker`.
pub struct RouteClient(RouteConn);

impl RouteClient {
    /// Create a new `RouteClient`
    ///
    /// # Errors
    ///
    /// * Socket could not be created
    /// * Socket could not be configured
    pub fn new() -> Result<Self, ConnErr> {
        let socket = (**DEFAULT_CONTEXT).as_mut().socket(zmq::REQ)?;
        Ok(RouteClient(RouteConn::new(socket)?))
    }

    pub fn connect<T>(&self, endpoint: T) -> Result<(), ConnErr>
    where
        T: AsRef<str>,
    {
        self.0.connect(endpoint)?;
        Ok(())
    }

    /// Routes a message to the connected broker, through a router, and to appropriate service,
    /// waits for a response, and then parses and returns the value of the response.
    ///
    /// # Errors
    ///
    /// * One or more message frames cannot be sent to the RouteBroker's queue
    ///
    /// # Panics
    ///
    /// * Could not serialize message
    pub fn route<M, R>(&mut self, msg: &M) -> NetResult<R>
    where
        M: Routable,
        R: protobuf::MessageStatic,
    {
        unimplemented!()
        // if self.route_async(msg).is_err() {
        //     return Err(protocol::net::err(ErrCode::ZMQ, "net:route:1"));
        // }
        // match self.recv() {
        //     Ok(rep) => {
        //         if rep.get_message_id() == "NetError" {
        //             let err = parse_from_bytes(rep.get_body()).unwrap();
        //             return Err(err);
        //         }
        //         match parse_from_bytes::<R>(rep.get_body()) {
        //             Ok(entity) => Ok(entity),
        //             Err(err) => {
        //                 error!("route-recv bad-reply, err={}, reply={:?}", err, rep);
        //                 Err(protocol::net::err(ErrCode::BUG, "net:route:2"))
        //             }
        //         }
        //     }
        //     Err(Error::Zmq(zmq::Error::EAGAIN)) => {
        //         Err(protocol::net::err(ErrCode::TIMEOUT, "net:route:3"))
        //     }
        //     Err(Error::Zmq(err)) => {
        //         error!("route-recv, code={}, msg={}", err, err.to_raw());
        //         Err(protocol::net::err(ErrCode::ZMQ, "net:route:4"))
        //     }
        //     Err(err) => {
        //         error!("route-recv, err={:?}", err);
        //         Err(protocol::net::err(ErrCode::BUG, "net:route:5"))
        //     }
        // }
    }

    /// Asynchronously routes a message to the connected broker, through a router, and to
    /// appropriate service.
    ///
    /// # Errors
    ///
    /// * One or more message frames cannot be sent to the RouteBroker's queue
    ///
    /// # Panics
    ///
    /// * Could not serialize message
    pub fn route_async<M>(&mut self, msg: &M) -> Result<(), ConnErr>
    where
        M: Routable,
    {
        unimplemented!()
        // let route_hash = msg.route_key().map(
        //     |key| key.hash(&mut FnvHasher::default()),
        // );
        // let req = Message::new().routing(route_hash).body(msg).build()?;
        // let bytes = req.to_bytes()?;
        // self.sock.send_str(
        //     ControlFrame::Request.as_str(),
        //     zmq::SNDMORE,
        // )?;
        // self.sock.send(&bytes, 0)?;
        // Ok(())
    }

    /// Receives a message from the connected broker. This function will block the calling thread
    /// until a message is received or a timeout occurs.
    ///
    /// # Errors
    ///
    /// * `RouteBroker` Queue became unavailable
    /// * Message was not received within the timeout
    /// * Received an unparseable message
    fn recv(&mut self) -> Result<Message, ConnErr> {
        unimplemented!()
        // let envelope = self.sock.recv_msg(0)?;
        // let msg: protocol::net::Msg = parse_from_bytes(&envelope)?;
        // Ok(msg)
    }
}

pub struct RouteConn {
    /// Internal message buffer for receiving message parts.
    msg_buf: zmq::Message,
    /// Listening socket for all connections into the Router.
    socket: zmq::Socket,
}

impl RouteConn {
    pub fn new(mut socket: zmq::Socket) -> Result<Self, ConnErr> {
        socket.set_rcvtimeo(RECV_TIMEOUT_MS)?;
        socket.set_sndtimeo(SEND_TIMEOUT_MS)?;
        socket.set_immediate(true)?;
        Ok(RouteConn {
            msg_buf: zmq::Message::new()?,
            socket: socket,
        })
    }

    pub fn bind<T>(&self, endpoint: T) -> Result<(), ConnErr>
    where
        T: AsRef<str>,
    {
        Ok(self.socket.bind(endpoint.as_ref())?)
    }

    pub fn connect<T>(&self, endpoint: T) -> Result<(), ConnErr>
    where
        T: AsRef<str>,
    {
        self.socket.connect(endpoint.as_ref())?;
        Ok(())
    }

    pub fn send(&self, message: &Message) -> Result<(), ConnErr> {
        send(&self.socket, message)
    }

    pub fn send_complete(&self, message: &mut Message) -> Result<(), ConnErr> {
        send_complete(&self.socket, message)
    }

    /// Attempts to wait for a value on this receiver, returning an error if the corresponding
    /// connection has shutdown, or if it waits more than timeout.
    ///
    /// This function will always block the current thread if there is no data available and. Once
    /// a message is sent to the corresponding connection, the thread will wake up write the
    /// the contents into `message`.
    pub fn wait_recv(&mut self, message: &mut Message, timeout: i64) -> Result<(), ConnErr> {
        match self.socket.poll(zmq::POLLIN, timeout) {
            Ok(count) if count < 0 => unreachable!("zmq::poll, returned a negative count"),
            Ok(count) => {
                trace!("zmq::poll received '{}' POLLIN events", count);
                if let Err(err) = read_into(&self.socket, message, &mut self.msg_buf) {
                    if let Err(err) = read_until_end(&self.socket, &mut self.msg_buf) {
                        error!("error while reading to end of message, {}", err)
                    }
                    return Err(err);
                }
                Ok(())
            }
            Err(ZError::EAGAIN) => Err(ConnErr::Timeout),
            Err(e @ ZError::EINTR) |
            Err(e @ ZError::ETERM) => Err(ConnErr::Shutdown(e)),
            Err(ZError::EFAULT) => {
                unreachable!("zmq::poll, the provided _items_ was not valid (NULL)")
            }
            Err(err) => unreachable!("zmq::poll, returned an unexpected error, {:?}", err),
        }
    }
}

impl Deref for RouteClient {
    type Target = RouteConn;

    fn deref(&self) -> &RouteConn {
        &self.0
    }
}

impl DerefMut for RouteClient {
    fn deref_mut(&mut self) -> &mut RouteConn {
        &mut self.0
    }
}

pub fn send(socket: &zmq::Socket, message: &Message) -> Result<(), ConnErr> {
    debug!("send, {}", message);
    for identity in message.identities.iter() {
        socket.send_str(identity, zmq::SNDMORE)?;
    }
    socket.send(&[], zmq::SNDMORE)?;
    send_header(socket, message)?;
    send_route_info(socket, message)?;
    send_txn(socket, message)?;
    send_body(socket, message)
}

pub fn send_complete(socket: &zmq::Socket, message: &mut Message) -> Result<(), ConnErr> {
    // JW TODO: Don't panic, just error.
    message.txn_mut().unwrap().set_complete(true);
    send(socket, message)
}

fn read_into(
    socket: &zmq::Socket,
    message: &mut Message,
    buf: &mut zmq::Message,
) -> Result<(), ConnErr> {
    read_identity(socket, message, buf)?;
    read_header(socket, message, buf)?;
    read_route_info(socket, message, buf)?;
    read_txn(socket, message, buf)?;
    read_body(socket, message, buf)
}

fn read_identity(
    socket: &zmq::Socket,
    message: &mut Message,
    buf: &mut zmq::Message,
) -> Result<(), ConnErr> {
    // JW TODO: I need to figure out if this is a full message or a probe
    let mut first = true;
    loop {
        socket.recv(buf, 0)?;
        if buf.len() == 0 && first {
            return Err(ConnErr::NoIdentity);
        }
        if buf.len() == 0 {
            break;
        }
        match message.push_identity(buf.to_vec()) {
            Ok(()) => first = false,
            Err(ProtocolError::IdentityDecode(err)) => return Err(ConnErr::BadIdentity(err)),
            Err(err) => return Err(ConnErr::Protocol(err)),
        }
    }
    Ok(())
}

fn read_header(
    socket: &zmq::Socket,
    message: &mut Message,
    buf: &mut zmq::Message,
) -> Result<(), ConnErr> {
    socket.recv(buf, 0)?;
    message.set_header(&*buf)?;
    if !message.header().has_route_info() {
        return Err(ConnErr::NoRouteInfo);
    }
    Ok(())
}

fn read_route_info(
    socket: &zmq::Socket,
    message: &mut Message,
    buf: &mut zmq::Message,
) -> Result<(), ConnErr> {
    socket.recv(buf, 0)?;
    if buf.len() == 0 {
        return Err(ConnErr::NoRouteInfo);
    }
    message.set_routing(&*buf)?;
    Ok(())
}

fn read_txn(
    socket: &zmq::Socket,
    message: &mut Message,
    buf: &mut zmq::Message,
) -> Result<(), ConnErr> {
    if !message.header().has_txn() {
        return Ok(());
    }
    socket.recv(buf, 0)?;
    if buf.len() == 0 {
        return Err(ConnErr::NoTxn);
    }
    message.set_txn(&*buf)?;
    Ok(())
}

fn read_body(
    socket: &zmq::Socket,
    message: &mut Message,
    buf: &mut zmq::Message,
) -> Result<(), ConnErr> {
    socket.recv_into(&mut *message.body, 0)?;
    if message.body.len() == 0 {
        return Err(ConnErr::NoBody);
    }
    Ok(())
}

fn read_until_end(socket: &zmq::Socket, buf: &mut zmq::Message) -> Result<(), ConnErr> {
    loop {
        if !socket.get_rcvmore()? {
            break;
        }
        socket.recv(buf, 0)?;
    }
    Ok(())
}

fn send_body(socket: &zmq::Socket, message: &Message) -> Result<(), ConnErr> {
    socket.send(&*message.body, 0)?;
    Ok(())
}

fn send_header(socket: &zmq::Socket, message: &Message) -> Result<(), ConnErr> {
    let bytes = message.header().to_bytes()?;
    socket.send(&bytes, zmq::SNDMORE)?;
    Ok(())
}

fn send_route_info(socket: &zmq::Socket, message: &Message) -> Result<(), ConnErr> {
    let bytes = message.route_info().as_ref().unwrap().to_bytes()?;
    socket.send(&bytes, zmq::SNDMORE)?;
    Ok(())
}

fn send_txn(socket: &zmq::Socket, message: &Message) -> Result<(), ConnErr> {
    if let Some(txn) = message.txn() {
        let bytes = txn.to_bytes()?;
        socket.send(&bytes, zmq::SNDMORE)?;
    }
    Ok(())
}
