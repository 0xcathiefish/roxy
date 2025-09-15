use reqwest::{Client,Proxy};
use oping::{Ping};
use serde::Deserialize;
use serde_json::Value;

use dotenvy::dotenv;
use tokio::time::Sleep;
use std::env;
use std::sync::Arc;

use sqlx::{PgPool,query};


use roxy::structs::IpInfo;


#[tokio::main]
async fn main() {

    dotenv().ok();

    let pg_url = env::var("DATABASE_URL").unwrap();

    let pool = Arc::new(PgPool::connect(&pg_url).await.unwrap());

    for port in 10001..=10100 {

        let db_pool = Arc::clone(&pool);
        tokio::spawn(get_proxy(port,db_pool));
    }

    tokio::time::sleep(std::time::Duration::from_secs(200)).await;
}


async fn get_proxy(num: i32,db_pool: Arc<PgPool>) {

    dotenv().ok();

    let url: &str = &env::var("info_url").unwrap();
    let basic_url: String = env::var("basic_url").unwrap();

    let proxy_user: &str = &env::var("proxy_user").unwrap();
    let proxy_pass: &str = &env::var("proxy_pass").unwrap();
    let proxy_url_1: String = env::var("proxy_url").unwrap();

    let proxy_url: &str = &format!("{}{}", proxy_url_1, num.to_string());

    let proxy = Proxy::https(proxy_url).expect("Failed to get proxy").basic_auth(proxy_user, proxy_pass);

    let client = Client::builder()
    .proxy(proxy)
    .build().expect("Failed to build client");

    let response = client.get(url).send().await.expect("Failed to get response");

    let data: Value = response.json().await.expect("Failed to deserilize");

    let format_url = format!("{}{}", basic_url, num.to_string());

    let info = IpInfo {

        url: format_url,
        ip: data["proxy"]["ip"].as_str().unwrap().to_string(),
        isp: data["isp"]["isp"].as_str().unwrap().to_string(),
        country: data["country"]["name"].as_str().unwrap().to_string(),
        latency: 0,
        code: data["country"]["code"].as_str().unwrap().to_string(),
    };

    println!("Get IP infos: {}, port {} finish", info.country, num);

    insert_data(info,db_pool).await;
}


async fn insert_data(IpInfo: IpInfo,db_pool: Arc<PgPool>) {

    sqlx::query!(
        r#"
        INSERT INTO proxies (url, ip, isp, country, latency, code)
        VALUES ($1,$2,$3,$4,$5,$6)
        "#,
        IpInfo.url,
        IpInfo.ip,
        IpInfo.isp,
        IpInfo.country,
        IpInfo.latency,
        IpInfo.code
    ).execute(&*db_pool).await.unwrap();
}


// async fn get_data() -> Vec<IpInfo> {

//     let ip_pool: Vec<IpInfo> = vec![];

//     let pg_url = env::var("DATABASE_URL").unwrap();

//     let latency_limit = 300;

//     let pool = PgPool::connect(&pg_url).await.unwrap();

//     sqlx::query_as!(
//         IpInfo,
//         "SELECT url FROM proxies WHERE latency < $1",
//         latency_require
//     ).fetch_all(&pool).await.unwrap();

//     ip_pool
// }
