use rs2sky::context::trace_context::TracingContext;
use rs2sky::reporter::grpc::{flush, ReporterClient};
use tokio;

#[tokio::main]
async fn main() {
    let mut reporter = ReporterClient::connect("http://0.0.0.0:11800")
        .await
        .unwrap();
    let mut context = TracingContext::default("service", "instance");

    {
        let mut span1 = context.create_entry_span(String::from("op1")).unwrap();
        span1.close();
    }

    // flush(&mut reporter, context.convert_segment_object()).await.unwrap();
}
