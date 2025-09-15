use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method, Uri, StatusCode},
    response::Response,
    Router as AxumRouter,
    body::Body,
};
use reqwest::{Client, Proxy};
use dotenvy::dotenv;

use crate::route::Router;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;


#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub router: Router,
    pub is_updating: Arc<AtomicBool>,
}

pub async fn start_proxy_server() {
    dotenv().ok();
    
    let state = AppState {
        client: Client::new(),
        router: Router::new(),
        is_updating: Arc::new(AtomicBool::new(false)),
    };

    let app = AxumRouter::new()
        .fallback(standard_proxy_handler)  // 简化路由，只用fallback
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .unwrap();
        
    println!("HTTP Proxy server running on http://0.0.0.0:8080");
    println!("Strategies (using headers):");
    println!("  minlatency: curl --proxy http://localhost:8080 https://api.example.com");
    println!("  random:     curl --proxy http://localhost:8080 -H 'X-Proxy-Strategy: random' https://api.example.com");
    println!("  country:    curl --proxy http://localhost:8080 -H 'X-Proxy-Strategy: country/DE' https://api.example.com");
    println!("  binance:    curl --proxy http://localhost:8080 -H 'X-Proxy-Strategy: binance' https://fapi.binance.com/...");
    
    axum::serve(listener, app).await.unwrap();
}

// 修改 standard_proxy_handler
async fn standard_proxy_handler(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    request: Request<Body>,
) -> Result<Response<Body>, StatusCode> {
    
    println!("DEBUG: Received request - Method: {}, URI: {}", method, uri);
    
    // 处理HTTPS CONNECT请求
    if method == Method::CONNECT {
        return handle_connect(state, uri).await;
    }
    
    // 从headers和URL路径中解析策略
    let (strategy, country) = parse_strategy_from_request(&headers, uri.path());
    println!("DEBUG: Final parsed strategy: {}, country: {:?}", strategy, country);
    
    handle_proxy_request(state, method, uri, headers, request, strategy, country).await
}

// 从headers和URL路径中解析策略
pub fn parse_strategy_from_request(headers: &HeaderMap, path: &str) -> (String, Option<String>) {
    // 首先检查URL路径中的策略（兼容旧格式）
    if let Some(strategy_from_path) = parse_strategy_from_path(path) {
        return strategy_from_path;
    }
    
    // 然后检查headers中的策略
    parse_strategy_from_headers(headers)
}

// 从URL路径解析策略（支持旧格式）
fn parse_strategy_from_path(path: &str) -> Option<(String, Option<String>)> {
    if path.starts_with("/proxy/") {
        let parts: Vec<&str> = path[7..].split('/').collect(); // 去掉 "/proxy/"
        if !parts.is_empty() && !parts[0].is_empty() {
            let strategy = parts[0].to_string();
            let country = if parts.len() > 1 && !parts[1].is_empty() {
                Some(parts[1].to_string())
            } else {
                None
            };
            println!("DEBUG: Parsed strategy from path: {}, country: {:?}", strategy, country);
            return Some((strategy, country));
        }
    }
    None
}

// 从headers中解析策略
pub fn parse_strategy_from_headers(headers: &HeaderMap) -> (String, Option<String>) {
    // 检查组合策略头 X-Proxy-Strategy: country/DE 或 X-Proxy-Strategy: binance
    if let Some(strategy_header) = headers.get("X-Proxy-Strategy") {
        if let Ok(strategy_str) = strategy_header.to_str() {
            if let Some((strategy, country)) = strategy_str.split_once('/') {
                println!("DEBUG: Parsed strategy from header: {}, country: {:?}", strategy, Some(country));
                return (strategy.to_string(), Some(country.to_string()));
            } else {
                println!("DEBUG: Parsed strategy from header: {}, country: None", strategy_str);
                return (strategy_str.to_string(), None);
            }
        }
    }
    
    // 检查分离的国家头
    if let Some(country_header) = headers.get("X-Proxy-Country") {
        if let Ok(country_str) = country_header.to_str() {
            println!("DEBUG: Parsed strategy from separate headers: country, country: {:?}", Some(country_str));
            return ("country".to_string(), Some(country_str.to_string()));
        }
    }
    
    // 默认策略
    println!("DEBUG: Using default strategy: minlatency");
    ("minlatency".to_string(), None)
}

