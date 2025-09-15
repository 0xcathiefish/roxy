use dotenvy::dotenv;
use std::env;
use sqlx::PgPool;
use rand::Rng;

use crate::structs::IpInfo;

#[derive(Clone)]
pub struct Router {
    max_latency: i32,
}

impl Router {
    pub fn new() -> Self {
        Self {
            max_latency: 300,
        }
    }

    // 策略1：最小延迟策略
    pub async fn get_best_proxy(&self) -> Result<Option<IpInfo>, sqlx::Error> {
        dotenv().ok();
        let pg_url = env::var("DATABASE_URL").unwrap();
        let pool = PgPool::connect(&pg_url).await?;

        let proxy = sqlx::query_as!(
            IpInfo,
            r#"
            SELECT 
                COALESCE(url, '') as "url!",
                COALESCE(ip, '') as "ip!",
                COALESCE(isp, '') as "isp!",
                COALESCE(country, '') as "country!",
                latency as "latency!",
                COALESCE(code, '') as "code!"
            FROM proxies 
            WHERE latency > 0 AND latency < $1 AND latency IS NOT NULL
            AND url IS NOT NULL 
            AND ip IS NOT NULL 
            ORDER BY latency ASC
            LIMIT 1
            "#,
            self.max_latency
        )
        .fetch_optional(&pool)
        .await?;

        Ok(proxy)
    }

    // 策略2：随机策略（从最快的30个中随机选择）
    pub async fn get_random_proxy(&self) -> Result<Option<IpInfo>, sqlx::Error> {
        dotenv().ok();
        let pg_url = env::var("DATABASE_URL").unwrap();
        let pool = PgPool::connect(&pg_url).await?;

        let proxies = sqlx::query_as!(
            IpInfo,
            r#"
            SELECT 
                COALESCE(url, '') as "url!",
                COALESCE(ip, '') as "ip!",
                COALESCE(isp, '') as "isp!",
                COALESCE(country, '') as "country!",
                latency as "latency!",
                COALESCE(code, '') as "code!"
            FROM proxies 
            WHERE latency > 0 AND latency < $1 AND latency IS NOT NULL
            AND url IS NOT NULL 
            AND ip IS NOT NULL 
            ORDER BY latency ASC
            LIMIT 30
            "#,
            self.max_latency
        )
        .fetch_all(&pool)
        .await?;

        if proxies.is_empty() {
            return Ok(None);
        }

        // 随机选择一个
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..proxies.len());
        Ok(Some(proxies[index].clone()))
    }

    // 策略3：国家策略
    pub async fn get_proxy_by_country(&self, country_code: &str) -> Result<Option<IpInfo>, sqlx::Error> {
        dotenv().ok();
        let pg_url = env::var("DATABASE_URL").unwrap();
        let pool = PgPool::connect(&pg_url).await?;

        let proxy = sqlx::query_as!(
            IpInfo,
            r#"
            SELECT 
                COALESCE(url, '') as "url!",
                COALESCE(ip, '') as "ip!",
                COALESCE(isp, '') as "isp!",
                COALESCE(country, '') as "country!",
                latency as "latency!",
                COALESCE(code, '') as "code!"
            FROM proxies 
            WHERE latency > 0 AND latency < $1 AND latency IS NOT NULL
            AND url IS NOT NULL 
            AND ip IS NOT NULL 
            AND code = $2
            ORDER BY latency ASC
            LIMIT 1
            "#,
            self.max_latency,
            country_code.to_uppercase()
        )
        .fetch_optional(&pool)
        .await?;

        Ok(proxy)
    }

    // 策略4：Binance策略（排除JP，从最快的20个中随机选择）
    pub async fn get_binance_proxy(&self) -> Result<Option<IpInfo>, sqlx::Error> {
        dotenv().ok();
        let pg_url = env::var("DATABASE_URL").unwrap();
        let pool = PgPool::connect(&pg_url).await?;

        let proxies = sqlx::query_as!(
            IpInfo,
            r#"
            SELECT 
                COALESCE(url, '') as "url!",
                COALESCE(ip, '') as "ip!",
                COALESCE(isp, '') as "isp!",
                COALESCE(country, '') as "country!",
                latency as "latency!",
                COALESCE(code, '') as "code!"
            FROM proxies 
            WHERE latency > 0 AND latency < $1 AND latency IS NOT NULL
            AND url IS NOT NULL 
            AND ip IS NOT NULL 
            AND code != 'JP'
            ORDER BY latency ASC
            LIMIT 20
            "#,
            self.max_latency
        )
        .fetch_all(&pool)
        .await?;

        if proxies.is_empty() {
            return Ok(None);
        }

        // 从前20个最快的代理中随机选择一个
        let mut rng = rand::thread_rng();
        let index = rng.gen_range(0..proxies.len());
        Ok(Some(proxies[index].clone()))
    }
}















