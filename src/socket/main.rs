// Heavily inspired, taken from:
// https://www.steadylearner.com/blog/read/How-to-start-Rust-Chat-App
// https://github.com/housleyjk/ws-site-chat/blob/master/src/main.rs
// https://github.com/jonalmeida/ws-p2p/blob/master/src/handler.rs
//
// Also see:
// https://www.reddit.com/r/rust/comments/7drfuv/trying_to_understand_websockets/

use ws::{
    listen, CloseCode, Error, Handler, Handshake, Message, Request, Response, Result as WSResult,
    Sender,
};

// use std::error::Error;
use std::cell::Cell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use serde_json::{Result as SerdeResult, Value};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct Peer {
    sender: Sender,
    peer_addr: String,
}
// Server web application handler
struct Server {
    sender: Sender,
    me: Option<String>,
    peers: Arc<Mutex<HashMap<String, Peer>>>,
    
    // Deprecate
    // count: Rc<Cell<u32>>,
    // connections_map: Arc<Mutex<HashMap<String, String>>>,
}

enum MsgType {
    ConnectionOpened,
    PeerNetworkRefresh,
    P2PCommunication,
    PeerCall,
}

#[derive(Serialize, Deserialize, Debug)]
struct MsgEnvelope<Content> {
    msg_type: String,
    content: Content,
}

// msg_type: "connection_opened"
#[derive(Serialize, Deserialize, Debug)]
struct ConnectionOpenedMsgContent {
    my_address: String,
}

// msg_type: "network_status_change"
#[derive(Serialize, Deserialize, Debug)]
struct NetworkStatusChangeMsgContent {
    count: u32,
    peers: HashMap<String, String>,
}

// msg_type: "peer_call"
#[derive(Serialize, Deserialize, Debug)]
struct PeerCallMsgContent {
    peer_address: String,
}

// msg_type: "p2p_ommunication"
#[derive(Serialize, Deserialize, Debug)]
struct P2PCommunicationMsgContent {
    peer_address: String,
    forward: String,
}


fn serialize_msg<MsgContent: serde::Serialize>(
    msg_type: MsgType,
    content: MsgContent,
) -> String {
    let msg_type = match msg_type {
        MsgType::ConnectionOpened => "connection_opened",
        MsgType::PeerNetworkRefresh => "peer_network_refresh",
        MsgType::PeerCall => "peer_call",
        MsgType::P2PCommunication => "p2p_communication",
    };

    let msg = MsgEnvelope {
        msg_type: String::from(msg_type),
        content,
    };

    serde_json::to_string(&msg).unwrap()
}

type PeerAddressesMap = HashMap<String, String>;

trait PeerNetworkStatus {
    fn insert_peer_address(&self, addr: String);
    fn remove_peer_address(&self, addr: String);
    fn all_peer_addresses(&self) -> PeerAddressesMap;
}

impl PeerNetworkStatus for Server {
    fn insert_peer_address(&self, addr: String) {
        let mut peers = self.peers.lock().unwrap();

        peers.insert(addr.to_string(), Peer {
            peer_addr: addr.to_string(),
            sender: self.sender.clone(),
        });
    }

    fn remove_peer_address(&self, addr: String) {
        let mut peers = self.peers.lock().unwrap();
        
        peers.remove(&addr);
    }

    fn all_peer_addresses(&self) -> PeerAddressesMap {
        let peers = self.peers.lock().unwrap();

        let peers_address_map: HashMap<String, String> = peers.iter().map(|(key, _)| {
            // Since the peers already idexes by address I can simply just used that
            let peer_addr = String::from(key);
            (peer_addr.to_string(), peer_addr.to_string())
        }).collect();

        peers_address_map
    }
}

impl Handler for Server {
    fn on_request(&mut self, req: &Request) -> WSResult<(Response)> {
        match req.resource() {
            "/ws" => {
                // used once for const socket = new WebSocket("ws://" + window.location.host + "/ws");
                // https://blog.stanko.io/do-you-really-need-websockets-343aed40aa9b
                // and no need for reconnect later

                // https://ws-rs.org/api_docs/ws/struct.Request.html
                println!("Browser Request from {:?}", req.origin().unwrap().unwrap());
                println!("Client found is {:?}", req.client_addr().unwrap());

                let resp = Response::from_request(req);
                // println!("{:?} \n", &resp);
                resp
            }

            _ => Ok(Response::new(404, "Not Found", b"404 - Not Found".to_vec())),
        }
    }

