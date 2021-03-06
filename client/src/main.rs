use actix_web::{error, http::header, middleware, web, App, Error, HttpResponse, HttpServer};
use base64::encode;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use tera::Tera;
use url::Url;

#[derive(Debug, Clone)]
struct Constants {
    authorize_endpoint: String,
    token_endpoint: String,
    client_id: String,
    client_secret: String,
    redirect_uris: Vec<String>,
}

fn constants() -> Constants {
    Constants {
        authorize_endpoint: "http://localhost:9001/authorize".to_string(),
        token_endpoint: "http://localhost:9001/token".to_string(),
        client_id: "oauth-client-1".to_string(),
        client_secret: "oauth-client-secret-1".to_string(),
        redirect_uris: vec!["http://localhost:9000/callback".to_string()],
    }
    .clone()
}

async fn index(
    tmpl: web::Data<tera::Tera>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    println!("Access index");
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
        constants().authorize_endpoint.as_str(),
        vec![
            ("response_type", "code"),
            ("client_id", constants().client_id.as_str()),
            ("redirect_uri", constants().redirect_uris[0].as_str()),
        ],
    )
    .map_err(|e| error::ErrorInternalServerError(e))?
    .to_string();

    Ok(HttpResponse::SeeOther()
        .header(header::LOCATION, target_uri)
        .finish())
}

fn encode_client_credentials(client_id: String, client_secret: String) -> String {
    encode(&format!("{}:{}", client_id, client_secret))
}

#[derive(Serialize, Deserialize)]
struct TokenPostParams {
    grant_type: String,
    code: String,
    redirect_uri: String,
}

async fn callback(
    tmpl: web::Data<tera::Tera>,
    query: web::Query<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    println!("Access callback");
    match query.get("error").cloned() {
        Some(error) => Err(error::ErrorInternalServerError(error)),
        None => {
            let state = "".to_string(); // TODO 仮の値
            let query_state = query.get("state").cloned().unwrap_or("".to_string());
            if query_state != state {
                return Err(error::ErrorBadRequest("State value did not match"));
            }

            let code = query.get("code").cloned().unwrap_or("".to_string());
            let post_data = serde_json::to_string(&TokenPostParams {
                grant_type: "authorization_code".to_string(),
                code: code.clone(),
                redirect_uri: constants().redirect_uris[0].clone(),
            })
            .map_err(|e| error::ErrorInternalServerError(e))?;

            println!("Requesting access token for code {}", &code);

            let req_client = reqwest::Client::new();
            let token_res = req_client
                .post(constants().token_endpoint.clone().as_str())
                .header(header::CONTENT_TYPE, "application/json")
                .header(
                    header::AUTHORIZATION,
                    format!(
                        "Basic {}",
                        encode_client_credentials(constants().client_id, constants().client_secret)
                    )
                    .as_str(),
                )
                .body(post_data)
                .send()
                .await
                .map_err(|e| error::ErrorInternalServerError(e))?;
            let parsed_res: HashMap<String, String> = token_res
                .json::<HashMap<String, String>>()
                .await
                .map_err(|e| error::ErrorInternalServerError(e))?;
            let access_token = parsed_res
                .get("access_token")
                .expect("Undefined access_token")
                .clone();
            println!("Got access token: {}", &access_token);

            let mut ctx = tera::Context::new();
            ctx.insert(
                "access_token",
                &access_token
            );
            ctx.insert("scope", "None");
            Ok(HttpResponse::Ok().content_type("text/html").body(
                tmpl.render("index.html", &ctx)
                    .map_err(|e| error::ErrorInternalServerError(e))?,
            ))
        }
    }
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
            .service(web::resource("/callback").route(web::get().to(callback)))
    })
    .bind("localhost:9000")?
    .run()
    .await
}
