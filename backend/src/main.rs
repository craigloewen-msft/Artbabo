#[macro_use] extern crate rocket;
use rocket::fs::FileServer;

#[get("/test")]
fn test() -> &'static str {
    "This is a test endpoint"
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/api", routes![test])
        .mount("/", FileServer::from("./websitesrc"))
}