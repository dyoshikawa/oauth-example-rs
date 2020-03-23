use redis::Connection;

pub fn create_connection() -> Connection {
    let redis_client = redis::Client::open("redis://localhost/").expect("url error");
    redis_client.get_connection().unwrap()
}
