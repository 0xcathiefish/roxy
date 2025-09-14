use reqwest::{Client,Proxy};
use oping::{Ping};
use serde::Deserialize;
use serde_json::Value;

use dotenvy::dotenv;
use std::env;

use sqlx::{PgPool,query};

