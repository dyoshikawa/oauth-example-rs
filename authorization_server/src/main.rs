use actix_web::{error, middleware, web, App, Error, HttpResponse, HttpServer};
use std::collections::HashMap;
use tera::Tera;

#[derive(Debug, Clone)]
struct Client {
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
    scope: String,
}

#[derive(Debug, Clone)]
struct Constants {
    clients: Vec<Client>,
}

fn constants() -> Constants {
    Constants {
        clients: vec![Client {
            client_id: "oauth-client-1".to_string(),
            client_secret: "oauth-client-secret-1".to_string(),
            redirect_uris: vec!["http://localhost:9000/callback".to_string()],
            scope: "foo bar".to_string(),
        }],
    }
}

async fn index(tmpl: web::Data<tera::Tera>) -> Result<HttpResponse, Error> {
    let mut ctx = tera::Context::new();
    ctx.insert("client_id", "oauth-client-1");
    ctx.insert("client_secret", "oauth-client-secret-1");
    ctx.insert("scope", "foo bar");
    ctx.insert("redirect_uri", "");
    let s = tmpl
        .render("index.html", &ctx)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

async fn authorize(
    tmpl: web::Data<tera::Tera>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    query.get("response_type");
    query.get("client_id");
    query.get("redirect_uri");
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
            .service(web::resource("/authorize").route(web::get().to(authorize)))
    })
    .bind("localhost:9001")?
    .run()
    .await
}
