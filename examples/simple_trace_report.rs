use rs2sky::context::system_time::UnixTimeStampFetcher;
use rs2sky::context::trace_context::TracingContext;
use rs2sky::reporter::grpc::Reporter;
use std::sync::Arc;
use tokio;

#[tokio::main]
async fn main() {
    let tx = Reporter::start("http://0.0.0.0:11800".to_string()).await;
    let time_fetcher = UnixTimeStampFetcher::default();
    let mut context =
        TracingContext::default_internal(Arc::new(time_fetcher), "service", "instance");
    {
        let span = context.create_entry_span("op1").unwrap();
        context.finalize_span(span);
    }
    let _ = tx.send(context).await;
}
