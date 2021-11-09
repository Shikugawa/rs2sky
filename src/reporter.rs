pub mod skywalking {
    pub mod v3 {
        tonic::include_proto!("skywalking.v3");
    }
}

use async_stream::stream;
use skywalking::v3::trace_segment_report_service_client::TraceSegmentReportServiceClient;
use skywalking::v3::SegmentObject;
use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::{Receiver, Sender};
use tonic::transport::Channel;
use tonic::Request;

pub struct Reporter {
    client: TraceSegmentReportServiceClient<Channel>,
    receiver: Receiver<SegmentObject>,
    sender: Sender<SegmentObject>,
}

impl Reporter {
    async fn connect(host: &'static str, port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let client = TraceSegmentReportServiceClient::connect(host).await?;
        let (tx, mut rx) = channel(1024);

        Ok(Reporter {
            client: client,
            receiver: rx,
            sender: tx,
        })
    }

    async fn send_message(&mut self, message: SegmentObject) {
        self.sender.send(message);
    }

    async fn flush(&mut self) -> Result<(), tonic::Status> {
        let (tx, mut rx) = channel(10);
        let s = stream! {
          while let Some(msg) = rx.recv().await {
            yield msg;
          }
        };

        match self.client.collect(s).await {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }
}
