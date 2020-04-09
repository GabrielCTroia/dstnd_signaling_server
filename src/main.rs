#![feature(
    proc_macro_hygiene, 
    decl_macro
)]

#[macro_use] extern crate rocket;

use std::thread;

extern crate ws;

mod socket;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

fn main() {
    thread::Builder::new()
        .name("Thread for Rust Chat with ws-rs".into())
        // .stack_size(83886 * 1024) // 80mib in killobytes
        .spawn(|| {
            socket::main::websocket();
        })
        .unwrap();

    rocket::ignite().mount("/", routes![index]).launch();
}