    fn on_open(&mut self, handshake: Handshake) -> WSResult<()> {
        let me = &handshake.peer_addr.unwrap();
        println!("Connection opened for {:?}", me);

        // Send the ConnectionOpened Message to the "me" peer 
        let msg_to_me = serialize_msg(
            MsgType::ConnectionOpened,
            ConnectionOpenedMsgContent {
                my_address: me.to_string(),
            },
        );

        self.sender.send(msg_to_me.to_string());

        // Update the Network Status
        self.me = Some(me.to_string());
        self.insert_peer_address(me.to_string());
        
        let next_peers = self.all_peer_addresses();

        // Broadcast the Updated Network Status to all the peers
        let msg_to_all = serialize_msg(
            MsgType::PeerNetworkRefresh,
            NetworkStatusChangeMsgContent {
                count: next_peers.len() as u32,
                peers: next_peers,
            },
        );
        
        // Broadcast to all connections including other servers:
        // See: https://docs.rs/ws/0.9.1/ws/struct.Sender.html#method.broadcast
        self.sender.broadcast(msg_to_all);

        Ok(())
    }


    fn on_close(&mut self, code: CloseCode, reason: &str) {
        match code {
            CloseCode::Normal => println!("The client is done with the connection."),
            CloseCode::Away => println!("The client is leaving the site."),
            CloseCode::Abnormal => {
                println!("Closing handshake failed! Unable to obtain closing status from client.")
            }
            _ => println!("The client encountered an error: {}", reason),
        }

        if let Some(me) = self.me.as_ref() {
            self.remove_peer_address(me.to_string());

            let next_peers = self.all_peer_addresses();

            let msg_to_all = serialize_msg(
                MsgType::PeerNetworkRefresh,
                NetworkStatusChangeMsgContent {
                    count: next_peers.len() as u32,
                    peers: next_peers.clone(),
                }
            );

            // Broadcast to all connections including other servers:
            // See: https://docs.rs/ws/0.9.1/ws/struct.Sender.html#method.broadcast
            self.sender.broadcast(msg_to_all);
        } else {
            println!("The previous connection attempted to close whith no active connection.");
        }
    }

    // Handle messages recieved in the websocket (in this case, only on /ws)
    fn on_message(&mut self, message: Message) -> WSResult<()> {
        let raw_message = message.into_text()?;
        println!("The message from the client is {:#?}", &raw_message);
        // println!("Message received from client");

        // TODO This needs to be refactored a bit to pattern match per message_type
        //  But it's OK for now, until I get more knowledge on how the whole
        //  infrastructure looks like
        match serde_json::from_str::<MsgEnvelope<P2PCommunicationMsgContent>>(&raw_message) {
            Ok (msg) => {
                if msg.msg_type == "p2p_communication" {
                    // TODO: This should read it super fast so maybe no lock() if possible since it's just a read
                    if let Some(peer) = self.peers.lock().unwrap().get(&msg.content.peer_address) {
                        if let Some(me) = self.me.as_ref() {
                            // Set the peer_address to the "from" peer so it knows where to send it back
                            let msg_with_interchanged_peer = serialize_msg(
                                MsgType::P2PCommunication,
                                P2PCommunicationMsgContent {
                                    peer_address: me.to_string(),
                                    forward: msg.content.forward,
                                },
                            );

                            peer.sender.send(String::from(msg_with_interchanged_peer.clone()));

                            println!("Message forwarded to peer {:?}", msg_with_interchanged_peer);
                        } else {
                            println!("No self.me Error");
                        }
                    } else {
                        println!("Can't get peer by address, {}", msg.content.peer_address);
                    }
                } else {
                    println!("Message not p2p_communication, {:?}", msg);
                }
            },
            Err(e) => {
                println!("Not a P2PCommunicationMsgContent {:?}", e);
            }
        }

        Ok(())
    }

    fn on_error(&mut self, err: Error) {
        println!("The server encountered an error: {:?}", err);
    }
}

pub fn websocket() -> () {
    println!("Web Socket Server is ready at ws://127.0.0.1:7777/ws");
    println!("Server is ready at http://127.0.0.1:7777/");

    let mut peers: Arc<Mutex<HashMap<String, Peer>>> = Arc::new(Mutex::new(HashMap::new()));
    
    // Listen on an address and call the closure for each connection
    listen("127.0.0.1:7777", |sender| Server {
        sender,
        peers: peers.clone(),
        me: None,
    })
    .unwrap()
}
