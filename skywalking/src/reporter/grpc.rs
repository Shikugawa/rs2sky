// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use crate::reporter::reporter_trait::Reporter;
use crate::skywalking_proto::v3::trace_segment_report_service_client::TraceSegmentReportServiceClient;
use crate::skywalking_proto::v3::SegmentObject;
use async_stream;
use tokio::sync::mpsc;
use tonic::transport::Channel;
use tonic::Request;

pub struct GrpcReporter {
    client: TraceSegmentReportServiceClient<Channel>,
    tx: mpsc::Sender<SegmentObject>,
    rx: mpsc::Receiver<SegmentObject>,
}

impl Reporter for GrpcReporter {
    fn report(
        &mut self,
        ctx: SegmentObject,
    ) -> Result<(), mpsc::error::TrySendError<SegmentObject>> {
        self.tx.try_send(ctx)
    }
}

impl GrpcReporter {
    pub async fn connect(host: &str, port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let client =
            TraceSegmentReportServiceClient::connect(format!("{}:{:?}", host, port)).await?;
        let (tx, rx): (mpsc::Sender<SegmentObject>, mpsc::Receiver<SegmentObject>) =
            mpsc::channel(1024);

        Ok(GrpcReporter { client, tx, rx })
    }

    pub async fn flush(&'static mut self) -> Result<(), tonic::Status> {
        _flush(&mut self.client, &mut self.rx).await
    }
}

async fn _flush(
    client: &'static mut TraceSegmentReportServiceClient<Channel>,
    rx: &'static mut mpsc::Receiver<SegmentObject>,
) -> Result<(), tonic::Status> {
    let stream = async_stream::stream! {
      while let Some(msg) = rx.recv().await {
        yield msg
      }
    };
    match client.collect(Request::new(stream)).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}
