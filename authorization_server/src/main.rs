use actix_web::{error, http::header, middleware, web, App, Error, HttpResponse, HttpServer};
use base64;
use redis::Commands;
use redis_client::create_connection;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_json::json;
use std::collections::HashMap;
use tera::Tera;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
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

fn get_client(client_id: &String) -> Option<Client> {
    constants()
        .clients
        .into_iter()
        .find(|c| c.client_id == *client_id)
        .clone()
}

fn is_different_scope(rscope: Vec<String>, cscope: Vec<String>) -> bool {
    for s in rscope.into_iter() {
        if !cscope.contains(&s) {
            return true;
        }
    }
    false
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
    let client_id = query.get("client_id").cloned().unwrap_or("".to_string());
    let client = constants()
        .clients
        .into_iter()
        .find(|c| c.client_id == *client_id);
    match client {
        None => Err(error::ErrorInternalServerError("Unknown client")),
        Some(client) => {
            let redirect_uri = query.get("redirect_uri").cloned().unwrap_or("".to_string());
            if !client.redirect_uris.contains(&redirect_uri) {
                Err(error::ErrorInternalServerError("Invalid redirect URI"))
            } else {
                let rscope_str = query.get("scope").cloned().unwrap_or("".to_string());
                let rscope: Vec<String> = rscope_str
                    .split(' ')
                    .collect::<Vec<&str>>()
                    .into_iter()
                    .filter(|s| s != &"")
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();
                let cscope: Vec<String> = client
                    .scope
                    .split(' ')
                    .collect::<Vec<&str>>()
                    .into_iter()
                    .filter(|s| s != &"")
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();
                if is_different_scope(rscope.clone(), cscope.clone()) {
                    return Err(error::ErrorInternalServerError("Invalid scope"));
                }

                let reqid = format!("request_{}", Uuid::new_v4());
                let mut redis_con = create_connection();
                let v = serde_json::to_string(&query.into_inner()).unwrap();
                let _: () = redis_con.set(&reqid, v).unwrap();

                let mut ctx = tera::Context::new();
                ctx.insert("client_id", &client.client_id);
                ctx.insert("client_secret", &client.client_secret);
                ctx.insert("redirect_uris", &client.redirect_uris);
                ctx.insert("reqid", &reqid);
                ctx.insert("scope", &rscope);
                Ok(HttpResponse::Ok().content_type("text/html").body(
                    tmpl.render("approve.html", &ctx)
                        .map_err(|e| error::ErrorInternalServerError(e))?,
                ))
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
struct User {}

#[derive(Serialize, Deserialize)]
struct ApproveParams {
    authorization_endpoint_request: HashMap<String, String>,
    scope: Vec<String>,
    user: Option<User>,
}

async fn approve(body: web::Form<HashMap<String, String>>) -> Result<HttpResponse, Error> {
    println!("{:?}", body);

    let reqid = body.get("reqid").cloned().unwrap_or("".to_string());
    let mut redis_con = create_connection();
    let query_str: String = redis_con
        .get(&reqid)
        .map_err(|e| error::ErrorInternalServerError(e))?;
    let query = serde_json::from_str::<HashMap<String, String>>(query_str.as_str())
        .map_err(|e| error::ErrorInternalServerError(e))?;
    let _: () = redis_con.del(&reqid).expect("Failed delete");

    match query.get("redirect_uri") {
        None => Err(error::ErrorInternalServerError("Undefined redirect_uri")),
        Some(redirect_uri) => match body.get("approve") {
            None => Ok(HttpResponse::TemporaryRedirect()
                .header(
                    header::LOCATION,
                    Url::parse_with_params(redirect_uri, vec![("error", "access_denied")])
                        .unwrap()
                        .as_str(),
                )
                .finish()),
            Some(_) => {
                let response_type = query
                    .get("response_type")
                    .cloned()
                    .unwrap_or("".to_string());
                if response_type != "code".to_string() {
                    return Ok(HttpResponse::TemporaryRedirect()
                        .header(
                            header::LOCATION,
                            Url::parse_with_params(
                                redirect_uri,
                                vec![("error", "unsupported_response_type")],
                            )
                            .unwrap()
                            .as_str(),
                        )
                        .finish());
                }

                let code = Uuid::new_v4().to_string();
                // TODO scope, user
                let approve_params = ApproveParams {
                    authorization_endpoint_request: query.clone(),
                    scope: vec![],
                    user: None,
                };

                let _: () = redis_con
                    .set(
                        format!("code_{}", &code),
                        serde_json::to_string(&approve_params).unwrap(),
                    )
                    .unwrap();

                let state = query.get("state").cloned().unwrap_or("".to_string());

                Ok(HttpResponse::TemporaryRedirect()
                    .header(
                        header::LOCATION,
                        Url::parse_with_params(
                            redirect_uri,
                            vec![("code", &code), ("state", &state)],
                        )
                        .unwrap()
                        .as_str(),
                    )
                    .finish())
            }
        },
    }
}

#[derive(Serialize, Deserialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    scope: Vec<String>,
}

async fn token(
    query: web::Query<HashMap<String, String>>,
    body: web::Json<HashMap<String, String>>,
) -> Result<HttpResponse, Error> {
    let auth = query
        .get("authorization")
        .cloned()
        .unwrap_or("".to_string());
    let mut client_id = "".to_string();
    let mut client_secret = "".to_string();
    if auth != "".to_string() {
        let decoded_auth = base64::decode(&auth)
            .map_err(|e| error::ErrorInternalServerError(json! {{"error": e.to_string()}}))?
            .iter()
            .map(|&s| s as char)
            .collect::<String>();
        let client_credentials = decoded_auth
            .split(' ')
            .collect::<Vec<&str>>()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        client_id = client_credentials[0].clone();
        client_secret = client_credentials[1].clone();
    };

    // otherwise, check the post body
    if body.get("client_id").is_some() {
        if client_id != "" {
            println!("Client attempted to authenticate with multiple methods");
            return Err(error::ErrorBadRequest(json! {{"error": "invalid_client"}}));
        }

        client_id = body.get("client_id").unwrap().clone();
        client_secret = body
            .get("client_secret")
            .expect("Undefined client_secret")
            .clone();
    }

    let client = get_client(&client_id);
    match client {
        None => {
            println!("Unknown client {}", client_id);
            Err(error::ErrorBadRequest(json! {{"error": "invalid_client"}}))
        }
        Some(client) => {
            if client.client_secret != client_secret {
                return Err(error::ErrorBadRequest(json! {{"error": "invalid_client"}}));
            }

            let grant_type = body.get("grant_type").cloned().unwrap_or("".to_string());
            if grant_type != "authorization_code" {}

            let code = body.get("code").cloned().unwrap_or("".to_string());
            let mut con = create_connection();
            let code_params_str: String = con
                .get(format!("code_{}", &code))
                .map_err(|e| error::ErrorInternalServerError(json! {{"error": e.to_string()}}))?;

            let token_response = TokenResponse {
                access_token: "".to_string(),
                token_type: "".to_string(),
                scope: vec!["".to_string()],
            };
            Ok(HttpResponse::Ok().json(token_response))
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
            .service(web::resource("/approve").route(web::post().to(approve)))
            .service(web::resource("/token").route(web::post().to(token)))
    })
    .bind("localhost:9001")?
    .run()
    .await
}
