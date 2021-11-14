use rs2sky::context::system_time::UnixTimeStampFetcher;
use rs2sky::context::trace_context::TracingContext;
use rs2sky::reporter::grpc::{flush, ReporterClient};
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() {
    let mut reporter = ReporterClient::connect("http://0.0.0.0:11800")
        .await
        .unwrap();
    let time_fetcher = UnixTimeStampFetcher::new();
    let mut context = TracingContext::default(Arc::new(time_fetcher), "service", "instance");

    {
        let span = context.create_entry_span(String::from("op1")).unwrap();
        span.close();
    }

    flush(&mut reporter, context.convert_segment_object())
        .await
        .unwrap();
}
