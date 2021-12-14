use hyper::client::HttpConnector;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Method, Request, Response, Server, StatusCode};
use rs2sky::context::propagation::encoder::encode_propagation;
use rs2sky::context::propagation::decoder::decode_propagation;
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
    let time_fetcher = UnixTimeStampFetcher::default();
    let mut context = TracingContext::default(Arc::new(time_fetcher), "producer", "node_0");
    let span = context.create_entry_span(String::from("/ping")).unwrap();
    {
        let span2 = context.create_exit_span(String::from("/pong"), String::from("consumer:8082"));
        let header = encode_propagation(&context, "/pong", "consumer:8082");
        let req = Request::builder()
            .method(Method::GET)
            .header("sw8", header)
            .uri("http://consumer:8082/pong")
            .body(Body::from(""))
            .unwrap();

        client.request(req).await.unwrap();
        context.finalize_span(span2);
    }
    context.finalize_span(span);
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
    let addr = SocketAddr::from((host, 8081));
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

async fn handle_pong(
    _req: Request<Body>,
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) -> Result<Response<Body>, Infallible> {
    let time_fetcher = UnixTimeStampFetcher::default();
    let ctx = decode_propagation(&_req.headers()["sw8"].to_str().unwrap()).unwrap();
    let mut context = TracingContext::from_propagation_context(Arc::new(time_fetcher), "consumer", "node_0" , ctx);
    let span = context.create_entry_span(String::from("/pong")).unwrap();
    context.finalize_span(span);
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
    tx: mpsc::Sender<TracingContext<UnixTimeStampFetcher>>,
) {
    let make_svc = make_service_fn(|_| {
        let tx = tx.clone();

        async { Ok::<_, Infallible>(service_fn(move |req| consumer_response(req, tx.to_owned()))) }
    });
    let addr = SocketAddr::from((host, 8082));
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
    let mut reporter = ReporterClient::connect("http://collector:19876")
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
        run_consumer_service([0, 0, 0, 0], tx).await;
    } else if opt.mode == "producer" {
        run_producer_service([0, 0, 0, 0], tx).await;
    }
}
