#[macro_use]
extern crate rocket;
use std::path::Path;

use backend_responses::*;
use rocket::fs::FileServer;
use rocket::serde::json::Json;
use rocket::response::status;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::Header;
use rocket::{Request, Response};

mod backend_responses;

#[post("/join_random_room", data = "<room_creation_request>")]
fn test(room_creation_request: Json<RoomCreationRequest>) -> Json<RoomCreationResponse> {
    let room_request = room_creation_request.into_inner();
    println!("Received request: {:?}", room_request);
    let response = RoomCreationResponse { success: true };
    Json(response)
}

/// Catches all OPTION requests in order to get the CORS related Fairing triggered.
#[options("/<_..>")]
fn all_options() -> status::NoContent {
    status::NoContent
}

#[launch]
fn rocket() -> _ {
    let rocket_app = rocket::build()
        .attach(Cors)
        .mount("/api", routes![test, all_options]);

    let file_server_path = "./websitesrc";

    if Path::new(file_server_path).exists() {
        rocket_app.mount("/", FileServer::from(file_server_path))
    } else {
        eprintln!("The file server path '{}' does not exist.", file_server_path);
        rocket_app
    } 
}

pub struct Cors;

#[rocket::async_trait]
impl Fairing for Cors {
    fn info(&self) -> Info {
        Info {
            name: "Cross-Origin-Resource-Sharing Fairing",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
        response.set_header(Header::new("Access-Control-Allow-Origin", "*"));
        response.set_header(Header::new(
            "Access-Control-Allow-Methods",
            "POST, PATCH, PUT, DELETE, HEAD, OPTIONS, GET",
        ));
        response.set_header(Header::new("Access-Control-Allow-Headers", "*"));
        response.set_header(Header::new("Access-Control-Allow-Credentials", "true"));
    }
}
