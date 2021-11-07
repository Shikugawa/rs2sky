use base64::decode;

pub struct PropagationContext {
    /// It defines whether next span should be trace or not.
    /// In SkyWalking, If `do_sample == true`, the span should be reported to
    /// OAP server and can be analyzed.
    do_sample: bool,

    /// It defines trace ID that previous span has. It expresses unique value of entire trace.
    parent_trace_id: String,

    /// It defines segment ID that previos span has. It expresses unique value of entire trace.
    parent_trace_segment_id: String,

    /// It defines parent span's span ID.
    parent_span_id: u32,

    /// Service name of service parent belongs.
    parent_service: String,

    /// Instance name of service parent belongs.
    parent_service_instance: String,

    /// An endpoint name that parent requested to.
    destination_endpoint: String,

    /// An address that parent requested to. It can be authority or network address.
    destination_address: String,
}

impl PropagationContext {
    fn new(
        do_sample: bool,
        parent_trace_id: String,
        parent_trace_segment_id: String,
        parent_span_id: u32,
        parent_service: String,
        parent_service_instance: String,
        destination_endpoint: String,
        destination_address: String,
    ) -> PropagationContext {
        PropagationContext {
            do_sample: do_sample,
            parent_trace_id: parent_trace_id,
            parent_trace_segment_id: parent_trace_segment_id,
            parent_span_id: parent_span_id,
            parent_service: parent_service,
            parent_service_instance: parent_service_instance,
            destination_endpoint: destination_endpoint,
            destination_address: destination_address,
        }
    }
}

pub struct ContextDecoder<'a> {
    header_value: &'a str,
}

impl<'a> ContextDecoder<'a> {
    pub fn new(header_value: &str) -> ContextDecoder<'_> {
        let decoder = ContextDecoder {
            header_value: header_value,
        };
        decoder
    }

    pub fn decode(&self) -> Result<PropagationContext, &str> {
        let pieces: Vec<&str> = self.header_value.split('-').collect();

        if pieces.len() != 8 {
            return Err("failed to parse propagation context: it must have 8 properties.");
        }

        let do_sample = self.try_parse_sample_status(pieces[0])?;
        let parent_trace_id = self.b64_encoded_into_string(pieces[1])?;
        let parent_trace_segment_id = self.b64_encoded_into_string(pieces[2])?;
        let parent_span_id: u32 = self.try_parse_parent_span_id(pieces[3])?;
        let parent_service = self.b64_encoded_into_string(pieces[4])?;
        let parent_service_instance = self.b64_encoded_into_string(pieces[5])?;
        let destination_endpoint = self.b64_encoded_into_string(pieces[6])?;
        let destination_address = self.b64_encoded_into_string(pieces[7])?;

        let context = PropagationContext::new(
            do_sample,
            parent_trace_id,
            parent_trace_segment_id,
            parent_span_id,
            parent_service,
            parent_service_instance,
            destination_endpoint,
            destination_address,
        );

        Ok(context)
    }

    fn try_parse_parent_span_id(&self, id: &str) -> Result<u32, &str> {
        if let Ok(result) = id.parse::<u32>() {
            Ok(result)
        } else {
            Err("failed to parse span id from parent.")
        }
    }

    fn try_parse_sample_status(&self, status: &str) -> Result<bool, &str> {
        if status == "0" {
            Ok(false)
        } else if status == "1" {
            Ok(true)
        } else {
            Err("failed to parse sample status.")
        }
    }

    fn b64_encoded_into_string(&self, enc: &str) -> Result<String, &str> {
        if let Ok(result) = decode(enc) {
            if let Ok(decoded_str) = String::from_utf8(result) {
                return Ok(decoded_str);
            }
        }

        Err("failed to decode value.")
    }
}

#[test]
fn basic() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode().unwrap();

    assert_eq!(res.do_sample, true);
    assert_eq!(res.parent_trace_id, "1");
    assert_eq!(res.parent_trace_segment_id, "5");
    assert_eq!(res.parent_span_id, 3);
    assert_eq!(res.parent_service, "mesh");
    assert_eq!(res.parent_service_instance, "instance");
    assert_eq!(res.destination_endpoint, "/api/v1/health");
    assert_eq!(res.destination_address, "example.com:8080");
}

#[test]
fn less_field() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode();

    assert_eq!(res.is_err(), true);
}

#[test]
fn more_field() {
    let data = "1-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==-hogehoge";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode();

    assert_eq!(res.is_err(), true);
}

#[test]
fn invalid_sample() {
    let data = "3-MQ==-NQ==-3-bWVzaA==-aW5zdGFuY2U=-L2FwaS92MS9oZWFsdGg=-ZXhhbXBsZS5jb206ODA4MA==";
    let decoder = ContextDecoder::new(data);
    let res = decoder.decode();

    assert_eq!(res.is_err(), true);
}
