use serde::{Deserialize,Serialize};
use sqlx;

#[derive(Debug,Deserialize,Serialize,sqlx::FromRow)]
pub struct IpInfo {

    pub url: String,
    pub ip: String,
    pub isp: String,
    pub country: String,
    pub latency: i32,
    pub code: String,
}