// 核心代理处理逻辑
pub async fn handle_proxy_request(
    state: AppState,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    request: Request<Body>,
    strategy: String,
    country: Option<String>,
) -> Result<Response<Body>, StatusCode> {
    
    // 1. 构建目标URL
    let target_url = if uri.scheme().is_some() {
        uri.to_string()
    } else {
        let host = headers.get("host")
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::BAD_REQUEST)?;
        // 强制使用HTTPS
        format!("https://{}{}", host, uri)
    };
    
    println!("Proxying {} request to: {}", method, target_url);
    println!("Strategy: {}, Country: {:?}", strategy, country);
    
    // 2. 根据策略获取代理
    let proxy_info = match strategy.as_str() {
        "random" => {
            state.router.get_random_proxy().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        "country" => {
            if let Some(country_code) = country {
                state.router.get_proxy_by_country(&country_code).await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        }
        "binance" => {
            state.router.get_binance_proxy().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
        _ => {
            state.router.get_best_proxy().await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        }
    };
    
    let proxy_info = proxy_info.ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    println!("Using proxy: {} ({}ms) - {} ({})", proxy_info.url, proxy_info.latency, proxy_info.country, proxy_info.code);
    
    // 3. 创建带代理的客户端
    let proxy = Proxy::all(&proxy_info.url)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let client = Client::builder()
        .proxy(proxy)
        .danger_accept_invalid_certs(true) // 开发环境，生产环境请移除
        .build()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // 4. 读取请求体
    let body_bytes = axum::body::to_bytes(request.into_body(), usize::MAX).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // 5. 构建请求 - 过滤代理专用头
    let mut req_builder = client.request(method, &target_url);
    
    // 复制请求头，但排除代理专用头和标准代理头
    for (name, value) in headers.iter() {
        let name_str = name.as_str().to_lowercase();
        if !["host", "connection", "proxy-connection", "content-length", "proxy-authorization",
             "x-proxy-strategy", "x-proxy-country"].contains(&name_str.as_str()) {
            req_builder = req_builder.header(name, value);
        }
    }
    
    // 添加请求体
    if !body_bytes.is_empty() {
        req_builder = req_builder.body(body_bytes);
    }
    
    // 6. 发送请求
    let response = req_builder.send().await
        .map_err(|e| {
            println!("Request failed: {}", e);
            StatusCode::BAD_GATEWAY
        })?;
    
    // 7. 构建响应
    let status = response.status();
    let response_headers = response.headers().clone();
    let response_bytes = response.bytes().await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    
    let mut axum_response = Response::builder().status(status);
    
    // 复制响应头
    for (name, value) in response_headers.iter() {
        axum_response = axum_response.header(name, value);
    }
    
    axum_response
        .body(Body::from(response_bytes))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// 处理HTTPS CONNECT方法
pub async fn handle_connect(
    state: AppState,
    uri: Uri,
) -> Result<Response<Body>, StatusCode> {
    
    let host_port = uri.to_string();
    println!("CONNECT request to: {}", host_port);
    
    // 获取最佳代理
    let proxy_info = state.router.get_best_proxy().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
    
    println!("Using proxy for CONNECT: {} ({}ms)", proxy_info.url, proxy_info.latency);
    
    // 对于CONNECT，我们返回200 Connection established
    // 实际的隧道建立需要更复杂的实现
    Response::builder()
        .status(StatusCode::OK)
        .header("Proxy-agent", "roxy/1.0")
        .body(Body::empty())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

