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

pub mod skywalking_proto {
  pub mod v3 {
      tonic::include_proto!("skywalking.v3");
  }
}

use crate::context::propagation::PropagationContext;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct Span {
    pub span_internal: skywalking_proto::v3::SpanObject,
}

impl Span {
    pub fn new(
        parent_span_id: i32,
        operation_name: String,
        remote_peer: String,
        span_type: skywalking_proto::v3::SpanType,
        span_layer: skywalking_proto::v3::SpanLayer,
        skip_analysis: bool,
    ) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let span_internal = skywalking_proto::v3::SpanObject {
            span_id: parent_span_id + 1,
            parent_span_id: parent_span_id,
            start_time: current_time as i64,
            end_time: 0, // not set
            refs: Vec::<skywalking_proto::v3::SegmentReference>::new(),
            operation_name: operation_name,
            peer: remote_peer,
            span_type: span_type as i32,
            span_layer: span_layer as i32,
            // TODO(shikugawa): define this value in
            // https://github.com/apache/skywalking/blob/6452e0c2d983c85c392602d50436e8d8e421fec9/oap-server/server-starter/src/main/resources/component-libraries.yml
            component_id: 11000,
            is_error: false,
            tags: Vec::<skywalking_proto::v3::KeyStringValuePair>::new(),
            logs: Vec::<skywalking_proto::v3::Log>::new(),
            skip_analysis: skip_analysis,
        };

        Span {
            span_internal: span_internal,
        }
    }

    // TODO(shikugawa): not to call `close()` explicitly.
    pub fn close(&mut self) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.span_internal.end_time = current_time as i64;
    }
}

pub struct SpanSet {
    spans: Vec<Span>,
}

impl SpanSet {
    fn new() -> Self {
        SpanSet { spans: Vec::new() }
    }

    pub fn convert_span_objects(&self) -> Vec<skywalking_proto::v3::SpanObject> {
        let mut objects = Vec::<skywalking_proto::v3::SpanObject>::new();

        for span in self.spans.iter() {
            objects.push(span.span_internal.clone());
        }

        objects
    }

    pub fn push(&mut self, span: Span) {
        self.spans.push(span);
    }

    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn last_span_mut(&mut self) -> &mut Span {
        self.spans.last_mut().unwrap()
    }
}

pub struct TracingContext {
    pub trace_id: u128,
    pub trace_segment_id: u128,
    pub service: String,
    pub service_instance: String,
    pub spans: SpanSet,
}

impl TracingContext {
    /// Used to generate a new trace context. Typically called when no context has
    /// been propagated and a new trace is to be started.
    pub fn default(service_name: &'static str, instance_name: &'static str) -> Self {
        let trace_id = Uuid::new_v4().as_u128();
        let trace_segment_id = Uuid::new_v4().as_u128();

        TracingContext {
            trace_id,
            trace_segment_id,
            service: String::from(service_name),
            service_instance: String::from(instance_name),
            spans: SpanSet::new(),
        }
    }

    /// Generate a trace context using the propagated context.
    /// It is generally used when tracing is to be performed continuously.
    pub fn from_propagation_context(context: PropagationContext) -> Self {
        let trace_segment_id = Uuid::new_v4().as_u128();

        TracingContext {
            trace_id: context.parent_trace_id.parse::<u128>().unwrap(),
            trace_segment_id,
            service: context.parent_service,
            service_instance: context.parent_service_instance,
            spans: SpanSet::new(),
        }
    }

    /// Create a new entry span, which is an initiator of collection of spans.
    /// This should be called by invocation of the function which is triggered by
    /// external service.
    pub fn create_entry_span(&mut self, operation_name: String) -> Result<&mut Span, &str> {
        if self.spans.len() > 0 {
            return Err("failed to create entry span: the entry span has exist already");
        }

        let parent_span_id = self.spans.len() as i32 - 1;
        self.spans.push(Span::new(
            parent_span_id as i32,
            operation_name,
            String::default(),
            skywalking_proto::v3::SpanType::Entry,
            skywalking_proto::v3::SpanLayer::Http,
            false,
        ));

        Ok(self.spans.last_span_mut())
    }

    /// Create a new exit span, which will be created when tracing context will generate
    /// new span for function invocation.
    /// Currently, this SDK supports RPC call. So we must set `remote_peer`.
    pub fn create_exit_span(&mut self, operation_name: String, remote_peer: String) -> &mut Span {
        let parent_span_id = self.spans.len() - 1;
        self.spans.push(Span::new(
            parent_span_id as i32,
            operation_name,
            remote_peer,
            skywalking_proto::v3::SpanType::Exit,
            skywalking_proto::v3::SpanLayer::Http,
            false,
        ));

        self.spans.last_span_mut()
    }

    /// It converts tracing context into segment object.
    /// This conversion should be done before sending segments into OAP.
    pub fn convert_segment_object(&self) -> skywalking_proto::v3::SegmentObject {
        skywalking_proto::v3::SegmentObject {
            trace_id: self.trace_id.to_string(),
            trace_segment_id: self.trace_segment_id.to_string(),
            spans: self.spans.convert_span_objects(),
            service: self.service.clone(),
            service_instance: self.service_instance.clone(),
            is_size_limited: false,
        }
    }
}