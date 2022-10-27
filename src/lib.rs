use std::net::TcpListener;

use actix_web::{dev::Server, web, App, HttpResponse, HttpServer, Responder};

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    // Route::new().guard(guard::Get())
    let server = HttpServer::new(|| App::new().route("/health_check", web::get().to(health_check)))
        // .bind(address)?
        .listen(listener)?
        .run();

    Ok(server)
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().finish()
}
