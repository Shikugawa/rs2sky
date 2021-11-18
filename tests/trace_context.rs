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

#![allow(unused_imports)]

pub mod skywalking_proto {
    pub mod v3 {
        tonic::include_proto!("skywalking.v3");
    }
}

use prost::Message;
use rs2sky::common::time::TimeFetcher;
use rs2sky::context::propagation::ContextDecoder;
use rs2sky::context::trace_context::TracingContext;
use std::sync::Arc;

/// Serialize from A should equal Serialize from B
#[allow(dead_code)]
pub fn check_serialize_equivalent<M, N>(msg_a: &M, msg_b: &N)
where
    M: Message + Default + PartialEq,
    N: Message + Default + PartialEq,
{
    let mut buf_a = Vec::new();
    msg_a.encode(&mut buf_a).unwrap();
    let mut buf_b = Vec::new();
    msg_b.encode(&mut buf_b).unwrap();
    assert_eq!(buf_a, buf_b);
}

struct MockTimeFetcher {}

impl TimeFetcher for MockTimeFetcher {
    fn get(&self) -> i64 {
        100
    }
}

#[test]
fn create_span() {
    let time_fetcher = MockTimeFetcher {};
    let mut context = TracingContext::default(Arc::new(time_fetcher), "service", "instance");
    assert_eq!(context.service, "service");
    assert_eq!(context.service_instance, "instance");

    {
        let mut span1 = context.create_entry_span(String::from("op1")).unwrap();
        let span1_expected = skywalking_proto::v3::SpanObject {
            span_id: 1,
            parent_span_id: 0,
            start_time: 100,
            end_time: 100,
            refs: Vec::<skywalking_proto::v3::SegmentReference>::new(),
            operation_name: String::from("op1"),
            peer: String::default(),
            span_type: skywalking_proto::v3::SpanType::Entry as i32,
            span_layer: skywalking_proto::v3::SpanLayer::Http as i32,
            component_id: 11000,
            is_error: false,
            tags: Vec::<skywalking_proto::v3::KeyStringValuePair>::new(),
            logs: Vec::<skywalking_proto::v3::Log>::new(),
            skip_analysis: false,
        };
        context.finalize_span_for_test(&mut span1);
        check_serialize_equivalent(&span1.span_internal, &span1_expected);
    }

    {
        let span2 = context.create_entry_span(String::from("op2"));
        assert_eq!(span2.is_err(), true);
    }

    {
        let mut span3 =
            context.create_exit_span(String::from("op3"), String::from("example.com/test"));
        let span3_expected = skywalking_proto::v3::SpanObject {
            span_id: 2,
            parent_span_id: 1,
            start_time: 100,
            end_time: 100,
            refs: Vec::<skywalking_proto::v3::SegmentReference>::new(),
            operation_name: String::from("op3"),
            peer: String::from("example.com/test"),
            span_type: skywalking_proto::v3::SpanType::Exit as i32,
            span_layer: skywalking_proto::v3::SpanLayer::Http as i32,
            component_id: 11000,
            is_error: false,
            tags: Vec::<skywalking_proto::v3::KeyStringValuePair>::new(),
            logs: Vec::<skywalking_proto::v3::Log>::new(),
            skip_analysis: false,
        };
        context.finalize_span_for_test(&mut span3);
        check_serialize_equivalent(&span3.span_internal, &span3_expected);
    }

    let segment = context.convert_segment_object();
    assert_eq!(segment.trace_id.len() != 0, true);
    assert_eq!(segment.trace_segment_id.len() != 0, true);
    assert_eq!(segment.service, "service");
    assert_eq!(segment.service_instance, "instance");
    assert_eq!(segment.is_size_limited, false);
}

#[test]
fn create_span_from_context() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==";
    let decoder = ContextDecoder::new(data);
    let prop = decoder.decode().unwrap();
    let time_fetcher = MockTimeFetcher {};
    let context = TracingContext::from_propagation_context(Arc::new(time_fetcher), prop);

    let segment = context.convert_segment_object();
    assert_eq!(segment.trace_id.len() != 0, true);
    assert_eq!(segment.trace_segment_id.len() != 0, true);
    assert_eq!(segment.service, "mesh");
    assert_eq!(segment.service_instance, "instance");
    assert_eq!(segment.is_size_limited, false);
}
