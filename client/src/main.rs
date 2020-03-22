use actix_web::{error, http::header, middleware, web, App, Error, HttpResponse, HttpServer};
use std::collections::HashMap;
use tera::Tera;
use url::Url;

#[derive(Debug, Clone)]
struct Constants {
    authorize_uri: String,
    client_id: String,
    redirect_uris: Vec<String>,
}

fn constants() -> Constants {
    Constants {
        authorize_uri: "http://localhost:9001/authorize".to_string(),
        client_id: "oauth-client-1".to_string(),
        redirect_uris: vec!["http://localhost:9000/callback".to_string()],
    }
    .clone()
}

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

async fn authorize() -> Result<HttpResponse, Error> {
    let target_uri = Url::parse_with_params(
        constants().authorize_uri.as_str(),
        vec![
            ("response_type", "code"),
            ("client_id", constants().client_id.as_str()),
            ("redirect_uri", constants().redirect_uris[0].as_str()),
        ],
    )
    .map_err(|e| error::ErrorInternalServerError(e))?
    .to_string();

    Ok(HttpResponse::TemporaryRedirect()
        .header(header::LOCATION, target_uri)
        .finish())
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
    .bind("localhost:9000")?
    .run()
    .await
}
