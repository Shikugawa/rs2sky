pub mod skywalking {
    pub mod v3 {
        tonic::include_proto!("skywalking.v3");
    }
}

use crate::propagation::{ContextDecoder, PropagationContext};
use prost::Message;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct Span {
    span_internal: skywalking::v3::SpanObject,
}

impl Span {
    pub fn new(
        parent_span_id: i32,
        operation_name: String,
        remote_peer: String,
        span_type: skywalking::v3::SpanType,
        span_layer: skywalking::v3::SpanLayer,
        skip_analysis: bool,
    ) -> Self {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let span_internal = skywalking::v3::SpanObject {
            span_id: parent_span_id + 1,
            parent_span_id: parent_span_id,
            start_time: current_time as i64,
            end_time: 0, // not set
            refs: Vec::<skywalking::v3::SegmentReference>::new(),
            operation_name: operation_name,
            peer: remote_peer,
            span_type: span_type as i32,
            span_layer: span_layer as i32,
            // TODO(shikugawa): define this value in
            // https://github.com/apache/skywalking/blob/6452e0c2d983c85c392602d50436e8d8e421fec9/oap-server/server-starter/src/main/resources/component-libraries.yml
            component_id: 11000,
            is_error: false,
            tags: Vec::<skywalking::v3::KeyStringValuePair>::new(),
            logs: Vec::<skywalking::v3::Log>::new(),
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

struct SpanSet {
    spans: Vec<Span>,
}

impl SpanSet {
    fn new() -> Self {
        SpanSet { spans: Vec::new() }
    }

    fn convert_span_objects(&self) -> Vec<skywalking::v3::SpanObject> {
        let mut objects = Vec::<skywalking::v3::SpanObject>::new();

        for span in self.spans.iter() {
            objects.push(span.span_internal.clone());
        }

        objects
    }

    fn push(&mut self, span: Span) {
        self.spans.push(span);
    }

    fn len(&self) -> usize {
        self.spans.len()
    }

    fn last_span_mut(&mut self) -> &mut Span {
        self.spans.last_mut().unwrap()
    }
}

pub struct TracingContext {
    trace_id: u128,
    trace_segment_id: u128,
    service: String,
    service_instance: String,
    spans: SpanSet,
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
    pub fn from_parent_span(context: PropagationContext) -> Self {
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
            skywalking::v3::SpanType::Entry,
            skywalking::v3::SpanLayer::Http,
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
            skywalking::v3::SpanType::Exit,
            skywalking::v3::SpanLayer::Http,
            false,
        ));

        self.spans.last_span_mut()
    }

    /// It converts tracing context into segment object.
    /// This conversion should be done before sending segments into OAP.
    pub fn convert_segment_object(&self) -> skywalking::v3::SegmentObject {
        skywalking::v3::SegmentObject {
            trace_id: self.trace_id.to_string(),
            trace_segment_id: self.trace_segment_id.to_string(),
            spans: self.spans.convert_span_objects(),
            service: self.service.clone(),
            service_instance: self.service_instance.clone(),
            is_size_limited: false,
        }
    }
}

/// Serialize from A should equal Serialize from B
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

#[test]
fn create_span() {
    let mut context = TracingContext::default("service", "instance");
    assert_eq!(context.service, "service");
    assert_eq!(context.service_instance, "instance");

    {
        let mut span1 = context.create_entry_span(String::from("op1")).unwrap();
        span1.span_internal.start_time = 100;
        let span1_expected = skywalking::v3::SpanObject {
            span_id: 0,
            parent_span_id: -1,
            start_time: 100,
            end_time: 0, // not set
            refs: Vec::<skywalking::v3::SegmentReference>::new(),
            operation_name: String::from("op1"),
            peer: String::default(),
            span_type: skywalking::v3::SpanType::Entry as i32,
            span_layer: skywalking::v3::SpanLayer::Http as i32,
            component_id: 11000,
            is_error: false,
            tags: Vec::<skywalking::v3::KeyStringValuePair>::new(),
            logs: Vec::<skywalking::v3::Log>::new(),
            skip_analysis: false,
        };

        check_serialize_equivalent(&span1.span_internal, &span1_expected);
        span1.close();
    }

    assert_ne!(context.spans.last_span_mut().span_internal.end_time, 0);
    assert_eq!(context.spans.len(), 1);

    {
        let mut span2 = context.create_entry_span(String::from("op2"));
        assert_eq!(span2.is_err(), true);
    }

    assert_eq!(context.spans.len(), 1);

    {
        let mut span3 =
            context.create_exit_span(String::from("op3"), String::from("example.com/test"));
        span3.span_internal.start_time = 100;
        let mut span3_expected = skywalking::v3::SpanObject {
            span_id: 1,
            parent_span_id: 0,
            start_time: 100,
            end_time: 0, // not set
            refs: Vec::<skywalking::v3::SegmentReference>::new(),
            operation_name: String::from("op3"),
            peer: String::from("example.com/test"),
            span_type: skywalking::v3::SpanType::Exit as i32,
            span_layer: skywalking::v3::SpanLayer::Http as i32,
            component_id: 11000,
            is_error: false,
            tags: Vec::<skywalking::v3::KeyStringValuePair>::new(),
            logs: Vec::<skywalking::v3::Log>::new(),
            skip_analysis: false,
        };

        check_serialize_equivalent(&span3.span_internal, &span3_expected);
        span3.close();
    }

    assert_ne!(context.spans.last_span_mut().span_internal.end_time, 0);
    assert_eq!(context.spans.len(), 2);

    let segment = context.convert_segment_object();
    assert_eq!(segment.trace_id.len() != 0, true);
    assert_eq!(segment.trace_segment_id.len() != 0, true);
    assert_eq!(segment.spans.len() == 2, true);
    assert_eq!(segment.service, "service");
    assert_eq!(segment.service_instance, "instance");
    assert_eq!(segment.is_size_limited, false);
}

#[test]
fn create_span_from_context() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==";
    let decoder = ContextDecoder::new(data);
    let prop = decoder.decode().unwrap();
    let context = TracingContext::from_parent_span(prop);

    let segment = context.convert_segment_object();
    assert_eq!(segment.trace_id.len() != 0, true);
    assert_eq!(segment.trace_segment_id.len() != 0, true);
    assert_eq!(segment.service, "mesh");
    assert_eq!(segment.service_instance, "instance");
    assert_eq!(segment.is_size_limited, false);
}
