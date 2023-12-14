use crossbeam_channel::{unbounded, Sender};
use std::thread;

use websocket::{sync::Server, OwnedMessage};

pub fn init_websockets() -> (thread::JoinHandle<()>, Sender<()>) {
    let (tx, rx) = unbounded::<()>();

    let thread = thread::spawn(move || {
        let server = Server::bind("127.0.0.1:2794").unwrap();

        for request in server.filter_map(Result::ok) {
            let rx = rx.clone();

            // spawn a new thread for each connection
            // the thread just waits for rx, then closes the connection
            thread::spawn(move || {
                if !request.protocols().contains(&"sorg".to_string()) {
                    request.reject().unwrap();
                    return;
                }

                let mut client = request.use_protocol("sorg").accept().unwrap();

                let _ = rx.recv();

                client
                    .send_message(&OwnedMessage::Text("reload".to_string()))
                    .unwrap();
            });
        }
    });

    (thread, tx)
}
