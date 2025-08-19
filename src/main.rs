use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use clap::Parser;
use hyper::client::Client;
use hyper::header::{HeaderValue, PROXY_AUTHENTICATE, PROXY_AUTHORIZATION};
use hyper::server::conn::AddrStream;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use tokio::io::copy_bidirectional;
use tokio::net::TcpStream;

static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Listen address, e.g. 127.0.0.1:8080
    #[arg(long, default_value = "0.0.0.0:8080")]
    listen: String,

    /// Proxy username (empty = no auth)
    #[arg(long, default_value = "")]
    username: String,

    /// Proxy password
    #[arg(long, default_value = "")]
    password: String,

    /// Show debug logs
    #[arg(long, default_value_t = false)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let addr: SocketAddr = args.listen.parse()?;
    let auth = if args.username.is_empty() {
        None
    } else {
        Some((args.username.clone(), args.password.clone()))
    };
    let debug = args.debug;

    // Share auth and debug via closure capture
    let make_svc = hyper::service::make_service_fn(move |conn: &AddrStream| {
        let remote_addr = conn.remote_addr();
        let auth = auth.clone();
        let debug = debug;
        async move {
            Ok::<_, Infallible>(hyper::service::service_fn(move |req| {
                proxy_handler(req, auth.clone(), debug, remote_addr)
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    eprintln!("Listening on http://{} (debug={})", addr, debug);
    server.await?;
    Ok(())
}

fn check_proxy_auth(
    auth: &Option<(String, String)>,
    req: &Request<Body>,
) -> Result<(), Response<Body>> {
    if let Some((username, password)) = auth {
        // Expect Proxy-Authorization: Basic base64(user:pass)
        if let Some(hv) = req.headers().get(PROXY_AUTHORIZATION) {
            if let Ok(s) = hv.to_str() {
                if s.starts_with("Basic ") {
                    let encoded = &s[6..];
                    if let Ok(decoded) = STANDARD.decode(encoded) {
                        if let Ok(creds) = std::str::from_utf8(&decoded) {
                            let expected = format!("{}:{}", username, password);
                            if creds == expected {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        // If we reach here, auth failed
        let mut resp = Response::new(Body::from("Proxy Authentication Required"));
        *resp.status_mut() = StatusCode::PROXY_AUTHENTICATION_REQUIRED;
        resp.headers_mut().insert(
            PROXY_AUTHENTICATE,
            HeaderValue::from_static("Basic realm=\"dshp\"")
        );
        return Err(resp);
    }

    Ok(())
}

async fn proxy_handler(
    req: Request<Body>,
    auth: Option<(String, String)>,
    debug: bool,
    remote_addr: SocketAddr,
) -> Result<Response<Body>, Infallible> {
    let req_id = REQ_COUNTER.fetch_add(1, Ordering::Relaxed);
    if debug {
        eprintln!("[req {}] {} {} from {}", req_id, req.method(), req.uri(), remote_addr);
    }

    // Enforce proxy auth if configured
    if let Err(resp) = check_proxy_auth(&auth, &req) {
        if debug {
            eprintln!("[req {}] auth failed", req_id);
        }
        return Ok(resp);
    }

    // Handle CONNECT for HTTPS tunneling using hyper upgrade
    if req.method() == Method::CONNECT {
        if let Some(authority) = req.uri().authority() {
            let target = authority.as_str().to_string();
            if debug {
                eprintln!("[req {}] CONNECT to {}", req_id, target);
            }

            // Prepare the upgrade future before responding
            let upgrade_fut = hyper::upgrade::on(req);

            // Respond 200 so client will begin TLS handshake over the tunnel
            let resp = Response::builder()
                .status(StatusCode::OK)
                .body(Body::empty())
                .unwrap();

            // Spawn a task to complete the tunnel once the client upgrades
            tokio::spawn(async move {
                match upgrade_fut.await {
                    Ok(mut upgraded) => {
                        if debug {
                            eprintln!("[req {}] upgrade completed, connecting to target {}", req_id, target);
                        }
                        // Connect to the target server
                        match TcpStream::connect(&target).await {
                            Ok(mut server_conn) => {
                                if debug {
                                    eprintln!("[req {}] connected to target {}", req_id, target);
                                }
                                // Copy data in both directions until EOF
                                let _ = copy_bidirectional(&mut upgraded, &mut server_conn).await;
                                if debug {
                                    eprintln!("[req {}] tunnel closed {}", req_id, target);
                                }
                            }
                            Err(e) => {
                                eprintln!("[req {}] CONNECT target connect error {}: {}", req_id, target, e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[req {}] upgrade error: {}", req_id, e);
                    }
                }
            });

            return Ok(resp);
        }
    }

    // For normal HTTP requests, forward using hyper client
    if debug {
        eprintln!("[req {}] forwarding HTTP request {}", req_id, req.uri());
    }
    let client: Client<hyper::client::HttpConnector> = Client::new();

    match client.request(req).await {
        Ok(resp) => {
            if debug {
                eprintln!("[req {}] upstream response {}", req_id, resp.status());
            }
            Ok(resp)
        }
        Err(e) => {
            if debug {
                eprintln!("[req {}] upstream error: {}", req_id, e);
            }
            Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Body::from(format!("Upstream error: {}", e)))
                .unwrap())
        }
    }
}
