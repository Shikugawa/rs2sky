pub mod skywalking {
    pub mod v3 {
        tonic::include_proto!("skywalking.v3");
    }
}

use uuid::Uuid;

pub struct TracingContext {
    trace_id: u128,
    trace_segment_id: u128,
    service: &'static str,
    service_instance: &'static str,
}

impl TracingContext {
    /// Used to generate a new trace context. Typically called when no context has
    /// been propagated and a new trace is to be started.
    pub fn new_default(service_name: &'static str, instance_name: &'static str) -> Self {
        let trace_id = Uuid::new_v4().as_u128();
        let trace_segment_id = Uuid::new_v4().as_u128();

        TracingContext {
            trace_id,
            trace_segment_id,
            service: service_name,
            service_instance: instance_name,
        }
    }

    /// Generate a trace context using the propagated context.
    /// It is generally used when tracing is to be performed continuously.
    pub fn new_from_parent_span() -> Self {
        unimplemented!()
    }
}
