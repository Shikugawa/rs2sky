use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Method, Request, Response, Server, StatusCode};
use rs2sky::context::system_time::UnixTimeStampFetcher;
use rs2sky::context::trace_context::TracingContext;
use rs2sky::reporter::grpc::{flush, ReporterClient};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use structopt::StructOpt;
use tokio::sync::mpsc;

static NOT_FOUND_MSG: &str = "not found";

async fn handle_ping(
    _req: Request<Body>,
    client: Client<HttpConnector>,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) -> Result<Response<Body>, Infallible> {
    let time_fetcher = UnixTimeStampFetcher::new();
    let mut context = TracingContext::default(Arc::new(time_fetcher), "service", "instance");

    {
        let span = context.create_entry_span(String::from("op1")).unwrap();
        let uri = "http://127.0.0.1:8082/pong".parse().unwrap();
        client.get(uri).await.unwrap();
        span.close();
    }

    let _ = tx.send(context).await;
    Ok(Response::new(Body::from("hoge")))
}

async fn producer_response(
    _req: Request<Body>,
    client: Client<HttpConnector>,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) -> Result<Response<Body>, Infallible> {
    match (_req.method(), _req.uri().path()) {
        (&Method::GET, "/ping") => handle_ping(_req, client, tx).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(NOT_FOUND_MSG))
            .unwrap()),
    }
}

async fn run_producer_service(
    host: [u8; 4],
    port: u16,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) {
    let client = Client::new();
    let make_svc = make_service_fn(|_| {
        let tx = tx.clone();
        let client = client.clone();

        async {
            Ok::<_, Infallible>(service_fn(move |req| {
                producer_response(req, client.to_owned(), tx.to_owned())
            }))
        }
    });
    let addr = SocketAddr::from((host, port));
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

async fn handle_pong(
    _req: Request<Body>,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) -> Result<Response<Body>, Infallible> {
    let time_fetcher = UnixTimeStampFetcher::new();
    let context = TracingContext::default(Arc::new(time_fetcher), "service", "consumer");
    let _ = tx.send(context).await;
    Ok(Response::new(Body::from("hoge")))
}

async fn consumer_response(
    _req: Request<Body>,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) -> Result<Response<Body>, Infallible> {
    match (_req.method(), _req.uri().path()) {
        (&Method::GET, "/pong") => handle_pong(_req, tx).await,
        _ => Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(NOT_FOUND_MSG))
            .unwrap()),
    }
}

async fn run_consumer_service(
    host: [u8; 4],
    port: u16,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) {
    let make_svc = make_service_fn(|_| {
        let tx = tx.clone();

        async { Ok::<_, Infallible>(service_fn(move |req| consumer_response(req, tx.to_owned()))) }
    });
    let addr = SocketAddr::from((host, port));
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

#[derive(StructOpt)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(short, long)]
    mode: String,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();
    let (tx, mut rx): (
        mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
        mpsc::Receiver<TracingContext<UnixTimeStampFetcher>>,
    ) = mpsc::channel(32);
    let mut reporter = ReporterClient::connect("http://0.0.0.0:11800")
        .await
        .unwrap();

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            flush(&mut reporter, message.convert_segment_object())
                .await
                .unwrap();
        }
    });

    if opt.mode == "consumer" {
        run_consumer_service([127, 0, 0, 1], 8082, tx).await;
    } else if opt.mode == "producer" {
        run_producer_service([127, 0, 0, 1], 8081, tx).await;
    }
}
