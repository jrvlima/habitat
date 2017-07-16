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

use protocol::{net, routesrv};
use protocol::message::{ErrCode, Message};

use super::ServerMap;
use conn::Conn;
use error::Result;

pub fn on_connect(conn: &Conn, message: &mut Message, servers: &mut ServerMap) -> Result<()> {
    let protocol = message.route_info().unwrap().protocol();
    let sender = message.sender()?.to_string();
    let shards = message.parse::<routesrv::Connect>()?.take_shards();
    if servers.add(protocol, sender, shards) {
        message.set_body(&net::NetOk::new())?;
        conn.send(message)?;
    } else {
        let err = net::err(ErrCode::REG_CONFLICT, "rt:connect:1");
        warn!("{}", err);
        message.set_body(&err)?;
        conn.send(message)?;
    }
    Ok(())
}

pub fn on_disconnect(_: &Conn, message: &mut Message, servers: &mut ServerMap) -> Result<()> {
    servers.drop(&message.route_info().unwrap().protocol(), message.sender()?);
    Ok(())
}

pub fn on_heartbeat(_: &Conn, message: &mut Message, servers: &mut ServerMap) -> Result<()> {
    servers.renew(message.sender()?);
    Ok(())
}
