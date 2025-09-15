use std::process::Command;
use dotenvy::dotenv;
use std::env;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use crate::structs::{IpList};

pub async fn system_ping_latency(ip: &str) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    let ip = ip.to_string();
    let result = tokio::task::spawn_blocking(move || -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("ping")
            .arg("-c")
            .arg("1") 
            .arg("-W")
            .arg("3000") 
            .arg(&ip)
            .output()?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        for line in stdout.lines() {
            if line.contains("time=") {
                if let Some(time_part) = line.split("time=").nth(1) {
                    if let Some(time_str) = time_part.split_whitespace().next() {
                        if let Ok(time_f64) = time_str.parse::<f64>() {
                            return Ok(time_f64 as i32);
                        }
                    }
                }
            }
        }
        
        Err("Could not parse ping output".into())
    }).await?;
    
    result
}

pub async fn test_proxy_ip_latency(proxy_ip: &str) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    system_ping_latency(proxy_ip).await
}

pub async fn update_latency() {
    println!("Starting latency update with 20s timeout...");
    
    // 设置总体超时时间为20秒
    match timeout(Duration::from_secs(20), update_latency_internal()).await {
        Ok(_) => println!("Latency update completed successfully!"),
        Err(_) => println!("Latency update timed out after 20 seconds!"),
    }
}

async fn update_latency_internal() {
    let ip_list = get_all_ips().await;
    println!("Found {} IPs to update", ip_list.len());
    
    dotenv().ok();

    let pg_url = env::var("DATABASE_URL").unwrap();
    let pool = Arc::new(PgPool::connect(&pg_url).await.unwrap());
    
    let mut handles = Vec::new();

    for n in 0..ip_list.len().min(50) {  // 减少并发数量防止过载
        let ip = ip_list[n].ip.clone();
        let db_pool = Arc::clone(&pool);
        
        let handle = tokio::spawn(async move {
            // 为单个ping添加5秒超时
            match timeout(Duration::from_secs(5), system_ping_latency(&ip)).await {
                Ok(Ok(latency)) => {
                    match sqlx::query!(
                        "UPDATE proxies SET latency = $1 WHERE ip = $2",
                        latency,
                        ip
                    ).execute(&*db_pool).await {
                        Ok(_) => println!("IP {}: {}ms - updated", ip, latency),
                        Err(e) => println!("IP {}: database update failed - {}", ip, e),
                    }
                }
                Ok(Err(e)) => println!("IP {}: ping failed - {}", ip, e),
                Err(_) => println!("IP {}: ping timeout (5s)", ip),
            }
        });
        
        handles.push(handle);
    }

    println!("Waiting for {} ping tasks to complete...", handles.len());
    for handle in handles {
        let _ = handle.await;
    }
    
    // 确保连接池关闭
    pool.close().await;
    println!("Latency update internal process completed!");
}

async fn get_all_ips() -> Vec<IpList> {
    dotenv().ok();
    let pg_url = env::var("DATABASE_URL").unwrap();
    let pool = PgPool::connect(&pg_url).await.unwrap();

    let ip_list = sqlx::query_as!(
        IpList,
        "SELECT ip FROM proxies"
    ).fetch_all(&pool).await.unwrap();

    ip_list
}

#[cfg(test)]
mod test_ping {
    use super::*;

    #[tokio::test]
    async fn test_ping_google_dns() {
        match system_ping_latency("8.8.8.8").await {
            Ok(latency) => {
                println!("Google DNS latency: {}ms", latency);
                assert!(latency > 0 && latency < 1000);
            }
            Err(e) => println!("Ping failed: {}", e),
        }
    }

    #[tokio::test]
    async fn update() {
        update_latency().await;
    }
}
