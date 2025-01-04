# event_work_server

This is a simple implementation of a server that integrates with [Rocket](https://crates.io/crates/rocket) to provide a web socket server for the [Bevy eventwork websocket mod](https://crates.io/crates/bevy_eventwork_mod_websockets).

## Sample code

The goal of this crate is to mimic the same interface that is seen by the original bevy_eventwork implementation with as little changes as possible.

Here is a sample showcasing all of the features with a Rocket web server:

```rust
use rocket::futures::lock::Mutex;
use std::sync::Arc;

use event_work_server::{EventWorkSender, EventWorkServer, NetworkEvent};
use rocket::State;
use server_responses::*;

#[macro_use]
extern crate rocket;

#[get("/")]
fn hello() -> &'static str {
    "Hello, world!"
}

#[get("/ws")]
fn websocket_connect<'r>(
    ws: ws::WebSocket,
    eventwork_server: &'r State<Arc<Mutex<EventWorkServer>>>,
) -> ws::Channel<'r> {
    ws.channel(move |stream| {
        Box::pin(async move {
            let server_listen_await_function_result = {
                eventwork_server
                    .lock()
                    .await
                    .handle_new_connection(stream)
                    .await
            };
            match server_listen_await_function_result {
                Ok(server_listen_function) => {
                    server_listen_function().await.unwrap();
                }
                Err(e) => {
                    eprintln!("Failed to handle connection: {}", e);
                }
            }
            Ok(())
        })
    })
}

// TODO: Add in a connections event handler

async fn sample_event_handler(mut sender: EventWorkSender) -> Result<(), String> {
    let sample_data = sender.get_network_data::<SampleEvent>()?;

    println!(
        "Received message from connection id {}: {:?}",
        sender.connection_id, sample_data
    );

    // Send a reply back
    match sender
        .send_message::<SampleEvent>(
            sender.connection_id,
            SampleEvent {
                value: format!(
                    "Hello from the server connection id: {}!",
                    sender.connection_id
                ),
            },
        )
        .await
    {
        Ok(_) => {
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to queue message: {}", e);
            Err(e.to_string())
        }
    }
}

async fn on_connection_handler(network_event: NetworkEvent) -> Result<(), String> {
    match network_event {
        NetworkEvent::Connected(connection_id) => {
            println!("Connection established with id: {}", connection_id);
            Ok(())
        }
        NetworkEvent::Disconnected(connection_id) => {
            println!("Connection closed with id: {}", connection_id);
            Ok(())
        }
    }
}

#[launch]
async fn rocket() -> _ {
    let mut eventwork_server = EventWorkServer::default();
    eventwork_server.init().await;

    if let Err(e) = eventwork_server
        .register_message::<SampleEvent>(Arc::new(|sender: EventWorkSender| {
            Box::pin(sample_event_handler(sender))
        }))
        .await
    {
        eprintln!("Failed to register message: {}", e);
    }

    eventwork_server
        .on_network_event(Arc::new(|network_event: NetworkEvent| {
            Box::pin(on_connection_handler(network_event))
        }))
        .await;

    rocket::build()
        .manage(Arc::new(Mutex::new(eventwork_server)))
        .mount("/", routes![hello, websocket_connect])
}
```

And server_responses could be a shared library like this:

```rust
#[derive(Debug, Event, Clone, Serialize, Deserialize, Default)]
pub struct SampleEvent {
    pub value: String,
}

impl NetworkMessage for SampleEvent {
    const NAME: &'static str = "SampleEvent";
}
```