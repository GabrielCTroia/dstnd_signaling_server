// Heavily inspired, taken from:
// https://www.steadylearner.com/blog/read/How-to-start-Rust-Chat-App
// https://github.com/housleyjk/ws-site-chat/blob/master/src/main.rs
// https://github.com/jonalmeida/ws-p2p/blob/master/src/handler.rs
//
// Also see:
// https://www.reddit.com/r/rust/comments/7drfuv/trying_to_understand_websockets/

use ws::{
    listen, CloseCode, Error, Handler, Handshake, Message, Request, Response, Result,
    Sender,
};

use std::cell::Cell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::{
    Arc,
    Mutex,
};

// Server web application handler
struct Server {
    sender: Sender,
    count: Rc<Cell<u32>>,
    me: Option<String>,
    connections_map: Arc<Mutex<HashMap<String, String>>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ConnectionOpenedPayload {
    address: String,
    connections_count: u32,
    connections: HashMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ConnectionClosedPayload {
    address: String,
    connections_count: u32,
    connections: HashMap<String, String>,
}

impl Handler for Server {
    fn on_request(&mut self, req: &Request) -> Result<(Response)> {
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

    fn on_open(&mut self, handshake: Handshake) -> Result<()> {
        let peer_id = &handshake.peer_addr.unwrap().to_string();

        self.me = Some(peer_id.to_string());

        println!("New Connection Opened {}", peer_id);

        // We have a new connection, so we increment the connection counter
        self.count.set(self.count.get() + 1);
        let number_of_connection = self.count.get();
        
        let mut connections = self.connections_map.lock().unwrap();

        connections.insert(peer_id.to_string(), peer_id.to_string());

        let payload = ConnectionOpenedPayload {
            address: peer_id.to_string(),
            connections_count: number_of_connection,
            connections: connections.clone(),
        };
        let serialized = serde_json::to_string(&payload).unwrap();

        println!("Response {}", &serialized);
        self.sender.broadcast(serialized);

        Ok(())
    }

    // Handle messages recieved in the websocket (in this case, only on /ws)
    fn on_message(&mut self, message: Message) -> Result<()> {
        let raw_message = message.into_text()?;
        println!("The message from the client is {:#?}", &raw_message);

        let message = if raw_message.contains("!warn") {
            let warn_message = "One of the clients sent warning to the server.";
            println!("{}", &warn_message);
            Message::Text("There was warning from another user.".to_string())
        } else {
            Message::Text(raw_message)
        };

        // Broadcast to all connections including other servers:
        // See: https://docs.rs/ws/0.9.1/ws/struct.Sender.html#method.broadcast
        self.sender.broadcast(message)
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
            self.count.set(self.count.get() - 1);

            let mut connections = self.connections_map.lock().unwrap();

            connections.remove(me);

            // Once a connecton closes broadcast it
            let number_of_connection = self.count.get();

            let payload = ConnectionClosedPayload {
                address: me.to_string(),
                connections_count: number_of_connection,
                connections: connections.clone(),
            };
            let serialized = serde_json::to_string(&payload).unwrap();

            println!("{}", &serialized);

            self.sender.broadcast(serialized);
        } else {
            println!("The previous connection attempted to close whith no active connection.");
        }
    }

    fn on_error(&mut self, err: Error) {
        println!("The server encountered an error: {:?}", err);
    }
}

pub fn websocket() -> () {
    println!("Web Socket Server is ready at ws://127.0.0.1:7777/ws");
    println!("Server is ready at http://127.0.0.1:7777/");

    // Rc is a reference-counted box for sharing the count between handlers
    // since each handler needs to own its contents.
    // Cell gives us interior mutability so we can increment
    // or decrement the count between handlers.

    // Listen on an address and call the closure for each connection
    let count = Rc::new(Cell::new(0));
    // let connections = Rc::new(Cell::new(vec![]));
    // let mut connections:Vec<String>  = vec![];
    let mut connections_map: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    listen("127.0.0.1:7777", |sender| Server {
        sender,
        count: count.clone(),
        connections_map: connections_map.clone(),
        me: None,
    })
    .unwrap()
}
