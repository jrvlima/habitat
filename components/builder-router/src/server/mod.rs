// Copyright (c) 2016 Chef Software Inc. and/or applicable contributors
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

mod handlers;

use std::collections::HashMap;

use protocol::net;
use protocol::message::{ErrCode, Message, Protocol};
use protocol::sharding::{ShardId, SHARD_COUNT};
use rand::{self, Rng};
use time;
use zmq;

use config::Config;
use conn::{Conn, ConnErr};
use error::{Error, Result};

const PING_INTERVAL: i64 = 2_000;
const SERVER_TTL: i64 = 6_000;

pub struct Server {
    /// Currently loaded configuration.
    config: Config,
    /// ZeroMQ socket Context. Use this when creating new sockets if you wish them to be able to
    /// participate in the server's main thread.
    context: zmq::Context,
    /// Random seed used in calculating shard destinations.
    rng: rand::ThreadRng,
    /// Map of all registered servers and, if applicable, the shards they are hosting.
    servers: ServerMap,
}

impl Server {
    fn new(config: Config) -> Self {
        Server {
            config: config,
            context: zmq::Context::new(),
            rng: rand::thread_rng(),
            servers: ServerMap::default(),
        }
    }

    fn handle_message(&mut self, conn: &Conn, message: &mut Message) -> Result<()> {
        trace!("handle-message, {}", message);
        let handler = match message.message_id() {
            "Connect" => handlers::on_connect,
            "Disconnect" => handlers::on_disconnect,
            "Heartbeat" => handlers::on_heartbeat,
            message_id => {
                warn!("handle-message, recv unknown message, {}", message_id);
                return Ok(());
            }
        };
        handler(conn, message, &mut self.servers)
    }

    fn route_message(&mut self, conn: &Conn, message: &mut Message) {
        match message.route_info().unwrap().protocol() {
            Protocol::RouteSrv => {
                if let Err(err) = self.handle_message(&conn, message) {
                    error!("{}", err);
                }
            }
            Protocol::Net => warn!("route-message, recv unroutable message, {}", message),
            _ => {
                match self.select_shard(message) {
                    Some(identity) => {
                        if let Err(err) = conn.forward(message, identity.as_bytes().to_vec()) {
                            error!("{}", err);
                        }
                    }
                    None => {
                        let body = net::err(ErrCode::NO_SHARD, "rt:route:1");
                        error!("{}", body);
                        message.set_body(&body).unwrap();
                        if let Err(err) = conn.send_complete(message) {
                            error!("{}", err);
                        }
                    }
                }
            }
        }
    }

    fn run(&mut self) -> Result<()> {
        let mut conn = Conn::new(&mut self.context)?;
        let mut message = Message::default();
        conn.bind(&self.config.addr())?;
        println!("Listening on ({})", self.config.addr());
        info!("builder-router is ready to go.");
        loop {
            message.reset();
            match conn.wait_recv(&mut message, self.wait_timeout()) {
                Ok(()) => trace!("OnMessage, {}", message),
                Err(ConnErr::Shutdown(signal)) => {
                    info!("received shutdown signal ({}), shutting down...", signal);
                    break;
                }
                Err(err @ ConnErr::Socket(_)) => {
                    return Err(Error::from(err));
                }
                Err(ConnErr::Timeout) => continue,
                Err(err) => {
                    error!("{}", err);
                    continue;
                }
            }
            self.route_message(&conn, &mut message);
            // who the fuck do we timeout?
            // set next responsible time for a timeout
        }
        Ok(())
    }

    fn select_shard(&mut self, message: &Message) -> Option<&str> {
        let shard_id = match message.route_info().and_then(|m| m.hash()) {
            Some(hash) => (hash % SHARD_COUNT as u64) as u32,
            None => (self.rng.gen::<u64>() % SHARD_COUNT as u64) as u32,
        };
        self.servers
            .get(&message.route_info().unwrap().protocol(), &shard_id)
            .and_then(|m| Some(m.endpoint.as_str()))
    }

    fn wait_timeout(&self) -> i64 {
        // JW TODO: calculate this based on who we've heard from recently
        30_000
    }
}

#[derive(Default)]
pub struct ServerMap(HashMap<Protocol, HashMap<ShardId, ServerReg>>);

impl ServerMap {
    pub fn add(&mut self, protocol: Protocol, netid: String, shards: Vec<ShardId>) -> bool {
        if !self.0.contains_key(&protocol) {
            self.0.insert(protocol, HashMap::default());
        }
        let registrations = self.0.get_mut(&protocol).unwrap();
        for shard in shards.iter() {
            if let Some(reg) = registrations.get(&shard) {
                if &reg.endpoint != &netid {
                    return false;
                }
            }
        }
        let registration = ServerReg::new(netid);
        for shard in shards {
            registrations.insert(shard, registration.clone());
        }
        true
    }

    pub fn drop(&mut self, protocol: &Protocol, netid: &str) {
        if let Some(map) = self.0.get_mut(protocol) {
            map.retain(|_, reg| reg.endpoint != netid);
        }
    }

    pub fn get(&self, protocol: &Protocol, shard: &ShardId) -> Option<&ServerReg> {
        self.0.get(protocol).and_then(|shards| shards.get(shard))
    }

    pub fn renew(&mut self, netid: &str) {
        // JW TODO: We can't iterate like this every heartbeat
        for registrations in self.0.values_mut() {
            for registration in registrations.values_mut() {
                if registration.endpoint == netid {
                    registration.renew();
                }
            }
        }
    }
}

#[derive(Clone, Eq, Hash)]
pub struct ServerReg {
    /// Server identifier
    pub endpoint: String,
    /// True if known to be alive
    pub alive: bool,
    /// Next ping at this time
    pub ping_at: i64,
    /// Connection expires at this time
    pub expires: i64,
}

impl ServerReg {
    pub fn new(endpoint: String) -> Self {
        let now_ms = Self::clock_time();
        ServerReg {
            endpoint: endpoint,
            alive: false,
            ping_at: now_ms + PING_INTERVAL,
            expires: now_ms + SERVER_TTL,
        }
    }

    pub fn clock_time() -> i64 {
        let timespec = time::get_time();
        (timespec.sec as i64 * 1000) + (timespec.nsec as i64 / 1000 / 1000)
    }

    pub fn renew(&mut self) {
        let now_ms = Self::clock_time();
        self.ping_at = now_ms + PING_INTERVAL;
        self.expires = now_ms + SERVER_TTL;
    }
}

impl PartialEq for ServerReg {
    fn eq(&self, other: &ServerReg) -> bool {
        if self.endpoint != other.endpoint {
            return false;
        }
        true
    }
}

pub fn run(config: Config) -> Result<()> {
    Server::new(config).run()
}
