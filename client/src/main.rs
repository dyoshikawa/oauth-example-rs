use actix_web::{error, middleware, web, App, Error, HttpResponse, HttpServer};
use std::collections::HashMap;
use tera::Tera;

async fn index(
    tmpl: web::Data<tera::Tera>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    ctx.insert(
        "access_token",
        query.get("access_token").unwrap_or(&"None".to_string()),
    );
    ctx.insert("scope", query.get("scope").unwrap_or(&"None".to_string()));
    let s = tmpl
        .render("index.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");

    HttpServer::new(|| {
        let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/template/**/*")).unwrap();

        App::new()
            .data(tera)
            .wrap(middleware::Logger::default()) // enable logger
            .service(web::resource("/").route(web::get().to(index)))
    })
    .bind("localhost:9000")?
    .run()
    .await
}
