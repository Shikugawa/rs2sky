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

use crate::common::random_generator::RandomGenerator;
use crate::common::time::TimeFetcher;
use crate::context::propagation::context::PropagationContext;
use crate::skywalking_proto::v3::{
    KeyStringValuePair, Log, RefType, SegmentObject, SegmentReference, SpanLayer, SpanObject,
    SpanType,
};
use std::sync::Arc;

pub struct Span {
    span_internal: SpanObject,
    time_fetcher: Arc<dyn TimeFetcher + Sync + Send>,
}

impl Span {
    pub fn new(
        parent_span_id: i32,
        operation_name: String,
        remote_peer: String,
        span_type: SpanType,
        span_layer: SpanLayer,
        skip_analysis: bool,
        time_fetcher: Arc<dyn TimeFetcher + Sync + Send>,
    ) -> Self {
        let span_internal = SpanObject {
            span_id: parent_span_id + 1,
            parent_span_id,
            start_time: time_fetcher.get(),
            end_time: 0, // not set
            refs: Vec::<SegmentReference>::new(),
            operation_name,
            peer: remote_peer,
            span_type: span_type as i32,
            span_layer: span_layer as i32,
            // TODO(shikugawa): define this value in
            // https://github.com/apache/skywalking/blob/6452e0c2d983c85c392602d50436e8d8e421fec9/oap-server/server-starter/src/main/resources/component-libraries.yml
            component_id: 11000,
            is_error: false,
            tags: Vec::<KeyStringValuePair>::new(),
            logs: Vec::<Log>::new(),
            skip_analysis,
        };

        Span {
            span_internal,
            time_fetcher,
        }
    }

    // TODO(shikugawa): not to call `close()` explicitly.
    pub fn close(&mut self) {
        self.span_internal.end_time = self.time_fetcher.get();
    }

    pub fn span_object(&self) -> &SpanObject {
        &self.span_internal
    }

    pub fn add_log(&mut self, message: Vec<(String, String)>) {
        let log = Log {
            time: self.time_fetcher.get(),
            data: message
                .into_iter()
                .map(|v| {
                    let (key, value) = v;
                    KeyStringValuePair { key, value }
                })
                .collect(),
        };
        self.span_internal.logs.push(log);
    }

    pub fn add_tag(&mut self, tag: (String, String)) {
        let (key, value) = tag;
        self.span_internal
            .tags
            .push(KeyStringValuePair { key, value });
    }

    fn add_segment_reference(&mut self, segment_reference: SegmentReference) {
        self.span_internal.refs.push(segment_reference);
    }
}

pub struct TracingContext {
    pub trace_id: String,
    pub trace_segment_id: String,
    pub service: String,
    pub service_instance: String,
    pub next_span_id: i32,
    pub spans: Vec<Box<Span>>,
    time_fetcher: Arc<dyn TimeFetcher + Sync + Send>,
    segment_link: Option<PropagationContext>,
}

impl TracingContext {
    /// Used to generate a new trace context. Typically called when no context has
    /// been propagated and a new trace is to be started.
    pub fn default(
        time_fetcher: Arc<dyn TimeFetcher + Sync + Send>,
        service_name: &'static str,
        instance_name: &'static str,
    ) -> Self {
        TracingContext {
            trace_id: RandomGenerator::generate(),
            trace_segment_id: RandomGenerator::generate(),
            service: String::from(service_name),
            service_instance: String::from(instance_name),
            next_span_id: 0,
            time_fetcher,
            spans: Vec::new(),
            segment_link: None,
        }
    }

    /// Generate a trace context using the propagated context.
    /// It is generally used when tracing is to be performed continuously.
    pub fn from_propagation_context(
        time_fetcher: Arc<dyn TimeFetcher + Sync + Send>,
        context: PropagationContext,
    ) -> Self {
        let trace_id = context.parent_trace_id.clone();
        let parent_service = context.parent_service.clone();
        let parent_service_instance = context.parent_service_instance.clone();

        TracingContext {
            trace_id: trace_id,
            trace_segment_id: RandomGenerator::generate(),
            service: parent_service,
            service_instance: parent_service_instance,
            next_span_id: 0,
            time_fetcher,
            spans: Vec::new(),
            segment_link: Some(context),
        }
    }

    /// Create a new entry span, which is an initiator of collection of spans.
    /// This should be called by invocation of the function which is triggered by
    /// external service.
    pub fn create_entry_span(&mut self, operation_name: String) -> Result<Box<Span>, &'static str> {
        if self.next_span_id >= 1 {
            return Err("entry span have already exist.");
        }

        let mut span = Box::new(Span::new(
            self.next_span_id,
            operation_name,
            String::default(),
            SpanType::Entry,
            SpanLayer::Http,
            false,
            self.time_fetcher.clone(),
        ));

        if self.segment_link.is_some() {
            span.add_segment_reference(SegmentReference {
                ref_type: RefType::CrossProcess as i32,
                trace_id: self.trace_id.clone(),
                parent_trace_segment_id: self
                    .segment_link
                    .as_ref()
                    .unwrap()
                    .parent_trace_segment_id
                    .clone(),
                parent_span_id: self.segment_link.as_ref().unwrap().parent_span_id,
                parent_service: self.segment_link.as_ref().unwrap().parent_service.clone(),
                parent_service_instance: self
                    .segment_link
                    .as_ref()
                    .unwrap()
                    .parent_service_instance
                    .clone(),
                parent_endpoint: self
                    .segment_link
                    .as_ref()
                    .unwrap()
                    .destination_endpoint
                    .clone(),
                network_address_used_at_peer: self
                    .segment_link
                    .as_ref()
                    .unwrap()
                    .destination_address
                    .clone(),
            });
        }
        self.next_span_id += 1;
        Ok(span)
    }

    /// Create a new exit span, which will be created when tracing context will generate
    /// new span for function invocation.
    /// Currently, this SDK supports RPC call. So we must set `remote_peer`.
    pub fn create_exit_span(
        &mut self,
        operation_name: String,
        remote_peer: String,
    ) -> Result<Box<Span>, &'static str> {
        if self.next_span_id == 0 {
            return Err("entry span must be existed.");
        }

        let span = Box::new(Span::new(
            self.next_span_id,
            operation_name,
            remote_peer,
            SpanType::Exit,
            SpanLayer::Http,
            false,
            self.time_fetcher.clone(),
        ));
        self.next_span_id += 1;
        Ok(span)
    }

    pub fn finalize_span(&mut self, mut span: Box<Span>) {
        span.close();
        self.spans.push(span);
    }

    pub fn finalize_span_for_test(&self, span: &mut Box<Span>) {
        span.close();
    }

    /// It converts tracing context into segment object.
    /// This conversion should be done before sending segments into OAP.
    pub fn convert_segment_object(&self) -> SegmentObject {
        let mut objects = Vec::<SpanObject>::new();

        for span in self.spans.iter() {
            objects.push(span.span_internal.clone());
        }

        SegmentObject {
            trace_id: self.trace_id.to_string(),
            trace_segment_id: self.trace_segment_id.to_string(),
            spans: objects,
            service: self.service.clone(),
            service_instance: self.service_instance.clone(),
            is_size_limited: false,
        }
    }
}
