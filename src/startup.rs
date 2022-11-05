use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpServer};

use crate::routes::{health_check, subscribe};


pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    // Route::new().guard(guard::Get())
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
    })
    // .bind(address)?
    .listen(listener)?
    .run();

    Ok(server)
}
